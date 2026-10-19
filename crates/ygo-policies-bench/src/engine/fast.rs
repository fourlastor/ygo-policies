//! Bounded rollout, transaction-beam and information-set MCTS search.
//!
//! By default both simulated players use Generic; an explicit benchmark switch
//! can instead simulate the supplied specialist. Alternatives share sampled
//! hidden worlds, and only complete rounds contribute to their comparison.
//! The time budget is checked between engine steps; one engine call may overrun
//! it. Hidden-card sampling currently assumes the opponent's deck composition
//! is known, as in an open-deck benchmark. It never preserves the actual hidden
//! arrangement or the engine's future random stream.
use super::search::Table;
use super::*;
use super::planner::{self, PlannedAction, PlannerOptions, PlannerResult};
use super::evaluation::{Evaluator, EvaluationProfile};
use super::ismcts;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use ygo_policies_search::memory::PublicIdentityMemory;
use ygo_policies_search::worlds::HiddenWorlds;
use ygo_policies_search::{
    candidates, choose_staged, expired, pending_known_slots, prepare, public_opponent, rollout, unknown_set_monsters,
    unsafe_selection, LibraryPilots, Listed, Trial,
};
pub use ygo_policies_search::StagedOptions;

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

/// What the bench's methods count, around what the search itself counts.
#[derive(Default)]
struct Stats {
    search: ygo_policies_search::Stats,
    searched: u64,
    truncated_playouts: u64,
    budget_exhausted: u64,
    incomplete_worlds: u64,
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
    snapshot_copies: Vec<ygo_policies_duel::SnapshotMetrics>,
}

impl std::ops::Deref for Stats {
    type Target = ygo_policies_search::Stats;
    fn deref(&self) -> &Self::Target {
        &self.search
    }
}

impl std::ops::DerefMut for Stats {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.search
    }
}

/// The search's context, with what only the bench's methods read.
struct Context<'a> {
    search: ygo_policies_search::Context<LibraryPilots<'a>>,
    options: FastOptions,
    evaluator: Option<Evaluator>,
}

impl<'a> std::ops::Deref for Context<'a> {
    type Target = ygo_policies_search::Context<LibraryPilots<'a>>;
    fn deref(&self) -> &Self::Target {
        &self.search
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
        let opponent = public_opponent(&context.search, &table.logs[1 - context.viewer as usize], decisions)?;
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
            let seats = prepare(duel, &context.search, &table.seats[context.viewer as usize], &opponent)?;
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
                    &context.search,
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
        let opponent = public_opponent(&context.search, &table.logs[1 - context.viewer as usize], decisions)?;
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
            let seats = prepare(duel, &context.search, &table.seats[context.viewer as usize], &opponent)?;
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
            let seats = prepare(duel, &context.search, &table.seats[context.viewer as usize], &opponent)?;
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
        let opponent = public_opponent(&context.search, &table.logs[1 - context.viewer as usize], decisions)?;
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
            let seats = prepare(duel, &context.search, &table.seats[context.viewer as usize], &opponent)?;
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
            let seats = prepare(duel, &context.search, &table.seats[context.viewer as usize], &opponent)?;
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
        search: ygo_policies_search::Context {
            pilots: LibraryPilots {
                library: policies[if fast.specialist_opponent { 1 - fast.searcher } else { fast.searcher }],
                cards,
            },
            model_name: if fast.specialist_opponent { names[1 - fast.searcher] } else { "generic" }.to_owned(),
            viewer: fast.searcher as u8,
            seed,
            limit,
            horizon: fast.horizon,
            profiles,
            specialist_opponent: fast.specialist_opponent,
            adversarial_opponent: fast.mcts.is_some_and(|options| options.adversarial_opponent),
        },
        options: fast,
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
                            choose_staged(&mut duel, &context.search, &table.seats[context.viewer as usize], &table.logs[1 - context.viewer as usize], &listed, decisions, &mut stats, staged, unknown)?
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

    /// The duel `play_fast` plays, as it plays it, each message to each seat
    /// through `feed`: the answers given.
    fn host(
        duel: &std::cell::RefCell<Duel>,
        limit: usize,
        mut feed: impl FnMut(usize, &[u8]) -> Option<Vec<u8>>,
    ) -> Vec<(u8, Vec<u8>)> {
        for player in 0..2 {
            let start = duel.borrow().start_message(player as u8);
            assert_eq!(feed(player, &start), None);
        }
        let mut responses = Vec::new();
        loop {
            let step = duel.borrow_mut().step().unwrap();
            let mut response = None;
            for sent in &step.messages {
                for player in 0..2 {
                    if let Some(answer) = feed(player, &sent.bytes) {
                        assert!(response.replace((player as u8, answer)).is_none(), "two answers in one batch");
                    }
                }
            }
            if matches!(step.state, State::Over(_)) || responses.len() >= limit {
                return responses;
            }
            if let Some((player, answer)) = response {
                duel.borrow_mut().respond(&answer);
                responses.push((player, answer));
            } else {
                assert_ne!(step.state, State::Awaiting, "the engine waits with no answer");
            }
        }
    }

    /// The duel `play_fast` plays, with a search seat in the searching seat:
    /// the seat is fed what that seat was, and steps nothing itself.  It
    /// describes its cards with the stored profiles, `play_fast` with those
    /// its engine reads from the scripts.
    fn play_with_search_seat<P: ygo_policies_search::Pilots>(
        core: &Core,
        decks: &[Deck; 2],
        other: LibrarySeat<'_>,
        searcher: usize,
        pilots: P,
        run: PlayOptions,
        options: ygo_policies_search::SearchOptions,
    ) -> (Vec<(u8, Vec<u8>)>, serde_json::Value) {
        let dealt = DuelOptions { life_points: run.life_points, ..DuelOptions::seeded(run.seed) };
        let duel = std::rc::Rc::new(std::cell::RefCell::new(core.deal(&dealt, decks).unwrap()));
        let mut seat = ygo_policies_search::SearchSeat::new(duel.clone(), pilots, searcher as u8, run.seed, options).unwrap();
        let responses = host(&duel, run.limit, |player, message| {
            if player == searcher { seat.feed(message).unwrap() } else { other.feed(message).unwrap() }
        });
        (responses, seat.stats().to_json())
    }

    /// The two duels the search seat is checked on, and how `play_fast` plays
    /// them: the searching seat, the specialist in the other one, its decks.
    fn search_seat_duels() -> [(u64, usize, &'static str, [&'static str; 2]); 2] {
        [(2700011, 1, "blackwing", ["Blackwing Assassin", "Emperor, Arise!"]), (2700012, 0, "deckout", ["Last Page", "Last Page"])]
    }

    fn search_seat_fast(searcher: usize, staged: StagedOptions) -> FastOptions {
        FastOptions {
            searcher, worlds: 4, depth: 1, max_candidates: 8, max_playouts: 1_000_000, horizon: 0, millis: 0,
            strict: true, effects: true, planner: None, mcts: None, mcts_worlds: 4, mcts_validate_roots: false,
            specialist_opponent: false, evaluation_weights: None, evaluation_profile: EvaluationProfile::Off,
            staged: Some(staged), borrow: 0,
        }
    }

    #[test]
    #[ignore = "build the release policy library first"]
    fn a_search_seat_answers_as_play_fast_does() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cards = root.join("vendor/BabelCdb/cards.cdb");
        let core = Core::open(None, &cards, &root.join("vendor/CardScripts")).unwrap();
        let library = PolicyLibrary::open(&root.join(format!(
            "target/release/{}ygo_policies{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )))
        .unwrap();
        let local = ygo_policies_search::LocalPilots {
            cards: std::sync::Arc::new(ygo_policies_ocgcore::SqliteCards::open(&cards).unwrap()),
        };
        let staged = StagedOptions { stages: [2, 4, 8], ..StagedOptions::MEASURED };
        for (seed, searcher, specialist, lists) in search_seat_duels() {
            let decks = lists.map(|name| Deck::load(&root.join(format!("decks/{name}.ydk"))).unwrap());
            let mut names = [specialist; 2];
            names[searcher] = "generic";
            let run = PlayOptions { seed, limit: 2048, trace: false, life_points: [8000, 8000], record: true };
            let played = play_fast(&core, &decks, [&library, &library], names, &cards, run, search_seat_fast(searcher, staged))
                .unwrap();
            let expected = Recorded::from_json(&played["record"]).unwrap().responses;
            let options = ygo_policies_search::SearchOptions { staged, candidates: 8, limit: run.limit };
            let other = || library.seat(specialist, &cards, 1 - searcher as i32, seed + 1 - searcher as u64).unwrap();
            let pilots = ygo_policies_search::LibraryPilots { library: &library, cards: &cards };
            let (libraries, through_library) = play_with_search_seat(&core, &decks, other(), searcher, pilots, run, options);
            let (locals, in_process) = play_with_search_seat(&core, &decks, other(), searcher, local.clone(), run, options);
            eprintln!("{seed}: {} answers, {through_library}", expected.len());
            for (stats, responses) in [(&through_library, &libraries), (&in_process, &locals)] {
                assert_eq!(responses, &expected, "{seed}: the answers differ from play_fast's");
                assert_eq!(stats["searched"], played["searched"], "{seed}");
                assert_eq!(stats["deviations"], played["deviations"], "{seed}");
                assert_eq!(stats["failed_searches"], 0, "{seed}: {}", stats["search_errors"]);
                assert_eq!(stats["unprofiled_cards"], serde_json::json!([]), "{seed}");
            }
            assert!(through_library["searched"].as_u64().unwrap() > 0, "{seed}: nothing was searched");
        }
    }

    /// As a game seats it: the duel is the host's, on an engine library of
    /// its own, and the search seat is a handle of the policy library's C
    /// ABI, given that duel and that library.
    #[test]
    #[ignore = "build the release policy library first, and vendor/ocgcore as a library (YGO_TEST_ENGINE)"]
    fn a_search_seat_through_the_c_abi_answers_as_play_fast_does() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let engine = std::path::PathBuf::from(
            std::env::var_os("YGO_TEST_ENGINE").expect("YGO_TEST_ENGINE: vendor/ocgcore built as a library"),
        );
        let (cards, scripts) = (root.join("vendor/BabelCdb/cards.cdb"), root.join("vendor/CardScripts"));
        let core = Core::open(None, &cards, &scripts).unwrap();
        let hosted = Core::open(Some(&engine), &cards, &scripts).unwrap();
        let library = PolicyLibrary::open(&root.join(format!(
            "target/release/{}ygo_policies{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )))
        .unwrap();
        let staged = StagedOptions { stages: [2, 4, 8], ..StagedOptions::MEASURED };
        for (seed, searcher, specialist, lists) in search_seat_duels() {
            let decks = lists.map(|name| Deck::load(&root.join(format!("decks/{name}.ydk"))).unwrap());
            let mut names = [specialist; 2];
            names[searcher] = "generic";
            let run = PlayOptions { seed, limit: 2048, trace: false, life_points: [8000, 8000], record: true };
            let played = play_fast(&core, &decks, [&library, &library], names, &cards, run, search_seat_fast(searcher, staged))
                .unwrap();
            let expected = Recorded::from_json(&played["record"]).unwrap().responses;
            let other = library.seat(specialist, &cards, 1 - searcher as i32, seed + 1 - searcher as u64).unwrap();
            let dealt = DuelOptions { life_points: run.life_points, ..DuelOptions::seeded(seed) };
            let duel = hosted.deal(&dealt, &decks).unwrap();
            let options = serde_json::json!({"stages": staged.stages, "limit": run.limit});
            // No effect profiles given, as a game gives none: the seat has its own.
            let search = unsafe { library.search_seat(&cards, searcher as i32, seed, &engine, duel.handle(), Some(&options)) }
                .unwrap();
            let responses = host(&std::cell::RefCell::new(duel), run.limit, |player, message| {
                if player == searcher { search.feed(message).unwrap() } else { other.feed(message).unwrap() }
            });
            let stats = search.search_stats().unwrap();
            eprintln!("{seed}: {} answers, {stats}", expected.len());
            assert_eq!(responses, expected, "{seed}: the answers differ from play_fast's");
            assert_eq!(stats["searched"], played["searched"], "{seed}");
            assert_eq!(stats["deviations"], played["deviations"], "{seed}");
            assert_eq!(stats["failed_searches"], 0, "{seed}: {}", stats["search_errors"]);
            assert_eq!(stats["unprofiled_cards"], serde_json::json!([]), "{seed}");
        }
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
