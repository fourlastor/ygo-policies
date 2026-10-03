//! A one-step search on top of a pilot.
//!
//! One seat is played by its policy and a search: at each of its decisions
//! the duel is snapshotted, every alternative the policy listed is tried in
//! worlds where the cards this seat cannot see are dealt again, and both
//! policies play each world out.  The seat leaves its pilot's answer only for
//! an alternative that wins clearly more often over the same worlds.
//!
//! The other player's face-down monsters cannot be dealt again, so a world
//! would show the search what they are.  A strict search leaves every
//! decision taken while one is on the field to the pilot.
//!
//! This needs an engine with arena snapshots and a hidden-card swap
//! (`OCG_DuelCreateSnapshot`, `YGO_DuelSwapHiddenCards`): the `ygo`
//! repository's build of its OCGCore fork, given as `--core`.
use super::*;

/// What the search needs beyond the public OCGCore API.
pub(super) struct Extensions {
    snapshot: unsafe extern "C" fn(Handle, *mut Handle) -> i32,
    restore: unsafe extern "C" fn(Handle, Handle) -> i32,
    discard: unsafe extern "C" fn(Handle),
    swap: unsafe extern "C" fn(Handle, u8, u32, u32, u32, u32) -> i32,
    count: unsafe extern "C" fn(Handle, u8, u32) -> u32,
    card: unsafe extern "C" fn(Handle, *mut u32, *const Query) -> *const u8,
}

impl Extensions {
    pub(super) unsafe fn load(library: &Library) -> Option<Self> {
        Some(Self {
            snapshot: symbol(library, b"OCG_DuelCreateSnapshot").ok()?,
            restore: symbol(library, b"OCG_DuelRestoreSnapshot").ok()?,
            discard: symbol(library, b"OCG_DuelDestroySnapshot").ok()?,
            swap: symbol(library, b"YGO_DuelSwapHiddenCards").ok()?,
            count: symbol(library, b"OCG_DuelQueryCount").ok()?,
            card: symbol(library, b"OCG_DuelQuery").ok()?,
        })
    }
}

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
    /// Search nothing: at each decision, play the pilot's answer out in the
    /// world as it is, and check that it ends as the duel itself does.
    pub validate: bool,
    /// Not a player: play every alternative out in the world as it is (the
    /// real hidden cards, the draws to come) and leave the pilot's answer
    /// whenever it loses and another one wins.  What it still loses, no
    /// single change of answer could have won.
    pub foresight: bool,
}

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
    seats: Vec<Seat<'a>>,
    logs: [Vec<u8>; 2],
}

impl Table<'_> {
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

impl Core {
    /// The cards `viewer` cannot see: its own Deck, and the other player's
    /// Deck, hand and Set Spells and Traps.  Revealed cards stay known.
    unsafe fn hidden(&self, handle: Handle, viewer: u8) -> Vec<Slot> {
        let extensions = self.extensions.as_ref().unwrap();
        let mut slots = Vec::new();
        let mut add = |controller: u8, location: u32, sequence: u32, face_down_only: bool| {
            let q = Query { flags: query::CODE | query::POSITION | QUERY_IS_PUBLIC, controller, location, sequence, overlay: 0 };
            let mut length = 0;
            let bytes = (extensions.card)(handle, &mut length, &q);
            if length == 0 {
                return;
            }
            let (code, position, public) = card_query(std::slice::from_raw_parts(bytes, length as usize));
            if code != 0 && !public && (!face_down_only || position & position::FACEDOWN != 0) {
                slots.push(Slot { code, controller, location, sequence });
            }
        };
        let opponent = 1 - viewer;
        for (controller, location) in [(viewer, 1), (opponent, 1), (opponent, 2)] {
            for sequence in 0..(extensions.count)(handle, controller, location) {
                add(controller, location, sequence, false);
            }
        }
        for sequence in 0..7 {
            add(opponent, 8, sequence, true);
        }
        slots
    }

    /// Deal again what `viewer` cannot see: its own Deck in another order,
    /// and the other player's hidden cards among their Deck, hand and Set
    /// cards (a Set card stays a Spell or Trap).  Not possible in a chain.
    unsafe fn redeal(&self, handle: Handle, viewer: u8, seed: u64) -> Result<()> {
        let extensions = self.extensions.as_ref().unwrap();
        let slots = self.hidden(handle, viewer);
        let mut rng = SplitMix(seed);
        let permute = |group: &[usize], wanted: &[usize]| -> Result<()> {
            let mut current = group.to_vec();
            for destination in 0..group.len() {
                let source = current.iter().position(|slot| *slot == wanted[destination]).ok_or("Lost a hidden card")?;
                if source == destination {
                    continue;
                }
                let (a, b) = (slots[group[destination]], slots[group[source]]);
                if (extensions.swap)(handle, a.controller, a.location, a.sequence, b.location, b.sequence) == 0 {
                    return Err(format!("The engine refused to swap {:x}:{} and {:x}:{}", a.location, a.sequence, b.location, b.sequence));
                }
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
            .filter(|i| self.resources.cards.get(&slots[*i].code).is_some_and(|(card, _)| card.kind & TYPE_SPELL_OR_TRAP != 0))
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
        permute(&destinations, &sources)
    }

    /// Play a duel on from `snapshot` with `response` to its pending
    /// question, in the world as it is or in one dealt again for `viewer`.
    /// Both seats are built again from what the live ones were fed.
    /// Returns the winner, if the duel ends before the decision limit.
    unsafe fn playout(
        &mut self,
        handle: Handle,
        snapshot: Handle,
        setup: &Setup,
        logs: &[Vec<u8>; 2],
        world: Option<(u8, u64)>,
        response: &[u8],
        mut decisions: usize,
    ) -> Result<Option<u8>> {
        let status = (self.extensions.as_ref().unwrap().restore)(handle, snapshot);
        if status != 0 {
            return Err(format!("OCG_DuelRestoreSnapshot: {status}"));
        }
        self.resources.errors.clear();
        if let Some((viewer, seed)) = world {
            self.redeal(handle, viewer, seed)?;
        }
        let mut seats = Vec::new();
        for p in 0..2 {
            // A dealt world also breaks ties its own way: the search must not
            // learn from a playout which of two equal choices the other
            // player's policy is about to make in the duel.
            let ties = match world {
                Some((_, seed)) => seed.rotate_left(17) ^ p as u64,
                None => setup.seed + p as u64,
            };
            let seat = setup.policies[p].seat(setup.names[p], setup.cards, p as i32, ties)?;
            seat.feed_buffer(&logs[p])?;
            seats.push(seat);
        }
        (self.respond)(handle, response.as_ptr(), response.len() as u32);
        decisions += 1;
        loop {
            let status = (self.process)(handle);
            let mut length = 0;
            let bytes = (self.messages)(handle, &mut length);
            let buffer = if length == 0 { vec![] } else { std::slice::from_raw_parts(bytes, length as usize).to_vec() };
            let (mut offset, mut answer, mut winner) = (0, None, None);
            while offset + 4 <= buffer.len() {
                let n = u32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap()) as usize;
                let message = buffer.get(offset + 4..offset + 4 + n).ok_or("Truncated message")?;
                offset += 4 + n;
                let id = *message.first().ok_or("Empty message")?;
                if id == msg::RETRY {
                    return Err("Engine rejected response (MSG_RETRY)".into());
                }
                if id == msg::WIN {
                    winner = message.get(1).copied();
                }
                if message::is_selection(id) {
                    for update in self.field(handle) {
                        for seat in &seats {
                            seat.feed(&update)?;
                        }
                    }
                }
                for seat in &seats {
                    if let Some(given) = seat.feed(message)? {
                        answer = Some(given);
                    }
                }
            }
            if !self.resources.errors.is_empty() {
                return Err(self.resources.errors.join("\n"));
            }
            if winner.is_some() || status == 0 || decisions >= setup.limit {
                return Ok(winner.filter(|w| *w < 2));
            }
            match answer {
                Some(given) => {
                    (self.respond)(handle, given.as_ptr(), given.len() as u32);
                    decisions += 1;
                }
                None if status == 1 => return Err("Engine awaiting with no policy answer".into()),
                None => {}
            }
        }
    }

    /// Search one decision of `viewer`: the response to give, and what the
    /// search saw when that is not the pilot's.  `counts` is playouts run
    /// and playouts that failed.
    unsafe fn decide(
        &mut self,
        handle: Handle,
        setup: &Setup,
        logs: &[Vec<u8>; 2],
        viewer: u8,
        decision: &Listed,
        search: &SearchOptions,
        decisions: usize,
        counts: &mut [u64; 2],
    ) -> Result<(usize, serde_json::Value)> {
        let mut snapshot = ptr::null_mut();
        let status = (self.extensions.as_ref().unwrap().snapshot)(handle, &mut snapshot);
        if status != 0 {
            return Err(format!("OCG_DuelCreateSnapshot: {status}"));
        }
        let _guard = SnapshotGuard { handle: snapshot, discard: self.extensions.as_ref().unwrap().discard };
        let mut candidates: Vec<Candidate> = (0..decision.responses.len()).map(|index| Candidate { index, payoffs: vec![] }).collect();
        let mut alive: Vec<usize> = (1..candidates.len()).collect();
        let mut seen = serde_json::Value::Null;
        let mut chosen = 0;
        let mut done = 0;
        for (stage, total) in search.stages.iter().copied().enumerate() {
            for world in done..total {
                let seed = setup.seed ^ (decisions as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ (world as u64 + 1).wrapping_mul(0xD1B54A32D192ED03);
                for index in std::iter::once(0).chain(alive.iter().copied()) {
                    counts[0] += 1;
                    let played = self.playout(handle, snapshot, setup, logs, Some((viewer, seed)), &decision.responses[index], decisions);
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
        let status = (self.extensions.as_ref().unwrap().restore)(handle, snapshot);
        if status != 0 {
            return Err(format!("OCG_DuelRestoreSnapshot: {status}"));
        }
        self.resources.errors.clear();
        Ok((candidates[chosen].index, seen))
    }

    /// With foresight (see [`SearchOptions::foresight`]): the first alternative
    /// that wins in the world as it is, when the pilot's answer does not.
    unsafe fn foresee(
        &mut self,
        handle: Handle,
        setup: &Setup,
        logs: &[Vec<u8>; 2],
        viewer: u8,
        decision: &Listed,
        decisions: usize,
        counts: &mut [u64; 2],
    ) -> Result<usize> {
        let mut snapshot = ptr::null_mut();
        let extensions = self.extensions.as_ref().unwrap();
        let (restore, discard) = (extensions.restore, extensions.discard);
        let status = (extensions.snapshot)(handle, &mut snapshot);
        if status != 0 {
            return Err(format!("OCG_DuelCreateSnapshot: {status}"));
        }
        let _guard = SnapshotGuard { handle: snapshot, discard };
        let mut chosen = 0;
        for index in 0..decision.responses.len() {
            counts[0] += 1;
            match self.playout(handle, snapshot, setup, logs, None, &decision.responses[index], decisions) {
                Ok(Some(winner)) if winner == viewer => {
                    chosen = index;
                    break;
                }
                Ok(_) => {}
                Err(_) => counts[1] += 1,
            }
        }
        let status = restore(handle, snapshot);
        if status != 0 {
            return Err(format!("OCG_DuelRestoreSnapshot: {status}"));
        }
        self.resources.errors.clear();
        Ok(chosen)
    }

    /// [`Core::play`] with one seat searching (see the module).  Without a
    /// deviation the duel is the one `play` plays, answer for answer.
    pub fn play_searching(
        &mut self,
        decks: &[Deck; 2],
        policies: [&PolicyLibrary; 2],
        names: [&str; 2],
        cards: &Path,
        run: PlayOptions,
        search: SearchOptions,
    ) -> Result<serde_json::Value> {
        if self.extensions.is_none() {
            return Err("search needs --core: an OCGCore build with arena snapshots and the hidden-card swap".into());
        }
        let PlayOptions { seed, limit, life_points, .. } = run;
        self.resources.errors.clear();
        for code in decks.iter().flat_map(|d| d.main.iter().chain(&d.extra)) {
            if !self.resources.cards.contains_key(code) {
                return Err(format!("Deck card {code} is absent from the database"));
            }
        }
        let payload = (&mut *self.resources) as *mut Resources as Handle;
        let player = |lp| Player { lp, draw: 5, per_turn: 1 };
        let options = Options {
            seed: [seed, (seed.wrapping_mul(1103515245) + 12345) & 0xffffffff, 3, 4],
            flags: 0xD0700,
            p0: player(life_points[0]),
            p1: player(life_points[1]),
            reader,
            payload1: payload,
            scripts,
            payload2: payload,
            log,
            payload3: payload,
            done,
            payload4: payload,
            unsafe_libraries: 0,
        };
        let setup = Setup { policies, names, cards, seed, limit };
        let viewer = search.searcher as u8;
        unsafe {
            let mut handle = ptr::null_mut();
            let status = (self.create)(&mut handle, &options);
            if status != 0 {
                return Err(format!("OCG_CreateDuel: {status}"));
            }
            let _duel = DuelGuard { handle, destroy: self.destroy };
            for name in ["constant.lua", "utility.lua"] {
                let name = CString::new(name).unwrap();
                if scripts(payload, handle, name.as_ptr()) == 0 {
                    return Err(format!("Failed to load {name:?}"));
                }
            }
            let mut table = Table { seats: Vec::new(), logs: [Vec::new(), Vec::new()] };
            let mut rng = seed;
            for p in 0..2 {
                table.seats.push(policies[p].seat(names[p], cards, p as i32, seed + p as u64)?);
                table.feed(
                    p,
                    &message::start_message(
                        p as u8,
                        life_points,
                        [decks[0].main.len() as u16, decks[1].main.len() as u16],
                        [decks[0].extra.len() as u16, decks[1].extra.len() as u16],
                    ),
                )?;
                let mut main = decks[p].main.clone();
                shuffle(&mut main, &mut rng);
                for (location, pile) in [(1, &main), (0x40, &decks[p].extra)] {
                    for code in pile {
                        (self.new_card)(
                            handle,
                            &NewCard { team: p as u8, duelist: 0, code: *code, controller: p as u8, location, sequence: 0, position: 8 },
                        );
                    }
                }
            }
            (self.start)(handle);
            let mut decisions = 0;
            let mut winner = None;
            let (mut reason, mut turns) = (None, 0u32);
            let mut digest = 0xcbf29ce484222325u64;
            let mut chain = false;
            // Decisions searched, left to the pilot in a chain or facing a
            // face-down monster, playouts, failed playouts.
            let (mut searched, mut in_chain, mut facing_set, mut counts) = (0u64, 0u64, 0u64, [0u64; 2]);
            let mut deviations = Vec::new();
            let mut predictions: Vec<Option<u8>> = Vec::new();
            loop {
                let status = (self.process)(handle);
                let mut length = 0;
                let bytes = (self.messages)(handle, &mut length);
                let buffer = if length == 0 { vec![] } else { std::slice::from_raw_parts(bytes, length as usize).to_vec() };
                let mut offset = 0;
                let mut response: Option<(usize, Vec<u8>, Vec<u8>)> = None;
                while offset < buffer.len() {
                    if offset + 4 > buffer.len() {
                        return Err("Truncated message length".into());
                    }
                    let n = u32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap()) as usize;
                    let message = buffer.get(offset + 4..offset + 4 + n).ok_or("Truncated message")?;
                    offset += 4 + n;
                    let id = *message.first().ok_or("Empty message")?;
                    match id {
                        msg::RETRY => return Err("Engine rejected response (MSG_RETRY)".into()),
                        msg::WIN => {
                            winner = message.get(1).copied();
                            reason = message.get(2).copied();
                        }
                        msg::NEW_TURN => turns += 1,
                        msg::CHAINING => chain = true,
                        msg::CHAIN_END => chain = false,
                        _ => {}
                    }
                    if message::is_selection(id) {
                        for update in self.field(handle) {
                            for p in 0..2 {
                                table.feed(p, &update)?;
                            }
                        }
                    }
                    for p in 0..2 {
                        if let Some(answer) = table.feed(p, message)? {
                            if response.is_some() {
                                return Err("Multiple answers in one engine batch".into());
                            }
                            response = Some((p, answer, message.to_vec()));
                        }
                    }
                }
                if !self.resources.errors.is_empty() {
                    return Err(self.resources.errors.join("\n"));
                }
                if winner.is_some() || status == 0 || decisions >= limit {
                    if status == 0 && winner.is_none() {
                        return Err("Engine ended without MSG_WIN".into());
                    }
                    let mispredicted = predictions.iter().filter(|p| **p != winner.filter(|w| *w < 2)).count();
                    return Ok(serde_json::json!({"winner": winner, "reason": reason, "turns": turns, "decisions": decisions,
                        "limit": winner.is_none(), "digest": format!("{digest:016x}"), "searched": searched, "in_chain": in_chain,
                        "facing_set": facing_set,
                        "playouts": counts[0], "failed_playouts": counts[1], "deviations": deviations,
                        "predictions": predictions.len(), "mispredicted": mispredicted}));
                }
                let Some((p, mut answer, message)) = response else {
                    if status == 1 {
                        return Err("Engine awaiting with no policy answer".into());
                    }
                    continue;
                };
                if p == search.searcher {
                    if search.validate {
                        // The pilot's own answer, played out in the world as it is.
                        counts[0] += 1;
                        let mut snapshot = ptr::null_mut();
                        let extensions = self.extensions.as_ref().unwrap();
                        let (restore, discard) = (extensions.restore, extensions.discard);
                        let status = (extensions.snapshot)(handle, &mut snapshot);
                        if status != 0 {
                            return Err(format!("OCG_DuelCreateSnapshot: {status}"));
                        }
                        let _guard = SnapshotGuard { handle: snapshot, discard };
                        predictions.push(self.playout(handle, snapshot, &setup, &table.logs, None, &answer, decisions)?);
                        let status = restore(handle, snapshot);
                        if status != 0 {
                            return Err(format!("OCG_DuelRestoreSnapshot: {status}"));
                        }
                    } else if let Some(decision) = listed(&table.seats[p].last_answer()?, &answer) {
                        if search.foresight {
                            searched += 1;
                            let index = self.foresee(handle, &setup, &table.logs, viewer, &decision, decisions, &mut counts)?;
                            if index != 0 {
                                answer = decision.responses[index].clone();
                                deviations.push(serde_json::json!({"kind": decision.kind, "from": decision.labels[0],
                                    "to": decision.labels[index], "decision": decisions, "turn": turns}));
                            }
                        } else if chain {
                            in_chain += 1;
                        } else if search.strict
                            && self.monsters(handle).iter().any(|m| m.controller != viewer && m.position & position::FACEDOWN != 0)
                        {
                            facing_set += 1;
                        } else {
                            searched += 1;
                            let (index, seen) = self.decide(handle, &setup, &table.logs, viewer, &decision, &search, decisions, &mut counts)?;
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
                (self.respond)(handle, answer.as_ptr(), answer.len() as u32);
                decisions += 1;
            }
        }
    }
}

struct SnapshotGuard {
    handle: Handle,
    discard: unsafe extern "C" fn(Handle),
}
impl Drop for SnapshotGuard {
    fn drop(&mut self) {
        unsafe { (self.discard)(self.handle) }
    }
}
