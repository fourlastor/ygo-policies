//! The staged choice, the playouts it is decided on, and the guards that
//! leave a decision to the pilot.
use std::time::Instant;

use ygo_policies_duel::{Duel, State};
use ygo_policies_ocgcore::wire::msg;

use crate::memory::PublicIdentityMemory;
use crate::seat::{PilotSeat, Pilots};
use crate::worlds::HiddenWorlds;
use crate::Result;

/// A decision as the policy's answer describes it: the alternatives worth
/// trying (the pilot's own first), with a word on each for the report.
pub struct Listed {
    pub kind: String,
    pub responses: Vec<Vec<u8>>,
    pub labels: Vec<serde_json::Value>,
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


/// What a search counts while it decides.
#[derive(Default)]
pub struct Stats {
    /// Worlds played out, from a root or (depth two) from a second decision.
    pub playouts: u64,
    /// Depth two: second decisions searched, and those that played a world.
    pub inner_searches: u64,
    pub inner_playouts: u64,
    /// Worlds that failed, and worlds that reached the answer limit.
    pub failed_playouts: u64,
    pub unresolved_playouts: u64,
    /// Searches that failed: the pilot's answer is given.
    pub failed_searches: u64,
    /// Staged roots left to the pilot because their worlds could not be dealt
    /// (the engine refused to swap a Set monster that was not untouched).
    pub undealt_roots: u64,
    /// The first distinct errors, at most eight.
    pub errors: Vec<String>,
}

impl Stats {
    pub fn error(&mut self, error: String) {
        if self.errors.len() < 8 && !self.errors.contains(&error) {
            self.errors.push(error);
        }
    }
}

pub fn expired(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

/// Public information only. Hidden monster attributes/codes are never used.
/// Terminal outcomes are evaluated separately and dominate any leaf estimate.
pub fn leaf_value(observation: &serde_json::Value, viewer: u8) -> f64 {
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
pub fn known_slots(observation: &serde_json::Value) -> Vec<(u8, u32, u32)> {
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

pub fn member_slot(member: &serde_json::Value) -> Option<(u8, u32, u32)> {
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

pub fn choice_members(answer: &serde_json::Value) -> impl Iterator<Item = &serde_json::Value> {
    answer["decision"]["choices"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|choice| {
            std::iter::once(&choice["card"])
                .chain(choice["members"].as_array().into_iter().flatten())
        })
}

pub fn pending_known_slots(answer: &serde_json::Value, viewer: u8) -> Vec<(u8, u32, u32)> {
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
pub fn unsafe_selection(answer: &serde_json::Value, viewer: u8) -> bool {
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

pub enum Trial {
    Scored(f64, bool),
    Budget,
    Unresolved,
}

/// What a search plays its worlds with.
pub struct Context<P> {
    /// Where the other player's model is seated from, and which policy it is.
    pub pilots: P,
    pub model_name: String,
    /// The searching seat.
    pub viewer: u8,
    /// What the worlds are dealt and reseeded from, with the decision count.
    pub seed: u64,
    /// Answers after which a world is a draw.
    pub limit: usize,
    /// Rollouts: future decisions before a quiet board is scored; zero
    /// plays every world to the end.
    pub horizon: usize,
    /// The searching seat's deck effects, also given to a generic model.
    pub profiles: Option<serde_json::Value>,
    /// The model is the other player's own policy (a benchmark diagnostic).
    pub specialist_opponent: bool,
    /// The model takes response overrides (adversarial MCTS proposals).
    pub adversarial_opponent: bool,
}

/// Build the selected opponent model from public events only. Replaying the real
/// opponent's private prompts would let its actual hand influence memory even
/// after sampling a different hand, so spectator redaction deliberately drops
/// every historical choice and hides every private draw/update.
pub fn public_opponent<P: Pilots>(
    context: &Context<P>,
    history: &[u8],
    decisions: usize,
) -> Result<P::Seat> {
    let player = 1 - context.viewer as usize;
    let seat = context.pilots.seat(
        &context.model_name,
        player as u8,
        context.seed ^ (decisions as u64).wrapping_mul(0xD1B54A32D192ED03),
    )?;
    if !context.specialist_opponent {
        if let Some(profiles) = &context.profiles { seat.set_effect_profiles(profiles)?; }
    }
    if context.adversarial_opponent {
        seat.enable_response_overrides()?;
    }
    seat.feed_buffer(&public_history(history)?)?;
    Ok(seat)
}

pub fn public_history(history: &[u8]) -> Result<Vec<u8>> {
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
pub fn prepare<P: Pilots>(
    duel: &Duel,
    context: &Context<P>,
    pilot: &P::Seat,
    opponent: &P::Seat,
) -> Result<Vec<P::Seat>> {
    let mut seats = Vec::with_capacity(2);
    for player in 0..2 {
        seats.push(if player == context.viewer as usize {
            pilot.fork()?
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
pub fn candidates(answer: &serde_json::Value, given: &[u8], maximum: usize) -> Option<Listed> {
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
pub fn rollout<S: PilotSeat, P>(
    duel: &mut Duel,
    seats: &[S],
    context: &Context<P>,
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
                                .map(PilotSeat::fork)
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
        if context.horizon > 0
            && future >= context.horizon
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
        if context.horizon > 0 && future >= context.horizon.saturating_add(64) {
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


/// Mean, paired z and count of a candidate's payoff differences from the
/// pilot's answer over the worlds both were played out in.
pub fn paired(candidate: &[Option<f64>], pilot: &[Option<f64>]) -> (f64, f64, usize) {
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
pub fn unknown_set_monsters(memory: &PublicIdentityMemory, observation: &serde_json::Value, viewer: u8) -> Vec<u32> {
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
/// `pilot` is the searching seat's own policy, waiting on the decision;
/// `history` is everything the other player's seat was fed.
#[allow(clippy::too_many_arguments)]
pub fn choose_staged<P: Pilots>(
    duel: &mut Duel,
    context: &Context<P>,
    pilot: &P::Seat,
    history: &[u8],
    listed: &Listed,
    decisions: usize,
    stats: &mut Stats,
    staged: StagedOptions,
    monsters: Vec<u32>,
) -> Result<(usize, usize, f64)> {
    const EARLY_ACCEPT: f64 = 2.33;
    const EARLY_REJECT: f64 = 0.67;
    let root = duel.snapshot()?;
    let known = pending_known_slots(&pilot.last_answer()?, context.viewer);
    let mut hidden = HiddenWorlds::with_known(known).with_monsters(monsters);
    let n = listed.responses.len();
    let mut payoffs: Vec<Vec<Option<f64>>> = vec![Vec::new(); n];
    let mut alive: Vec<usize> = (1..n).collect();
    let (mut chosen, mut gain, mut worlds_used) = (0usize, 0.0f64, 0usize);
    let mut undealt = false;
    let result = (|| -> Result<()> {
        let opponent = public_opponent(context, history, decisions)?;
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
                    let seats = prepare(duel, context, pilot, &opponent)?;
                    let sampled = duel.snapshot()?;
                    let indices: Vec<usize> = if pilot_pass { vec![0] } else { alive.clone() };
                    for index in indices {
                        duel.restore(&sampled)?;
                        let copies = seats.iter().map(PilotSeat::fork).collect::<Result<Vec<_>>>()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LibraryPilots;
    use ygo_policies_duel::PolicyLibrary;
    use ygo_policies_ocgcore::message;
    use ygo_policies_ocgcore::wire::{position, query};

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
            pilots: LibraryPilots { library: &opponent, cards: &cards }, model_name: "demise".into(),
            viewer: 0, seed: 2472000, limit: 8192, horizon: 1,
            // Nonempty metadata must not be sent to the old specialist ABI.
            profiles: Some(serde_json::json!({"95492061":[]})),
            specialist_opponent: true, adversarial_opponent: false,
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
        let first = public_opponent(&context, &history(72426662)[1], 17).unwrap();
        let second = public_opponent(&context, &history(46427957)[1], 17).unwrap();
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
}
