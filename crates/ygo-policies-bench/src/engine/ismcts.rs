//! Online information-set UCT over already sampled native worlds.
//!
//! Every tree decision uses only this player's original visible prompt and
//! visible history. One node is expanded per simulation; the generic pilots
//! finish the rollout. The frozen policy is a proposal, not a demonstrated
//! improvement: callers must validate it against an unchanged generic control
//! on separate common worlds before executing any of its actions.
use super::planner::{prompt_signature, resolve_action, PlannedAction};
use super::*;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub iterations: usize,
    pub actions: usize,
    pub nodes: usize,
    pub decisions: usize,
    pub replies: usize,
    pub terminal: bool,
}

/// The caller samples these worlds without consulting their hidden contents
/// when choosing actions. Seats must have pending root answers, and the viewer
/// must have response overrides enabled before receiving that prompt.
pub(super) struct World<'a> {
    pub snapshot: Snapshot,
    pub seats: Vec<LibrarySeat<'a>>,
}

/// A length-delimited, dual 64-bit digest of viewer-redacted events and own
/// responses since the root. The original root prompt is not fed into this
/// path: start with Default and append the actual root response first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Path {
    first: u64,
    second: u64,
}

impl Default for Path {
    fn default() -> Self {
        Self {
            first: 0xcbf29ce484222325,
            second: 0x84222325cbf29ce4,
        }
    }
}

impl Path {
    fn bytes(&mut self, tag: u8, bytes: &[u8]) {
        for byte in std::iter::once(tag)
            .chain((bytes.len() as u64).to_le_bytes())
            .chain(bytes.iter().copied())
        {
            self.first = (self.first ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            self.second = (self.second ^ u64::from(byte))
                .rotate_left(9)
                .wrapping_mul(0x9e3779b185ebca87);
        }
    }

    /// Feed every sent message exactly once, including field refreshes. Never
    /// hash raw native messages: they contain the opponent's private cards.
    pub fn message(&mut self, raw: &[u8], viewer: u8) -> Result<()> {
        if let Some(visible) = ygo_policies_ocgcore::redact::redact(raw, Some(viewer))
            .map_err(|error| error.to_string())?
        {
            self.bytes(1, &visible);
        }
        Ok(())
    }

    /// Only the viewer's responses enter the path. Opponent private selections
    /// are represented by their subsequent viewer-visible effects instead.
    pub fn own_response(&mut self, response: &[u8]) {
        self.bytes(2, response);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    path: Path,
    signature: Arc<str>,
}

impl Key {
    fn new(original: &Value, path: Path) -> Self {
        Self {
            path,
            signature: prompt_signature(original),
        }
    }
}

#[derive(Clone, Default, Debug)]
pub(super) struct FrozenPolicy {
    choices: HashMap<Key, PlannedAction>,
}

impl FrozenPolicy {
    pub fn len(&self) -> usize {
        self.choices.len()
    }

    pub fn resolve(&self, original: &Value, expanded: &Value, path: Path) -> Option<Vec<u8>> {
        resolve_action(self.choices.get(&Key::new(original, path))?, expanded)
    }

    pub fn has(&self, original: &Value, path: Path) -> bool {
        self.choices.contains_key(&Key::new(original, path))
    }
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub(super) struct Stats {
    pub simulations: usize,
    pub completed: usize,
    pub terminal: usize,
    pub failed: usize,
    pub cutoffs: usize,
    pub nodes: usize,
    pub steps: usize,
    pub tree_hits: usize,
    pub applied: usize,
    pub missing: usize,
    pub mismatches: usize,
    pub errors: Vec<String>,
    #[cfg(test)]
    root_trials: Vec<(usize, usize)>,
    #[cfg(test)]
    observation_reads: usize,
    #[cfg(test)]
    responses: Vec<(usize, Vec<u8>)>,
    #[cfg(test)]
    caller_snapshots: usize,
    #[cfg(test)]
    caller_restores: usize,
    #[cfg(test)]
    world_restores: usize,
}

impl Stats {
    fn error(&mut self, error: String) {
        self.failed += 1;
        if self.errors.len() < 8 && !self.errors.contains(&error) {
            self.errors.push(error);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RootAction {
    pub response: Vec<u8>,
    pub mean: Option<f64>,
    pub visits: usize,
    pub attempts: usize,
}

pub(super) struct SearchResult {
    pub policy: FrozenPolicy,
    pub root_response: Option<Vec<u8>>,
    pub root_actions: Vec<RootAction>,
    pub stats: Stats,
}

#[derive(Default, Debug)]
pub(super) struct Evaluation {
    pub value: Option<f64>,
    pub terminal: bool,
    pub stats: Stats,
}

struct Action {
    planned: PlannedAction,
    visits: usize,
    attempts: usize,
    total: f64,
}

struct Node {
    actions: Vec<Action>,
}

/// UCT selects a root action before any sampled world is inspected. Each
/// action then sees the same world cycle, rather than inheriting the global
/// iteration's world. Scheduled trials advance even when simulation fails or
/// reaches its cap; completed returns alone still determine action values.
#[derive(Default)]
struct RootWorldSchedule {
    scheduled: Vec<usize>,
}

impl RootWorldSchedule {
    fn next(&mut self, action: usize, worlds: usize) -> usize {
        self.scheduled
            .resize(self.scheduled.len().max(action + 1), 0);
        let world = self.scheduled[action] % worlds;
        self.scheduled[action] += 1;
        world
    }
}

impl Node {
    fn select(&self) -> usize {
        if let Some(index) = self.actions.iter().position(|action| action.attempts == 0) {
            return index;
        }
        let attempts: usize = self.actions.iter().map(|action| action.attempts).sum();
        let log = (attempts as f64 + 1.0).ln();
        let mut best = (f64::NEG_INFINITY, 0);
        for (index, action) in self.actions.iter().enumerate() {
            let mean = if action.visits == 0 {
                0.0
            } else {
                action.total / action.visits as f64
            };
            // Attempts prevent a looping/capped generic continuation from
            // monopolizing expansion. Only completed returns enter the mean.
            let uct = mean + std::f64::consts::SQRT_2 * (log / action.attempts as f64).sqrt();
            if uct > best.0 + 1e-12 {
                best = (uct, index);
            }
        }
        best.1
    }

    /// Unmeasured actions have no evidence of an improvement. Exact ties keep
    /// the generic first action; fresh paired validation is still required.
    fn best(&self) -> usize {
        let mut best = 0;
        let mut value = self
            .actions
            .first()
            .filter(|a| a.visits > 0)
            .map_or(0.0, |a| a.total / a.visits as f64);
        for (index, action) in self.actions.iter().enumerate().skip(1) {
            if action.visits > 0 && action.total / action.visits as f64 > value + 1e-12 {
                best = index;
                value = action.total / action.visits as f64;
            }
        }
        best
    }
}

fn bytes(value: &Value) -> Option<Vec<u8>> {
    value
        .as_array()?
        .iter()
        .map(|byte| u8::try_from(byte.as_u64()?).ok())
        .collect()
}

fn given(answer: &Value) -> Result<Vec<u8>> {
    let index = answer["choice"]
        .as_u64()
        .ok_or("ISMCTS prompt lacks original choice")? as usize;
    bytes(&answer["responses"][index]).ok_or_else(|| "ISMCTS prompt lacks original response".into())
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

fn extra_summon_identity(choice: &Value) -> Option<(u64, u64, u64)> {
    if choice["kind"] != "SpecialSummon" || choice["card"]["at"]["location"] != "Extra" {
        return None;
    }
    let code = choice["card"]["code"].as_u64().filter(|code| *code != 0)?;
    Some((
        code,
        choice["description"].as_u64()?,
        choice["card"]["at"]["controller"].as_u64()?,
    ))
}

/// Prefer different Extra Deck summon identities before further copies. This
/// changes ordering only: copies can have different state, so their distinct
/// legal responses remain eligible once the budget reaches them. Unknown-code
/// and non-Extra choices keep their positions within the action-kind group.
fn prioritize_extra_summons(group: &mut [usize], choices: &[Value], generic: usize) {
    let mut seen: Vec<_> = extra_summon_identity(&choices[generic])
        .into_iter()
        .collect();
    let mut positions = Vec::new();
    let mut distinct = Vec::new();
    let mut copies = Vec::new();
    for (position, &index) in group.iter().enumerate() {
        let Some(identity) = extra_summon_identity(&choices[index]) else {
            continue;
        };
        positions.push(position);
        if seen.contains(&identity) {
            copies.push(index);
        } else {
            seen.push(identity);
            distinct.push(index);
        }
    }
    for (position, index) in positions
        .into_iter()
        .zip(distinct.into_iter().chain(copies))
    {
        group[position] = index;
    }
}

/// Generic first, then action-kind diversity, preferring distinct Extra Deck
/// summons within that kind. Sequential candidates remain bounded whole
/// subsets supplied by LibrarySeat, not all combinations.
fn actions(original: &Value, expanded: &Value, maximum: usize) -> Result<Vec<Action>> {
    let original_response = given(original)?;
    let choices = expanded["decision"]["choices"]
        .as_array()
        .ok_or("ISMCTS choices missing")?;
    let responses = expanded["responses"]
        .as_array()
        .ok_or("ISMCTS responses missing")?;
    if choices.len() != responses.len() {
        return Err("ISMCTS choices/responses differ".into());
    }
    let mut offered = Vec::new();
    for (choice, response) in choices.iter().zip(responses) {
        offered.push((
            choice.clone(),
            bytes(response).ok_or("Invalid ISMCTS response")?,
        ));
    }
    let first = offered
        .iter()
        .position(|(_, response)| response == &original_response)
        .ok_or("ISMCTS generic response disappeared")?;
    let mut indices = vec![first];
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    if supported(original) {
        for (index, (choice, response)) in offered.iter().enumerate() {
            if index == first || choice["kind"] == "ShuffleHand" || choice["kind"] == "Cancel" {
                continue;
            }
            if offered[..index]
                .iter()
                .any(|(_, previous)| previous == response)
            {
                continue;
            }
            let kind = choice["kind"].to_string();
            if let Some((_, items)) = groups.iter_mut().find(|(previous, _)| previous == &kind) {
                items.push(index);
            } else {
                groups.push((kind, vec![index]));
            }
        }
        if let Some((_, group)) = groups
            .iter_mut()
            .find(|(kind, _)| kind == "\"SpecialSummon\"")
        {
            prioritize_extra_summons(group, choices, first);
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
        for depth in 0..offered.len() {
            for (_, group) in &groups {
                if indices.len() >= maximum {
                    break;
                }
                if let Some(index) = group.get(depth) {
                    indices.push(*index);
                }
            }
            if indices.len() >= maximum {
                break;
            }
        }
    }
    let signature = prompt_signature(original);
    Ok(indices
        .into_iter()
        .map(|index| Action {
            planned: PlannedAction {
                signature: signature.clone(),
                response: offered[index].1.clone(),
                action: Arc::new(offered[index].0.clone()),
            },
            visits: 0,
            attempts: 0,
            total: 0.0,
        })
        .collect())
}

fn goal_turn(observation: &Value, viewer: u8, replies: usize) -> u64 {
    let own = observation["turn_player"].as_u64() == Some(viewer as u64);
    let advance = if replies == 0 {
        1
    } else if own {
        replies.saturating_mul(2)
    } else {
        replies.saturating_mul(2).saturating_sub(1)
    };
    observation["turn"]
        .as_u64()
        .unwrap_or(0)
        .saturating_add(advance as u64)
}

fn expired(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|end| Instant::now() >= end)
}

enum Mode<'a> {
    Search {
        tree: &'a mut HashMap<Key, Node>,
        root_action: usize,
    },
    Frozen(&'a FrozenPolicy),
}

impl Mode<'_> {
    fn tracks_history(&self, tree_active: bool) -> bool {
        match self {
            Self::Search { .. } => tree_active,
            Self::Frozen(policy) => !policy.choices.is_empty(),
        }
    }
}

struct Trial {
    value: Option<f64>,
    terminal: bool,
    visited: Vec<(Key, usize)>,
}

#[allow(clippy::too_many_arguments)]
fn simulate<const LEGACY_HORIZON: bool>(
    duel: &mut Duel,
    seats: &[LibrarySeat<'_>],
    viewer: u8,
    root_original: &Value,
    forced_root: Option<&[u8]>,
    root_decisions: usize,
    game_limit: usize,
    options: Options,
    deadline: Option<Instant>,
    evaluator: &dyn Fn(&Value, u8) -> f64,
    mut mode: Mode<'_>,
    stats: &mut Stats,
) -> Result<Trial> {
    let seats = seats
        .iter()
        .map(LibrarySeat::fork)
        .collect::<Result<Vec<_>>>()?;
    let mut pending_player = viewer as usize;
    let mut original = root_original.clone();
    let mut response = given(root_original)?;
    let goal = goal_turn(&root_original["observation"], viewer, options.replies);
    // Projection advances its turn exactly once per MSG_NEW_TURN. Keep that
    // small public counter here so pre-horizon prompts do not need to build,
    // serialize and parse an entire observation just to read its turn.
    let mut turn = root_original["observation"]["turn"].as_u64().unwrap_or(0);
    let mut path = Path::default();
    let mut tree_active = true;
    let mut first = true;
    let mut chain = !root_original["observation"]["chain"]
        .as_array()
        .map_or(true, Vec::is_empty);
    let mut trial = Trial {
        value: None,
        terminal: false,
        visited: Vec::new(),
    };
    let mut future = 0usize;
    loop {
        if expired(deadline)
            || future >= options.decisions
            || root_decisions.saturating_add(future) >= game_limit
        {
            stats.cutoffs += 1;
            return Ok(trial);
        }
        if pending_player == viewer as usize {
            if first && forced_root.is_some() {
                // A supplied root can be a whole sequential alternative whose
                // commit state has not yet been cached in this fresh fork.
                seats[pending_player].alternative_responses(12)?;
                response = forced_root.unwrap().to_vec();
            } else {
                match &mut mode {
                    Mode::Search { tree, root_action } if tree_active => {
                        let key = Key::new(&original, path);
                        let expanded = seats[pending_player].alternative_responses(12)?;
                        if !tree.contains_key(&key) && tree.len() < options.nodes {
                            let candidates = actions(&original, &expanded, options.actions)?;
                            // Forced and administrative prompts are executed
                            // and enter visible history, but should not spend
                            // this simulation's one useful tree expansion.
                            if candidates.len() > 1 {
                                tree.insert(
                                    key.clone(),
                                    Node {
                                        actions: candidates,
                                    },
                                );
                                tree_active = false;
                            }
                        }
                        if let Some(node) = tree.get_mut(&key) {
                            // The root was selected before its world. Keep it
                            // on the ordinary tree path so attempts, visited
                            // edges and completed backpropagation are intact.
                            let selected = if first { *root_action } else { node.select() };
                            let planned = &node
                                .actions
                                .get(selected)
                                .ok_or("ISMCTS scheduled root action is unavailable")?
                                .planned;
                            response = resolve_action(planned, &expanded)
                                .ok_or("ISMCTS stored action differs from freshly expanded legal alternatives")?;
                            node.actions[selected].attempts += 1;
                            trial.visited.push((key, selected));
                            stats.tree_hits += 1;
                        } else if tree.len() >= options.nodes {
                            tree_active = false;
                        }
                    }
                    Mode::Frozen(policy) if !policy.choices.is_empty() => {
                        if policy.has(&original, path) {
                            let expanded = seats[pending_player].alternative_responses(12)?;
                            if let Some(planned) = policy.resolve(&original, &expanded, path) {
                                response = planned;
                                stats.applied += 1;
                            } else {
                                stats.mismatches += 1;
                            }
                        } else {
                            stats.missing += 1;
                        }
                    }
                    _ => {}
                }
            }
            seats[pending_player].override_response(&response)?;
            if mode.tracks_history(tree_active) {
                path.own_response(&response);
            }
        }
        #[cfg(test)]
        stats.responses.push((pending_player, response.clone()));
        duel.respond(&response);
        first = false;
        future += 1;
        stats.steps += 1;
        loop {
            if expired(deadline) {
                stats.cutoffs += 1;
                return Ok(trial);
            }
            let step = duel.step()?;
            let mut answer = None;
            let mut quiescent = false;
            for sent in &step.messages {
                if mode.tracks_history(tree_active) {
                    path.message(&sent.bytes, viewer)?;
                }
                if sent.refresh {
                    seats[0].feed_update_pair(&seats[1], &sent.bytes)?;
                    continue;
                }
                match sent.bytes[0] {
                    msg::RETRY => return Err("Engine rejected ISMCTS response (MSG_RETRY)".into()),
                    msg::CHAINING => chain = true,
                    msg::CHAIN_END => chain = false,
                    msg::NEW_TURN => turn += 1,
                    msg::SELECT_IDLECMD | msg::SELECT_BATTLECMD => quiescent = true,
                    _ => {}
                }
                for (player, seat) in seats.iter().enumerate() {
                    if let Some(given) = seat.feed(&sent.bytes)? {
                        if answer.replace((player, given)).is_some() {
                            return Err("Multiple ISMCTS answers in one batch".into());
                        }
                    }
                }
            }
            if let State::Over(outcome) = step.state {
                let outcome = outcome.ok_or("ISMCTS ended without MSG_WIN")?;
                trial.value = Some(if outcome.winner > 1 {
                    0.0
                } else if outcome.winner == viewer {
                    1.0
                } else {
                    -1.0
                });
                trial.terminal = true;
                return Ok(trial);
            }
            if let Some((player, given)) = answer {
                if !options.terminal && quiescent && !chain && (LEGACY_HORIZON || turn >= goal) {
                    #[cfg(test)]
                    {
                        stats.observation_reads += 1;
                    }
                    let observation = seats[viewer as usize].observation()?;
                    debug_assert_eq!(observation["turn"].as_u64().unwrap_or(0), turn);
                    if observation["turn"].as_u64().unwrap_or(0) >= goal {
                        let value = evaluator(&observation, viewer);
                        if !value.is_finite() {
                            return Err("ISMCTS evaluator returned a non-finite value".into());
                        }
                        trial.value = Some(value.clamp(-0.999_999, 0.999_999));
                        return Ok(trial);
                    }
                }
                pending_player = player;
                response = given;
                if player == viewer as usize && mode.tracks_history(tree_active) {
                    original = seats[player].last_answer()?;
                }
                break;
            }
            if step.state == State::Awaiting {
                return Err("ISMCTS awaits an unanswered prompt".into());
            }
        }
    }
}

fn validate(worlds: &[World<'_>], viewer: u8, options: Options) -> Result<()> {
    if viewer > 1 || worlds.is_empty() || worlds.iter().any(|world| world.seats.len() != 2) {
        return Err("ISMCTS requires sampled worlds with two seats and viewer0/1".into());
    }
    if options.iterations == 0
        || options.actions == 0
        || options.nodes == 0
        || options.decisions == 0
    {
        return Err("ISMCTS iterations/actions/nodes/decisions must be positive".into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn search(
    duel: &mut Duel,
    worlds: &[World<'_>],
    viewer: u8,
    root_original: &Value,
    root_decisions: usize,
    game_limit: usize,
    options: Options,
    deadline: Option<Instant>,
    evaluator: &dyn Fn(&Value, u8) -> f64,
) -> Result<SearchResult> {
    search_impl::<false>(
        duel,
        worlds,
        viewer,
        root_original,
        root_decisions,
        game_limit,
        options,
        deadline,
        evaluator,
    )
}

// The true specialization is instantiated only by native differential tests.
// Production has no runtime switch and always skips pre-horizon observations.
#[allow(clippy::too_many_arguments)]
fn search_impl<const LEGACY_HORIZON: bool>(
    duel: &mut Duel,
    worlds: &[World<'_>],
    viewer: u8,
    root_original: &Value,
    root_decisions: usize,
    game_limit: usize,
    options: Options,
    deadline: Option<Instant>,
    evaluator: &dyn Fn(&Value, u8) -> f64,
) -> Result<SearchResult> {
    validate(worlds, viewer, options)?;
    let root = duel.snapshot()?;
    let mut tree = HashMap::<Key, Node>::new();
    let root_key = Key::new(root_original, Path::default());
    let mut schedule = RootWorldSchedule::default();
    let mut stats = Stats::default();
    let result = (|| -> Result<()> {
        for _ in 0..options.iterations {
            if expired(deadline) {
                break;
            }
            // An unseen root expands generic-first, so its first action is 0.
            // No hidden world data participates in this selection.
            let root_action = tree.get(&root_key).map_or(0, Node::select);
            let world_index = schedule.next(root_action, worlds.len());
            let world = &worlds[world_index];
            duel.restore(&world.snapshot)?;
            stats.simulations += 1;
            #[cfg(test)]
            stats.root_trials.push((root_action, world_index));
            match simulate::<LEGACY_HORIZON>(
                duel,
                &world.seats,
                viewer,
                root_original,
                None,
                root_decisions,
                game_limit,
                options,
                deadline,
                evaluator,
                Mode::Search {
                    tree: &mut tree,
                    root_action,
                },
                &mut stats,
            ) {
                Ok(trial) => {
                    if let Some(value) = trial.value {
                        stats.completed += 1;
                        stats.terminal += usize::from(trial.terminal);
                        for (key, index) in trial.visited {
                            let action = &mut tree.get_mut(&key).unwrap().actions[index];
                            action.visits += 1;
                            action.total += value;
                        }
                    }
                }
                Err(error) => stats.error(error),
            }
        }
        Ok(())
    })();
    duel.restore(&root)?;
    result?;
    stats.nodes = tree.len();
    let root_response = tree
        .get(&root_key)
        .map(|node| node.actions[node.best()].planned.response.clone());
    let root_actions = tree
        .get(&root_key)
        .map(|node| {
            node.actions
                .iter()
                .map(|action| RootAction {
                    response: action.planned.response.clone(),
                    visits: action.visits,
                    attempts: action.attempts,
                    mean: (action.visits > 0).then(|| action.total / action.visits as f64),
                })
                .collect()
        })
        .unwrap_or_default();
    let policy = FrozenPolicy {
        choices: tree
            .into_iter()
            .filter_map(|(key, node)| {
                let best = node.best();
                (node.actions[best].visits > 0).then(|| (key, node.actions[best].planned.clone()))
            })
            .collect(),
    };
    Ok(SearchResult {
        policy,
        root_response,
        root_actions,
        stats,
    })
}

/// Fixed-policy validation never updates its decisions from the sampled
/// world's returns. An empty map is the unchanged generic continuation.
/// Restores the caller's original duel state before returning.
#[allow(dead_code, clippy::too_many_arguments)]
pub(super) fn evaluate_frozen(
    duel: &mut Duel,
    world: &World<'_>,
    viewer: u8,
    root_original: &Value,
    forced_root: &[u8],
    root_decisions: usize,
    game_limit: usize,
    options: Options,
    deadline: Option<Instant>,
    evaluator: &dyn Fn(&Value, u8) -> f64,
    policy: &FrozenPolicy,
) -> Result<Evaluation> {
    evaluate_frozen_impl::<false, true>(
        duel,
        world,
        viewer,
        root_original,
        forced_root,
        root_decisions,
        game_limit,
        options,
        deadline,
        evaluator,
        policy,
    )
}

/// Validation for a caller that already owns a root snapshot and guarantees
/// its restoration on every exit. Restores the sampled world before each
/// trial, but leaves the duel at that trial's terminal, cutoff or error state.
/// Do not inspect or continue this duel before restoring a known snapshot.
#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_frozen_in_place(
    duel: &mut Duel,
    world: &World<'_>,
    viewer: u8,
    root_original: &Value,
    forced_root: &[u8],
    root_decisions: usize,
    game_limit: usize,
    options: Options,
    deadline: Option<Instant>,
    evaluator: &dyn Fn(&Value, u8) -> f64,
    policy: &FrozenPolicy,
) -> Result<Evaluation> {
    evaluate_frozen_impl::<false, false>(
        duel,
        world,
        viewer,
        root_original,
        forced_root,
        root_decisions,
        game_limit,
        options,
        deadline,
        evaluator,
        policy,
    )
}

#[allow(clippy::too_many_arguments)]
fn evaluate_frozen_impl<const LEGACY_HORIZON: bool, const RESTORE_CALLER: bool>(
    duel: &mut Duel,
    world: &World<'_>,
    viewer: u8,
    root_original: &Value,
    forced_root: &[u8],
    root_decisions: usize,
    game_limit: usize,
    options: Options,
    deadline: Option<Instant>,
    evaluator: &dyn Fn(&Value, u8) -> f64,
    policy: &FrozenPolicy,
) -> Result<Evaluation> {
    validate(std::slice::from_ref(world), viewer, options)?;
    let root = if RESTORE_CALLER {
        Some(duel.snapshot()?)
    } else {
        None
    };
    let mut stats = Stats::default();
    #[cfg(test)]
    {
        stats.caller_snapshots = usize::from(RESTORE_CALLER);
    }
    let result = (|| -> Result<Trial> {
        duel.restore(&world.snapshot)?;
        #[cfg(test)]
        {
            stats.world_restores += 1;
        }
        stats.simulations = 1;
        simulate::<LEGACY_HORIZON>(
            duel,
            &world.seats,
            viewer,
            root_original,
            Some(forced_root),
            root_decisions,
            game_limit,
            options,
            deadline,
            evaluator,
            Mode::Frozen(policy),
            &mut stats,
        )
    })();
    if let Some(root) = root {
        duel.restore(&root)?;
        #[cfg(test)]
        {
            stats.caller_restores += 1;
        }
    }
    match result {
        Ok(trial) => {
            stats.completed = usize::from(trial.value.is_some());
            stats.terminal = usize::from(trial.terminal);
            Ok(Evaluation {
                value: trial.value,
                terminal: trial.terminal,
                stats,
            })
        }
        Err(error) => {
            stats.error(error);
            Ok(Evaluation {
                stats,
                ..Evaluation::default()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_stats_equivalent(optimized: &Stats, legacy: &Stats) {
        assert!(optimized.observation_reads <= legacy.observation_reads);
        let mut optimized = optimized.clone();
        let mut legacy = legacy.clone();
        optimized.observation_reads = 0;
        legacy.observation_reads = 0;
        // Includes exact response traces, world allocations, failures, caps
        // and completed/terminal counts, not only the winning root action.
        assert_eq!(optimized, legacy);
    }

    fn assert_search_equivalent(optimized: &SearchResult, legacy: &SearchResult) {
        assert_eq!(optimized.root_response, legacy.root_response);
        assert_eq!(optimized.root_actions, legacy.root_actions);
        assert_stats_equivalent(&optimized.stats, &legacy.stats);
        assert_eq!(optimized.policy.len(), legacy.policy.len());
        for (key, action) in &optimized.policy.choices {
            let reference = legacy.policy.choices.get(key).unwrap();
            assert_eq!(action.signature, reference.signature);
            assert_eq!(action.response, reference.response);
            assert_eq!(action.action, reference.action);
        }
    }

    fn assert_evaluation_equivalent(optimized: &Evaluation, legacy: &Evaluation) {
        assert_eq!(optimized.value, legacy.value);
        assert_eq!(optimized.terminal, legacy.terminal);
        assert_stats_equivalent(&optimized.stats, &legacy.stats);
    }

    fn assert_in_place_equivalent(in_place: &Evaluation, restoring: &Evaluation) {
        assert_eq!(in_place.value, restoring.value);
        assert_eq!(in_place.terminal, restoring.terminal);
        assert_eq!(restoring.stats.caller_snapshots, 1);
        assert_eq!(restoring.stats.caller_restores, 1);
        assert_eq!(in_place.stats.caller_snapshots, 0);
        assert_eq!(in_place.stats.caller_restores, 0);
        let mut expected = restoring.stats.clone();
        expected.caller_snapshots = 0;
        expected.caller_restores = 0;
        assert_eq!(in_place.stats, expected);
    }

    fn answer() -> Value {
        serde_json::json!({"observation":{"turn":1,"cards":[]},"decision":{"kind":"YesNo","choices":[{"kind":"No"},{"kind":"Yes"}]},"responses":[[0],[1]],"choice":0})
    }

    fn extra_summon(code: u32, sequence: usize) -> Value {
        serde_json::json!({
            "kind": "SpecialSummon", "description": 0,
            "card": {"code": code, "at": {
                "controller": 1, "location": "Extra", "sequence": sequence
            }},
            "members": [], "place": null
        })
    }

    #[test]
    fn extra_summon_diversity_keeps_distinct_trace_choices_before_copies() {
        // Archived v7 Fabled seed 1072687461, seat 1, decision 118: the pilot
        // chose Valkyrus with an empty hand while two Ragins and two Unicores
        // consumed scarce search slots ahead of other legal Synchro summons.
        let mut choices: Vec<_> = [
            47395382, 47395382, 44155002, 44155002, 54048462, 26593852, 50321796, 73580471,
            44508094, 76774528, 52687916, 27315304, 43385557, 29071332,
        ]
        .into_iter()
        .enumerate()
        .map(|(sequence, code)| extra_summon(code, sequence))
        .collect();
        choices.push(serde_json::json!({"kind": "EnterBattle"}));
        choices.push(serde_json::json!({"kind": "EndTurn"}));
        let responses: Vec<_> = (0..choices.len()).map(|index| vec![index as u8]).collect();
        let original = serde_json::json!({
            "observation": {"turn": 2, "cards": []},
            "decision": {"kind": "Idle", "choices": choices},
            "responses": responses, "choice": 4
        });
        let retained = |maximum| {
            actions(&original, &original, maximum)
                .unwrap()
                .into_iter()
                .map(|action| action.planned.response[0])
                .collect::<Vec<_>>()
        };
        // Keep the generic choice and phase diversity, but admit Brionac and
        // Black Rose at eight choices, and Trishula and Mist Wurm at twelve.
        assert_eq!(retained(1), vec![4]);
        assert_eq!(retained(8), vec![4, 0, 14, 15, 2, 5, 6, 7]);
        assert_eq!(retained(12), vec![4, 0, 14, 15, 2, 5, 6, 7, 8, 9, 10, 11]);
        let mut all = retained(32);
        assert_eq!(all[0], 4);
        assert_eq!(&all[all.len() - 2..], &[1, 3]);
        all.sort_unstable();
        assert_eq!(all, (0..16).collect::<Vec<_>>());
    }

    #[test]
    fn extra_summon_order_keeps_unknown_and_other_locations_in_place() {
        let mut choices: Vec<_> = (0..8).map(|sequence| extra_summon(100, sequence)).collect();
        choices[2]["card"]["code"] = Value::Null;
        choices[3]["description"] = serde_json::json!(1);
        choices[4]["card"]["at"]["controller"] = serde_json::json!(0);
        choices[5]["card"]["at"]["location"] = serde_json::json!("Hand");
        choices[6]["card"]["code"] = serde_json::json!(200);
        choices[7]["card"]["code"] = serde_json::json!(0);
        let mut group = vec![1, 2, 3, 4, 5, 6, 7];
        prioritize_extra_summons(&mut group, &choices, 0);
        // Different effect descriptions and controllers are distinct. The
        // generic identity already counts as seen, but its copy is deferred,
        // not removed. Unknown codes and the hand summon do not move.
        assert_eq!(group, vec![3, 2, 4, 6, 5, 1, 7]);
        choices[1]["kind"] = serde_json::json!("Activate");
        let mut unaffected = vec![1, 2, 5, 7];
        prioritize_extra_summons(&mut unaffected, &choices, 0);
        assert_eq!(unaffected, vec![1, 2, 5, 7]);
    }

    #[test]
    fn path_hides_opponent_draws_and_private_prompts() {
        let draw = |code: u32| {
            let mut message = vec![msg::DRAW, 1];
            message.extend_from_slice(&1u32.to_le_bytes());
            message.extend_from_slice(&code.to_le_bytes());
            message.extend_from_slice(&position::FACEDOWN.to_le_bytes());
            message
        };
        let mut a = Path::default();
        let mut b = Path::default();
        a.message(&draw(123), 0).unwrap();
        b.message(&draw(456), 0).unwrap();
        assert_eq!(a, b);
        let mut own_a = Path::default();
        let mut own_b = Path::default();
        own_a.message(&draw(123), 1).unwrap();
        own_b.message(&draw(456), 1).unwrap();
        assert_ne!(own_a, own_b);
        let before = a;
        a.message(&[msg::SELECT_YESNO, 1], 0).unwrap();
        assert_eq!(a, before);
        a.own_response(&[0]);
        b.own_response(&[1]);
        assert_ne!(a, b);
    }

    #[test]
    fn frozen_policy_requires_exact_information_path_and_offered_action() {
        let original = answer();
        let candidates = actions(&original, &original, 2).unwrap();
        let mut policy = FrozenPolicy::default();
        policy.choices.insert(
            Key::new(&original, Path::default()),
            candidates[1].planned.clone(),
        );
        assert_eq!(
            policy.resolve(&original, &original, Path::default()),
            Some(vec![1])
        );
        let mut different = Path::default();
        different.own_response(&[0]);
        assert!(policy.resolve(&original, &original, different).is_none());
        let mut changed = original.clone();
        changed["responses"][1] = serde_json::json!([2]);
        assert!(policy
            .resolve(&original, &changed, Path::default())
            .is_none());
    }

    #[test]
    fn capped_trials_do_not_score_and_do_not_starve_other_actions() {
        let original = answer();
        let mut node = Node {
            actions: actions(&original, &original, 2).unwrap(),
        };
        assert_eq!(node.select(), 0);
        node.actions[0].attempts += 1;
        assert_eq!(node.select(), 1);
        assert_eq!(node.best(), 0);
        node.actions[0].visits = 2;
        node.actions[0].total = 1.0;
        node.actions[1].visits = 2;
        node.actions[1].total = 1.0;
        assert_eq!(node.best(), 0);
        node.actions[1].total = 2.0;
        assert_eq!(node.best(), 1);
    }

    fn assert_balanced_worlds(trials: &[(usize, usize)], worlds: usize) {
        let mut counts = HashMap::<usize, Vec<usize>>::new();
        for &(action, world) in trials {
            let counts = counts.entry(action).or_insert_with(|| vec![0; worlds]);
            // Check every prefix, not merely an accidentally balanced total.
            assert_eq!(world, counts.iter().sum::<usize>() % worlds);
            counts[world] += 1;
            assert!(counts.iter().max().unwrap() - counts.iter().min().unwrap() <= 1);
        }
    }

    #[test]
    fn root_world_cycles_remove_action_order_bias_from_world_only_returns() {
        let run = |balanced: bool| {
            let original = answer();
            let mut node = Node {
                actions: actions(&original, &original, 2).unwrap(),
            };
            let mut schedule = RootWorldSchedule::default();
            let mut trials = Vec::new();
            for iteration in 0..8 {
                let action = node.select();
                let world = if balanced {
                    schedule.next(action, 2)
                } else {
                    iteration % 2
                };
                // Both actions have identical outcomes. Only hidden-world
                // difficulty differs, so neither action merits a higher mean.
                let value = if world == 0 { 1.0 } else { -1.0 };
                node.actions[action].attempts += 1;
                node.actions[action].visits += 1;
                node.actions[action].total += value;
                trials.push((action, world));
            }
            (node, trials)
        };
        let (legacy, _) = run(false);
        let legacy_means: Vec<_> = legacy
            .actions
            .iter()
            .map(|a| a.total / a.visits as f64)
            .collect();
        assert!(legacy_means[0] - legacy_means[1] > 1.0, "{legacy_means:?}");
        let (balanced, trials) = run(true);
        assert_balanced_worlds(&trials, 2);
        assert!(balanced
            .actions
            .iter()
            .all(|action| action.visits == 4 && action.total == 0.0));
        assert_eq!(balanced.best(), 0);
    }

    #[test]
    fn scheduled_worlds_advance_independently_of_tree_visits_and_completion() {
        for worlds in [1, 3, 4] {
            let mut schedule = RootWorldSchedule::default();
            let mut trials = Vec::new();
            // Unequal, adaptive allocation, including the same root selected
            // again after a failure before its tree attempt was recorded.
            for action in [0, 0, 0, 1, 2, 0, 5, 1, 5, 5, 0, 2, 1, 1, 0] {
                trials.push((action, schedule.next(action, worlds)));
            }
            assert_balanced_worlds(&trials, worlds);
            assert_eq!(schedule.scheduled.iter().sum::<usize>(), trials.len());
            assert_eq!(schedule.scheduled[0], 6);
        }
    }

    /// Native engine + real FFI seats: deterministic search, executable
    /// conditional continuations, caps/errors, and exact caller restoration.
    #[test]
    #[ignore = "build the release policy library first"]
    fn native_search_replays_conditionals_and_restores_after_errors() {
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
                main: [vec![69140098; 20], vec![55144522; 20]].concat(),
                extra: vec![],
            },
            Deck {
                main: vec![5053103; 40],
                extra: vec![],
            },
        ];
        let mut duel = core.deal(&DuelOptions::seeded(2493000), &decks).unwrap();
        let seats = (0..2)
            .map(|player| {
                library
                    .seat("generic", &cards, player, 2493000 + player as u64)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        seats[0].enable_response_overrides().unwrap();
        for (player, seat) in seats.iter().enumerate() {
            seat.feed(&duel.start_message(player as u8)).unwrap();
        }
        let (root_response, decisions) = {
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
                            if let Some(response) = seat.feed(&sent.bytes).unwrap() {
                                assert!(pending.replace((player, response)).is_none());
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
        let original = seats[0].last_answer().unwrap();
        let snapshot = duel.snapshot().unwrap();
        let next_bytes = |duel: &mut Duel<'_>| {
            duel.respond(&root_response);
            duel.step()
                .unwrap()
                .messages
                .into_iter()
                .map(|sent| (sent.refresh, sent.bytes))
                .collect::<Vec<_>>()
        };
        let expected = next_bytes(&mut duel);
        duel.restore(&snapshot).unwrap();
        let world = World { snapshot, seats };
        let mut worlds = Vec::new();
        for seed in [2493001, 2493002, 2493003] {
            duel.restore(&world.snapshot).unwrap();
            duel.reseed(seed).unwrap();
            worlds.push(World {
                snapshot: duel.snapshot().unwrap(),
                seats: world
                    .seats
                    .iter()
                    .map(LibrarySeat::fork)
                    .collect::<Result<Vec<_>>>()
                    .unwrap(),
            });
        }
        duel.restore(&world.snapshot).unwrap();
        let options = Options {
            iterations: 32,
            actions: 3,
            nodes: 64,
            decisions: 128,
            replies: 0,
            terminal: false,
        };
        let evaluator = |observation: &Value, viewer: u8| {
            observation["cards"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|card| {
                    card["at"]["controller"].as_u64() == Some(viewer as u64)
                        && card["at"]["location"] == "MonsterZone"
                })
                .count() as f64
                * 0.2
        };
        let first = search(
            &mut duel, &worlds, 0, &original, decisions, 1024, options, None, &evaluator,
        )
        .unwrap();
        assert_eq!(first.stats.failed, 0, "{:?}", first.stats.errors);
        assert_eq!(first.stats.completed, options.iterations);
        assert_balanced_worlds(&first.stats.root_trials, worlds.len());
        assert_eq!(first.stats.root_trials.len(), options.iterations);
        assert_eq!(
            first
                .root_actions
                .iter()
                .map(|action| action.attempts)
                .sum::<usize>(),
            options.iterations
        );
        assert_eq!(
            first
                .root_actions
                .iter()
                .map(|action| action.visits)
                .sum::<usize>(),
            first.stats.completed
        );
        assert!(first.stats.nodes > 1);
        assert!(first.policy.len() > 1);
        assert_eq!(world.seats[0].last_answer().unwrap(), original);
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        let legacy = search_impl::<true>(
            &mut duel, &worlds, 0, &original, decisions, 1024, options, None, &evaluator,
        )
        .unwrap();
        assert_search_equivalent(&first, &legacy);
        assert_eq!(first.stats.observation_reads, first.stats.completed);
        assert!(first.stats.observation_reads < legacy.stats.observation_reads);
        eprintln!(
            "horizon replies=0: observation reads {} -> {} across {} identical simulations",
            legacy.stats.observation_reads, first.stats.observation_reads, options.iterations
        );
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        let replayed = evaluate_frozen(
            &mut duel,
            &world,
            0,
            &original,
            first.root_response.as_ref().unwrap(),
            decisions,
            1024,
            options,
            None,
            &evaluator,
            &first.policy,
        )
        .unwrap();
        assert!(replayed.value.is_some());
        assert_eq!(replayed.stats.failed, 0, "{:?}", replayed.stats.errors);
        assert!(replayed.stats.applied > 0);
        assert_eq!(replayed.stats.mismatches, 0);
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        let legacy_replayed = evaluate_frozen_impl::<true, true>(
            &mut duel,
            &world,
            0,
            &original,
            first.root_response.as_ref().unwrap(),
            decisions,
            1024,
            options,
            None,
            &evaluator,
            &first.policy,
        )
        .unwrap();
        assert_evaluation_equivalent(&replayed, &legacy_replayed);
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        let failed = evaluate_frozen(
            &mut duel,
            &world,
            0,
            &original,
            &root_response,
            decisions,
            1024,
            options,
            None,
            &|_, _| f64::NAN,
            &FrozenPolicy::default(),
        )
        .unwrap();
        assert_eq!(failed.stats.failed, 1);
        assert_eq!(failed.value, None);
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        let legacy_failed = evaluate_frozen_impl::<true, true>(
            &mut duel,
            &world,
            0,
            &original,
            &root_response,
            decisions,
            1024,
            options,
            None,
            &|_, _| f64::NAN,
            &FrozenPolicy::default(),
        )
        .unwrap();
        assert_evaluation_equivalent(&failed, &legacy_failed);
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        let failed_search = search(
            &mut duel,
            &worlds,
            0,
            &original,
            decisions,
            1024,
            Options {
                iterations: 9,
                ..options
            },
            None,
            &|_, _| f64::NAN,
        )
        .unwrap();
        assert_eq!(failed_search.stats.failed, 9);
        assert_eq!(failed_search.stats.completed, 0);
        assert_balanced_worlds(&failed_search.stats.root_trials, worlds.len());
        assert_eq!(
            failed_search
                .root_actions
                .iter()
                .map(|action| action.attempts)
                .sum::<usize>(),
            9
        );
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        let legacy_failed_search = search_impl::<true>(
            &mut duel,
            &worlds,
            0,
            &original,
            decisions,
            1024,
            Options {
                iterations: 9,
                ..options
            },
            None,
            &|_, _| f64::NAN,
        )
        .unwrap();
        assert_search_equivalent(&failed_search, &legacy_failed_search);
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        let capped = search(
            &mut duel,
            &worlds,
            0,
            &original,
            decisions,
            1024,
            Options {
                decisions: 1,
                ..options
            },
            None,
            &evaluator,
        )
        .unwrap();
        assert_eq!(capped.stats.completed, 0);
        assert_eq!(capped.stats.cutoffs, options.iterations);
        assert_balanced_worlds(&capped.stats.root_trials, worlds.len());
        assert_eq!(
            capped
                .root_actions
                .iter()
                .map(|action| action.attempts)
                .sum::<usize>(),
            options.iterations
        );
        assert!(capped
            .root_actions
            .iter()
            .take(3)
            .all(|action| action.attempts > 0));
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        let legacy_capped = search_impl::<true>(
            &mut duel,
            &worlds,
            0,
            &original,
            decisions,
            1024,
            Options {
                decisions: 1,
                ..options
            },
            None,
            &evaluator,
        )
        .unwrap();
        assert_search_equivalent(&capped, &legacy_capped);
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();

        // This draw-heavy fixture can end before a longer horizon. Terminal
        // results must remain exact and must not request a horizon observation.
        for (replies, terminal) in [(1, false), (2, false), (0, true)] {
            let options = Options {
                iterations: 6,
                replies,
                terminal,
                ..options
            };
            let optimized = search(
                &mut duel, &worlds, 0, &original, decisions, 1024, options, None, &evaluator,
            )
            .unwrap();
            assert_eq!(next_bytes(&mut duel), expected);
            duel.restore(&world.snapshot).unwrap();
            let legacy = search_impl::<true>(
                &mut duel, &worlds, 0, &original, decisions, 1024, options, None, &evaluator,
            )
            .unwrap();
            assert_search_equivalent(&optimized, &legacy);
            assert_eq!(optimized.stats.failed, 0, "{:?}", optimized.stats.errors);
            if terminal {
                assert_eq!(optimized.stats.observation_reads, 0);
                assert!(optimized.stats.terminal > 0);
                assert_eq!(optimized.stats.completed, optimized.stats.terminal);
            } else {
                assert!(optimized.stats.completed > 0);
                assert!(optimized.stats.observation_reads < legacy.stats.observation_reads);
            }
            eprintln!(
                "horizon replies={replies}, terminal={terminal}: observation reads {} -> {} across {} identical simulations",
                legacy.stats.observation_reads, optimized.stats.observation_reads, options.iterations
            );
            assert_eq!(next_bytes(&mut duel), expected);
            duel.restore(&world.snapshot).unwrap();
        }

        // Compare a whole validation batch. The in-place version deliberately
        // does not restore between trials, including after terminal and error
        // results. Its next trial must reset the world before touching it.
        let mut foreign_duel = core.deal(&DuelOptions::seeded(2493200), &decks).unwrap();
        loop {
            let step = foreign_duel.step().unwrap();
            if step.state == State::Awaiting {
                break;
            }
            assert!(!matches!(step.state, State::Over(_)));
        }
        let foreign_world = World {
            snapshot: foreign_duel.snapshot().unwrap(),
            seats: world
                .seats
                .iter()
                .map(LibrarySeat::fork)
                .collect::<Result<Vec<_>>>()
                .unwrap(),
        };
        let control = FrozenPolicy::default();
        let frozen_root = first.root_response.as_deref().unwrap();
        let bad_evaluator = |_: &Value, _: u8| f64::NAN;
        let evaluator: &dyn Fn(&Value, u8) -> f64 = &evaluator;
        let trial_cases = [
            (
                "control",
                &worlds[0],
                &control,
                root_response.as_slice(),
                options,
                None,
                evaluator,
            ),
            (
                "frozen",
                &worlds[1],
                &first.policy,
                frozen_root,
                options,
                None,
                evaluator,
            ),
            (
                "cap",
                &world,
                &control,
                root_response.as_slice(),
                Options {
                    decisions: 1,
                    ..options
                },
                None,
                evaluator,
            ),
            (
                "expired",
                &world,
                &control,
                root_response.as_slice(),
                options,
                Some(Instant::now() - std::time::Duration::from_secs(1)),
                evaluator,
            ),
            (
                "evaluator error",
                &world,
                &control,
                root_response.as_slice(),
                options,
                None,
                &bad_evaluator,
            ),
            (
                "after error",
                &worlds[2],
                &control,
                root_response.as_slice(),
                options,
                None,
                evaluator,
            ),
            (
                "terminal",
                &world,
                &control,
                root_response.as_slice(),
                Options {
                    terminal: true,
                    ..options
                },
                None,
                evaluator,
            ),
            (
                "foreign snapshot",
                &foreign_world,
                &control,
                root_response.as_slice(),
                options,
                None,
                evaluator,
            ),
            (
                "after foreign snapshot",
                &world,
                &first.policy,
                frozen_root,
                options,
                None,
                evaluator,
            ),
        ];
        let mut references = Vec::new();
        for (_, trial_world, policy, response, options, deadline, evaluate) in &trial_cases {
            references.push(
                evaluate_frozen(
                    &mut duel,
                    trial_world,
                    0,
                    &original,
                    response,
                    decisions,
                    1024,
                    *options,
                    *deadline,
                    evaluate,
                    policy,
                )
                .unwrap(),
            );
            assert_eq!(next_bytes(&mut duel), expected);
            duel.restore(&world.snapshot).unwrap();
        }
        // This is the same restoration boundary as choose_mcts: all work is
        // captured first, then the live root is restored before handling it.
        let result = (|| -> Result<Vec<Evaluation>> {
            trial_cases
                .iter()
                .map(
                    |(_, trial_world, policy, response, options, deadline, evaluate)| {
                        evaluate_frozen_in_place(
                            &mut duel,
                            trial_world,
                            0,
                            &original,
                            response,
                            decisions,
                            1024,
                            *options,
                            *deadline,
                            evaluate,
                            policy,
                        )
                    },
                )
                .collect()
        })();
        duel.restore(&world.snapshot).unwrap();
        let in_place = result.unwrap();
        for (((name, ..), reference), actual) in trial_cases.iter().zip(&references).zip(&in_place)
        {
            assert_in_place_equivalent(actual, reference);
            match *name {
                "cap" => {
                    assert_eq!(actual.value, None);
                    assert_eq!(actual.stats.cutoffs, 1);
                }
                "expired" => {
                    assert_eq!(actual.value, None);
                    assert_eq!(actual.stats.simulations, 1);
                    assert_eq!(actual.stats.cutoffs, 1);
                    assert_eq!(actual.stats.steps, 0);
                }
                "evaluator error" => {
                    assert_eq!(actual.value, None);
                    assert_eq!(actual.stats.failed, 1);
                    assert!(actual.stats.errors[0].contains("non-finite"));
                }
                "foreign snapshot" => {
                    assert_eq!(actual.value, None);
                    assert_eq!(actual.stats.failed, 1);
                    assert_eq!(actual.stats.simulations, 0);
                    assert_eq!(actual.stats.world_restores, 0);
                    assert!(actual.stats.errors[0].contains("OCG_DuelRestoreSnapshot"));
                }
                "terminal" => assert!(actual.terminal),
                _ => assert!(actual.value.is_some(), "{name}: {:?}", actual.stats.errors),
            }
        }
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        assert_eq!(world.seats[0].last_answer().unwrap(), original);
        eprintln!(
            "in-place validation: {} caller snapshots and {} caller restores removed across {} identical trials",
            references.iter().map(|trial| trial.stats.caller_snapshots).sum::<usize>(),
            references.iter().map(|trial| trial.stats.caller_restores).sum::<usize>(),
            in_place.len()
        );

        // Validation errors remain outer errors. Exercise that exit while the
        // duel is terminal from a prior in-place trial, then restore the root.
        let invalid_options = Options {
            iterations: 0,
            ..options
        };
        let reference_error = evaluate_frozen(
            &mut duel,
            &world,
            0,
            &original,
            &root_response,
            decisions,
            1024,
            invalid_options,
            None,
            evaluator,
            &control,
        )
        .unwrap_err();
        let terminal = evaluate_frozen_in_place(
            &mut duel,
            &world,
            0,
            &original,
            &root_response,
            decisions,
            1024,
            Options {
                terminal: true,
                ..options
            },
            None,
            evaluator,
            &control,
        )
        .unwrap();
        assert!(terminal.terminal);
        let error = evaluate_frozen_in_place(
            &mut duel,
            &world,
            0,
            &original,
            &root_response,
            decisions,
            1024,
            invalid_options,
            None,
            evaluator,
            &control,
        );
        duel.restore(&world.snapshot).unwrap();
        assert_eq!(error.unwrap_err(), reference_error);
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
    }

    #[test]
    #[ignore = "build the release policy library first"]
    fn native_horizon_matches_reference_from_later_turns() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cards = root.join("vendor/BabelCdb/cards.cdb");
        let core = Core::open(None, &cards, &root.join("vendor/CardScripts")).unwrap();
        let library = PolicyLibrary::open(&root.join(format!(
            "target/release/{}ygo_policies{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )))
        .unwrap();
        let decks = std::array::from_fn(|_| Deck {
            main: vec![5053103; 40],
            extra: vec![],
        });
        for viewer in 0..2u8 {
            // Enough LP to reach every requested horizon without an early
            // terminal result masking an incorrectly initialized turn count.
            let mut duel_options = DuelOptions::seeded(2493100 + u64::from(viewer));
            duel_options.life_points = [100_000; 2];
            let mut duel = core.deal(&duel_options, &decks).unwrap();
            let seats = (0..2u8)
                .map(|player| {
                    let seat = library
                        .seat(
                            "generic",
                            &cards,
                            i32::from(player),
                            2493100 + u64::from(player),
                        )
                        .unwrap();
                    seat.enable_response_overrides().unwrap();
                    seat.feed(&duel.start_message(player)).unwrap();
                    seat
                })
                .collect::<Vec<_>>();
            let (root_response, decisions) = {
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
                                if let Some(response) = seat.feed(&sent.bytes).unwrap() {
                                    assert!(pending.replace((player, response)).is_none());
                                }
                            }
                        }
                    }
                    assert!(!matches!(step.state, State::Over(_)));
                    if let Some((player, response)) = pending {
                        if player == usize::from(viewer)
                            && idle
                            && seats[player].observation().unwrap()["turn"]
                                .as_u64()
                                .unwrap()
                                >= 3
                        {
                            break (response, decisions);
                        }
                        duel.respond(&response);
                        decisions += 1;
                    } else {
                        assert_ne!(step.state, State::Awaiting);
                    }
                    assert!(decisions < 128);
                }
            };
            let original = seats[usize::from(viewer)].last_answer().unwrap();
            assert!(original["observation"]["turn"].as_u64().unwrap() >= 3);
            let world = World {
                snapshot: duel.snapshot().unwrap(),
                seats,
            };
            let next_bytes = |duel: &mut Duel<'_>| {
                duel.respond(&root_response);
                duel.step().unwrap().messages
            };
            let expected = next_bytes(&mut duel);
            duel.restore(&world.snapshot).unwrap();
            for replies in [0, 1, 2] {
                let options = Options {
                    iterations: 6,
                    actions: 3,
                    nodes: 32,
                    decisions: 128,
                    replies,
                    terminal: false,
                };
                let goal = goal_turn(&original["observation"], viewer, replies);
                let evaluator = |observation: &Value, _: u8| {
                    assert_eq!(observation["turn"].as_u64(), Some(goal));
                    observation["cards"].as_array().unwrap().len() as f64 * 0.001
                };
                let optimized = search(
                    &mut duel,
                    std::slice::from_ref(&world),
                    viewer,
                    &original,
                    decisions,
                    1024,
                    options,
                    None,
                    &evaluator,
                )
                .unwrap();
                assert_eq!(next_bytes(&mut duel), expected);
                duel.restore(&world.snapshot).unwrap();
                let legacy = search_impl::<true>(
                    &mut duel,
                    std::slice::from_ref(&world),
                    viewer,
                    &original,
                    decisions,
                    1024,
                    options,
                    None,
                    &evaluator,
                )
                .unwrap();
                assert_search_equivalent(&optimized, &legacy);
                assert_eq!(optimized.stats.failed, 0, "{:?}", optimized.stats.errors);
                assert_eq!(optimized.stats.completed, options.iterations);
                assert_eq!(optimized.stats.terminal, 0);
                assert_eq!(optimized.stats.observation_reads, options.iterations);
                assert!(optimized.stats.observation_reads < legacy.stats.observation_reads);
                eprintln!(
                    "later root viewer={viewer}, replies={replies}: observation reads {} -> {} across {} identical simulations",
                    legacy.stats.observation_reads, optimized.stats.observation_reads, options.iterations
                );
                assert_eq!(next_bytes(&mut duel), expected);
                duel.restore(&world.snapshot).unwrap();
            }
        }
    }
}
