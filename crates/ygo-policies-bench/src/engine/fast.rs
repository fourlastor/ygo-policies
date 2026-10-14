//! Bounded root Monte Carlo search for the generic policy.
//!
//! Both simulated players use the generic policy. Alternatives share sampled
//! hidden worlds, and only complete rounds contribute to their comparison.
//! The time budget is checked between engine steps; one engine call may overrun
//! it. Hidden-card sampling currently assumes the opponent's deck composition
//! is known, as in an open-deck benchmark. It never preserves the actual hidden
//! arrangement or the engine's future random stream.
use super::search::{HiddenWorlds, Listed, Table};
use super::*;
use super::planner::{self, PlannedAction, PlannerOptions, PlannerResult};
use super::evaluation::Evaluator;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct FastOptions {
    pub searcher: usize,
    /// Maximum complete sampled worlds per decision.
    pub worlds: usize,
    /// One root choice, or two choices within each already sampled world.
    pub depth: usize,
    pub max_candidates: usize,
    /// Hard bound on attempted rollouts per decision; zero plays the plain pilot.
    pub max_playouts: usize,
    /// Minimum future decisions before a quiescent leaf, or zero for full duels.
    /// A truncated rollout may extend at most 64 decisions to resolve effects.
    pub horizon: usize,
    /// Soft wall-clock budget per decision, or zero for deterministic work caps.
    pub millis: u64,
    /// Skip search when the opponent has an unsampleable face-down monster.
    pub strict: bool,
    /// Attach public registered-effect metadata to the generic pilot.
    pub effects: bool,
    /// Transaction-complete beam proposals, evaluated unchanged across worlds.
    pub planner: Option<PlannerOptions>,
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
    library: &'a PolicyLibrary,
    cards: &'a Path,
    viewer: u8,
    seed: u64,
    limit: usize,
    options: FastOptions,
    profiles: Option<serde_json::Value>,
    evaluator: Option<Evaluator>,
}

/// Build a generic opponent from public events only. Replaying the real
/// opponent's private prompts would let its actual hand influence memory even
/// after sampling a different hand, so spectator redaction deliberately drops
/// every historical choice and hides every private draw/update.
fn public_opponent<'a>(
    context: &Context<'a>,
    logs: &[Vec<u8>; 2],
    decisions: usize,
) -> Result<LibrarySeat<'a>> {
    let player = 1 - context.viewer as usize;
    let seat = context.library.seat(
        "generic",
        context.cards,
        player as i32,
        context.seed ^ (decisions as u64).wrapping_mul(0xD1B54A32D192ED03),
    )?;
    if let Some(profiles) = &context.profiles { seat.set_effect_profiles(profiles)?; }
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
    let evaluate = |observation: &serde_json::Value, viewer| evaluator.value(observation, viewer);
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
    if fast.planner.is_some_and(|p| p.width == 0 || p.actions == 0 || p.nodes == 0 || p.decisions == 0) {
        return Err("Beam width/actions/nodes/decisions must be positive".into());
    }
    if fast.planner.is_some() && fast.depth != 1 {
        return Err("Nested rollout depth does not apply to beam planning".into());
    }
    if fast.max_playouts > 0 && ![1, 2].contains(&fast.depth) {
        return Err("Generic search depth must be 1 or 2".into());
    }
    if fast.max_playouts > 0 && !fast.strict {
        return Err(
            "Generic search requires strict=true: face-down monsters cannot be sampled safely"
                .into(),
        );
    }
    if fast.searcher > 1 {
        return Err("Generic searcher must be seat 0 or 1".into());
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
        if player == fast.searcher && fast.max_playouts > 0 {
            table.seats[player].enable_response_overrides()?;
        }
        table.feed(player, &duel.start_message(player as u8))?;
    }
    let evaluator = if fast.planner.is_some() {
        let mut evaluator = Evaluator::open(cards)?;
        if let Some(profiles) = &profiles { evaluator.set_profiles(profiles)?; }
        Some(evaluator)
    } else { None };
    let context = Context {
        library: policies[fast.searcher],
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
    let mut decision_ms: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    let mut search_decision_ms: Vec<f64> = Vec::new();
    loop {
        let step = duel.step()?;
        let mut response = None;
        for sent in &step.messages {
            if sent.refresh {
                table.update(&sent.bytes)?;
                continue;
            }
            match sent.bytes[0] {
                msg::RETRY => return Err("Engine rejected response (MSG_RETRY)".into()),
                msg::NEW_TURN => turns += 1,
                msg::CHAINING => chain = true,
                msg::CHAIN_END => chain = false,
                _ => {}
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
            if let Some(recorded) = &recorded {
                row["record"] = record_json(recorded, names);
            }
            return Ok(row);
        }
        let Some((player, mut answer, message, mut elapsed)) = response else {
            if step.state == State::Awaiting {
                return Err("Engine awaiting without policy answer".into());
            }
            continue;
        };
        let mut searched_this_decision = false;
        if player == fast.searcher && fast.max_playouts > 0 {
            let started = Instant::now();
            // Consume saved continuation BEFORE active-chain guards. Only the
            // original visible prompt and exact offered action authorize replay.
            let original = table.seats[player].last_answer()?;
            let mut replayed = false;
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
                let expanded_limit = if fast.planner.is_some() { 12 } else { fast.max_candidates };
                let described = table.seats[player].alternative_responses(expanded_limit)?;
                if let Some(listed) = candidates(&described, &answer,
                    fast.max_candidates.min(fast.max_playouts)) {
                    if chain { in_chain += 1; }
                    else if fast.strict && duel.monsters().iter().any(|m| {
                        m.controller != context.viewer && m.position & position::FACEDOWN != 0
                    }) { facing_set += 1; }
                    else if unsafe_selection(&described, context.viewer) { stats.unsafe_selection += 1; }
                    else {
                        stats.searched += 1;
                        searched_this_decision = true;
                        let (chosen, worlds, gain) = if fast.planner.is_some() {
                            let (chosen, worlds, gain, continuation) = choose_plan(
                                &mut duel, &context, &table, &listed, decisions, &mut stats)?;
                            plan = continuation.into();
                            (chosen, worlds, gain)
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
            elapsed += started.elapsed().as_secs_f64() * 1000.0;
        }
        if searched_this_decision {
            search_decision_ms.push(elapsed);
        }
        decision_ms[player].push(elapsed);
        for byte in message.iter().chain(&answer) {
            digest = (digest ^ *byte as u64).wrapping_mul(0x100000001b3);
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
            planner: Some(PlannerOptions {
                width: 6, actions: 4, nodes: 64, decisions: 256, replies: 1,
            }),
        };
        let play = || play_fast(
            &core, &decks, [&library, &opponent], ["generic", "blackwing"],
            &cards, run, options,
        ).unwrap();
        let first = play();
        let second = play();
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
}
