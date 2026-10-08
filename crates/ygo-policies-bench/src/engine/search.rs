//! A one-step search on top of a pilot.
//!
//! One seat is played by its policy and a search: at each of its decisions
//! the duel is snapshotted, every alternative the policy listed is tried in
//! worlds where the cards this seat cannot see are dealt again, and both
//! policies play each world out.  The seat leaves its pilot's answer only for
//! an alternative that wins clearly more often over the same worlds.  The
//! pilot's answer is tried first: where it wins every first-stage world no
//! alternative can be ahead of it, and none is tried.
//!
//! The other player's face-down monsters cannot be dealt again, so a world
//! would show the search what they are.  A strict search leaves every
//! decision taken while one is on the field to the pilot.
//!
//! This needs an engine with arena snapshots and a hidden-card swap
//! ([`Duel::snapshot`], [`Duel::swap`]).  The built-in engine has both; an
//! engine given as `--core` must have them too.
use super::*;

#[derive(Clone, Copy)]
pub struct SearchOptions {
    /// The seat that searches.
    pub searcher: usize,
    /// Worlds every alternative has been tried in after each stage.  Only
    /// alternatives ahead of the pilot's answer go on to the next stage.
    pub stages: [usize; 3],
    /// How far ahead (paired z) an alternative must be after the last stage.
    pub z: f64,
    /// Leave decisions to the pilot while the other player has a face-down
    /// monster, whose identity the worlds would give away.
    pub strict: bool,
    /// Write every decision examined, with the situation it was taken in and
    /// how each alternative did in the first-stage worlds.
    pub log: bool,
    /// Search nothing: at each decision, play the pilot's answer out in the
    /// world as it is, and check that it ends as the duel itself does.
    pub validate: bool,
    /// Not a player: play every alternative out in the world as it is (the
    /// real hidden cards, the draws to come) and leave the pilot's answer
    /// whenever it loses and another one wins.  What it still loses, no
    /// single change of answer could have won.
    pub foresight: bool,
}

/// The other player of a searched duel when no policy of the library plays
/// it: the seat that answers for it in the duel, and who plays it in the
/// try-outs.  That is the policy `stand_in`, and the seat is then never
/// asked in a try-out: what the search plans against is the stand-in.  Or,
/// with `itself`, a copy of the seat as it stands at the decision, a new
/// one for each try-out; `stand_in` then only sits at the table.
pub struct Outside<'a> {
    pub seat: Box<dyn LiveSeat + 'a>,
    pub stand_in: &'a str,
    pub itself: bool,
}

/// The seat whose copies play a seat of the try-outs, and which seat.
type Itself<'a> = Option<(&'a dyn LiveSeat, usize)>;

/// After the middle stage: far enough ahead to stop there, or too close to go on.
const EARLY_ACCEPT: f64 = 2.33;
const EARLY_REJECT: f64 = 0.67;
/// Alternatives tried per decision, the pilot's answer included.
const MAX_CANDIDATES: usize = 12;
/// Alternatives that go on after the first stage.
const KEPT: usize = 2;

const QUERY_IS_PUBLIC: u32 = 0x100000;
const TYPE_SPELL_OR_TRAP: u32 = 0x2 | 0x4;

/// A card the searching seat cannot see, and where it is.
#[derive(Clone, Copy)]
struct Slot {
    code: u32,
    controller: u8,
    location: u32,
    sequence: u32,
}

struct SplitMix(u64);
impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn shuffle(&mut self, values: &mut [usize]) {
        for i in (1..values.len()).rev() {
            values.swap(i, self.next() as usize % (i + 1));
        }
    }
}

/// One alternative of a decision, and how it did against the pilot's answer.
struct Candidate {
    index: usize,
    /// The seat's payoff in each world: 1 won, -1 lost, 0 drawn; `None` when
    /// the world could not be played out.
    payoffs: Vec<Option<f64>>,
}

/// Mean and z of the paired difference between a candidate and the pilot.
fn paired(candidate: &Candidate, pilot: &Candidate) -> (f64, f64, usize) {
    let differences: Vec<f64> = candidate
        .payoffs
        .iter()
        .zip(&pilot.payoffs)
        .filter_map(|(a, b)| Some((*a)? - (*b)?))
        .collect();
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

/// The fields of one `OCG_DuelQuery` answer the search reads.
fn card_query(bytes: &[u8]) -> (u32, u32, bool) {
    let (mut code, mut position, mut public) = (0, 0, false);
    let mut offset = 0;
    while offset + 6 <= bytes.len() {
        let size = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        if size < 4 || offset + 2 + size > bytes.len() {
            break;
        }
        let flag = u32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap());
        let value = &bytes[offset + 6..offset + 2 + size];
        if value.len() >= 4 {
            let number = u32::from_le_bytes(value[..4].try_into().unwrap());
            if flag == query::CODE {
                code = number;
            } else if flag == query::POSITION {
                position = number;
            }
        }
        if flag == QUERY_IS_PUBLIC && !value.is_empty() {
            public = value[0] != 0;
        }
        offset += 2 + size;
    }
    (code, position, public)
}

/// The situation a decision is taken in, as the seat sees it: what a rule
/// could look at before deciding whether the decision is worth a search.
fn situation(answer: &serde_json::Value, me: u8) -> serde_json::Value {
    let observation = &answer["observation"];
    let count = |mine: bool, location: &str, face_down: bool| -> usize {
        observation["cards"].as_array().map_or(0, |cards| {
            cards
                .iter()
                .filter(|card| {
                    (card["at"]["controller"] == me) == mine
                        && card["at"]["location"] == location
                        && (!face_down || card["position"]["face_up"] == false)
                })
                .count()
        })
    };
    let life = |player: usize| observation["life_points"][player].as_u64().unwrap_or(0);
    serde_json::json!({
        "phase": observation["phase"],
        "own_turn": observation["turn_player"] == me,
        "life": [life(me as usize), life(1 - me as usize)],
        "monsters": [count(true, "MonsterZone", false), count(false, "MonsterZone", false)],
        "backrow": [count(true, "SpellTrapZone", false), count(false, "SpellTrapZone", false)],
        "set_backrow": [count(true, "SpellTrapZone", true), count(false, "SpellTrapZone", true)],
        "hand": [count(true, "Hand", false), count(false, "Hand", false)],
        "attack": !observation["battle_attacker"].is_null(),
    })
}

/// A decision as the policy's answer describes it: the alternatives worth
/// trying (the pilot's own first), with a word on each for the report.
struct Listed {
    kind: String,
    responses: Vec<Vec<u8>>,
    labels: Vec<serde_json::Value>,
}

fn listed(answer: &serde_json::Value, given: &[u8]) -> Option<Listed> {
    let decision = &answer["decision"];
    let kind = match &decision["kind"] {
        serde_json::Value::String(kind) if ["Idle", "Battle", "YesNo", "Position"].contains(&kind.as_str()) => kind.clone(),
        serde_json::Value::Object(kind) if kind.get("Chain").is_some_and(|chain| chain["forced"] == false) => "Chain".to_owned(),
        _ => return None,
    };
    let choice = answer["choice"].as_u64()? as usize;
    let responses = answer["responses"].as_array()?;
    let choices = decision["choices"].as_array()?;
    if responses.len() != choices.len() {
        return None;
    }
    let bytes = |value: &serde_json::Value| -> Option<Vec<u8>> {
        value.as_array()?.iter().map(|b| b.as_u64().map(|b| b as u8)).collect()
    };
    let label = |choice: &serde_json::Value| serde_json::json!({"kind": choice["kind"], "code": choice["card"]["code"]});
    // Two copies of a card in the same pile are one alternative: what is done
    // with which card, from where.  Cards on the field are told apart.
    let same = |choice: &serde_json::Value| -> Option<String> {
        let at = &choice["card"]["at"];
        let pile = ["Hand", "Deck", "Graveyard", "Banished", "Extra"].contains(&at["location"].as_str()?);
        pile.then(|| format!("{} {} {} {} {}", choice["kind"], choice["card"]["code"], at["controller"], at["location"], choice["description"]))
    };
    // The pilot's answer as the engine got it, or this is not a plain choice.
    if bytes(responses.get(choice)?)? != given {
        return None;
    }
    let mut out = Listed { kind, responses: vec![given.to_vec()], labels: vec![label(&choices[choice])] };
    let mut tried: Vec<String> = same(&choices[choice]).into_iter().collect();
    for (index, (response, described)) in responses.iter().zip(choices).enumerate() {
        // Shuffling the hand and backing out of a choice lead straight back here.
        if index == choice || described["kind"] == "ShuffleHand" || described["kind"] == "Cancel" {
            continue;
        }
        if let Some(key) = same(described) {
            if tried.contains(&key) {
                continue;
            }
            tried.push(key);
        }
        if out.responses.len() == MAX_CANDIDATES {
            break;
        }
        out.responses.push(bytes(response)?);
        out.labels.push(label(described));
    }
    (out.responses.len() > 1).then_some(out)
}

/// Both seats of a duel, with everything each was fed: a seat can be built
/// again in the same state by feeding another one the same messages.
struct Table<'a> {
    seats: Vec<LibrarySeat<'a>>,
    logs: [Vec<u8>; 2],
}

impl Table<'_> {
    fn update(&mut self, message: &[u8]) -> Result<()> {
        for log in &mut self.logs {
            log.extend_from_slice(&(message.len() as u32).to_le_bytes());
            log.extend_from_slice(message);
        }
        self.seats[0].feed_update_pair(&self.seats[1], message)
    }

    fn feed(&mut self, player: usize, message: &[u8]) -> Result<Option<Vec<u8>>> {
        self.logs[player].extend_from_slice(&(message.len() as u32).to_le_bytes());
        self.logs[player].extend_from_slice(message);
        self.seats[player].feed(message)
    }
}

/// What a duel needs to be played from a snapshot again.
struct Setup<'a> {
    policies: [&'a PolicyLibrary; 2],
    names: [&'a str; 2],
    cards: &'a Path,
    seed: u64,
    limit: usize,
}

/// The cards `viewer` cannot see: its own Deck, and the other player's
/// Deck, hand and Set Spells and Traps.  Revealed cards stay known.
fn hidden(duel: &Duel, viewer: u8) -> Result<Vec<Slot>> {
    let mut slots = Vec::new();
    let mut add = |controller: u8, location: u32, sequence: u32, face_down_only: bool| -> Result<()> {
        let bytes = duel.card(query::CODE | query::POSITION | QUERY_IS_PUBLIC, controller, location, sequence)?;
        if bytes.is_empty() {
            return Ok(());
        }
        let (code, position, public) = card_query(&bytes);
        if code != 0 && !public && (!face_down_only || position & position::FACEDOWN != 0) {
            slots.push(Slot { code, controller, location, sequence });
        }
        Ok(())
    };
    let opponent = 1 - viewer;
    for (controller, location) in [(viewer, 1), (opponent, 1), (opponent, 2)] {
        for sequence in 0..duel.count(controller, location)? {
            add(controller, location, sequence, false)?;
        }
    }
    for sequence in 0..7 {
        add(opponent, 8, sequence, true)?;
    }
    Ok(slots)
}

/// Exact exchanges for a root inventory and world seed. Planning consumes
/// the same random numbers and keeps the original exchange order.
fn swap_plan(duel: &Duel, slots: &[Slot], viewer: u8, seed: u64) -> Result<Vec<(Slot, Slot)>> {
    let mut rng = SplitMix(seed);
    let mut swaps = Vec::new();
    let mut permute = |group: &[usize], wanted: &[usize]| -> Result<()> {
        let mut current = group.to_vec();
        for destination in 0..group.len() {
            let source = current.iter().position(|slot| *slot == wanted[destination]).ok_or("Lost a hidden card")?;
            if source == destination {
                continue;
            }
            let (a, b) = (slots[group[destination]], slots[group[source]]);
            swaps.push((a, b));
            current.swap(destination, source);
        }
        Ok(())
    };
    let own: Vec<usize> = (0..slots.len()).filter(|i| slots[*i].controller == viewer).collect();
    let mut wanted = own.clone();
    rng.shuffle(&mut wanted);
    permute(&own, &wanted)?;

    let theirs: Vec<usize> = (0..slots.len()).filter(|i| slots[*i].controller != viewer).collect();
    let set: Vec<usize> = theirs.iter().copied().filter(|i| slots[*i].location == 8).collect();
    let mut backrow: Vec<usize> = theirs
        .iter()
        .copied()
        .filter(|i| duel.printed(slots[*i].code).is_some_and(|card| card.kind & TYPE_SPELL_OR_TRAP != 0))
        .collect();
    if backrow.len() < set.len() {
        return Err("Fewer hidden Spells and Traps than Set cards".into());
    }
    rng.shuffle(&mut backrow);
    backrow.truncate(set.len());
    let mut rest: Vec<usize> = theirs.iter().copied().filter(|i| !backrow.contains(i)).collect();
    rng.shuffle(&mut rest);
    let destinations: Vec<usize> = set.iter().copied().chain(theirs.iter().copied().filter(|i| slots[*i].location != 8)).collect();
    let sources: Vec<usize> = backrow.into_iter().chain(rest).collect();
    permute(&destinations, &sources)?;
    Ok(swaps)
}

/// Cached only for one root decision. The snapshot after inventory queries
/// preserves their native query-buffer/allocation effects on every branch.
#[derive(Default)]
struct HiddenWorlds {
    inventory: Option<Result<(Vec<Slot>, Snapshot)>>,
    plans: HashMap<u64, Result<Vec<(Slot, Slot)>>>,
}

impl HiddenWorlds {
    fn restore(&mut self, duel: &mut Duel, root: &Snapshot, viewer: u8, seed: u64) -> Result<()> {
        if self.inventory.is_none() {
            duel.restore(root)?;
            self.inventory = Some(hidden(duel, viewer).and_then(|slots| Ok((slots, duel.snapshot()?))));
        }
        let (slots, queried) = self.inventory.as_ref().unwrap().as_ref().map_err(Clone::clone)?;
        duel.restore(queried)?;
        let swaps = self.plans.entry(seed).or_insert_with(|| swap_plan(duel, slots, viewer, seed));
        for &(a, b) in swaps.as_ref().map_err(Clone::clone)? {
            if !duel.swap(a.controller, (a.location, a.sequence), (b.location, b.sequence))? {
                return Err(format!("The engine refused to swap {:x}:{} and {:x}:{}", a.location, a.sequence, b.location, b.sequence));
            }
        }
        Ok(())
    }
}

/// Reconstructed policy state, scoped to one root decision and search stage.
type PolicyWorlds<'a> = HashMap<Option<(u8, u64)>, Vec<LibrarySeat<'a>>>;

/// Play a duel on from `snapshot` with `response` to its pending
/// question, in the world as it is or in one dealt again for `viewer`.
/// Both seats copy the world's reconstructed history when the library supports it.
/// Returns the winner, if the duel ends before the decision limit.
fn playout<'a>(
    duel: &mut Duel,
    snapshot: &Snapshot,
    setup: &Setup<'a>,
    logs: &[Vec<u8>; 2],
    prepared: &mut PolicyWorlds<'a>,
    hidden: &mut HiddenWorlds,
    world: Option<(u8, u64)>,
    response: &[u8],
    mut decisions: usize,
    itself: Itself,
) -> Result<Option<u8>> {
    if let Some((viewer, seed)) = world {
        hidden.restore(duel, snapshot, viewer, seed)?;
    } else {
        duel.restore(snapshot)?;
    }
    // A world has its own policy tie seed. Rebuild that history once, then
    // copy its complete seats for each alternative, never the live seats.
    let rebuild = || -> Result<Vec<LibrarySeat<'a>>> {
        let mut seats = Vec::new();
        for p in 0..2 {
            let ties = match world {
                Some((_, seed)) => seed.rotate_left(17) ^ p as u64,
                None => setup.seed + p as u64,
            };
            let seat = setup.policies[p].seat(setup.names[p], setup.cards, p as i32, ties)?;
            seat.feed_buffer(&logs[p])?;
            seats.push(seat);
        }
        Ok(seats)
    };
    let seats = if setup.policies.iter().all(|library| library.can_fork()) {
        let prepared = match prepared.entry(world) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(rebuild()?),
        };
        prepared.iter().map(LibrarySeat::fork).collect::<Result<Vec<_>>>()?
    } else {
        rebuild()?
    };
    // The outside seat's copy plays its seat from here; the policy at the
    // table in its place is fed along and what it answers dropped.
    let mut copy = match itself {
        Some((live, seat)) => Some((live.fork()?, seat)),
        None => None,
    };
    duel.respond(response);
    decisions += 1;
    loop {
        let step = duel.step()?;
        let mut answer = None;
        for sent in &step.messages {
            if !sent.refresh && sent.bytes[0] == msg::RETRY {
                return Err("Engine rejected response (MSG_RETRY)".into());
            }
            let mut its = match &mut copy {
                Some((copy, _)) => copy.feed(&sent.bytes, duel)?,
                None => None,
            };
            if sent.refresh {
                seats[0].feed_update_pair(&seats[1], &sent.bytes)?;
                continue;
            }
            for (p, seat) in seats.iter().enumerate() {
                let given = seat.feed(&sent.bytes)?;
                if !sent.refresh && given.is_some() {
                    answer = match &copy {
                        Some((_, seat)) if *seat == p => Some(its.take().ok_or("The copy did not answer its own prompt")?),
                        _ => given,
                    };
                }
            }
        }
        if matches!(step.state, State::Over(_)) || decisions >= setup.limit {
            let winner = match step.state {
                State::Over(Some(outcome)) => Some(outcome.winner),
                _ => None,
            };
            return Ok(winner.filter(|w| *w < 2));
        }
        match answer {
            Some(given) => {
                duel.respond(&given);
                decisions += 1;
            }
            None if step.state == State::Awaiting => return Err("Engine awaiting with no policy answer".into()),
            None => {}
        }
    }
}

/// Search one decision of `viewer`: the response to give, and what the
/// search saw when that is not the pilot's.  `counts` is playouts run
/// and playouts that failed.
#[allow(clippy::too_many_arguments)]
fn decide(
    duel: &mut Duel,
    setup: &Setup,
    logs: &[Vec<u8>; 2],
    viewer: u8,
    decision: &Listed,
    search: &SearchOptions,
    decisions: usize,
    counts: &mut [u64; 2],
    log: Option<&mut Vec<serde_json::Value>>,
    itself: Itself,
) -> Result<(usize, serde_json::Value)> {
    let before = counts[0];
    let snapshot = duel.snapshot()?;
    let mut hidden = HiddenWorlds::default();
    let mut candidates: Vec<Candidate> = (0..decision.responses.len()).map(|index| Candidate { index, payoffs: vec![] }).collect();
    let mut alive: Vec<usize> = (1..candidates.len()).collect();
    let mut seen = serde_json::Value::Null;
    let mut chosen = 0;
    let mut done = 0;
    let mut won_everywhere = false;
    for (stage, total) in search.stages.iter().copied().enumerate() {
        // Only this stage needs these worlds; bound retained policy state.
        let mut prepared = PolicyWorlds::new();
        // The pilot's answer goes first.  Where it wins every first-stage
        // world no alternative can be ahead of it, so none is tried: the
        // answer is the one the full stage would keep.
        for pilot_pass in [true, false] {
            if !pilot_pass && won_everywhere {
                break;
            }
            for world in done..total {
                let seed = setup.seed ^ (decisions as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ (world as u64 + 1).wrapping_mul(0xD1B54A32D192ED03);
                let indices: Vec<usize> = if pilot_pass { vec![0] } else { alive.clone() };
                for index in indices {
                    counts[0] += 1;
                    let played = playout(duel, &snapshot, setup, logs, &mut prepared, &mut hidden, Some((viewer, seed)), &decision.responses[index], decisions, itself);
                    candidates[index].payoffs.push(match played {
                        Ok(Some(winner)) => Some(if winner == viewer { 1.0 } else { -1.0 }),
                        Ok(None) => Some(0.0),
                        Err(_) => {
                            counts[1] += 1;
                            None
                        }
                    });
                }
            }
            if pilot_pass && stage == 0 {
                won_everywhere = candidates[0].payoffs.iter().all(|payoff| payoff.map_or(true, |p| p >= 1.0));
            }
        }
        if won_everywhere {
            break;
        }
        done = total;
        let mut ranked: Vec<(usize, f64, f64, usize)> = alive
            .iter()
            .map(|index| {
                let (mean, z, n) = paired(&candidates[*index], &candidates[0]);
                (*index, mean, z, n)
            })
            .filter(|(_, mean, _, _)| *mean > 0.0)
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        let Some(best) = ranked.first().copied() else { break };
        let last = stage + 1 == search.stages.len();
        let accept = if last { best.2 >= search.z } else { stage > 0 && best.2 >= EARLY_ACCEPT };
        if accept {
            chosen = best.0;
            seen = serde_json::json!({"kind": decision.kind, "from": decision.labels[0], "to": decision.labels[best.0],
                "gain": best.1, "z": if best.2.is_finite() { best.2 } else { 99.0 }, "worlds": best.3});
            break;
        }
        if last || (stage > 0 && best.2 < EARLY_REJECT) {
            break;
        }
        alive = ranked.iter().take(KEPT).map(|r| r.0).collect();
    }
    // The live duel goes on from the decision itself.
    duel.restore(&snapshot)?;
    if let Some(log) = log {
        // How each alternative did in the first-stage worlds (wins, of
        // the worlds played), the pilot's answer first.
        let first = search.stages[0];
        let record: Vec<serde_json::Value> = candidates
            .iter()
            .map(|candidate| {
                let played: Vec<f64> = candidate.payoffs.iter().take(first).filter_map(|p| *p).collect();
                serde_json::json!([played.iter().filter(|p| **p > 0.0).count(), played.len()])
            })
            .collect();
        // Every world each alternative was played in: + won, - lost,
        // 0 drawn, x not played out.
        let played: Vec<String> = candidates
            .iter()
            .map(|candidate| {
                candidate
                    .payoffs
                    .iter()
                    .map(|payoff| match payoff {
                        Some(p) if *p > 0.0 => '+',
                        Some(p) if *p < 0.0 => '-',
                        Some(_) => '0',
                        None => 'x',
                    })
                    .collect()
            })
            .collect();
        log.push(serde_json::json!({"kind": decision.kind, "labels": decision.labels, "first": record, "played": played,
            "worlds": candidates[0].payoffs.len(), "playouts": counts[0] - before, "chosen": chosen,
            "skipped": won_everywhere}));
    }
    Ok((candidates[chosen].index, seen))
}

/// With foresight (see [`SearchOptions::foresight`]): the first alternative
/// that wins in the world as it is, when the pilot's answer does not.
fn foresee(
    duel: &mut Duel,
    setup: &Setup,
    logs: &[Vec<u8>; 2],
    viewer: u8,
    decision: &Listed,
    decisions: usize,
    counts: &mut [u64; 2],
) -> Result<usize> {
    let snapshot = duel.snapshot()?;
    let mut chosen = 0;
    let mut prepared = PolicyWorlds::new();
    for index in 0..decision.responses.len() {
        counts[0] += 1;
        match playout(duel, &snapshot, setup, logs, &mut prepared, &mut HiddenWorlds::default(), None, &decision.responses[index], decisions, None) {
            Ok(Some(winner)) if winner == viewer => {
                chosen = index;
                break;
            }
            Ok(_) => {}
            Err(_) => counts[1] += 1,
        }
    }
    duel.restore(&snapshot)?;
    Ok(chosen)
}

/// [`play`] with one seat searching (see the module).  Without a
/// deviation the duel is the one `play` plays, answer for answer.
///
/// With `outside` the other seat is answered by that seat and not by a
/// policy.  Its stand-in sits at the table in its place, fed all the seat is
/// fed, so that the try-outs have a policy to play it and that policy has
/// seen the duel; what the stand-in answers in the duel itself is dropped.
#[allow(clippy::too_many_arguments)]
pub fn play_searching(
    core: &Core,
    decks: &[Deck; 2],
    policies: [&PolicyLibrary; 2],
    names: [&str; 2],
    cards: &Path,
    run: PlayOptions,
    search: SearchOptions,
    mut outside: Option<Outside>,
) -> Result<serde_json::Value> {
    if !core.has_snapshots() {
        return Err("the engine given as --core has no arena snapshots or no hidden-card swap, which a search needs".into());
    }
    if outside.is_some() && (search.validate || search.foresight) {
        return Err("--validate and --foresight play the other player itself out, which a stand-in is not".into());
    }
    let PlayOptions { seed, limit, life_points, record, .. } = run;
    let options = DuelOptions { life_points, flags: run.flags, ..DuelOptions::seeded(seed) };
    let mut duel = core.deal(&options, decks)?;
    // The duel as it is played, the search's answers in the place of the pilot's.
    let mut recorded = record.then(|| Recorded::dealt(&options, decks));
    let other = 1 - search.searcher;
    // The policies at the table, which are those of the try-outs.
    let mut seated = names;
    if let Some(outside) = &outside {
        seated[other] = outside.stand_in;
    }
    let setup = Setup { policies, names: seated, cards, seed, limit };
    let viewer = search.searcher as u8;
    let mut table = Table { seats: Vec::new(), logs: [Vec::new(), Vec::new()] };
    for p in 0..2 {
        table.seats.push(policies[p].seat(seated[p], cards, p as i32, seed + p as u64)?);
        table.feed(p, &duel.start_message(p as u8))?;
    }
    if let Some(outside) = &mut outside {
        outside.seat.feed(&duel.start_message(other as u8), &duel)?;
    }
    let mut decisions = 0;
    let mut turns = 0u32;
    let mut digest = 0xcbf29ce484222325u64;
    let mut chain = false;
    // Decisions searched, left to the pilot in a chain or facing a
    // face-down monster, playouts, failed playouts.
    let (mut searched, mut in_chain, mut facing_set, mut counts) = (0u64, 0u64, 0u64, [0u64; 2]);
    let mut deviations = Vec::new();
    let mut examined: Vec<serde_json::Value> = Vec::new();
    let mut predictions: Vec<Option<u8>> = Vec::new();
    loop {
        let step = duel.step()?;
        let mut response: Option<(usize, Vec<u8>, Vec<u8>)> = None;
        for sent in &step.messages {
            let message = &sent.bytes;
            // The seat answered from outside is fed what its stand-in is fed.
            let mut given = match &mut outside {
                Some(outside) => outside.seat.feed(message, &duel)?,
                None => None,
            };
            if sent.refresh {
                table.update(message)?;
                if given.is_some() {
                    return Err("The outside seat answered a field update".into());
                }
                continue;
            }
            match message[0] {
                msg::RETRY => return Err("Engine rejected response (MSG_RETRY)".into()),
                msg::NEW_TURN => turns += 1,
                msg::CHAINING => chain = true,
                msg::CHAIN_END => chain = false,
                _ => {}
            }
            for p in 0..2 {
                if let Some(answer) = table.feed(p, message)? {
                    if response.is_some() {
                        return Err("Multiple answers in one engine batch".into());
                    }
                    // The duel's answer is the seat's own, not its stand-in's.
                    let answer = if outside.is_some() && p == other {
                        given.take().ok_or("The stand-in answered a prompt the outside seat did not")?
                    } else {
                        answer
                    };
                    response = Some((p, answer, message.to_vec()));
                }
            }
            if given.is_some() {
                return Err("The outside seat answered a prompt its stand-in did not".into());
            }
        }
        if matches!(step.state, State::Over(_)) || decisions >= limit {
            let (winner, reason) = match step.state {
                State::Over(Some(outcome)) => (Some(outcome.winner), Some(outcome.reason)),
                State::Over(None) => return Err("Engine ended without MSG_WIN".into()),
                _ => (None, None),
            };
            let mispredicted = predictions.iter().filter(|p| **p != winner.filter(|w| *w < 2)).count();
            let mut row = serde_json::json!({"winner": winner, "reason": reason, "turns": turns, "decisions": decisions,
                "limit": winner.is_none(), "digest": format!("{digest:016x}"), "searched": searched, "in_chain": in_chain,
                "facing_set": facing_set,
                "playouts": counts[0], "failed_playouts": counts[1], "deviations": deviations, "examined": examined,
                "predictions": predictions.len(), "mispredicted": mispredicted});
            if let Some(recorded) = &recorded {
                row["record"] = record_json(recorded, names);
            }
            if let Some(outside) = &outside {
                row["model_seats"] = serde_json::json!([{"seat": other, "stats": outside.seat.stats()}]);
                // Who played the seat in the try-outs: a policy, or the player itself.
                row["stand_in"] = serde_json::json!(if outside.itself { names[other] } else { outside.stand_in });
            }
            return Ok(row);
        }
        let Some((p, mut answer, message)) = response else {
            if step.state == State::Awaiting {
                return Err("Engine awaiting with no policy answer".into());
            }
            continue;
        };
        if p == search.searcher {
            if search.validate {
                // The pilot's own answer, played out in the world as it is.
                counts[0] += 1;
                let snapshot = duel.snapshot()?;
                predictions.push(playout(&mut duel, &snapshot, &setup, &table.logs, &mut PolicyWorlds::new(), &mut HiddenWorlds::default(), None, &answer, decisions, None)?);
                duel.restore(&snapshot)?;
            } else if let Some((described, decision)) = {
                let described = table.seats[p].search_view()?;
                listed(&described, &answer).map(|decision| (described, decision))
            } {
                if search.foresight {
                    searched += 1;
                    let index = foresee(&mut duel, &setup, &table.logs, viewer, &decision, decisions, &mut counts)?;
                    if index != 0 {
                        answer = decision.responses[index].clone();
                        deviations.push(serde_json::json!({"kind": decision.kind, "from": decision.labels[0],
                            "to": decision.labels[index], "decision": decisions, "turn": turns}));
                    }
                } else if chain {
                    in_chain += 1;
                } else if search.strict && duel.monsters().iter().any(|m| m.controller != viewer && m.position & position::FACEDOWN != 0) {
                    facing_set += 1;
                } else {
                    searched += 1;
                    let log = search.log.then_some(&mut examined);
                    let itself: Itself = outside.as_ref().filter(|outside| outside.itself).map(|outside| (&*outside.seat, other));
                    let (index, seen) = decide(&mut duel, &setup, &table.logs, viewer, &decision, &search, decisions, &mut counts, log, itself)?;
                    if search.log {
                        if let Some(record) = examined.last_mut() {
                            record["decision"] = serde_json::json!(decisions);
                            record["turn"] = serde_json::json!(turns);
                            record["situation"] = described.get("situation").cloned()
                                .unwrap_or_else(|| situation(&described, viewer));
                            if index != 0 {
                                // Where the pilot's answer was changed: all the seat saw and
                                // every choice it had, for whoever writes the pilot's rules.
                                record["seen"] = table.seats[p].last_answer()?;
                            }
                        }
                    }
                    if index != 0 {
                        answer = decision.responses[index].clone();
                        let mut seen = seen;
                        seen["decision"] = serde_json::json!(decisions);
                        seen["turn"] = serde_json::json!(turns);
                        deviations.push(seen);
                    }
                }
            }
        }
        for byte in message.iter().chain(&answer) {
            digest = (digest ^ *byte as u64).wrapping_mul(0x100000001b3);
        }
        duel.respond(&answer);
        if let Some(recorded) = &mut recorded {
            // `decisions` is this answer's place among the record's responses.
            recorded.responses.push((p as u8, answer));
        }
        decisions += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A seat of the library behind the door the model's seat comes through.
    struct Proxy<'a>(LibrarySeat<'a>);
    impl LiveSeat for Proxy<'_> {
        fn feed(&mut self, bytes: &[u8], _: &Duel) -> Result<Option<Vec<u8>>> {
            self.0.feed(bytes)
        }
        fn stats(&self) -> serde_json::Value {
            serde_json::Value::Null
        }
        fn fork(&self) -> Result<Box<dyn LiveSeat + '_>> {
            Ok(Box::new(Proxy(self.0.fork()?)))
        }
    }

    /// With an outside seat that answers as its stand-in would, a searched
    /// duel is the one searched against the stand-in itself: the same
    /// try-outs, the same answers changed, the same digest.  And so it is
    /// with copies of that seat in the try-outs in the stand-in's place.
    /// Needs the built policy library; without it the test says so and passes.
    #[test]
    fn an_outside_seat_that_answers_as_its_stand_in_changes_nothing() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let built = root.join("target/release/libygo_policies.so");
        if !built.exists() {
            eprintln!("skipped: {} is not built", built.display());
            return;
        }
        let cards = root.join("vendor/BabelCdb/cards.cdb");
        let core = Core::open(None, &cards, &root.join("vendor/CardScripts")).unwrap();
        let library = PolicyLibrary::open(&built).unwrap();
        let deck = Deck::load(&root.join("decks/BlueEyes.ydk")).unwrap();
        let decks = [deck.clone(), deck];
        let mut changed = 0;
        for (seed, searcher) in [(880_001, 0), (880_002, 1), (880_003, 0)] {
            let run = PlayOptions { seed, limit: 4096, trace: false, life_points: [8000; 2], record: false, flags: DuelOptions::MASTER_RULE_5 };
            let search = SearchOptions { searcher, stages: [4, 8, 16], z: 1.645, strict: true, log: false, validate: false, foresight: false };
            let other = 1 - searcher;
            let mut names = ["blue-eyes"; 2];
            names[other] = "first";
            let plain = play_searching(&core, &decks, [&library; 2], names, &cards, run, search, None).unwrap();
            let proxy = Proxy(library.seat("first", &cards, other as i32, seed + other as u64).unwrap());
            // The other player is named as the model would be: only the stand-in says who plays it.
            names[other] = "someone";
            let outside = Outside { seat: Box::new(proxy), stand_in: "first", itself: false };
            let stood = play_searching(&core, &decks, [&library; 2], names, &cards, run, search, Some(outside)).unwrap();
            for key in ["digest", "winner", "decisions", "searched", "playouts", "failed_playouts", "deviations"] {
                assert_eq!(plain[key], stood[key], "{key} of seed {seed}");
            }
            assert_eq!(stood["stand_in"], "first");
            // The seat's own copies in the try-outs, another policy only sitting at the table.
            let proxy = Proxy(library.seat("first", &cards, other as i32, seed + other as u64).unwrap());
            let outside = Outside { seat: Box::new(proxy), stand_in: "blue-eyes", itself: true };
            let copied = play_searching(&core, &decks, [&library; 2], names, &cards, run, search, Some(outside)).unwrap();
            for key in ["digest", "winner", "decisions", "searched", "playouts", "failed_playouts", "deviations"] {
                assert_eq!(plain[key], copied[key], "{key} of seed {seed}, with copies of the seat");
            }
            assert_eq!(copied["stand_in"], "someone");
            changed += plain["deviations"].as_array().unwrap().len();
        }
        assert!(changed > 0, "no answer was changed in these duels: the test shows nothing of a search that acts");
    }
}
