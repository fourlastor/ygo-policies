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

/// Generic first, then action-kind diversity. Sequential candidates remain
/// bounded whole subsets supplied by LibrarySeat, not all combinations.
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
    Search(&'a mut HashMap<Key, Node>),
    Frozen(&'a FrozenPolicy),
}

impl Mode<'_> {
    fn tracks_history(&self, tree_active: bool) -> bool {
        match self {
            Self::Search(_) => tree_active,
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
fn simulate(
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
                    Mode::Search(tree) if tree_active => {
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
                            let selected = node.select();
                            response = resolve_action(&node.actions[selected].planned, &expanded)
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
                if !options.terminal && quiescent && !chain {
                    let observation = seats[viewer as usize].observation()?;
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
    validate(worlds, viewer, options)?;
    let root = duel.snapshot()?;
    let mut tree = HashMap::<Key, Node>::new();
    let mut stats = Stats::default();
    let result = (|| -> Result<()> {
        for iteration in 0..options.iterations {
            if expired(deadline) {
                break;
            }
            let world = &worlds[iteration % worlds.len()];
            duel.restore(&world.snapshot)?;
            stats.simulations += 1;
            match simulate(
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
                Mode::Search(&mut tree),
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
    let root_key = Key::new(root_original, Path::default());
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
#[allow(clippy::too_many_arguments)]
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
    validate(std::slice::from_ref(world), viewer, options)?;
    let root = duel.snapshot()?;
    let mut stats = Stats::default();
    let result = (|| -> Result<Trial> {
        duel.restore(&world.snapshot)?;
        stats.simulations = 1;
        simulate(
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
    duel.restore(&root)?;
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

    fn answer() -> Value {
        serde_json::json!({"observation":{"turn":1,"cards":[]},"decision":{"kind":"YesNo","choices":[{"kind":"No"},{"kind":"Yes"}]},"responses":[[0],[1]],"choice":0})
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
            &mut duel,
            std::slice::from_ref(&world),
            0,
            &original,
            decisions,
            1024,
            options,
            None,
            &evaluator,
        )
        .unwrap();
        assert_eq!(first.stats.failed, 0, "{:?}", first.stats.errors);
        assert_eq!(first.stats.completed, options.iterations);
        assert!(first.stats.nodes > 1);
        assert!(first.policy.len() > 1);
        assert_eq!(world.seats[0].last_answer().unwrap(), original);
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
        let repeated = search(
            &mut duel,
            std::slice::from_ref(&world),
            0,
            &original,
            decisions,
            1024,
            options,
            None,
            &evaluator,
        )
        .unwrap();
        assert_eq!(first.root_actions, repeated.root_actions);
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
        let capped = search(
            &mut duel,
            std::slice::from_ref(&world),
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
        assert!(capped
            .root_actions
            .iter()
            .take(3)
            .all(|action| action.attempts > 0));
        assert_eq!(next_bytes(&mut duel), expected);
        duel.restore(&world.snapshot).unwrap();
    }
}
