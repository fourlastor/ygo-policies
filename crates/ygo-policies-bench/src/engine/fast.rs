//! Bounded rollout, transaction-beam and information-set MCTS search.
//!
//! By default both simulated players use Generic; an explicit benchmark switch
//! can instead simulate the supplied specialist. Alternatives share sampled
//! hidden worlds, and only complete rounds contribute to their comparison.
//! The time budget is checked between engine steps; one engine call may overrun
//! it. Hidden-card sampling currently assumes the opponent's deck composition
//! is known, as in an open-deck benchmark. It never preserves the actual hidden
//! arrangement or the engine's future random stream.
use super::search::{HiddenWorlds, Listed, Table};
use super::*;
use super::planner::{self, PlannedAction, PlannerOptions, PlannerResult};
use super::evaluation::{Evaluator, EvaluationProfile};
use super::ismcts;
use super::public_memory::PublicIdentityMemory;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct FastOptions {
    pub searcher: usize,
    /// Maximum complete sampled worlds per decision.
    pub worlds: usize,
    /// Rollout method only: one root choice or two choices in a sampled world.
    pub depth: usize,
    pub max_candidates: usize,
    /// Work cap: rollout attempts, beam transitions plus validations, or MCTS
    /// iterations plus validations. Zero plays the plain pilot.
    pub max_playouts: usize,
    /// Rollouts: minimum future decisions before a quiescent leaf; zero runs
    /// to terminal. Beam/MCTS: zero validates to terminal, nonzero uses their
    /// requested turn horizon. Truncated rollouts get64 extra resolution steps.
    pub horizon: usize,
    /// Soft wall-clock budget per decision, or zero for deterministic work caps.
    pub millis: u64,
    /// Skip fresh search for unknown opposing face-down monsters or any
    /// opposing face-down banished card; these cannot be safely resampled.
    pub strict: bool,
    /// Attach public registered-effect metadata to the generic pilot.
    pub effects: bool,
    /// Transaction-complete beam proposals, evaluated unchanged across worlds.
    pub planner: Option<PlannerOptions>,
    /// Online information-set UCT; mutually exclusive with beam planning.
    pub mcts: Option<MctsOptions>,
    /// Separate sampled proposal worlds; validation uses fresh common worlds.
    pub mcts_worlds: usize,
    /// Compare each measured root with the frozen continuation on common worlds.
    pub mcts_validate_roots: bool,
    /// Benchmark diagnostic: simulate the supplied opposing specialist instead
    /// of Generic. Uses public history and sampled cards, never its live memory.
    pub specialist_opponent: bool,
    /// Explicit existing-feature coefficients, fixed across all branches.
    /// Only beam/MCTS search uses this evaluator; plain/rollout reject overrides.
    pub evaluation_weights: Option<[f64; 13]>,
    /// Opt-in own-deck composition experiment, fixed before any branch.
    pub evaluation_profile: EvaluationProfile,
    /// The staged search instead of the rollout choice: see [`StagedOptions`].
    pub staged: Option<StagedOptions>,
    /// Benchmark diagnostic, plain play only: on the decision kinds whose bits
    /// are set (see [`BORROW_KINDS`]), the searching seat plays the answer its
    /// deck's specialist would give in its place. Zero borrows nothing.
    pub borrow: u32,
}

/// The decision kinds [`FastOptions::borrow`] names, one bit each; the next
/// bit stands for any other kind. From bit 16, `Idle:` names a Main Phase
/// decision by what the specialist chose to do in it.
pub const BORROW_KINDS: [&str; 12] = [
    "Idle", "Battle", "Chain", "YesNo", "Option", "SelectCards", "SelectToggle", "SelectSum", "Position", "Place",
    "Sort", "Announce",
];
pub const BORROW_IDLE: [&str; 8] = [
    "NormalSummon", "SetMonster", "SpecialSummon", "SetSpellTrap", "Activate", "ChangePosition", "EnterBattle", "EndTurn",
];

fn borrow_bits(own: &serde_json::Value, theirs: &serde_json::Value) -> u32 {
    let name = match &own["decision"]["kind"] {
        serde_json::Value::String(kind) => kind.as_str(),
        serde_json::Value::Object(kind) => kind.keys().next().map_or("", String::as_str),
        _ => "",
    };
    let kind = 1 << BORROW_KINDS.iter().position(|&kind| kind == name).unwrap_or(BORROW_KINDS.len());
    if name != "Idle" {
        return kind;
    }
    let chosen = &theirs["decision"]["choices"][theirs["choice"].as_u64().unwrap_or(0) as usize]["kind"];
    let action = BORROW_IDLE.iter().position(|&action| chosen.as_str() == Some(action)).unwrap_or(BORROW_IDLE.len());
    kind | 1 << (16 + action)
}

/// Make a seat remember `response` as its answer to the pending prompt.
fn follow(seat: &LibrarySeat, response: &[u8]) -> bool {
    seat.override_response(response).is_ok()
        || seat.alternative_responses(12).and_then(|_| seat.override_response(response)).is_ok()
}

/// The staged choice of `policy-bench search` (`search.rs`) on this module's
/// worlds.  Every alternative is played out to the end of the duel, both
/// players Generic (or the specialist model), in the same sampled worlds as
/// the pilot's answer, which is tried first: where it wins every first-stage
/// world no alternative is tried.  After each stage only the alternatives
/// ahead of the pilot's answer go on, the best `kept` of them after the
/// first; after the middle stage one far enough ahead (paired z 2.33) is
/// taken and one too close (z 0.67) ends the search.  The pilot's answer is
/// left only for an alternative ahead by a paired z of at least `z` after
/// the last stage.
#[derive(Clone, Copy, Debug)]
pub struct StagedOptions {
    /// Worlds every surviving alternative has been played in after each stage.
    pub stages: [usize; 3],
    /// How far ahead (paired z) an alternative must be after the last stage.
    pub z: f64,
    /// Alternatives that go on after the first stage.
    pub kept: usize,
    /// Also search inside an open chain.  The native swap refuses any card a
    /// link of the chain is handled by or targets.
    pub chains: bool,
    /// Deal the other player's unknown Set monsters again (from the monsters
    /// it could have Set) instead of leaving every decision taken while one is
    /// on the field to the pilot.  Needs an engine that swaps Set monsters.
    pub set_monsters: bool,
}

impl StagedOptions {
    /// The settings measured on 2026-10-17 (benchmarks/generic-policy-v3).
    pub const MEASURED: Self = Self { stages: [8, 24, 64], z: 1.645, kept: 2, chains: true, set_monsters: true };
}

impl FastOptions {
    fn validate_evaluation_weights(&self) -> Result<()> {
        if self.evaluation_profile != EvaluationProfile::Off && self.evaluation_weights.is_some() {
            return Err("Evaluation profile and manual weights are mutually exclusive".into());
        }
        if self.evaluation_profile != EvaluationProfile::Off && !self.effects {
            return Err("Evaluation profile requires native effect metadata (--effects true)".into());
        }
        if self.evaluation_weights.is_some() || self.evaluation_profile != EvaluationProfile::Off {
            if self.max_playouts == 0 || self.planner.is_none() && self.mcts.is_none() {
                return Err("Evaluation weights/profile require active beam or MCTS search".into());
            }
            if self.mcts.is_some_and(|options| options.terminal) && self.horizon == 0 {
                return Err("Evaluation weights/profile are unused by terminal-only MCTS proposals and validation".into());
            }
        }
        if let Some(weights) = &self.evaluation_weights {
            super::evaluation::validate_weights(weights)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Stats {
    searched: u64,
    playouts: u64,
    inner_searches: u64,
    inner_playouts: u64,
    failed_playouts: u64,
    truncated_playouts: u64,
    unresolved_playouts: u64,
    budget_exhausted: u64,
    incomplete_worlds: u64,
    failed_searches: u64,
    unsafe_selection: u64,
    proposal_nodes: u64,
    proposal_leaves: u64,
    proposal_improvements: u64,
    proposal_completed_prefixes: u64,
    proposal_responses: u64,
    plan_evaluations: u64,
    plan_applied: u64,
    plan_mismatches: u64,
    live_plan_applied: u64,
    live_plan_mismatches: u64,
    mcts_simulations: u64,
    mcts_completed: u64,
    mcts_nodes: u64,
    mcts_steps: u64,
    mcts_tree_hits: u64,
    mcts_opponent_nodes: u64,
    mcts_opponent_actions: u64,
    mcts_opponent_tree_hits: u64,
    mcts_cutoffs: u64,
    mcts_live_hits: u64,
    mcts_live_missing: u64,
    mcts_frozen_actions: u64,
    mcts_accepted: u64,
    mcts_root_actions: u64,
    mcts_visited_root_actions: u64,
    mcts_root_actions_all_worlds_attempted: u64,
    mcts_root_actions_all_worlds_completed: u64,
    search_skipped_facedown_banished: u64,
    resampled_set_monsters: u64,
    /// Staged roots left to the pilot because their worlds could not be dealt
    /// (the engine refused to swap a Set monster that was not untouched).
    undealt_roots: u64,
    snapshot_copies: Vec<ygo_policies_duel::SnapshotMetrics>,
    errors: Vec<String>,
}

impl Stats {
    fn error(&mut self, error: String) {
        if self.errors.len() < 8 && !self.errors.contains(&error) {
            self.errors.push(error);
        }
    }
}

fn expired(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

/// Public information only. Hidden monster attributes/codes are never used.
/// Terminal outcomes are evaluated separately and dominate any leaf estimate.
fn leaf_value(observation: &serde_json::Value, viewer: u8) -> f64 {
    let mut material = [0.0f64; 2];
    if let Some(cards) = observation["cards"].as_array() {
        for card in cards {
            let Some(player) = card["at"]["controller"].as_u64().filter(|p| *p < 2) else {
                continue;
            };
            let player = player as usize;
            let face_up = card["position"]["face_up"].as_bool().unwrap_or(false);
            material[player] += match card["at"]["location"].as_str() {
                Some("Hand") => 1.0,
                Some("SpellTrapZone") => 0.8,
                Some("MonsterZone") => {
                    let power = if face_up || player == viewer as usize {
                        let attack = card["attack"].as_i64().unwrap_or(0).max(0);
                        let defense = card["defense"].as_i64().unwrap_or(0).max(0);
                        attack.max(defense) as f64
                    } else {
                        1200.0
                    };
                    1.0 + power / 4000.0
                }
                _ => 0.0,
            };
        }
    }
    let me = viewer as usize;
    let opponent = 1 - me;
    let own_lp = observation["life_points"][me].as_i64().unwrap_or(8000) as f64;
    let their_lp = observation["life_points"][opponent]
        .as_i64()
        .unwrap_or(8000) as f64;
    (0.16 * (material[me] - material[opponent]) + (own_lp - their_lp) / 16000.0).tanh() * 0.9
}

/// A public reveal can remain remembered after IS_PUBLIC has gone away.
/// These exact slots must not be exchanged for a different sampled card.
fn known_slots(observation: &serde_json::Value) -> Vec<(u8, u32, u32)> {
    observation["cards"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|card| {
            if card["code"].as_u64()? == 0 {
                return None;
            }
            let location = match card["at"]["location"].as_str()? {
                "Deck" => 1,
                "Hand" => 2,
                "SpellTrapZone" => 8,
                _ => return None,
            };
            let player = u8::try_from(card["at"]["controller"].as_u64()?).ok()?;
            let sequence = u32::try_from(card["at"]["sequence"].as_u64()?).ok()?;
            Some((player, location, sequence))
        })
        .collect()
}

fn member_slot(member: &serde_json::Value) -> Option<(u8, u32, u32)> {
    let at = &member["at"];
    let location = match at["location"].as_str()? {
        "Deck" => 1,
        "Hand" => 2,
        "SpellTrapZone" => 8,
        _ => return None,
    };
    Some((
        u8::try_from(at["controller"].as_u64()?).ok()?,
        location,
        u32::try_from(at["sequence"].as_u64()?).ok()?,
    ))
}

fn choice_members(answer: &serde_json::Value) -> impl Iterator<Item = &serde_json::Value> {
    answer["decision"]["choices"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|choice| {
            std::iter::once(&choice["card"])
                .chain(choice["members"].as_array().into_iter().flatten())
        })
}

fn pending_known_slots(answer: &serde_json::Value, viewer: u8) -> Vec<(u8, u32, u32)> {
    let mut known = known_slots(&answer["observation"]);
    for member in choice_members(answer) {
        if member["code"].as_u64().is_some_and(|code| code != 0) {
            if let Some(slot) = member_slot(member) {
                // A deck-search menu reveals identities, not future draw order.
                if slot.0 != viewer && slot.1 != 1 && !known.contains(&slot) {
                    known.push(slot);
                }
            }
        }
    }
    known
}

/// Native selections retain object pointers. Own-deck pointers remain valid
/// when cards shuffle within that deck. Opponent cards can cross sampled zones;
/// a pending pointer to an unknown card may then name the wrong zone. Freeze
/// revealed hand/backrow cards and skip unsafe root selections. A revealed deck
/// list does not justify freezing its actual future draw order.
fn unsafe_selection(answer: &serde_json::Value, viewer: u8) -> bool {
    if !["SelectCards", "SelectSum", "SelectToggle"]
        .contains(&answer["decision"]["kind"].as_str().unwrap_or(""))
    {
        return false;
    }
    let known = pending_known_slots(answer, viewer);
    choice_members(answer).any(|member| {
        member_slot(member).is_some_and(|slot| {
            if known.contains(&slot) {
                return false;
            }
            if slot.0 == viewer {
                return slot.1 == 1 && !member["code"].as_u64().is_some_and(|code| code != 0);
            }
            true
        })
    })
}

enum Trial {
    Scored(f64, bool),
    Budget,
    Unresolved,
}

struct Context<'a> {
    model_library: &'a PolicyLibrary,
    model_name: &'a str,
    cards: &'a Path,
    viewer: u8,
    seed: u64,
    limit: usize,
    options: FastOptions,
    profiles: Option<serde_json::Value>,
    evaluator: Option<Evaluator>,
}

/// Build the selected opponent model from public events only. Replaying the real
/// opponent's private prompts would let its actual hand influence memory even
/// after sampling a different hand, so spectator redaction deliberately drops
/// every historical choice and hides every private draw/update.
fn public_opponent<'a>(
    context: &Context<'a>,
    logs: &[Vec<u8>; 2],
    decisions: usize,
) -> Result<LibrarySeat<'a>> {
    let player = 1 - context.viewer as usize;
    let seat = context.model_library.seat(
        context.model_name,
        context.cards,
        player as i32,
        context.seed ^ (decisions as u64).wrapping_mul(0xD1B54A32D192ED03),
    )?;
    if !context.options.specialist_opponent {
        if let Some(profiles) = &context.profiles { seat.set_effect_profiles(profiles)?; }
    }
    if context.options.mcts.is_some_and(|options| options.adversarial_opponent) {
        seat.enable_response_overrides()?;
    }
    seat.feed_buffer(&public_history(&logs[player])?)?;
    Ok(seat)
}

fn public_history(history: &[u8]) -> Result<Vec<u8>> {
    let mut filtered = Vec::new();
    let mut offset = 0;
    while offset < history.len() {
        let length = history
            .get(offset..offset + 4)
            .ok_or("Truncated policy history")?;
        let length = u32::from_le_bytes(length.try_into().unwrap()) as usize;
        offset += 4;
        let raw = history
            .get(offset..offset + length)
            .ok_or("Truncated policy message")?;
        offset += length;
        if let Some(visible) =
            ygo_policies_ocgcore::redact::redact(raw, None).map_err(|error| error.to_string())?
        {
            filtered.extend_from_slice(&(visible.len() as u32).to_le_bytes());
            filtered.extend_from_slice(&visible);
        }
    }
    Ok(filtered)
}

/// Copy the real generic player's memory and the public opponent model.
/// Refreshing after sampling gives the model its sampled hidden hand.
fn prepare<'a>(
    duel: &Duel,
    context: &Context<'a>,
    table: &Table<'a>,
    opponent: &LibrarySeat<'a>,
) -> Result<Vec<LibrarySeat<'a>>> {
    let mut seats = Vec::with_capacity(2);
    for player in 0..2 {
        seats.push(if player == context.viewer as usize {
            table.seats[player].fork()?
        } else {
            opponent.fork()?
        });
    }
    for update in duel.field() {
        // The root viewer has the same visible state and retains its pending
        // response; only the modeled opponent needs a sampled-hand refresh.
        seats[1 - context.viewer as usize].feed(&update)?;
    }
    Ok(seats)
}

/// Bound the branching factor while retaining different kinds of action.
/// The seat supplies bounded legal completions of sequential target/material
/// selections; purely administrative choices stay with the pilot.
fn candidates(answer: &serde_json::Value, given: &[u8], maximum: usize) -> Option<Listed> {
    if maximum < 2 {
        return None;
    }
    let decision = &answer["decision"];
    let kind = match &decision["kind"] {
        serde_json::Value::String(kind)
            if [
                "Idle",
                "Battle",
                "YesNo",
                "Position",
                "Option",
                "SelectCards",
                "SelectToggle",
                "SelectSum",
            ]
            .contains(&kind.as_str()) =>
        {
            kind.clone()
        }
        serde_json::Value::Object(kind) if kind.get("Chain").is_some() => "Chain".to_owned(),
        _ => return None,
    };
    let choice = answer["choice"].as_u64()? as usize;
    let responses = answer["responses"].as_array()?;
    let choices = decision["choices"].as_array()?;
    if responses.len() != choices.len() {
        return None;
    }
    let bytes = |value: &serde_json::Value| -> Option<Vec<u8>> {
        value
            .as_array()?
            .iter()
            .map(|b| b.as_u64().and_then(|b| u8::try_from(b).ok()))
            .collect()
    };
    if bytes(responses.get(choice)?)? != given {
        return None;
    }
    let label = |choice: &serde_json::Value| serde_json::json!({"kind": choice["kind"], "code": choice["card"]["code"], "description": choice["description"], "members": choice["members"]});
    let key = |choice: &serde_json::Value| {
        let at = &choice["card"]["at"];
        let pile = ["Hand", "Deck", "Graveyard", "Banished", "Extra"]
            .contains(&at["location"].as_str().unwrap_or(""));
        if pile {
            format!(
                "{}:{}:{}:{}:{}",
                choice["kind"],
                choice["card"]["code"],
                at["controller"],
                at["location"],
                choice["description"]
            )
        } else {
            choice.to_string()
        }
    };
    let mut seen = vec![key(&choices[choice])];
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (index, described) in choices.iter().enumerate() {
        if index == choice || described["kind"] == "ShuffleHand" || described["kind"] == "Cancel" {
            continue;
        }
        let key = key(described);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let group = described["kind"].to_string();
        if let Some((_, indices)) = groups.iter_mut().find(|(kind, _)| *kind == group) {
            indices.push(index);
        } else {
            groups.push((group, vec![index]));
        }
    }
    let priorities = [
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
        priorities
            .iter()
            .position(|name| kind == &format!("\"{name}\""))
            .unwrap_or(priorities.len())
    });
    let mut out = Listed {
        kind,
        responses: vec![given.to_vec()],
        labels: vec![label(&choices[choice])],
    };
    for depth in 0..choices.len() {
        for (_, indices) in &groups {
            if out.responses.len() >= maximum {
                break;
            }
            if let Some(index) = indices.get(depth) {
                out.responses.push(bytes(&responses[*index])?);
                out.labels.push(label(&choices[*index]));
            }
        }
        if out.responses.len() >= maximum {
            break;
        }
    }
    (out.responses.len() > 1).then_some(out)
}

#[allow(clippy::too_many_arguments)]
fn rollout(
    duel: &mut Duel,
    seats: &[LibrarySeat<'_>],
    context: &Context<'_>,
    response: &[u8],
    root_decisions: usize,
    previous_future: usize,
    mut chain: bool,
    may_branch: bool,
    deadline: Option<Instant>,
    remaining: &mut usize,
    stats: &mut Stats,
) -> Result<Trial> {
    if *remaining == 0 || expired(deadline) {
        return Ok(Trial::Budget);
    }
    *remaining -= 1;
    stats.playouts += 1;
    seats[context.viewer as usize].override_response(response)?;
    duel.respond(response);
    let mut decisions = root_decisions + 1;
    let mut future = previous_future + 1;
    loop {
        if expired(deadline) {
            return Ok(Trial::Budget);
        }
        let step = duel.step()?;
        let mut answer = None;
        let mut quiescent = false;
        for sent in &step.messages {
            if sent.refresh {
                seats[0].feed_update_pair(&seats[1], &sent.bytes)?;
                continue;
            }
            if sent.bytes.first() == Some(&msg::RETRY) {
                return Err("Engine rejected search response (MSG_RETRY)".into());
            }
            match sent.bytes[0] {
                msg::CHAINING => chain = true,
                msg::CHAIN_END => chain = false,
                msg::SELECT_IDLECMD | msg::SELECT_BATTLECMD => quiescent = true,
                _ => {}
            }
            for (player, seat) in seats.iter().enumerate() {
                if let Some(given) = seat.feed(&sent.bytes)? {
                    if answer.replace((player, given)).is_some() {
                        return Err("Multiple rollout answers in one batch".into());
                    }
                }
            }
        }
        if let State::Over(outcome) = step.state {
            let outcome = outcome.ok_or("Search engine ended without MSG_WIN")?;
            return Ok(Trial::Scored(
                if outcome.winner > 1 {
                    0.0
                } else if outcome.winner == context.viewer {
                    1.0
                } else {
                    -1.0
                },
                false,
            ));
        }
        if decisions >= context.limit {
            // Never prefer looping until the game cap to an actual loss.
            return Ok(Trial::Unresolved);
        }
        // The sampled world is fixed here. A second choice can therefore be
        // explored inside a chain without touching any chain-referenced card.
        // Each candidate is a continuation policy, not an observation of the
        // live duel's hidden state. This determinized maximum is optimistic.
        if may_branch {
            if let Some((player, given)) = &answer {
                if *player == context.viewer as usize {
                    let described = seats[*player].alternative_responses(3)?;
                    if let Some(listed) = candidates(&described, given, 3) {
                        if *remaining < listed.responses.len() {
                            return Ok(Trial::Budget);
                        }
                        stats.inner_searches += 1;
                        let snapshot = duel.snapshot()?;
                        let mut best: Option<(f64, bool)> = None;
                        for candidate in &listed.responses {
                            duel.restore(&snapshot)?;
                            let copies = seats
                                .iter()
                                .map(LibrarySeat::fork)
                                .collect::<Result<Vec<_>>>()?;
                            let previous_playouts = stats.playouts;
                            let result = rollout(
                                duel, &copies, context, candidate, decisions, future, chain, false,
                                deadline, remaining, stats,
                            );
                            stats.inner_playouts += u64::from(stats.playouts > previous_playouts);
                            match result? {
                                Trial::Scored(value, truncated) => {
                                    if best.map_or(true, |(old, _)| value > old) {
                                        best = Some((value, truncated));
                                    }
                                }
                                other => return Ok(other),
                            }
                        }
                        let (value, truncated) = best.ok_or("No nested search continuation")?;
                        return Ok(Trial::Scored(value, truncated));
                    }
                }
            }
        }
        if context.options.horizon > 0
            && future >= context.options.horizon
            && step.state == State::Awaiting
            && quiescent
            && !chain
        {
            return Ok(Trial::Scored(
                leaf_value(
                    &seats[context.viewer as usize].observation()?,
                    context.viewer,
                ),
                true,
            ));
        }
        if context.options.horizon > 0 && future >= context.options.horizon.saturating_add(64) {
            // A half-paid cost or unresolved material/target choice is not a
            // meaningful leaf. Discard this world if no safe boundary arrives.
            return Ok(Trial::Unresolved);
        }
        match answer {
            Some((_, answer)) => {
                duel.respond(&answer);
                decisions += 1;
                future += 1;
            }
            None if step.state == State::Awaiting => {
                return Err("Search engine awaiting without an answer".into())
            }
            None => {}
        }
    }
}

/// Positive paired advantage only; exact ties preserve the plain pilot.
fn best_candidate(totals: &[f64], complete: usize) -> usize {
    if complete == 0 {
        return 0;
    }
    let mut best = 0;
    for index in 1..totals.len() {
        if totals[index] > totals[best] + 1e-12 {
            best = index;
        }
    }
    best
}

/// Mean, paired z and count of a candidate's payoff differences from the
/// pilot's answer over the worlds both were played out in.
fn paired(candidate: &[Option<f64>], pilot: &[Option<f64>]) -> (f64, f64, usize) {
    let differences: Vec<f64> = candidate.iter().zip(pilot).filter_map(|(a, b)| Some((*a)? - (*b)?)).collect();
    let n = differences.len();
    if n < 2 {
        return (0.0, 0.0, n);
    }
    let mean = differences.iter().sum::<f64>() / n as f64;
    let variance = differences.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    let z = if variance > 0.0 {
        mean / (variance / n as f64).sqrt()
    } else if mean > 0.0 {
        f64::INFINITY
    } else if mean < 0.0 {
        f64::NEG_INFINITY
    } else {
        0.0
    };
    (mean, z, n)
}

/// The other player's face-down monsters whose identity the viewer does not
/// know, by zone: read from the viewer's own observation and public memory.
fn unknown_set_monsters(memory: &PublicIdentityMemory, observation: &serde_json::Value, viewer: u8) -> Vec<u32> {
    let Some(cards) = observation["cards"].as_array() else { return Vec::new() };
    cards
        .iter()
        .filter_map(|card| {
            if card["at"]["location"] != "MonsterZone"
                || card["at"]["controller"].as_u64() == Some(viewer as u64)
                || card["position"]["face_up"] == true
            {
                return None;
            }
            let controller = u8::try_from(card["at"]["controller"].as_u64()?).ok()?;
            let sequence = u32::try_from(card["at"]["sequence"].as_u64()?).ok()?;
            (!memory.knows(controller, 4, sequence)).then_some(sequence)
        })
        .collect()
}

/// The staged search (see [`StagedOptions`]): the index of the response to
/// give, the worlds it was decided on, and its mean gain over the pilot's.
#[allow(clippy::too_many_arguments)]
fn choose_staged(
    duel: &mut Duel,
    context: &Context<'_>,
    table: &Table<'_>,
    listed: &Listed,
    decisions: usize,
    stats: &mut Stats,
    staged: StagedOptions,
    monsters: Vec<u32>,
) -> Result<(usize, usize, f64)> {
    const EARLY_ACCEPT: f64 = 2.33;
    const EARLY_REJECT: f64 = 0.67;
    let root = duel.snapshot()?;
    let known = pending_known_slots(&table.seats[context.viewer as usize].last_answer()?, context.viewer);
    let mut hidden = HiddenWorlds::with_known(known).with_monsters(monsters);
    let n = listed.responses.len();
    let mut payoffs: Vec<Vec<Option<f64>>> = vec![Vec::new(); n];
    let mut alive: Vec<usize> = (1..n).collect();
    let (mut chosen, mut gain, mut worlds_used) = (0usize, 0.0f64, 0usize);
    let mut undealt = false;
    let result = (|| -> Result<()> {
        let opponent = public_opponent(context, &table.logs, decisions)?;
        let mut done = 0;
        let mut won_everywhere = false;
        for (stage, total) in staged.stages.iter().copied().enumerate() {
            for pilot_pass in [true, false] {
                if !pilot_pass && won_everywhere {
                    break;
                }
                for world in done..total {
                    // The worlds of the rollout choice, the same for every alternative.
                    let world_seed = context.seed
                        ^ (decisions as u64).wrapping_mul(0x9E3779B97F4A7C15)
                        ^ (world as u64 + 1).wrapping_mul(0xD1B54A32D192ED03);
                    if let Err(error) = hidden.restore(duel, &root, context.viewer, world_seed) {
                        undealt = true;
                        return Err(error);
                    }
                    duel.reseed(world_seed ^ 0x94D049BB133111EB)?;
                    let seats = prepare(duel, context, table, &opponent)?;
                    let sampled = duel.snapshot()?;
                    let indices: Vec<usize> = if pilot_pass { vec![0] } else { alive.clone() };
                    for index in indices {
                        duel.restore(&sampled)?;
                        let copies = seats.iter().map(LibrarySeat::fork).collect::<Result<Vec<_>>>()?;
                        let mut remaining = usize::MAX;
                        let value = match rollout(duel, &copies, context, &listed.responses[index], decisions, 0, false, false, None, &mut remaining, stats) {
                            Ok(Trial::Scored(value, _)) => Some(value),
                            Ok(Trial::Unresolved) => {
                                // The duel limit: a draw.
                                stats.unresolved_playouts += 1;
                                Some(0.0)
                            }
                            Ok(Trial::Budget) => None,
                            Err(error) => {
                                stats.failed_playouts += 1;
                                stats.error(error);
                                None
                            }
                        };
                        payoffs[index].push(value);
                    }
                }
                if pilot_pass && stage == 0 {
                    won_everywhere = payoffs[0].iter().all(|p| p.map_or(true, |p| p >= 1.0));
                }
            }
            worlds_used = total;
            if won_everywhere {
                break;
            }
            done = total;
            let mut ranked: Vec<(usize, f64, f64)> = alive
                .iter()
                .map(|&index| {
                    let (mean, z, _) = paired(&payoffs[index], &payoffs[0]);
                    (index, mean, z)
                })
                .filter(|(_, mean, _)| *mean > 0.0)
                .collect();
            ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
            let Some(best) = ranked.first().copied() else { break };
            let last = stage + 1 == staged.stages.len();
            let accept = if last { best.2 >= staged.z } else { stage > 0 && best.2 >= EARLY_ACCEPT };
            if accept {
                chosen = best.0;
                gain = best.1;
                break;
            }
            if last || (stage > 0 && best.2 < EARLY_REJECT) {
                break;
            }
            alive = ranked.iter().take(staged.kept).map(|r| r.0).collect();
        }
        Ok(())
    })();
    // The live duel goes on from the decision itself.
    duel.restore(&root)?;
    if let Err(error) = result {
        // The pilot's answer.  A world the engine would not deal is expected
        // (a Set monster something has touched); anything else is a failure.
        if undealt {
            stats.undealt_roots += 1;
        } else {
            stats.failed_searches += 1;
            stats.error(error);
        }
        return Ok((0, 0, 0.0));
    }
    Ok((chosen, worlds_used, gain))
}

fn choose(
    duel: &mut Duel,
    context: &Context<'_>,
    table: &Table<'_>,
    listed: &Listed,
    decisions: usize,
    stats: &mut Stats,
) -> Result<(usize, usize, f64)> {
    let root = duel.snapshot()?;
    let deadline = (context.options.millis > 0)
        .then(|| Instant::now() + Duration::from_millis(context.options.millis));
    let known = pending_known_slots(
        &table.seats[context.viewer as usize].last_answer()?,
        context.viewer,
    );
    let mut hidden = HiddenWorlds::with_known(known);
    let mut totals = vec![0.0; listed.responses.len()];
    let mut complete = 0;
    let mut remaining = context.options.max_playouts;
    let result = (|| -> Result<()> {
        let opponent = public_opponent(context, &table.logs, decisions)?;
        for world in 0..context.options.worlds {
            if remaining < listed.responses.len() {
                break;
            }
            if expired(deadline) {
                stats.budget_exhausted += 1;
                break;
            }
            let world_seed = context.seed
                ^ (decisions as u64).wrapping_mul(0x9E3779B97F4A7C15)
                ^ (world as u64 + 1).wrapping_mul(0xD1B54A32D192ED03);
            hidden.restore(duel, &root, context.viewer, world_seed)?;
            duel.reseed(world_seed ^ 0x94D049BB133111EB)?;
            let seats = prepare(duel, context, table, &opponent)?;
            // A shared sampled snapshot avoids repeating swaps and their queries.
            let sampled = duel.snapshot()?;
            let mut scores = Vec::with_capacity(listed.responses.len());
            let mut timed_out = false;
            for response in &listed.responses {
                if expired(deadline) {
                    timed_out = true;
                    break;
                }
                duel.restore(&sampled)?;
                let copies = seats
                    .iter()
                    .map(LibrarySeat::fork)
                    .collect::<Result<Vec<_>>>()?;
                match rollout(
                    duel,
                    &copies,
                    context,
                    response,
                    decisions,
                    0,
                    false,
                    context.options.depth == 2,
                    deadline,
                    &mut remaining,
                    stats,
                ) {
                    Ok(Trial::Scored(value, truncated)) => {
                        stats.truncated_playouts += u64::from(truncated);
                        scores.push(Some(value));
                    }
                    Ok(Trial::Budget) => {
                        timed_out = true;
                        break;
                    }
                    Ok(Trial::Unresolved) => {
                        stats.unresolved_playouts += 1;
                        scores.push(None);
                    }
                    Err(error) => {
                        stats.failed_playouts += 1;
                        stats.error(error);
                        scores.push(None);
                    }
                }
            }
            if scores.len() == totals.len() && scores.iter().all(Option::is_some) {
                let baseline = scores[0].unwrap();
                for (total, score) in totals.iter_mut().zip(scores) {
                    *total += score.unwrap() - baseline;
                }
                complete += 1;
            } else {
                // Discard the entire paired round, including its baseline.
                stats.incomplete_worlds += 1;
            }
            if timed_out {
                stats.budget_exhausted += 1;
                break;
            }
        }
        Ok(())
    })();
    // Restore even when a sampled world or policy failed.
    duel.restore(&root)?;
    if let Err(error) = result {
        stats.failed_searches += 1;
        stats.error(error);
        return Ok((0, 0, 0.0));
    }
    let best = best_candidate(&totals, complete);
    Ok((
        best,
        complete,
        if complete == 0 {
            0.0
        } else {
            totals[best] / complete as f64
        },
    ))
}

/// Build each proposal in a separate world, then compare the fixed executable
/// proposals on common validation worlds. No validation-world maximization.
fn choose_plan(
    duel: &mut Duel,
    context: &Context<'_>,
    table: &Table<'_>,
    listed: &Listed,
    decisions: usize,
    stats: &mut Stats,
) -> Result<(usize, usize, f64, Vec<PlannedAction>)> {
    let options = context.options.planner.ok_or("Missing planner options")?;
    let evaluator = context.evaluator.as_ref().ok_or("Missing planner evaluator")?;
    let evaluate = |observation: &serde_json::Value, viewer, menu: Option<&serde_json::Value>|
        evaluator.value_with_menu(observation, viewer, menu);
    let root = duel.snapshot()?;
    let deadline = (context.options.millis > 0)
        .then(|| Instant::now() + Duration::from_millis(context.options.millis));
    let known = pending_known_slots(&table.seats[context.viewer as usize].last_answer()?, context.viewer);
    let mut hidden = HiddenWorlds::with_known(known);
    // Candidate zero is always the actual plain pilot. Optimizing its root
    // action's continuation creates a separate candidate, never a new control.
    let responses: Vec<_> = std::iter::once(&listed.responses[0]).chain(&listed.responses).collect();
    let mut totals = vec![0.0; responses.len()];
    let mut plans = vec![Vec::new(); responses.len()];
    let mut complete = 0;
    let mut remaining = context.options.max_playouts;
    let result = (|| -> Result<()> {
        let opponent = public_opponent(context, &table.logs, decisions)?;
        // Keep at least one complete paired validation round. All candidates
        // receive the same proposal cap, so early candidates cannot consume it.
        let count = responses.len();
        let reserve = count.saturating_mul(context.options.worlds);
        let proposal_cap = options.nodes.min(remaining.saturating_sub(reserve) / count);
        // Reserve half the remaining wall budget for paired validation, then
        // split proposal time evenly. A single costly root cannot consume it.
        let proposal_slice = deadline.map(|end| end.saturating_duration_since(Instant::now())
            / (2 * count.saturating_sub(1).max(1)) as u32);
        if proposal_cap > 0 {
            let proposal_seed = context.seed
                ^ (decisions as u64).wrapping_mul(0x9E3779B97F4A7C15)
                ^ 0xA0761D6478BD642F;
            hidden.restore(duel, &root, context.viewer, proposal_seed)?;
            duel.reseed(proposal_seed ^ 0x94D049BB133111EB)?;
            let seats = prepare(duel, context, table, &opponent)?;
            let sampled = duel.snapshot()?;
            for (index, response) in responses.iter().enumerate().skip(1) {
                if expired(deadline) { break; }
                duel.restore(&sampled)?;
                let proposal_deadline = proposal_slice.map(|slice| (Instant::now() + slice).min(deadline.unwrap()));
                let proposed = planner::evaluate(duel, &seats, context.viewer, response,
                    decisions, context.limit, PlannerOptions { nodes: proposal_cap, ..options },
                    proposal_deadline, &evaluate)?;
                remaining = remaining.saturating_sub(proposed.nodes);
                stats.proposal_nodes += proposed.nodes as u64;
                stats.proposal_leaves += proposed.leaves as u64;
                stats.proposal_improvements += proposed.improved_leaves as u64;
                stats.proposal_completed_prefixes += proposed.completed_prefixes as u64;
                stats.proposal_responses += proposed.responses as u64;
                record_planner(&proposed, stats);
                if proposed.value.is_some() { plans[index] = proposed.plan; }
            }
        }
        for world in 0..context.options.worlds {
            if remaining < count || expired(deadline) {
                stats.budget_exhausted += 1;
                break;
            }
            // This seed stream is disjoint from the proposal world.
            let world_seed = context.seed
                ^ (decisions as u64).wrapping_mul(0x9E3779B97F4A7C15)
                ^ (world as u64 + 1).wrapping_mul(0xD1B54A32D192ED03);
            hidden.restore(duel, &root, context.viewer, world_seed)?;
            duel.reseed(world_seed ^ 0x94D049BB133111EB)?;
            let seats = prepare(duel, context, table, &opponent)?;
            let sampled = duel.snapshot()?;
            let mut scores = Vec::with_capacity(count);
            for (index, response) in responses.iter().enumerate() {
                if expired(deadline) { break; }
                duel.restore(&sampled)?;
                let validation = if context.options.horizon == 0 {
                    planner::evaluate_plan_terminal
                } else { planner::evaluate_plan };
                let evaluated = validation(duel, &seats, context.viewer, response,
                    decisions, context.limit, PlannerOptions { nodes: 1, ..options },
                    deadline, &evaluate, &plans[index])?;
                remaining = remaining.saturating_sub(1);
                stats.plan_evaluations += 1;
                stats.playouts += 1;
                stats.plan_applied += evaluated.plan_applied as u64;
                stats.plan_mismatches += evaluated.plan_mismatches as u64;
                record_planner(&evaluated, stats);
                if evaluated.value.is_none() { stats.unresolved_playouts += 1; }
                stats.truncated_playouts += u64::from(evaluated.value.is_some() && !evaluated.terminal);
                scores.push(evaluated.value);
            }
            if scores.len() == count && scores.iter().all(Option::is_some) {
                let baseline = scores[0].unwrap();
                for (total, score) in totals.iter_mut().zip(scores) {
                    *total += score.unwrap() - baseline;
                }
                complete += 1;
            } else { stats.incomplete_worlds += 1; }
        }
        Ok(())
    })();
    duel.restore(&root)?;
    if let Err(error) = result {
        stats.failed_searches += 1;
        stats.error(error);
        return Ok((0, 0, 0.0, Vec::new()));
    }
    let best = best_candidate(&totals, complete);
    let gain = if complete == 0 { 0.0 } else { totals[best] / complete as f64 };
    // An unvalidated proposal must never leak into live play.
    let plan = if complete == 0 { Vec::new() } else { std::mem::take(&mut plans[best]) };
    Ok((best.saturating_sub(1), complete, gain, plan))
}

fn record_planner(result: &PlannerResult, stats: &mut Stats) {
    stats.failed_playouts += result.failed as u64;
    for error in &result.errors { stats.error(error.clone()); }
}

/// Discover a conditional policy in proposal worlds, then independently
/// compare the frozen policy against plain play on fresh common worlds.
fn choose_mcts(
    duel: &mut Duel,
    context: &Context<'_>,
    table: &Table<'_>,
    original: &serde_json::Value,
    given: &[u8],
    decisions: usize,
    stats: &mut Stats,
) -> Result<(Vec<u8>, usize, f64, Option<ismcts::FrozenPolicy>)> {
    // Diagnostics stay outside the restored native arena. Include preparation,
    // proposal, validation and the final live-root restore, even on errors.
    duel.enable_snapshot_metrics(true);
    duel.reset_snapshot_metrics();
    let result = choose_mcts_measured(duel, context, table, original, given, decisions, stats);
    if let Some(metrics) = duel.take_snapshot_metrics() { stats.snapshot_copies.push(metrics); }
    duel.enable_snapshot_metrics(false);
    result
}

fn choose_mcts_measured(
    duel: &mut Duel,
    context: &Context<'_>,
    table: &Table<'_>,
    original: &serde_json::Value,
    given: &[u8],
    decisions: usize,
    stats: &mut Stats,
) -> Result<(Vec<u8>, usize, f64, Option<ismcts::FrozenPolicy>)> {
    let options = context.options.mcts.ok_or("Missing MCTS options")?;
    let evaluator = context.evaluator.as_ref().ok_or("Missing MCTS evaluator")?;
    let evaluate = |observation: &serde_json::Value, viewer, menu: Option<&serde_json::Value>|
        evaluator.value_with_menu(observation, viewer, menu);
    let root = duel.snapshot()?;
    let deadline = (context.options.millis > 0)
        .then(|| Instant::now() + Duration::from_millis(context.options.millis));
    let mut hidden = HiddenWorlds::with_known(pending_known_slots(original, context.viewer));
    let mut complete = 0;
    let mut advantage = 0.0;
    let mut proposed_response = given.to_vec();
    let mut policy = None;
    let result = (|| -> Result<()> {
        let opponent = public_opponent(context, &table.logs, decisions)?;
        let reserve = context.options.worlds.saturating_mul(if context.options.mcts_validate_roots {
            context.options.max_candidates.saturating_add(1)
        } else { 2 });
        let iterations = options.iterations.min(context.options.max_playouts.saturating_sub(reserve));
        if iterations == 0 { stats.budget_exhausted += 1; return Ok(()); }
        let proposal_deadline = deadline.map(|end| Instant::now() + end.saturating_duration_since(Instant::now()) / 2);
        let mut worlds = Vec::new();
        for index in 0..context.options.mcts_worlds.min(iterations) {
            if expired(proposal_deadline) { break; }
            let seed = context.seed ^ (decisions as u64).wrapping_mul(0x9E3779B97F4A7C15)
                ^ (index as u64 + 1).wrapping_mul(0xA0761D6478BD642F);
            hidden.restore(duel, &root, context.viewer, seed)?;
            duel.reseed(seed ^ 0x94D049BB133111EB)?;
            let seats = prepare(duel, context, table, &opponent)?;
            worlds.push(ismcts::World { snapshot: duel.snapshot()?, seats });
        }
        if worlds.is_empty() { stats.budget_exhausted += 1; return Ok(()); }
        let searched = ismcts::search(duel, &worlds, context.viewer, original, decisions,
            context.limit, MctsOptions { iterations, ..options }, proposal_deadline, &evaluate)?;
        stats.mcts_root_actions += searched.root_actions.len() as u64;
        stats.mcts_visited_root_actions += searched.root_actions.iter().filter(|a| a.visits > 0).count() as u64;
        stats.mcts_root_actions_all_worlds_attempted += searched.stats.root_actions_all_worlds_attempted as u64;
        stats.mcts_root_actions_all_worlds_completed += searched.stats.root_actions_all_worlds_completed as u64;
        stats.mcts_simulations += searched.stats.simulations as u64;
        stats.mcts_completed += searched.stats.completed as u64;
        stats.mcts_nodes += searched.stats.nodes as u64;
        stats.mcts_steps += searched.stats.steps as u64;
        stats.mcts_tree_hits += searched.stats.tree_hits as u64;
        stats.mcts_opponent_nodes += searched.stats.opponent_nodes as u64;
        stats.mcts_opponent_actions += searched.stats.opponent_actions as u64;
        stats.mcts_opponent_tree_hits += searched.stats.opponent_tree_hits as u64;
        stats.mcts_cutoffs += searched.stats.cutoffs as u64;
        stats.failed_playouts += searched.stats.failed as u64;
        for error in &searched.stats.errors { stats.error(error.clone()); }
        if searched.stats.completed == 0 { return Ok(()); }
        let Some(response) = searched.root_response else { return Ok(()); };
        let responses = mcts_validation_responses(given, response, &searched.root_actions,
            context.options.max_candidates, context.options.mcts_validate_roots);
        let mut totals = vec![0.0; responses.len() + 1];
        let frozen = searched.policy;
        stats.mcts_frozen_actions += frozen.len() as u64;
        // Proposal snapshots and generic seats are no longer needed while
        // validation allocates independent worlds.
        drop(worlds);
        let control = ismcts::FrozenPolicy::default();
        let validation = MctsOptions { terminal: context.options.horizon == 0,
            decisions: if context.options.horizon == 0 { context.limit } else { options.decisions }, ..options };
        for index in 0..context.options.worlds {
            if expired(deadline) { stats.budget_exhausted += 1; break; }
            let seed = context.seed ^ (decisions as u64).wrapping_mul(0x9E3779B97F4A7C15)
                ^ (index as u64 + 1).wrapping_mul(0xD1B54A32D192ED03);
            hidden.restore(duel, &root, context.viewer, seed)?;
            duel.reseed(seed ^ 0x94D049BB133111EB)?;
            let seats = prepare(duel, context, table, &opponent)?;
            let world = ismcts::World { snapshot: duel.snapshot()?, seats };
            let mut scores = Vec::new();
            for (response, policy) in std::iter::once((given, &control))
                .chain(responses.iter().map(|response| (response.as_slice(), &frozen))) {
                if expired(deadline) { break; }
                // Each trial restores its world; this closure's outer restore
                // below owns recovery of the live root on every Result path.
                let trial = ismcts::evaluate_frozen_in_place(duel, &world, context.viewer, original,
                    response, decisions, context.limit, validation, deadline, &evaluate, policy)?;
                stats.playouts += 1;
                stats.plan_evaluations += 1;
                stats.plan_applied += trial.stats.applied as u64;
                stats.plan_mismatches += trial.stats.mismatches as u64;
                stats.failed_playouts += trial.stats.failed as u64;
                stats.unresolved_playouts += u64::from(trial.value.is_none());
                stats.truncated_playouts += u64::from(trial.value.is_some() && !trial.terminal);
                for error in trial.stats.errors { stats.error(error); }
                scores.push(trial.value);
            }
            if scores.len() == totals.len() && scores.iter().all(Option::is_some) {
                let baseline = scores[0].unwrap();
                for (total, score) in totals.iter_mut().zip(scores) { *total += score.unwrap() - baseline; }
                complete += 1;
            } else { stats.incomplete_worlds += 1; }
        }
        let best = best_candidate(&totals, complete);
        advantage = totals[best];
        if best > 0 {
            proposed_response = responses[best - 1].clone();
            stats.mcts_accepted += 1;
            policy = Some(frozen);
        }
        Ok(())
    })();
    duel.restore(&root)?;
    if let Err(error) = result {
        stats.failed_searches += 1;
        stats.error(error);
        return Ok((given.to_vec(), 0, 0.0, None));
    }
    let gain = if complete == 0 { 0.0 } else { advantage / complete as f64 };
    if policy.is_none() { proposed_response = given.to_vec(); }
    Ok((proposed_response, complete, gain, policy))
}

/// The plain root with a searched continuation is a different candidate from
/// plain play. Only completed root returns rank additional proposals; all are
/// subsequently compared on independent, common validation worlds.
fn mcts_validation_responses(given: &[u8], best: Vec<u8>, actions: &[ismcts::RootAction],
    limit: usize, all: bool) -> Vec<Vec<u8>> {
    if !all { return vec![best]; }
    let mut responses = vec![given.to_vec()];
    let mut ranked: Vec<_> = actions.iter().enumerate()
        .filter(|(_, action)| action.visits > 0 && action.mean.is_some_and(f64::is_finite)).collect();
    ranked.sort_by(|(ai, a), (bi, b)| b.mean.unwrap().total_cmp(&a.mean.unwrap()).then(ai.cmp(bi)));
    for (_, action) in ranked {
        if responses.len() >= limit { break; }
        if !responses.contains(&action.response) { responses.push(action.response.clone()); }
    }
    responses
}

fn latency(values: &[f64]) -> serde_json::Value {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let quantile = |p: f64| {
        sorted
            .get(((sorted.len() as f64 * p).ceil() as usize).saturating_sub(1))
            .copied()
            .unwrap_or(0.0)
    };
    serde_json::json!({"p50": quantile(0.5), "p95": quantile(0.95), "max": sorted.last().copied().unwrap_or(0.0),
        "total": values.iter().sum::<f64>(), "count": values.len()})
}

/// Play a generic seat with a bounded simulation budget. This runner owns the
/// duel so it can snapshot and sample hidden worlds; the exported ordinary
/// generic policy remains engine independent. Set `max_playouts=0` for a plain
/// policy game with the same per-decision latency instrumentation.
pub fn play_fast(
    core: &Core,
    decks: &[Deck; 2],
    policies: [&PolicyLibrary; 2],
    names: [&str; 2],
    cards: &Path,
    run: PlayOptions,
    fast: FastOptions,
) -> Result<serde_json::Value> {
    fast.validate_evaluation_weights()?;
    if fast.specialist_opponent && fast.mcts.is_some_and(|options| options.adversarial_opponent) {
        return Err("Specialist opponent diagnostics cannot be combined with adversarial opponent UCT".into());
    }
    if fast.planner.is_some() && fast.mcts.is_some() { return Err("Choose beam or MCTS, not both".into()); }
    if let Some(staged) = fast.staged {
        if fast.planner.is_some() || fast.mcts.is_some() {
            return Err("The staged search replaces the rollout choice; it cannot be combined with beam or MCTS".into());
        }
        if fast.depth != 1 || fast.horizon != 0 || fast.millis != 0 {
            return Err("The staged search plays every world to the end: depth 1, horizon 0, no time budget".into());
        }
        if staged.stages[0] == 0 || staged.stages[0] > staged.stages[1] || staged.stages[1] > staged.stages[2]
            || staged.kept == 0 || !staged.z.is_finite() {
            return Err("Staged search needs 0 < first stage <= middle stage <= last stage, kept > 0 and a finite z".into());
        }
        if staged.set_monsters && !fast.strict {
            return Err("Dealing Set monsters again replaces the strict guard; it needs strict=true".into());
        }
    }
    if fast.mcts.is_some_and(|p| p.iterations == 0 || p.actions == 0 || p.nodes == 0 || p.decisions == 0)
        || fast.mcts.is_some() && fast.mcts_worlds == 0 {
        return Err("MCTS iterations/actions/nodes/decisions/worlds must be positive".into());
    }
    if fast.planner.is_some_and(|p| p.width == 0 || p.actions == 0 || p.nodes == 0 || p.decisions == 0) {
        return Err("Beam width/actions/nodes/decisions must be positive".into());
    }
    if (fast.planner.is_some() || fast.mcts.is_some()) && fast.depth != 1 {
        return Err("Nested rollout depth does not apply to beam planning".into());
    }
    if fast.max_playouts > 0 && ![1, 2].contains(&fast.depth) {
        return Err("Generic search depth must be 1 or 2".into());
    }
    if fast.max_playouts > 0 && !fast.strict {
        return Err(
            "Generic search requires strict=true: unknown opposing face-down monsters and opposing face-down banished cards cannot be sampled safely"
                .into(),
        );
    }
    if fast.searcher > 1 {
        return Err("Generic searcher must be seat 0 or 1".into());
    }
    if fast.borrow != 0 && fast.max_playouts > 0 {
        return Err("Borrowing the specialist's answers is a plain-play diagnostic".into());
    }
    if fast.max_playouts > 0
        && (!core.has_snapshots()
            || !core.has_reseed()
            || fast.worlds == 0
            || fast.max_candidates < 2)
    {
        return Err(
            "Generic search requires snapshots, random reseeding, positive worlds and at least two candidates".into(),
        );
    }
    if fast.specialist_opponent && fast.max_playouts > 0 && !policies[1 - fast.searcher].can_fork() {
        return Err("Specialist opponent diagnostics require a copyable baseline policy library".into());
    }
    let PlayOptions {
        seed,
        limit,
        life_points,
        record,
        trace,
    } = run;
    let options = DuelOptions {
        life_points,
        ..DuelOptions::seeded(seed)
    };
    let mut duel = core.deal(&options, decks)?;
    let mut recorded = record.then(|| Recorded::dealt(&options, decks));
    let mut table = Table {
        seats: Vec::new(),
        logs: [Vec::new(), Vec::new()],
    };
    // Definitions are public rules. Extract only our deck's card definitions;
    // this enrichment does not require knowing the opponent's deck list.
    let profiles = if fast.effects {
        let codes: Vec<_> = decks[fast.searcher].main.iter()
            .chain(&decks[fast.searcher].extra).copied().collect();
        Some(core.effect_profiles(&codes)?)
    } else { None };
    for player in 0..2 {
        table.seats.push(policies[player].seat(
            names[player],
            cards,
            player as i32,
            seed + player as u64,
        )?);
        if names[player] == "generic" {
            if let Some(profiles) = &profiles { table.seats[player].set_effect_profiles(profiles)?; }
        }
        if player == fast.searcher && (fast.max_playouts > 0 || fast.borrow != 0) {
            table.seats[player].enable_response_overrides()?;
        }
        table.feed(player, &duel.start_message(player as u8))?;
    }
    // The deck's specialist, from the searching seat's library, in that seat:
    // it sees what the seat sees and remembers the answers actually played.
    let shadow = if fast.borrow != 0 {
        let seat = policies[fast.searcher].seat(names[1 - fast.searcher], cards, fast.searcher as i32,
            seed + fast.searcher as u64)?;
        seat.enable_response_overrides()?;
        seat.feed(&duel.start_message(fast.searcher as u8))?;
        Some(seat)
    } else { None };
    let mut shadow_answer: Option<Vec<u8>> = None;
    let (mut borrowed, mut disagreements, mut unborrowable, mut unfollowed) = (0u64, 0u64, 0u64, 0u64);
    let mut evaluation_profile = None;
    let evaluator = if fast.planner.is_some() || fast.mcts.is_some() {
        let mut evaluator = Evaluator::open(cards)?;
        if let Some(weights) = fast.evaluation_weights { evaluator.set_weights(weights)?; }
        if let Some(profiles) = &profiles { evaluator.set_profiles(profiles)?; }
        if fast.evaluation_profile == EvaluationProfile::LpDensityV1 {
            let profile = evaluator.lp_density_profile(&decks[fast.searcher].main)?;
            evaluator.set_weights(profile.weights)?;
            let mut described = profile.metadata();
            described["mode"] = serde_json::json!(fast.evaluation_profile.name());
            evaluation_profile = Some(described);
        }
        Some(evaluator)
    } else { None };
    let context = Context {
        model_library: policies[if fast.specialist_opponent { 1 - fast.searcher } else { fast.searcher }],
        model_name: if fast.specialist_opponent { names[1 - fast.searcher] } else { "generic" },
        cards,
        viewer: fast.searcher as u8,
        seed,
        limit,
        options: fast,
        profiles,
        evaluator,
    };
    let mut stats = Stats::default();
    let (mut decisions, mut turns, mut digest, mut chain) =
        (0usize, 0u32, 0xcbf29ce484222325u64, false);
    let (mut in_chain, mut facing_set) = (0u64, 0u64);
    let mut traces = Vec::new();
    let mut deviations = Vec::new();
    let mut plan: VecDeque<PlannedAction> = VecDeque::new();
    let mut tree: Option<(ismcts::FrozenPolicy, ismcts::Path)> = None;
    let mut public_memory = PublicIdentityMemory::default();
    let mut decision_ms: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    let mut search_decision_ms: Vec<f64> = Vec::new();
    loop {
        let step = duel.step()?;
        let mut response = None;
        for sent in &step.messages {
            public_memory.observe(&sent.bytes)?;
            if let Some((_, path)) = &mut tree { path.message(&sent.bytes, context.viewer)?; }
            if sent.refresh {
                table.update(&sent.bytes)?;
                if let Some(shadow) = &shadow { shadow.feed(&sent.bytes)?; }
                continue;
            }
            match sent.bytes[0] {
                msg::RETRY => return Err("Engine rejected response (MSG_RETRY)".into()),
                msg::NEW_TURN => turns += 1,
                msg::CHAINING => chain = true,
                msg::CHAIN_END => chain = false,
                _ => {}
            }
            if let Some(shadow) = &shadow {
                if let Some(answer) = shadow.feed(&sent.bytes)? { shadow_answer = Some(answer); }
            }
            for player in 0..2 {
                let started = Instant::now();
                if let Some(answer) = table.feed(player, &sent.bytes)? {
                    if response.is_some() {
                        return Err("Multiple answers in one engine batch".into());
                    }
                    response = Some((
                        player,
                        answer,
                        sent.bytes.clone(),
                        started.elapsed().as_secs_f64() * 1000.0,
                    ));
                }
            }
        }
        if matches!(step.state, State::Over(_)) || decisions >= limit {
            let (winner, reason) = match step.state {
                State::Over(Some(outcome)) => (Some(outcome.winner), Some(outcome.reason)),
                State::Over(None) => return Err("Engine ended without MSG_WIN".into()),
                _ => (None, None),
            };
            let mut row = serde_json::json!({"winner": winner, "reason": reason, "turns": turns, "decisions": decisions,
                "limit": winner.is_none(), "digest": format!("{digest:016x}"), "searched": stats.searched,
                "playouts": stats.playouts, "inner_searches": stats.inner_searches, "inner_playouts": stats.inner_playouts,
                "failed_playouts": stats.failed_playouts, "truncated_playouts": stats.truncated_playouts,
                "unresolved_playouts": stats.unresolved_playouts,
                "budget_exhausted": stats.budget_exhausted, "incomplete_worlds": stats.incomplete_worlds,
                "failed_searches": stats.failed_searches, "unsafe_selections": stats.unsafe_selection, "search_errors": stats.errors,
                "search_skipped_facedown_banished": stats.search_skipped_facedown_banished,
                "proposal_nodes": stats.proposal_nodes, "proposal_leaves": stats.proposal_leaves,
                "proposal_improvements": stats.proposal_improvements,
                "proposal_completed_prefixes": stats.proposal_completed_prefixes,
                "proposal_responses": stats.proposal_responses, "plan_evaluations": stats.plan_evaluations,
                "plan_applied": stats.plan_applied, "plan_mismatches": stats.plan_mismatches,
                "live_plan_applied": stats.live_plan_applied, "live_plan_mismatches": stats.live_plan_mismatches,
                "in_chain": in_chain, "facing_set": facing_set, "deviations": deviations, "trace": traces,
                "latency_ms": latency(&decision_ms[fast.searcher]), "decision_ms": decision_ms[fast.searcher],
                "search_latency_ms": latency(&search_decision_ms), "search_decision_ms": search_decision_ms,
                "soft_budget_overruns": if fast.millis == 0 { 0 } else { search_decision_ms.iter().filter(|ms| **ms > fast.millis as f64).count() },
                "decision_ms_by_seat": decision_ms,
                "search_information": "known deck composition; sampled hidden arrangement and future RNG; generic opponent model"});
            for (key, value) in [
                ("mcts_simulations", stats.mcts_simulations),
                ("mcts_completed", stats.mcts_completed),
                ("mcts_nodes", stats.mcts_nodes),
                ("mcts_steps", stats.mcts_steps),
                ("mcts_tree_hits", stats.mcts_tree_hits),
                ("mcts_opponent_nodes", stats.mcts_opponent_nodes),
                ("mcts_opponent_actions", stats.mcts_opponent_actions),
                ("mcts_opponent_tree_hits", stats.mcts_opponent_tree_hits),
                ("mcts_cutoffs", stats.mcts_cutoffs),
                ("mcts_live_hits", stats.mcts_live_hits),
                ("mcts_live_missing", stats.mcts_live_missing),
                ("mcts_frozen_actions", stats.mcts_frozen_actions),
                ("mcts_accepted", stats.mcts_accepted),
                ("mcts_root_actions", stats.mcts_root_actions),
                ("mcts_visited_root_actions", stats.mcts_visited_root_actions),
                ("mcts_root_actions_all_worlds_attempted", stats.mcts_root_actions_all_worlds_attempted),
                ("mcts_root_actions_all_worlds_completed", stats.mcts_root_actions_all_worlds_completed),
            ] { row[key] = serde_json::json!(value); }
            row["search_opponent_model"] = serde_json::json!(if fast.specialist_opponent { "specialist" } else { "generic" });
            row["resampled_set_monsters"] = serde_json::json!(stats.resampled_set_monsters);
            row["undealt_roots"] = serde_json::json!(stats.undealt_roots);
            if shadow.is_some() {
                row["borrow"] = serde_json::json!({"disagreements": disagreements, "borrowed": borrowed,
                    "unborrowable": unborrowable, "unfollowed": unfollowed});
            }
            if fast.specialist_opponent {
                row["search_information"] = serde_json::json!("known deck composition and specialist opponent policy; sampled hidden arrangement and future RNG; public-history model memory");
            } else if fast.mcts.is_some_and(|options| options.adversarial_opponent) {
                row["search_information"] = serde_json::json!("known deck composition; sampled hidden arrangement and future RNG; pessimistic sampled-world opponent UCT proposals; Generic opponent validation");
            }
            row["mcts_snapshot_copies"] = serde_json::Value::Array(stats.snapshot_copies.iter().map(|metrics| {
                serde_json::json!({
                    "snapshots": metrics.snapshots, "restores": metrics.restores,
                    "max_snapshot_bytes": metrics.max_snapshot_bytes,
                    "snapshotted_bytes": metrics.snapshotted_bytes,
                    "restored_bytes": metrics.restored_bytes,
                })
            }).collect());
            if let Some(recorded) = &recorded {
                row["record"] = record_json(recorded, names);
            }
            if let Some(profile) = &evaluation_profile {
                row["evaluation_profile"] = profile.clone();
            }
            return Ok(row);
        }
        let Some((player, mut answer, message, mut elapsed)) = response else {
            if step.state == State::Awaiting {
                return Err("Engine awaiting without policy answer".into());
            }
            continue;
        };
        if player == fast.searcher {
            if let (Some(shadow), Some(theirs)) = (&shadow, shadow_answer.take()) {
                if theirs != answer {
                    disagreements += 1;
                    let wanted = fast.borrow & borrow_bits(&table.seats[player].last_answer()?, &shadow.last_answer()?) != 0;
                    // The seat that does not decide remembers the answer played.
                    if wanted && follow(&table.seats[player], &theirs) {
                        answer = theirs;
                        borrowed += 1;
                    } else {
                        unborrowable += u64::from(wanted);
                        unfollowed += u64::from(!follow(shadow, &answer));
                    }
                }
            }
        }
        let mut searched_this_decision = false;
        if player == fast.searcher && fast.max_playouts > 0 {
            let started = Instant::now();
            // Consume saved continuation BEFORE active-chain guards. Only the
            // original visible prompt and exact offered action authorize replay.
            let original = table.seats[player].last_answer()?;
            let mut replayed = false;
            if let Some((policy, path)) = &tree {
                if policy.has(&original, *path) {
                    let expanded = table.seats[player].alternative_responses(12)?;
                    if let Some(planned) = policy.resolve(&original, &expanded, *path) {
                        answer = planned;
                        table.seats[player].override_response(&answer)?;
                        stats.mcts_live_hits += 1;
                        replayed = true;
                    }
                }
                if !replayed { stats.mcts_live_missing += 1; }
            }
            if let Some(action) = plan.front() {
                if action.signature == planner::prompt_signature(&original) {
                    let expanded = table.seats[player].alternative_responses(12)?;
                    if let Some(planned) = planner::resolve_action(action, &expanded) {
                        answer = planned;
                        table.seats[player].override_response(&answer)?;
                        plan.pop_front();
                        stats.live_plan_applied += 1;
                        replayed = true;
                    }
                }
                if !replayed {
                    plan.clear();
                    stats.live_plan_mismatches += 1;
                }
            }
            if !replayed {
                let expanded_limit = if fast.planner.is_some() || fast.mcts.is_some() { 12 } else { fast.max_candidates };
                let described = table.seats[player].alternative_responses(expanded_limit)?;
                if let Some(listed) = candidates(&described, &answer,
                    fast.max_candidates.min(fast.max_playouts)) {
                    let staged = fast.staged;
                    let resample = staged.is_some_and(|staged| staged.set_monsters);
                    let unknown = if resample {
                        unknown_set_monsters(&public_memory, &original["observation"], context.viewer)
                    } else {
                        Vec::new()
                    };
                    if chain && !staged.is_some_and(|staged| staged.chains) { in_chain += 1; }
                    else if fast.strict && !resample && public_memory.has_unknown_monsters(&original["observation"], context.viewer) {
                        facing_set += 1;
                    }
                    else if fast.strict && public_memory.has_facedown_banished(&original["observation"], context.viewer) {
                        stats.search_skipped_facedown_banished += 1;
                    }
                    else if unsafe_selection(&described, context.viewer) { stats.unsafe_selection += 1; }
                    else {
                        stats.searched += 1;
                        searched_this_decision = true;
                        if fast.mcts.is_some() {
                            let before = answer.clone();
                            let (selected, worlds, gain, policy) = choose_mcts(&mut duel, &context,
                                &table, &original, &answer, decisions, &mut stats)?;
                            tree = policy.map(|policy| (policy, ismcts::Path::default()));
                            answer = selected;
                            if answer != before {
                                table.seats[player].override_response(&answer)?;
                                deviations.push(serde_json::json!({"decision": decisions, "turn": turns,
                                    "kind": listed.kind, "from_response": before, "to_response": answer,
                                    "worlds": worlds, "gain": gain}));
                            }
                        } else {
                        let (chosen, worlds, gain) = if fast.planner.is_some() {
                            let (chosen, worlds, gain, continuation) = choose_plan(
                                &mut duel, &context, &table, &listed, decisions, &mut stats)?;
                            plan = continuation.into();
                            (chosen, worlds, gain)
                        } else if let Some(staged) = staged {
                            stats.resampled_set_monsters += u64::from(!unknown.is_empty());
                            choose_staged(&mut duel, &context, &table, &listed, decisions, &mut stats, staged, unknown)?
                        } else { choose(&mut duel, &context, &table, &listed, decisions, &mut stats)? };
                        if chosen != 0 {
                            answer = listed.responses[chosen].clone();
                            table.seats[player].override_response(&answer)?;
                            deviations.push(serde_json::json!({"decision": decisions, "turn": turns, "kind": listed.kind,
                                "from": listed.labels[0], "to": listed.labels[chosen], "worlds": worlds, "gain": gain}));
                        }
                        }
                    }
                }
            }
            elapsed += started.elapsed().as_secs_f64() * 1000.0;
        }
        if searched_this_decision {
            search_decision_ms.push(elapsed);
        }
        decision_ms[player].push(elapsed);
        for byte in message.iter().chain(&answer) {
            digest = (digest ^ *byte as u64).wrapping_mul(0x100000001b3);
        }
        if player == fast.searcher {
            if let Some((_, path)) = &mut tree { path.own_response(&answer); }
        }
        if trace { traces.push(table.seats[player].last_answer()?); }
        duel.respond(&answer);
        if let Some(recorded) = &mut recorded {
            recorded.responses.push((player as u8, answer));
        }
        decisions += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_api_rejects_ignored_or_invalid_evaluation_overrides() {
        let mut options = FastOptions {
            searcher: 0, worlds: 4, depth: 1, max_candidates: 6,
            max_playouts: 256, horizon: 1, millis: 0, strict: true,
            effects: true, planner: None, mcts: None, mcts_worlds: 4,
            mcts_validate_roots: false, specialist_opponent: false,
            evaluation_weights: Some(super::super::evaluation::HEURISTIC_COEFFICIENTS),
            evaluation_profile: EvaluationProfile::Off, staged: None, borrow: 0,
        };
        assert!(options.validate_evaluation_weights().is_err());
        options.mcts = Some(MctsOptions { iterations: 128, actions: 6, nodes: 1024,
            decisions: 256, replies: 1, terminal: false, adversarial_opponent: false, root_warmup: false });
        assert!(options.validate_evaluation_weights().is_ok());
        options.max_playouts = 0;
        assert!(options.validate_evaluation_weights().is_err());
        options.max_playouts = 256;
        options.mcts.as_mut().unwrap().terminal = true;
        options.horizon = 0;
        assert!(options.validate_evaluation_weights().is_err());
        options.horizon = 1;
        assert!(options.validate_evaluation_weights().is_ok());
        options.horizon = 0;
        options.mcts.as_mut().unwrap().terminal = false;
        assert!(options.validate_evaluation_weights().is_ok());
        options.evaluation_weights.as_mut().unwrap()[0] = f64::NAN;
        assert!(options.validate_evaluation_weights().is_err());
        options.evaluation_weights = None;
        options.mcts = None;
        options.max_playouts = 0;
        assert!(options.validate_evaluation_weights().is_ok());
        options.evaluation_profile = EvaluationProfile::LpDensityV1;
        assert!(options.validate_evaluation_weights().is_err());
        options.max_playouts = 256;
        options.planner = Some(PlannerOptions { width: 2, actions: 2, nodes: 8, decisions: 16, replies: 1 });
        assert!(options.validate_evaluation_weights().is_ok());
        options.evaluation_weights = Some(super::super::evaluation::HEURISTIC_COEFFICIENTS);
        assert!(options.validate_evaluation_weights().is_err(), "manual/profile conflict");
        options.evaluation_weights = None;
        options.effects = false;
        assert!(options.validate_evaluation_weights().is_err(), "incomplete metadata");
        options.effects = true;
        options.planner = None;
        options.mcts = Some(MctsOptions { iterations: 8, actions: 2, nodes: 16,
            decisions: 16, replies: 1, terminal: true, adversarial_opponent: false, root_warmup: false });
        assert!(options.validate_evaluation_weights().is_err(), "terminal-only ignores profiles");
        options.horizon = 1;
        assert!(options.validate_evaluation_weights().is_ok());
    }

    #[test]
    fn mcts_validation_keeps_plain_continuation_and_only_completed_distinct_roots() {
        let actions = vec![
            ismcts::RootAction { response: vec![1], mean: Some(0.2), visits: 2, attempts: 2 },
            ismcts::RootAction { response: vec![2], mean: Some(0.8), visits: 3, attempts: 3 },
            ismcts::RootAction { response: vec![3], mean: None, visits: 0, attempts: 4 },
            ismcts::RootAction { response: vec![4], mean: Some(0.8), visits: 1, attempts: 1 },
            ismcts::RootAction { response: vec![5], mean: Some(f64::NAN), visits: 1, attempts: 1 },
        ];
        assert_eq!(mcts_validation_responses(&[1], vec![2], &actions, 4, true),
            vec![vec![1], vec![2], vec![4]]);
        assert_eq!(mcts_validation_responses(&[1], vec![2], &actions, 2, true),
            vec![vec![1], vec![2]]);
        assert_eq!(mcts_validation_responses(&[1], vec![2], &actions, 4, false), vec![vec![2]]);
    }

    #[test]
    fn incomplete_worlds_and_ties_do_not_change_the_pilot() {
        assert_eq!(best_candidate(&[0.0, 2.0], 0), 0);
        assert_eq!(best_candidate(&[0.0, 0.0, -1.0], 2), 0);
        assert_eq!(best_candidate(&[0.0, 1.0, 2.0], 2), 2);
    }

    #[test]
    fn hidden_monster_stats_do_not_change_leaf_value() {
        let mut observation = serde_json::json!({"life_points": [8000,8000], "cards": [{
            "at": {"controller": 1, "location": "MonsterZone"}, "position": {"face_up": false},
            "code": null, "attack": 0, "defense": 0}]});
        let value = leaf_value(&observation, 0);
        observation["cards"][0]["code"] = serde_json::json!(123);
        observation["cards"][0]["attack"] = serde_json::json!(9999);
        observation["cards"][0]["defense"] = serde_json::json!(9999);
        assert_eq!(leaf_value(&observation, 0), value);
    }

    #[test]
    fn private_history_does_not_affect_the_opponent_model() {
        use ygo_policies_ocgcore::wire::Writer;
        let history = |code: u32| {
            let messages = [
                Writer::default()
                    .u8(msg::DRAW)
                    .u8(1)
                    .u32(1)
                    .u32(code)
                    .u32(position::FACEDOWN_DEFENSE)
                    .0,
                Writer::default()
                    .u8(msg::SELECT_YESNO)
                    .u8(1)
                    .u64(code as u64)
                    .0,
                Writer::default().u8(msg::NEW_TURN).u8(1).0,
            ];
            let mut buffer = Vec::new();
            for message in messages {
                buffer.extend_from_slice(&(message.len() as u32).to_le_bytes());
                buffer.extend(message);
            }
            buffer
        };
        let first = public_history(&history(100)).unwrap();
        assert_eq!(first, public_history(&history(200)).unwrap());
        assert!(!first.is_empty());
    }

    #[test]
    #[ignore = "load native policy libraries; cheap public-history specialist fixture"]
    fn specialist_public_history_uses_sampled_hand_and_forks_without_new_ffi_hooks() {
        use ygo_policies_ocgcore::wire::{location, Writer};
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cards = root.join("vendor/BabelCdb/cards.cdb");
        let candidate_path = std::env::var_os("YGO_MCTS_TEST_LIBRARY")
            .map(std::path::PathBuf::from).unwrap_or_else(|| root.join(format!(
                "target/release/{}ygo_policies{}", std::env::consts::DLL_PREFIX,
                std::env::consts::DLL_SUFFIX)));
        let opponent_path = std::env::var_os("YGO_MCTS_TEST_OPPONENT")
            .map(std::path::PathBuf::from).unwrap_or_else(|| candidate_path.clone());
        let candidate = PolicyLibrary::open(&candidate_path).unwrap();
        let opponent = PolicyLibrary::open(&opponent_path).unwrap();
        assert!(opponent.can_fork());
        let context = Context {
            model_library: &opponent, model_name: "demise", cards: &cards,
            viewer: 0, seed: 2472000, limit: 8192,
            options: FastOptions {
                searcher: 0, worlds: 4, depth: 1, max_candidates: 8,
                max_playouts: 256, horizon: 1, millis: 0, strict: true,
                effects: true, planner: None,
                mcts: Some(MctsOptions { iterations: 128, actions: 8, nodes: 1024,
                    decisions: 384, replies: 1, terminal: false, adversarial_opponent: false, root_warmup: false }),
                mcts_worlds: 4, mcts_validate_roots: false, specialist_opponent: true, evaluation_weights: None, evaluation_profile: EvaluationProfile::Off, staged: None, borrow: 0,
            },
            // Nonempty metadata must not be sent to the old specialist ABI.
            profiles: Some(serde_json::json!({"95492061":[]})), evaluator: None,
        };
        let hand_update = |code| {
            let record = Writer::default().u16(8).u32(query::CODE).u32(code)
                .u16(8).u32(query::POSITION).u32(position::FACEDOWN_DEFENSE)
                .u16(4).u32(query::END).0;
            let mut update = Writer::default().u8(msg::UPDATE_DATA).u8(1).u8(location::HAND)
                .u32(record.len() as u32).0;
            update.extend(record);
            update
        };
        let history = |private_code| {
            let messages = [
                message::start_message(1, [8000; 2], [40; 2], [0; 2]),
                Writer::default().u8(msg::DRAW).u8(1).u32(1).u32(private_code)
                    .u32(position::FACEDOWN_DEFENSE).0,
                hand_update(private_code),
                Writer::default().u8(msg::SELECT_OPTION).u8(1).u8(2)
                    .u64(u64::from(private_code)).u64(1).0,
                Writer::default().u8(msg::LPUPDATE).u8(0).u32(7600).0,
                Writer::default().u8(msg::NEW_TURN).u8(0).0,
                Writer::default().u8(msg::NEW_TURN).u8(1).0,
                Writer::default().u8(msg::NEW_PHASE).u16(0x04).0,
            ];
            let mut logs = [Vec::new(), Vec::new()];
            for bytes in messages {
                logs[1].extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                logs[1].extend(bytes);
            }
            logs
        };
        let first = public_opponent(&context, &history(72426662), 17).unwrap();
        let second = public_opponent(&context, &history(46427957), 17).unwrap();
        let public = first.observation().unwrap();
        assert_eq!(public, second.observation().unwrap());
        assert_eq!(public["life_points"][0], 7600, "public events must be retained");
        assert!(first.last_answer().unwrap().is_null());
        assert!(second.last_answer().unwrap().is_null(), "private historical prompts must never be answered");
        assert!(public["cards"].as_array().unwrap().iter().all(|card| card["code"].is_null()));

        // This is the hand refresh used after hidden-world sampling. Exercise
        // paired updates across the actual old/new library boundary as well.
        let viewers: Vec<_> = (0..2).map(|_| candidate.seat("generic", &cards, 0, 2472000).unwrap()).collect();
        let sampled = hand_update(95492061); // Manju, unlike either historical private card.
        for (viewer, model) in viewers.iter().zip([&first, &second]) {
            viewer.feed(&message::start_message(0, [8000; 2], [40; 2], [0; 2])).unwrap();
            viewer.feed_update_pair(model, &sampled).unwrap();
            let observation = model.observation().unwrap();
            let hand: Vec<_> = observation["cards"].as_array().unwrap().iter()
                .filter(|card| card["at"]["controller"] == 1 && card["at"]["location"] == "Hand")
                .map(|card| card["code"].clone()).collect();
            assert_eq!(hand, vec![serde_json::json!(95492061)]);
            assert!(viewer.observation().unwrap()["cards"].as_array().unwrap().iter()
                .all(|card| card["code"].is_null()), "sampled opponent hand stays hidden from the root viewer");
        }
        let first_fork = first.fork().unwrap();
        let second_fork = second.fork().unwrap();
        let mut idle = Writer::default().u8(msg::SELECT_IDLECMD).u8(1)
            .u32(1).u32(95492061).u8(1).u8(location::HAND).u32(0);
        for _ in 0..5 { idle = idle.u32(0); }
        let idle = idle.u8(0).u8(1).u8(0).0;
        let mut answers = Vec::new();
        for model in [&first, &second, &first_fork, &second_fork] {
            assert_eq!(model.feed(&idle).unwrap(), Some(0u32.to_le_bytes().to_vec()),
                "Demise should summon sampled Manju");
            answers.push(model.last_answer().unwrap());
        }
        assert!(answers.windows(2).all(|pair| pair[0] == pair[1]),
            "private histories and model forks must produce identical sampled decisions");
        let selected = &answers[0]["decision"]["choices"][answers[0]["choice"].as_u64().unwrap() as usize];
        assert_eq!(selected["kind"], "NormalSummon");
        assert_eq!(selected["card"]["code"], 95492061);
    }

    #[test]
    fn shortlist_retains_different_action_kinds() {
        let mut choices = Vec::new();
        let mut responses = Vec::new();
        for index in 0..8 {
            choices.push(serde_json::json!({"kind":"Activate", "card":{"code":index + 100,"at":{"controller":0,"location":"Hand","sequence":index}},"description":0}));
            responses.push(serde_json::json!([index]));
        }
        choices.push(serde_json::json!({"kind":"NormalSummon", "card":{"code":200,"at":{"controller":0,"location":"Hand","sequence":8}},"description":0}));
        responses.push(serde_json::json!([8]));
        let answer = serde_json::json!({"choice":0,"picks":null,"decision":{"kind":"Idle","choices":choices},"responses":responses});
        let listed = candidates(&answer, &[0], 3).unwrap();
        assert_eq!(listed.responses, vec![vec![0], vec![1], vec![8]]);
    }

    #[test]
    fn remembered_hand_and_deck_cards_are_preserved() {
        let observation = serde_json::json!({"cards":[
            {"code":10,"at":{"controller":1,"location":"Hand","sequence":2}},
            {"code":20,"at":{"controller":0,"location":"Deck","sequence":0}},
            {"code":null,"at":{"controller":1,"location":"Hand","sequence":3}},
            {"code":30,"at":{"controller":1,"location":"MonsterZone","sequence":0}}
        ]});
        assert_eq!(known_slots(&observation), vec![(1, 2, 2), (0, 1, 0)]);
    }

    #[test]
    fn pending_unknown_sampled_cards_block_root_search_without_freezing_deck_order() {
        let mut answer = serde_json::json!({"observation":{"cards":[]},"decision":{"kind":"SelectCards","choices":[
            {"kind":"Cards","members":[{"code":null,"at":{"controller":1,"location":"Hand","sequence":2}}]}
        ]}});
        assert!(unsafe_selection(&answer, 0));
        answer["decision"]["choices"][0]["members"][0]["code"] = serde_json::json!(123);
        assert!(!unsafe_selection(&answer, 0));
        assert_eq!(pending_known_slots(&answer, 0), vec![(1, 2, 2)]);
        answer["decision"]["choices"][0]["members"][0]["at"]["location"] =
            serde_json::json!("Deck");
        assert!(unsafe_selection(&answer, 0));
        assert!(pending_known_slots(&answer, 0).is_empty());
        answer["decision"]["choices"][0]["members"][0]["at"]["controller"] = serde_json::json!(0);
        assert!(!unsafe_selection(&answer, 0)); // A known own-deck pointer remains in our deck.
        assert!(pending_known_slots(&answer, 0).is_empty());
    }

    #[test]
    #[ignore = "build the release policy library first"]
    fn staged_search_deals_set_monsters_again_and_replays_deterministically() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cards = root.join("vendor/BabelCdb/cards.cdb");
        let core = Core::open(None, &cards, &root.join("vendor/CardScripts")).unwrap();
        let library = PolicyLibrary::open(&root.join(format!(
            "target/release/{}ygo_policies{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )))
        .unwrap();
        // A mirror of Set flip monsters (Morphing Jar #2 Special Summons face-down).
        let deck = Deck::load(&root.join("decks/Last Page.ydk")).unwrap();
        let decks = [deck.clone(), deck];
        let run = PlayOptions { seed: 2700001, limit: 2048, trace: false, life_points: [8000, 8000], record: false };
        let fast = FastOptions {
            searcher: 0, worlds: 4, depth: 1, max_candidates: 6, max_playouts: 1_000_000, horizon: 0, millis: 0,
            strict: true, effects: true, planner: None, mcts: None, mcts_worlds: 4, mcts_validate_roots: false,
            specialist_opponent: false, evaluation_weights: None, evaluation_profile: EvaluationProfile::Off,
            staged: Some(StagedOptions { stages: [2, 4, 8], ..StagedOptions::MEASURED }), borrow: 0,
        };
        let play = || play_fast(&core, &decks, [&library, &library], ["generic", "deckout"], &cards, run, fast).unwrap();
        let (first, second) = (play(), play());
        assert_eq!(first["digest"], second["digest"]);
        assert_eq!(first["winner"], second["winner"]);
        assert!(first["resampled_set_monsters"].as_u64().unwrap() > 0, "no Set monster was dealt again");
        assert_eq!(first["facing_set"], 0);
        assert_eq!(first["in_chain"], 0);
        for key in ["failed_playouts", "failed_searches"] {
            assert_eq!(first[key], 0, "{key}: {}", first["search_errors"]);
        }
        // Without a staged search the strict guards stay as they were.
        let strict = play_fast(&core, &decks, [&library, &library], ["generic", "deckout"], &cards, run,
            FastOptions { staged: Some(StagedOptions { stages: [2, 4, 8], chains: false, set_monsters: false, ..StagedOptions::MEASURED }), ..fast }).unwrap();
        assert_eq!(strict["resampled_set_monsters"], 0);
        assert!(strict["facing_set"].as_u64().unwrap() > 0);
    }

    #[test]
    #[ignore = "build the release policy library first"]
    fn depth_two_respects_budget_and_restores_a_replayable_live_duel() {
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
        let run = PlayOptions {
            seed: 2413000,
            limit: 512,
            trace: false,
            life_points: [8000, 8000],
            record: true,
        };
        let fast = FastOptions {
            searcher: 0,
            worlds: 2,
            depth: 2,
            max_candidates: 3,
            max_playouts: 12,
            horizon: 8,
            millis: 0,
            strict: true,
            effects: false,
            planner: None,
            mcts: None,
            mcts_worlds: 4, mcts_validate_roots: false, specialist_opponent: false, evaluation_weights: None, evaluation_profile: EvaluationProfile::Off, staged: None, borrow: 0,
        };
        let first = play_fast(
            &core,
            &decks,
            [&library, &library],
            ["generic", "generic"],
            &cards,
            run,
            fast,
        )
        .unwrap();
        let second = play_fast(
            &core,
            &decks,
            [&library, &library],
            ["generic", "generic"],
            &cards,
            run,
            fast,
        )
        .unwrap();
        assert_eq!(first["digest"], second["digest"]);
        assert_eq!(first["playouts"], second["playouts"]);
        assert_eq!(first["failed_playouts"], 0);
        assert_eq!(first["failed_searches"], 0);
        assert_eq!(first["limit"], false);
        assert!(first["inner_searches"].as_u64().unwrap() > 0);
        assert!(first["inner_playouts"].as_u64().unwrap() > 0);
        assert!(first["playouts"].as_u64().unwrap() <= first["searched"].as_u64().unwrap() * 12);
        let record = Recorded::from_json(&first["record"]).unwrap();
        let mut winner = None;
        let left = core
            .replay(&record, None, &mut |shown| {
                if let Replayed::Message(Message::Win { player, .. }) = shown {
                    winner = Some(*player);
                }
            })
            .unwrap();
        assert_eq!(left, 0);
        assert_eq!(winner.map(u64::from), first["winner"].as_u64());
    }

    #[test]
    #[ignore = "build the release policy library first; native beam integration"]
    fn beam_adapter_executes_validated_plans_and_replays_deterministically() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cards = root.join("vendor/BabelCdb/cards.cdb");
        let core = Core::open(None, &cards, &root.join("vendor/CardScripts")).unwrap();
        let library_path = std::env::var_os("YGO_PLANNER_TEST_LIBRARY")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| root.join(format!(
                "target/release/{}ygo_policies{}",
                std::env::consts::DLL_PREFIX,
                std::env::consts::DLL_SUFFIX,
            )));
        let library = PolicyLibrary::open(&library_path).unwrap();
        let opponent_path = std::env::var_os("YGO_PLANNER_TEST_OPPONENT")
            .map(std::path::PathBuf::from).unwrap_or(library_path);
        let opponent = PolicyLibrary::open(&opponent_path).unwrap();
        let deck = Deck::load(&root.join("decks").join(format!("{}.ydk", opponent.catalog["blackwing"]))).unwrap();
        let decks = [deck.clone(), deck];
        let run = PlayOptions {
            seed: 1810280040, limit: 8192, trace: false,
            life_points: [8000; 2], record: true,
        };
        let options = FastOptions {
            searcher: 0, worlds: 4, depth: 1, max_candidates: 6,
            max_playouts: 512, horizon: 1, millis: 0, strict: true,
            effects: true,
            mcts: None,
            mcts_worlds: 4, mcts_validate_roots: false, specialist_opponent: false, evaluation_weights: None, evaluation_profile: EvaluationProfile::Off, staged: None, borrow: 0,
            planner: Some(PlannerOptions {
                width: 6, actions: 4, nodes: 64, decisions: 256, replies: 1,
            }),
        };
        let play = |evaluation_profile| play_fast(
            &core, &decks, [&library, &opponent], ["generic", "blackwing"],
            &cards, run, FastOptions { evaluation_profile, ..options },
        ).unwrap();
        let first = play(EvaluationProfile::Off);
        let second = play(EvaluationProfile::LpDensityV1);
        assert!(first.get("evaluation_profile").is_none());
        assert_eq!(second["evaluation_profile"]["alpha"], 0.0);
        assert_eq!(second["evaluation_profile"]["effective_weights"],
            super::super::evaluation::named_weights(&super::super::evaluation::HEURISTIC_COEFFICIENTS));
        assert_eq!(first["winner"], second["winner"]);
        assert_eq!(first["digest"], second["digest"]);
        for key in ["proposal_nodes", "proposal_leaves", "proposal_improvements",
            "proposal_completed_prefixes", "plan_evaluations", "live_plan_applied"] {
            assert_eq!(first[key], second[key], "nondeterministic {key}");
            assert!(first[key].as_u64().unwrap() > 0, "no {key}; digest={}, errors={}", first["digest"], first["search_errors"]);
        }
        for result in [&first, &second] {
            assert_eq!(result["failed_playouts"], 0, "{}", result["search_errors"]);
            assert_eq!(result["failed_searches"], 0, "{}", result["search_errors"]);
            assert_eq!(result["limit"], false);
            let record = Recorded::from_json(&result["record"]).unwrap();
            let mut winner = None;
            let left = core.replay(&record, None, &mut |shown| {
                if let Replayed::Message(Message::Win { player, .. }) = shown {
                    winner = Some(*player);
                }
            }).unwrap();
            assert_eq!(left, 0);
            assert_eq!(winner.map(u64::from), result["winner"].as_u64());
        }
    }

    #[test]
    #[ignore = "build the release policy library first; native MCTS integration"]
    fn mcts_adapter_executes_validated_conditionals_and_replays_deterministically() {
        check_mcts_adapter(false);
    }

    #[test]
    #[ignore = "build the release policy library first; native specialist-model integration"]
    fn mcts_specialist_model_replays_without_requiring_new_opponent_ffi_hooks() {
        check_mcts_adapter(true);
    }

    fn check_mcts_adapter(specialist_opponent: bool) {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cards = root.join("vendor/BabelCdb/cards.cdb");
        let core = Core::open(None, &cards, &root.join("vendor/CardScripts")).unwrap();
        let library_path = std::env::var_os("YGO_MCTS_TEST_LIBRARY")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| root.join(format!(
                "target/release/{}ygo_policies{}",
                std::env::consts::DLL_PREFIX,
                std::env::consts::DLL_SUFFIX,
            )));
        let library = PolicyLibrary::open(&library_path).unwrap();
        let opponent_path = std::env::var_os("YGO_MCTS_TEST_OPPONENT")
            .map(std::path::PathBuf::from).unwrap_or(library_path);
        let opponent = PolicyLibrary::open(&opponent_path).unwrap();
        let deck = Deck::load(&root.join("decks").join(format!("{}.ydk", opponent.catalog["blackwing"]))).unwrap();
        let decks = [deck.clone(), deck];
        let run = PlayOptions {
            seed: 1810284039, limit: 8192, trace: false,
            life_points: [8000; 2], record: true,
        };
        let options = FastOptions {
            searcher: 1, worlds: 4, depth: 1, max_candidates: 6,
            max_playouts: 256, horizon: 1, millis: 0, strict: true,
            effects: true, planner: None, mcts_worlds: 4, mcts_validate_roots: false, specialist_opponent, evaluation_weights: None, evaluation_profile: EvaluationProfile::Off, staged: None, borrow: 0,
            mcts: Some(MctsOptions {
                iterations: 128, actions: 6, nodes: 1024, decisions: 256,
                replies: 1, terminal: false, adversarial_opponent: false, root_warmup: false,
            }),
        };
        let play = |evaluation_profile| play_fast(
            &core, &decks, [&opponent, &library], ["blackwing", "generic"],
            &cards, run, FastOptions { evaluation_profile, ..options },
        ).unwrap();
        let first = play(EvaluationProfile::Off);
        let second = play(EvaluationProfile::LpDensityV1);
        assert!(first.get("evaluation_profile").is_none());
        assert_eq!(second["evaluation_profile"]["alpha"], 0.0);
        assert_eq!(second["evaluation_profile"]["effective_weights"],
            super::super::evaluation::named_weights(&super::super::evaluation::HEURISTIC_COEFFICIENTS));
        assert_eq!(first["winner"], second["winner"]);
        assert_eq!(first["digest"], second["digest"]);
        for key in ["mcts_simulations", "mcts_completed", "mcts_nodes",
            "mcts_tree_hits", "mcts_frozen_actions", "mcts_accepted",
            "mcts_live_hits", "plan_evaluations"] {
            assert_eq!(first[key], second[key], "nondeterministic {key}");
            assert!(first[key].as_u64().unwrap() > 0,
                "no {key}; digest={}, errors={}", first["digest"], first["search_errors"]);
        }
        for result in [&first, &second] {
            assert_eq!(result["search_opponent_model"], if specialist_opponent { "specialist" } else { "generic" });
            for key in ["mcts_opponent_nodes", "mcts_opponent_actions", "mcts_opponent_tree_hits"] {
                assert_eq!(result[key], 0, "opponent UCT must remain disabled: {key}");
            }
            assert_eq!(result["failed_playouts"], 0, "{}", result["search_errors"]);
            assert_eq!(result["failed_searches"], 0, "{}", result["search_errors"]);
            assert_eq!(result["limit"], false);
            let copies = result["mcts_snapshot_copies"].as_array().unwrap();
            assert_eq!(copies.len() as u64, result["searched"].as_u64().unwrap());
            assert!(copies.iter().all(|copy| copy["snapshots"].as_u64().unwrap() > 0
                && copy["restores"].as_u64().unwrap() > 0
                && copy["max_snapshot_bytes"].as_u64().unwrap() > 0
                && copy["snapshotted_bytes"].as_u64().unwrap() > 0
                && copy["restored_bytes"].as_u64().unwrap() > 0));
            let searched = result["searched"].as_u64().unwrap();
            let simulated = result["mcts_simulations"].as_u64().unwrap();
            assert!(simulated <= searched * options.mcts.unwrap().iterations as u64);
            assert!(simulated + result["playouts"].as_u64().unwrap()
                <= searched * options.max_playouts as u64);
            let record = Recorded::from_json(&result["record"]).unwrap();
            let mut winner = None;
            let left = core.replay(&record, None, &mut |shown| {
                if let Replayed::Message(Message::Win { player, .. }) = shown {
                    winner = Some(*player);
                }
            }).unwrap();
            assert_eq!(left, 0);
            assert_eq!(winner.map(u64::from), result["winner"].as_u64());
        }
    }

    #[test]
    #[ignore = "build the release policy library first; native all-root MCTS integration"]
    fn mcts_all_roots_validate_within_budget_and_replay_the_chosen_path() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cards = root.join("vendor/BabelCdb/cards.cdb");
        let core = Core::open(None, &cards, &root.join("vendor/CardScripts")).unwrap();
        let library_path = std::env::var_os("YGO_MCTS_TEST_LIBRARY")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| root.join(format!(
                "target/release/{}ygo_policies{}",
                std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX,
            )));
        let library = PolicyLibrary::open(&library_path).unwrap();
        let opponent_path = std::env::var_os("YGO_MCTS_TEST_OPPONENT")
            .map(std::path::PathBuf::from).unwrap_or(library_path);
        let opponent = PolicyLibrary::open(&opponent_path).unwrap();
        let deck = Deck::load(&root.join("decks").join(format!("{}.ydk", opponent.catalog["blackwing"]))).unwrap();
        let options = FastOptions {
            searcher: 1, worlds: 4, depth: 1, max_candidates: 6,
            max_playouts: 256, horizon: 1, millis: 0, strict: true,
            effects: true, planner: None, mcts_worlds: 4, mcts_validate_roots: true, specialist_opponent: false, evaluation_weights: None, evaluation_profile: EvaluationProfile::Off, staged: None, borrow: 0,
            mcts: Some(MctsOptions {
                iterations: 128, actions: 6, nodes: 1024, decisions: 256,
                replies: 1, terminal: false, adversarial_opponent: false, root_warmup: false,
            }),
        };
        let result = play_fast(
            &core, &[deck.clone(), deck], [&opponent, &library], ["blackwing", "generic"],
            &cards, PlayOptions {
                seed: 1810284039, limit: 8192, trace: false,
                life_points: [8000; 2], record: true,
            }, options,
        ).unwrap();
        assert_eq!(result["failed_playouts"], 0, "{}", result["search_errors"]);
        assert_eq!(result["failed_searches"], 0, "{}", result["search_errors"]);
        assert_eq!(result["limit"], false);
        for key in ["mcts_completed", "mcts_accepted", "mcts_live_hits", "mcts_visited_root_actions"] {
            assert!(result[key].as_u64().unwrap() > 0,
                "no {key}; digest={}, errors={}", result["digest"], result["search_errors"]);
        }
        let searched = result["searched"].as_u64().unwrap();
        let simulated = result["mcts_simulations"].as_u64().unwrap();
        let validated = result["plan_evaluations"].as_u64().unwrap();
        let reserve = options.worlds * (options.max_candidates + 1);
        let allowed_iterations = options.mcts.unwrap().iterations
            .min(options.max_playouts.saturating_sub(reserve));
        assert!(simulated <= searched * allowed_iterations as u64);
        assert!(validated <= searched * reserve as u64);
        assert!(simulated + validated <= searched * options.max_playouts as u64);
        // Actual native work must include extra root proposals, rather than
        // merely setting the flag while still running the two-candidate path.
        assert!(validated > searched * (options.worlds * 2) as u64);
        assert_eq!(result["playouts"].as_u64(), Some(validated));
        let record = Recorded::from_json(&result["record"]).unwrap();
        let mut winner = None;
        let left = core.replay(&record, None, &mut |shown| {
            if let Replayed::Message(Message::Win { player, .. }) = shown {
                winner = Some(*player);
            }
        }).unwrap();
        assert_eq!(left, 0);
        assert_eq!(winner.map(u64::from), result["winner"].as_u64());
    }
}
