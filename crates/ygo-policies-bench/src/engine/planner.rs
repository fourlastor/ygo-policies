//! Transaction-complete beam planning inside one already sampled world.
//!
//! The caller owns hidden-state sampling. This module never reads hidden cards,
//! changes their arrangement, or reseeds the engine. Own costs, targets, materials
//! and chain responses branch, while the opponent uses its supplied generic seat.
//! Beam pruning occurs only at an unchained Idle/Battle boundary. A completed
//! proposal includes the own responses that produced its value; callers should
//! evaluate that SAME proposal across sampled worlds with `evaluate_plan`, rather
//! than average independently optimized world values.
use super::*;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;

/// Limits for one root response in one sampled world.
#[derive(Clone, Copy, Debug)]
pub struct PlannerOptions {
    pub width: usize,
    pub actions: usize,
    /// Attempted branches, including the complete generic baseline.
    pub nodes: usize,
    /// Maximum future engine responses along any one path.
    pub decisions: usize,
    /// Opponent turns to finish after the current own turn; usually one.
    /// Zero ends at the first safe boundary of the following turn.
    pub replies: usize,
}

#[derive(Clone, Debug)]
pub(super) struct PlannedAction {
    /// Exact original redacted observation and decision, before expansion.
    pub signature: Arc<str>,
    pub response: Vec<u8>,
    /// The offered action after expanding sequential completions to twelve.
    pub action: Arc<Value>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PlannerResult {
    /// Only a terminal result or a completed requested turn horizon is scored.
    pub value: Option<f64>,
    pub terminal: bool,
    /// Captured by proposal search. Fixed-plan validators return an empty plan.
    pub plan: Vec<PlannedAction>,
    pub nodes: usize,
    pub leaves: usize,
    pub improved_leaves: usize,
    pub completed_prefixes: usize,
    pub failed: usize,
    pub cutoffs: usize,
    pub responses: usize,
    pub exhausted: bool,
    pub plan_applied: usize,
    pub plan_mismatches: usize,
    pub errors: Vec<String>,
}

/// Use the ORIGINAL answer, before `alternative_responses(12)`. Keeping exact
/// data rather than a lossy hash makes a mismatch conservative and auditable.
pub(super) fn prompt_signature(answer: &Value) -> Arc<str> {
    serde_json::json!({"observation": answer["observation"], "decision": answer["decision"]})
        .to_string()
        .into()
}

/// After an original-signature match, expand with `alternative_responses(12)`
/// and verify that both the semantic action and its response bytes still match.
pub(super) fn resolve_action(action: &PlannedAction, expanded: &Value) -> Option<Vec<u8>> {
    let choices = expanded["decision"]["choices"].as_array()?;
    let responses = expanded["responses"].as_array()?;
    if choices.len() != responses.len() {
        return None;
    }
    choices
        .iter()
        .zip(responses)
        .find_map(|(choice, response)| {
            let bytes = response_bytes(response)?;
            (choice == action.action.as_ref() && bytes == action.response).then_some(bytes)
        })
}

fn response_bytes(value: &Value) -> Option<Vec<u8>> {
    value
        .as_array()?
        .iter()
        .map(|byte| u8::try_from(byte.as_u64()?).ok())
        .collect()
}

fn offered_action(answer: &Value, response: &[u8]) -> Option<Value> {
    let choices = answer["decision"]["choices"].as_array()?;
    let responses = answer["responses"].as_array()?;
    if choices.len() != responses.len() {
        return None;
    }
    choices.iter().zip(responses).find_map(|(choice, bytes)| {
        (response_bytes(bytes)?.as_slice() == response).then(|| choice.clone())
    })
}

fn supported(answer: &Value) -> bool {
    match &answer["decision"]["kind"] {
        Value::String(kind) => [
            "Idle",
            "Battle",
            "YesNo",
            "Position",
            "Option",
            "SelectCards",
            "SelectToggle",
            "SelectSum",
        ]
        .contains(&kind.as_str()),
        Value::Object(kind) => kind.contains_key("Chain"),
        _ => false,
    }
}

/// Keep the generic response, then diversify action kinds before second copies
/// of a kind. This only orders legal choices; no card/deck identity rules enter.
fn alternatives(answer: &Value, given: &[u8], maximum: usize) -> Vec<Vec<u8>> {
    let mut result = vec![given.to_vec()];
    if maximum < 2 || !supported(answer) {
        return result;
    }
    let Some(choices) = answer["decision"]["choices"].as_array() else {
        return result;
    };
    let Some(responses) = answer["responses"].as_array() else {
        return result;
    };
    if choices.len() != responses.len() {
        return result;
    }
    let mut groups: Vec<(String, Vec<Vec<u8>>)> = Vec::new();
    let mut seen = Vec::new();
    for (choice, response) in choices.iter().zip(responses) {
        let Some(response) = response_bytes(response) else {
            continue;
        };
        if response == given || choice["kind"] == "ShuffleHand" || choice["kind"] == "Cancel" {
            continue;
        }
        let at = &choice["card"]["at"];
        let key = if ["Hand", "Deck", "Graveyard", "Banished", "Extra"]
            .contains(&at["location"].as_str().unwrap_or(""))
        {
            serde_json::json!([
                choice["kind"],
                choice["card"]["code"],
                at["controller"],
                at["location"],
                choice["description"]
            ])
            .to_string()
        } else {
            choice.to_string()
        };
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let kind = choice["kind"].to_string();
        if let Some((_, items)) = groups.iter_mut().find(|(group, _)| group == &kind) {
            items.push(response);
        } else {
            groups.push((kind, vec![response]));
        }
    }
    let priority = [
        "Activate",
        "SpecialSummon",
        "NormalSummon",
        "Attack",
        "EnterBattle",
        "EndTurn",
        "Pass",
        "SetSpellTrap",
        "SetMonster",
        "ChangePosition",
        "EnterMain2",
    ];
    groups.sort_by_key(|(kind, _)| {
        priority
            .iter()
            .position(|name| kind == &format!("\"{name}\""))
            .unwrap_or(priority.len())
    });
    for depth in 0..choices.len() {
        for (_, responses) in &groups {
            if result.len() >= maximum {
                return result;
            }
            if let Some(response) = responses.get(depth) {
                if !result.contains(response) {
                    result.push(response.clone());
                }
            }
        }
    }
    result
}

#[derive(Clone, Copy)]
struct Progress {
    decisions: usize,
    future: usize,
    chain: bool,
}

struct Pending {
    player: usize,
    given: Vec<u8>,
    signature: Arc<str>,
    expanded: Arc<Value>,
}

struct Node<'a> {
    snapshot: Snapshot,
    seats: Vec<LibrarySeat<'a>>,
    pending: Pending,
    progress: Progress,
    plan: Vec<PlannedAction>,
    score: f64,
}

struct Leaf {
    value: f64,
    terminal: bool,
    plan: Vec<PlannedAction>,
}

enum Advanced<'a> {
    /// Completed action transaction: safe to score and prune.
    Board(Node<'a>),
    /// A cost/target/material/chain choice, whose partial state is not scored.
    Choice(Node<'a>),
    Leaf(Leaf),
    Cutoff,
}

struct Search<'a, 'c, 'e> {
    duel: &'c mut Duel<'a>,
    viewer: u8,
    game_limit: usize,
    goal_turn: u64,
    capture_plan: bool,
    options: PlannerOptions,
    deadline: Option<Instant>,
    evaluator: &'e dyn Fn(&Value, u8) -> f64,
    result: PlannerResult,
}

impl<'a, 'c, 'e> Search<'a, 'c, 'e> {
    fn expired(&mut self) -> bool {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.result.exhausted = true;
            true
        } else {
            false
        }
    }

    fn admit(&mut self) -> bool {
        if self.expired() || self.result.nodes >= self.options.nodes {
            self.result.exhausted = true;
            false
        } else {
            self.result.nodes += 1;
            true
        }
    }

    fn error(&mut self, error: String) {
        self.result.failed += 1;
        if self.result.errors.len() < 8 && !self.result.errors.contains(&error) {
            self.result.errors.push(error);
        }
    }

    fn value(&self, observation: &Value) -> Result<f64> {
        let value = (self.evaluator)(observation, self.viewer);
        if !value.is_finite() {
            return Err("Planner evaluator returned a non-finite value".into());
        }
        Ok(value.clamp(-0.999_999, 0.999_999))
    }

    fn retain(&mut self, leaf: Leaf) {
        self.result.leaves += 1;
        if self
            .result
            .value
            .map_or(true, |value| leaf.value > value + 1e-12)
        {
            self.result.improved_leaves += usize::from(self.result.value.is_some());
            self.result.value = Some(leaf.value);
            self.result.terminal = leaf.terminal;
            self.result.plan = leaf.plan;
        }
    }

    /// Advance one chosen response. Own intermediate choices can suspend this
    /// transaction, but only complete transactions become beam candidates.
    #[allow(clippy::too_many_arguments)]
    fn advance<'p>(
        &mut self,
        seats: Vec<LibrarySeat<'p>>,
        mut pending: Pending,
        first: &[u8],
        mut progress: Progress,
        mut plan: Vec<PlannedAction>,
        explore: bool,
        fixed: Option<&[PlannedAction]>,
        record_first: bool,
    ) -> Result<Advanced<'p>> {
        if !self.admit() {
            return Ok(Advanced::Cutoff);
        }
        let mut response = first.to_vec();
        let mut record = record_first && self.capture_plan;
        let mut fixed_cursor = 0;
        let mut fixed_valid = true;
        loop {
            if self.expired() {
                return Ok(Advanced::Cutoff);
            }
            if pending.player == self.viewer as usize {
                seats[pending.player].override_response(&response)?;
                if record {
                    let action = offered_action(&pending.expanded, &response)
                        .ok_or("Planner response disappeared from offered actions")?;
                    plan.push(PlannedAction {
                        signature: pending.signature.clone(),
                        response: response.clone(),
                        action: Arc::new(action),
                    });
                }
            } else if response != pending.given {
                return Err("Planner attempted to override its generic opponent".into());
            }
            self.duel.respond(&response);
            progress.decisions += 1;
            progress.future += 1;
            self.result.responses += 1;
            let (next_player, given, quiescent) = loop {
                if self.expired() {
                    return Ok(Advanced::Cutoff);
                }
                let step = self.duel.step()?;
                let mut answer = None;
                let mut quiescent = false;
                for sent in &step.messages {
                    if sent.refresh {
                        seats[0].feed_update_pair(&seats[1], &sent.bytes)?;
                        continue;
                    }
                    match sent.bytes[0] {
                        msg::RETRY => {
                            return Err("Engine rejected planner response (MSG_RETRY)".into())
                        }
                        msg::CHAINING => progress.chain = true,
                        msg::CHAIN_END => progress.chain = false,
                        msg::SELECT_IDLECMD | msg::SELECT_BATTLECMD => quiescent = true,
                        _ => {}
                    }
                    for (player, seat) in seats.iter().enumerate() {
                        if let Some(response) = seat.feed(&sent.bytes)? {
                            if answer.replace((player, response)).is_some() {
                                return Err("Multiple planner answers in an engine batch".into());
                            }
                        }
                    }
                }
                if let State::Over(outcome) = step.state {
                    let outcome = outcome.ok_or("Planner engine ended without MSG_WIN")?;
                    return Ok(Advanced::Leaf(Leaf {
                        value: if outcome.winner > 1 {
                            0.0
                        } else if outcome.winner == self.viewer {
                            1.0
                        } else {
                            -1.0
                        },
                        terminal: true,
                        plan,
                    }));
                }
                if let Some((player, given)) = answer {
                    break (player, given, quiescent && !progress.chain);
                }
                if step.state == State::Awaiting {
                    return Err("Planner engine awaits an unanswered prompt".into());
                }
            };
            // Generic validation tails need only feed/response. Materialize the
            // observer view solely where a heuristic can actually be consumed.
            let observation = if quiescent && (explore || self.goal_turn != u64::MAX) {
                Some(seats[self.viewer as usize].observation()?)
            } else {
                None
            };
            if observation
                .as_ref()
                .is_some_and(|view| view["turn"].as_u64().unwrap_or(0) >= self.goal_turn)
            {
                return Ok(Advanced::Leaf(Leaf {
                    value: self.value(observation.as_ref().unwrap())?,
                    terminal: false,
                    plan,
                }));
            }
            if progress.decisions >= self.game_limit || progress.future >= self.options.decisions {
                self.result.cutoffs += 1;
                return Ok(Advanced::Cutoff);
            }
            let planned = fixed_valid
                .then(|| fixed.and_then(|actions| actions.get(fixed_cursor)))
                .flatten();
            pending = if next_player == self.viewer as usize
                && (self.capture_plan || planned.is_some())
            {
                let original = seats[next_player].last_answer()?;
                let signature = prompt_signature(&original);
                // Expanding a sequential prompt runs up to twelve policy
                // branches. Fixed replay only needs that cache when its saved
                // response is absent from the original offered choices.
                let expand = self.capture_plan
                    || planned.is_some_and(|action| {
                        action.signature == signature && resolve_action(action, &original).is_none()
                    });
                Pending {
                    player: next_player,
                    given,
                    signature,
                    expanded: Arc::new(if expand {
                        seats[next_player].alternative_responses(12)?
                    } else {
                        original
                    }),
                }
            } else {
                Pending {
                    player: next_player,
                    given,
                    signature: Arc::from(""),
                    expanded: Arc::new(Value::Null),
                }
            };
            if explore {
                if quiescent {
                    return Ok(Advanced::Board(Node {
                        snapshot: self.duel.snapshot()?,
                        seats,
                        pending,
                        progress,
                        plan,
                        score: self.value(observation.as_ref().unwrap())?,
                    }));
                }
                if pending.player == self.viewer as usize
                    && alternatives(&pending.expanded, &pending.given, self.options.actions).len()
                        > 1
                {
                    return Ok(Advanced::Choice(Node {
                        snapshot: self.duel.snapshot()?,
                        seats,
                        pending,
                        progress,
                        plan,
                        score: 0.0,
                    }));
                }
            }
            response = pending.given.clone();
            if pending.player == self.viewer as usize && fixed_valid {
                if let Some(action) = fixed.and_then(|actions| actions.get(fixed_cursor)) {
                    let matched = action.signature == pending.signature;
                    if let Some(planned) = matched
                        .then(|| resolve_action(action, &pending.expanded))
                        .flatten()
                    {
                        response = planned;
                        fixed_cursor += 1;
                        self.result.plan_applied += 1;
                    } else {
                        fixed_valid = false;
                        self.result.plan_mismatches += 1;
                    }
                }
            }
            record = self.capture_plan;
        }
    }

    fn offer<'p>(&mut self, beam: &mut Vec<Node<'p>>, node: Node<'p>) {
        // Complete promising, transaction-safe prefixes with the generic
        // policy. A bounded beam may otherwise never finish an entire turn,
        // making all its explored actions irrelevant beside the baseline.
        // Their completed plans can win even if later breadth consumes the cap.
        let promising = beam.len() < self.options.width
            || beam.last().is_some_and(|last| node.score > last.score);
        if promising && !self.expired() && self.result.nodes < self.options.nodes {
            let completed = (|| -> Result<Advanced<'p>> {
                self.duel.restore(&node.snapshot)?;
                let seats = node
                    .seats
                    .iter()
                    .map(LibrarySeat::fork)
                    .collect::<Result<Vec<_>>>()?;
                let pending = Pending {
                    player: node.pending.player,
                    given: node.pending.given.clone(),
                    signature: node.pending.signature.clone(),
                    expanded: node.pending.expanded.clone(),
                };
                self.advance(
                    seats,
                    pending,
                    &node.pending.given,
                    node.progress,
                    node.plan.clone(),
                    false,
                    None,
                    true,
                )
            })();
            match completed {
                Ok(Advanced::Leaf(leaf)) => {
                    self.result.completed_prefixes += 1;
                    self.retain(leaf);
                }
                Ok(_) => {}
                Err(error) => self.error(error),
            }
        }
        beam.push(node);
        beam.sort_by(|a, b| b.score.total_cmp(&a.score));
        beam.truncate(self.options.width);
    }

    /// Resolve partial transactions without using their temporary material/LP
    /// states as a pruning score. The global node/response limits still apply.
    fn collect<'p>(&mut self, advanced: Advanced<'p>, beam: &mut Vec<Node<'p>>) {
        match advanced {
            Advanced::Leaf(leaf) => self.retain(leaf),
            Advanced::Board(node) => self.offer(beam, node),
            Advanced::Choice(node) => self.expand(node, beam),
            Advanced::Cutoff => {}
        }
    }

    fn expand<'p>(&mut self, node: Node<'p>, beam: &mut Vec<Node<'p>>) {
        let responses = if node.pending.player == self.viewer as usize {
            alternatives(
                &node.pending.expanded,
                &node.pending.given,
                self.options.actions,
            )
        } else {
            vec![node.pending.given.clone()]
        };
        for response in responses {
            if self.expired() || self.result.nodes >= self.options.nodes {
                self.result.exhausted = true;
                break;
            }
            let branch = (|| -> Result<Advanced<'p>> {
                self.duel.restore(&node.snapshot)?;
                let seats = node
                    .seats
                    .iter()
                    .map(LibrarySeat::fork)
                    .collect::<Result<Vec<_>>>()?;
                let pending = Pending {
                    player: node.pending.player,
                    given: node.pending.given.clone(),
                    signature: node.pending.signature.clone(),
                    expanded: node.pending.expanded.clone(),
                };
                self.advance(
                    seats,
                    pending,
                    &response,
                    node.progress,
                    node.plan.clone(),
                    true,
                    None,
                    true,
                )
            })();
            match branch {
                Ok(advanced) => self.collect(advanced, beam),
                Err(error) => self.error(error),
            }
            if self.result.value == Some(1.0) {
                break;
            }
        }
    }
}

fn target_turn(observation: &Value, viewer: u8, replies: usize) -> u64 {
    let turn = observation["turn"].as_u64().unwrap_or(0);
    let own = observation["turn_player"].as_u64() == Some(viewer as u64);
    let advance = if replies == 0 {
        1
    } else if own {
        replies.saturating_mul(2)
    } else {
        replies.saturating_mul(2).saturating_sub(1)
    };
    turn.saturating_add(advance as u64)
}

fn validate(options: PlannerOptions, viewer: u8, seats: &[LibrarySeat<'_>]) -> Result<()> {
    if viewer > 1 || seats.len() != 2 {
        return Err("Planner requires two seats and viewer 0 or 1".into());
    }
    if options.width == 0 || options.actions == 0 || options.nodes == 0 || options.decisions == 0 {
        return Err("Planner width/actions/nodes/decisions must be positive".into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run(
    duel: &mut Duel,
    seats: &[LibrarySeat<'_>],
    viewer: u8,
    response: &[u8],
    root_decisions: usize,
    game_limit: usize,
    options: PlannerOptions,
    deadline: Option<Instant>,
    evaluator: &dyn Fn(&Value, u8) -> f64,
    fixed: Option<&[PlannedAction]>,
    beam_search: bool,
    terminal_only: bool,
    capture_plan: bool,
) -> Result<PlannerResult> {
    validate(options, viewer, seats)?;
    let root = duel.snapshot()?;
    let original = Arc::new(if capture_plan {
        seats[viewer as usize].last_answer()?
    } else {
        Value::Null
    });
    let observation = if capture_plan || !terminal_only {
        seats[viewer as usize].observation()?
    } else {
        Value::Null
    };
    let mut search = Search {
        duel,
        viewer,
        game_limit,
        goal_turn: if terminal_only {
            u64::MAX
        } else {
            target_turn(&observation, viewer, options.replies)
        },
        capture_plan,
        options,
        deadline,
        evaluator,
        result: PlannerResult::default(),
    };
    let initial = || -> Result<(Vec<LibrarySeat<'_>>, Pending)> {
        let copies = seats
            .iter()
            .map(LibrarySeat::fork)
            .collect::<Result<Vec<_>>>()?;
        Ok((
            copies,
            Pending {
                player: viewer as usize,
                given: response.to_vec(),
                signature: prompt_signature(&original),
                expanded: original.clone(),
            },
        ))
    };
    let progress = Progress {
        decisions: root_decisions,
        future: 0,
        chain: !observation["chain"].as_array().map_or(true, Vec::is_empty),
    };
    // A complete generic continuation provides a usable fallback while the
    // beam spends its remaining budget discovering longer action sequences.
    match initial().and_then(|(seats, pending)| {
        search.advance(
            seats,
            pending,
            response,
            progress,
            Vec::new(),
            false,
            fixed,
            false,
        )
    }) {
        Ok(Advanced::Leaf(leaf)) => search.retain(leaf),
        Ok(_) => {}
        Err(error) => search.error(error),
    }
    if beam_search
        && search.result.value != Some(1.0)
        && !search.expired()
        && search.result.nodes < options.nodes
    {
        let mut beam = Vec::new();
        let first = (|| -> Result<_> {
            search.duel.restore(&root)?;
            let (seats, pending) = initial()?;
            search.advance(
                seats,
                pending,
                response,
                progress,
                Vec::new(),
                true,
                None,
                false,
            )
        })();
        match first {
            Ok(advanced) => search.collect(advanced, &mut beam),
            Err(error) => search.error(error),
        }
        while !beam.is_empty()
            && search.result.value != Some(1.0)
            && !search.expired()
            && search.result.nodes < options.nodes
        {
            let mut next = Vec::new();
            for node in beam {
                search.expand(node, &mut next);
            }
            beam = next;
        }
        if search.result.nodes >= options.nodes && search.result.value != Some(1.0) {
            search.result.exhausted = true;
        }
    }
    // The caller's sampled root is restored on every ordinary failure path.
    // A restore failure itself propagates: continuing that engine is unsafe.
    search.duel.restore(&root)?;
    Ok(search.result)
}

/// Propose a continuation in one sampled world. Its optimized value is
/// optimistic; use `evaluate_plan` on common worlds for root comparisons.
#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate(
    duel: &mut Duel,
    seats: &[LibrarySeat<'_>],
    viewer: u8,
    response: &[u8],
    root_decisions: usize,
    game_limit: usize,
    options: PlannerOptions,
    deadline: Option<Instant>,
    evaluator: &dyn Fn(&Value, u8) -> f64,
) -> Result<PlannerResult> {
    run(
        duel,
        seats,
        viewer,
        response,
        root_decisions,
        game_limit,
        options,
        deadline,
        evaluator,
        None,
        true,
        false,
        true,
    )
}

/// Evaluate one fixed, executable proposal. On its first public-prompt mismatch,
/// permanently use the generic continuation for this trial; never reoptimize in
/// the sampled world. The result records applied steps and the mismatch count,
/// without rebuilding a continuation that the caller already supplied.
#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_plan(
    duel: &mut Duel,
    seats: &[LibrarySeat<'_>],
    viewer: u8,
    response: &[u8],
    root_decisions: usize,
    game_limit: usize,
    options: PlannerOptions,
    deadline: Option<Instant>,
    evaluator: &dyn Fn(&Value, u8) -> f64,
    plan: &[PlannedAction],
) -> Result<PlannerResult> {
    run(
        duel,
        seats,
        viewer,
        response,
        root_decisions,
        game_limit,
        options,
        deadline,
        evaluator,
        Some(plan),
        false,
        false,
        false,
    )
}

/// Validate a fixed executable continuation by its actual game outcome. Once
/// its saved responses end or mismatch, the generic policies finish the duel.
/// No heuristic leaf is accepted; a decision/time cap returns `value: None`.
#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_plan_terminal(
    duel: &mut Duel,
    seats: &[LibrarySeat<'_>],
    viewer: u8,
    response: &[u8],
    root_decisions: usize,
    game_limit: usize,
    options: PlannerOptions,
    deadline: Option<Instant>,
    evaluator: &dyn Fn(&Value, u8) -> f64,
    plan: &[PlannedAction],
) -> Result<PlannerResult> {
    run(
        duel,
        seats,
        viewer,
        response,
        root_decisions,
        game_limit,
        PlannerOptions {
            decisions: game_limit.saturating_sub(root_decisions).max(1),
            ..options
        },
        deadline,
        evaluator,
        Some(plan),
        false,
        true,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_turn_and_opponent_reply_horizons_are_explicit() {
        let observation = serde_json::json!({"turn": 5,"turn_player":0});
        assert_eq!(target_turn(&observation, 0, 0), 6);
        assert_eq!(target_turn(&observation, 0, 1), 7);
        assert_eq!(target_turn(&observation, 1, 1), 6);
        assert_eq!(target_turn(&observation, 0, 2), 9);
    }

    #[test]
    fn plan_resolution_requires_exact_offered_action_and_bytes() {
        let original = serde_json::json!({"observation":{"turn":1,"cards":[]},"decision":{"kind":"YesNo","choices":[{"kind":"No"},{"kind":"Yes"}]},"responses":[[0],[1]],"choice":0});
        let action = PlannedAction {
            signature: prompt_signature(&original),
            response: vec![1],
            action: Arc::new(serde_json::json!({"kind":"Yes"})),
        };
        assert_eq!(resolve_action(&action, &original), Some(vec![1]));
        let mut changed = original.clone();
        changed["responses"][1] = serde_json::json!([2]);
        assert!(resolve_action(&action, &changed).is_none());
        changed = original.clone();
        changed["observation"]["turn"] = serde_json::json!(2);
        assert_ne!(prompt_signature(&changed), action.signature);
    }

    /// Exercise native snapshots, actual legal-response caches, and plan replay
    /// together. The normal-monster fixture deliberately removes script/chance
    /// noise, so any mismatch is a planner state-management regression.
    #[test]
    #[ignore = "build the release policy library first"]
    fn complete_plans_replay_and_failures_leave_the_root_unchanged() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cards = root.join("vendor/BabelCdb/cards.cdb");
        let core = Core::open(None, &cards, &root.join("vendor/CardScripts")).unwrap();
        let library = PolicyLibrary::open(&root.join(format!(
            "target/release/{}ygo_policies{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )))
        .unwrap();
        let decks = [
            Deck {
                main: vec![69140098; 40],
                extra: vec![],
            },
            Deck {
                main: vec![5053103; 40],
                extra: vec![],
            },
        ];
        let mut duel = core.deal(&DuelOptions::seeded(2416100), &decks).unwrap();
        let seats = (0..2)
            .map(|player| {
                library
                    .seat("generic", &cards, player, 2416100 + player as u64)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        seats[0].enable_response_overrides().unwrap();
        for (player, seat) in seats.iter().enumerate() {
            seat.feed(&duel.start_message(player as u8)).unwrap();
        }
        let (response, root_decisions) = {
            let mut decisions = 0;
            loop {
                let step = duel.step().unwrap();
                let mut pending = None;
                let mut idle = false;
                for sent in &step.messages {
                    if sent.refresh {
                        seats[0].feed_update_pair(&seats[1], &sent.bytes).unwrap();
                    } else {
                        idle |= sent.bytes[0] == msg::SELECT_IDLECMD;
                        for (player, seat) in seats.iter().enumerate() {
                            if let Some(answer) = seat.feed(&sent.bytes).unwrap() {
                                assert!(pending.replace((player, answer)).is_none());
                            }
                        }
                    }
                }
                assert!(!matches!(step.state, State::Over(_)));
                if let Some((player, response)) = pending {
                    if player == 0 && idle {
                        break (response, decisions);
                    }
                    duel.respond(&response);
                    decisions += 1;
                } else {
                    assert_ne!(step.state, State::Awaiting);
                }
                assert!(decisions < 32);
            }
        };
        let root_snapshot = duel.snapshot().unwrap();
        let before_answer = seats[0].last_answer().unwrap();
        let step_bytes = |duel: &mut Duel<'_>| {
            duel.respond(&response);
            duel.step()
                .unwrap()
                .messages
                .into_iter()
                .map(|sent| (sent.refresh, sent.bytes))
                .collect::<Vec<_>>()
        };
        let expected = step_bytes(&mut duel);
        duel.restore(&root_snapshot).unwrap();
        let options = PlannerOptions {
            width: 4,
            actions: 4,
            nodes: 48,
            decisions: 256,
            replies: 1,
        };
        let evaluator = |observation: &Value, viewer: u8| {
            let cards = observation["cards"].as_array().unwrap();
            let field = |player: u8| {
                cards
                    .iter()
                    .filter(|card| {
                        card["at"]["controller"] == player
                            && card["at"]["location"] == "MonsterZone"
                    })
                    .count() as f64
            };
            (field(viewer) - field(1 - viewer)) * 0.1
        };
        let proposed = evaluate(
            &mut duel,
            &seats,
            0,
            &response,
            root_decisions,
            512,
            options,
            None,
            &evaluator,
        )
        .unwrap();
        assert_eq!(proposed.failed, 0, "{:?}", proposed.errors);
        assert!(proposed.value.is_some());
        assert!(proposed.nodes > 1 && proposed.nodes <= options.nodes);
        assert!(proposed.completed_prefixes > 0);
        assert!(!proposed.plan.is_empty());
        assert_eq!(seats[0].last_answer().unwrap(), before_answer);
        assert_eq!(step_bytes(&mut duel), expected);
        duel.restore(&root_snapshot).unwrap();

        let replayed = evaluate_plan(
            &mut duel,
            &seats,
            0,
            &response,
            root_decisions,
            512,
            options,
            None,
            &evaluator,
            &proposed.plan,
        )
        .unwrap();
        assert_eq!(replayed.failed, 0, "{:?}", replayed.errors);
        assert_eq!(replayed.value, proposed.value);
        assert_eq!(replayed.plan_applied, proposed.plan.len());
        assert_eq!(replayed.plan_mismatches, 0);
        assert_eq!(replayed.nodes, 1);

        // Terminal validation ignores the short proposal horizon and never
        // calls the heuristic, including after the saved continuation ends.
        let terminal = evaluate_plan_terminal(
            &mut duel,
            &seats,
            0,
            &response,
            root_decisions,
            512,
            PlannerOptions {
                decisions: 1,
                ..options
            },
            None,
            &|_, _| f64::NAN,
            &proposed.plan,
        )
        .unwrap();
        assert_eq!(terminal.failed, 0, "{:?}", terminal.errors);
        assert!(terminal.terminal);
        assert_eq!(terminal.value, Some(1.0));
        assert!(terminal.responses > proposed.plan.len());

        let terminal_cutoff = evaluate_plan_terminal(
            &mut duel,
            &seats,
            0,
            &response,
            root_decisions,
            root_decisions + 1,
            options,
            None,
            &evaluator,
            &proposed.plan,
        )
        .unwrap();
        assert!(terminal_cutoff.value.is_none());
        assert!(!terminal_cutoff.terminal);
        assert_eq!(terminal_cutoff.cutoffs, 1);

        let mut mismatched = proposed.plan.clone();
        mismatched[0].signature = format!("{}!", mismatched[0].signature).into();
        let fallback = evaluate_plan(
            &mut duel,
            &seats,
            0,
            &response,
            root_decisions,
            512,
            options,
            None,
            &evaluator,
            &mismatched,
        )
        .unwrap();
        assert_eq!(fallback.failed, 0, "{:?}", fallback.errors);
        assert_eq!(fallback.plan_mismatches, 1);
        assert_eq!(fallback.plan_applied, 0);

        // Compare the metadata-free fixed validation with the original full
        // recording path on identical native roots. Cover an empty control,
        // successful replay, mismatch fallback, and a global decision cutoff.
        let outcomes = |result: &PlannerResult| {
            (
                result.value,
                result.terminal,
                result.nodes,
                result.leaves,
                result.failed,
                result.cutoffs,
                result.responses,
                result.exhausted,
                result.plan_applied,
                result.plan_mismatches,
                result.errors.clone(),
            )
        };
        let mut fast_elapsed = std::time::Duration::ZERO;
        let mut recorded_elapsed = std::time::Duration::ZERO;
        for saved in [&[][..], proposed.plan.as_slice(), mismatched.as_slice()] {
            for limit in [512, root_decisions + 1] {
                let started = Instant::now();
                let fast = evaluate_plan_terminal(
                    &mut duel,
                    &seats,
                    0,
                    &response,
                    root_decisions,
                    limit,
                    options,
                    None,
                    &evaluator,
                    saved,
                )
                .unwrap();
                fast_elapsed += started.elapsed();
                let started = Instant::now();
                let recorded = run(
                    &mut duel,
                    &seats,
                    0,
                    &response,
                    root_decisions,
                    limit,
                    PlannerOptions {
                        decisions: limit.saturating_sub(root_decisions).max(1),
                        ..options
                    },
                    None,
                    &evaluator,
                    Some(saved),
                    false,
                    true,
                    true,
                )
                .unwrap();
                recorded_elapsed += started.elapsed();
                assert_eq!(outcomes(&fast), outcomes(&recorded));
                assert!(fast.plan.is_empty());
            }
        }
        eprintln!("terminal fixed replay parity: metadata-free={fast_elapsed:?}, recording={recorded_elapsed:?}");

        // A partial path has no value, even if it happens to have a favorable
        // temporary board. This must not be mistaken for a completed leaf.
        let incomplete = evaluate_plan(
            &mut duel,
            &seats,
            0,
            &response,
            root_decisions,
            512,
            PlannerOptions {
                decisions: 1,
                ..options
            },
            None,
            &evaluator,
            &proposed.plan,
        )
        .unwrap();
        assert!(incomplete.value.is_none());
        assert_eq!(incomplete.failed, 0);
        assert_eq!(incomplete.cutoffs, 1);

        let failed = evaluate_plan(
            &mut duel,
            &seats,
            0,
            &response,
            root_decisions,
            512,
            options,
            None,
            &|_, _| f64::NAN,
            &proposed.plan,
        )
        .unwrap();
        assert!(failed.value.is_none());
        assert_eq!(failed.failed, 1);
        assert_eq!(step_bytes(&mut duel), expected);
        duel.restore(&root_snapshot).unwrap();
        assert_eq!(seats[0].last_answer().unwrap(), before_answer);
    }

    /// A target selected inside a resolving activation must be both optimized
    /// and present in the executable continuation. Otherwise a root-only search
    /// would claim the better result while its live generic target stayed worse.
    #[test]
    #[ignore = "build the release policy library first"]
    fn a_better_chain_target_is_discovered_and_replayed() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cards = root.join("vendor/BabelCdb/cards.cdb");
        let core = Core::open(None, &cards, &root.join("vendor/CardScripts")).unwrap();
        let library = PolicyLibrary::open(&root.join(format!(
            "target/release/{}ygo_policies{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX,
        )))
        .unwrap();
        let mut duel = core
            .duel(&DuelOptions {
                hand: 0,
                per_turn: 0,
                shuffle: None,
                ..DuelOptions::seeded(2416101)
            })
            .unwrap();
        for player in 0..2 {
            for _ in 0..40 {
                duel.add(Placed {
                    controller: player,
                    code: 69140098,
                    location: 1,
                    sequence: 0,
                    position: 8,
                });
            }
        }
        // Monster Reborn can revive either body. The test utility deliberately
        // prefers Battle Ox to the generic policy's stronger Gemini Elf.
        duel.add(Placed {
            controller: 0,
            code: 83764718,
            location: 2,
            sequence: 0,
            position: 8,
        });
        duel.add(Placed {
            controller: 0,
            code: 69140098,
            location: 0x10,
            sequence: 0,
            position: 1,
        });
        duel.add(Placed {
            controller: 0,
            code: 5053103,
            location: 0x10,
            sequence: 1,
            position: 1,
        });
        let seats = (0..2)
            .map(|player| {
                library
                    .seat("generic", &cards, player, 2416101 + player as u64)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        seats[0].enable_response_overrides().unwrap();
        for (player, seat) in seats.iter().enumerate() {
            seat.feed(&duel.start_message(player as u8)).unwrap();
        }
        duel.start();
        let response = loop {
            let step = duel.step().unwrap();
            let mut answer = None;
            for sent in &step.messages {
                if sent.refresh {
                    seats[0].feed_update_pair(&seats[1], &sent.bytes).unwrap();
                } else {
                    for (player, seat) in seats.iter().enumerate() {
                        if let Some(response) = seat.feed(&sent.bytes).unwrap() {
                            assert_eq!(player, 0);
                            answer = Some(response);
                        }
                    }
                }
            }
            assert!(!matches!(step.state, State::Over(_)));
            if let Some(response) = answer {
                break response;
            }
            assert_ne!(step.state, State::Awaiting);
        };
        let answer = seats[0].last_answer().unwrap();
        assert_eq!(answer["decision"]["kind"], "Idle");
        assert_eq!(
            offered_action(&answer, &response).unwrap()["kind"],
            "Activate"
        );
        let options = PlannerOptions {
            width: 4,
            actions: 6,
            nodes: 64,
            decisions: 128,
            replies: 0,
        };
        let evaluator = |observation: &Value, viewer: u8| {
            if observation["cards"].as_array().unwrap().iter().any(|card| {
                card["at"]["controller"] == viewer
                    && card["at"]["location"] == "MonsterZone"
                    && card["code"] == 5053103
            }) {
                0.8
            } else {
                0.0
            }
        };
        let baseline = evaluate_plan(
            &mut duel,
            &seats,
            0,
            &response,
            0,
            512,
            options,
            None,
            &evaluator,
            &[],
        )
        .unwrap();
        assert_eq!(baseline.failed, 0, "{:?}", baseline.errors);
        assert_eq!(baseline.value, Some(0.0));
        let searched = evaluate(
            &mut duel, &seats, 0, &response, 0, 512, options, None, &evaluator,
        )
        .unwrap();
        assert_eq!(searched.failed, 0, "{:?}", searched.errors);
        assert_eq!(searched.value, Some(0.8));
        assert!(searched.improved_leaves > 0);
        assert!(searched.plan.iter().any(|action| action.action["members"]
            .as_array()
            .is_some_and(|members| members.iter().any(|member| member["code"] == 5053103))));
        let replayed = evaluate_plan(
            &mut duel,
            &seats,
            0,
            &response,
            0,
            512,
            options,
            None,
            &evaluator,
            &searched.plan,
        )
        .unwrap();
        assert_eq!(replayed.failed, 0, "{:?}", replayed.errors);
        assert_eq!(replayed.value, searched.value);
        assert_eq!(replayed.plan_applied, searched.plan.len());
        assert_eq!(replayed.plan_mismatches, 0);
    }
}
