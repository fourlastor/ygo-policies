//! Replays recorded OCGCore duels from both seats and checks the projection
//! against the engine's own public snapshot at every decision.
//!
//! A trace (`tests/fixtures/*.json.gz`) holds, per decision, the messages a
//! policy seat was fed (the raw, unfiltered engine stream plus the
//! `MSG_START` / `MSG_UPDATE_DATA` its host synthesized) and OCGCore's
//! viewer-filtered snapshot for each player right after them.  Replaying the
//! stream for each viewer must reproduce that snapshot: every public card,
//! nothing hidden, and the same Life Points, turn, phase, piles and chain.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;
use ygo_policies::cards::MemoryCards;
use ygo_policies::model::{Location, Observation};
use ygo_policies::registry;
use ygo_policies_ocgcore::Seat;

fn base64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let digits: Vec<u8> = text.bytes().filter(|c| *c != b'=').map(value).collect();
    digits
        .chunks(4)
        .flat_map(|chunk| {
            let n = chunk.iter().fold(0u32, |acc, d| acc << 6 | *d as u32) << (6 * (4 - chunk.len()));
            let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
            bytes.into_iter().take(chunk.len().saturating_sub(1))
        })
        .collect()
}

fn location_bits(location: Location) -> u64 {
    match location {
        Location::Deck => 0x01,
        Location::Hand => 0x02,
        Location::MonsterZone => 0x04,
        Location::SpellTrapZone => 0x08,
        Location::Graveyard => 0x10,
        Location::Banished => 0x20,
        Location::Extra => 0x40,
        _ => 0,
    }
}

type Slot = (u64, u64, u64);

/// Differences between our observation and the engine snapshot for `viewer`.
fn compare(obs: &Observation, snapshot: &Value, viewer: u8, confirmed: &[Vec<u64>; 2]) -> Vec<String> {
    let mut problems = Vec::new();
    let n = |v: &Value| v.as_u64().unwrap_or(u64::MAX);
    if obs.life_points.iter().map(|lp| *lp as u64).collect::<Vec<_>>() != snapshot["lp"].as_array().unwrap().iter().map(n).collect::<Vec<_>>() {
        problems.push(format!("life points {:?} vs {}", obs.life_points, snapshot["lp"]));
    }
    if obs.turn as u64 != n(&snapshot["turn"]) || obs.turn_player.map(u64::from) != snapshot["turn_player"].as_u64() {
        problems.push(format!("turn {} / {:?} vs {} / {}", obs.turn, obs.turn_player, snapshot["turn"], snapshot["turn_player"]));
    }
    let theirs: BTreeMap<Slot, (u64, u64)> = snapshot["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| ((n(&c[0]), n(&c[1]), n(&c[2])), (n(&c[3]), n(&c[4]))))
        // We do not list the other player's Extra Deck, only its size.
        .filter(|((controller, location, _), _)| !(*location == 0x40 && *controller != viewer as u64))
        .collect();
    let ours: BTreeMap<Slot, (u64, u64)> = obs
        .cards
        .iter()
        .map(|c| {
            let position = if c.position.face_up { 1 } else { 2 } * if c.position.attack { 1 } else { 4 };
            ((c.at.controller as u64, location_bits(c.at.location), c.at.sequence as u64), (c.code.unwrap_or(0) as u64, position))
        })
        .collect();
    for (slot, (code, position)) in &theirs {
        match ours.get(slot) {
            None => problems.push(format!("missing card {slot:?} (code {code})")),
            Some((our_code, our_position)) => {
                // Activating a card's effect reveals it (MSG_CHAINING is public),
                // even from the hand, where the engine snapshot keeps it hidden.
                let activated = *code == 0
                    && obs.chain.iter().any(|l| {
                        (l.source.controller as u64, location_bits(l.source.location), l.source.sequence as u64) == *slot
                            && l.code as u64 == *our_code
                    });
                // A card shown with MSG_CONFIRM_CARDS stays known until its hand is shuffled.
                let shown = *code == 0 && slot.1 == 0x02 && confirmed[slot.0 as usize].contains(our_code);
                if our_code != code && !activated && !shown {
                    problems.push(format!("card {slot:?}: code {our_code} vs {code}"));
                }
                // Spells and Traps have no battle position: compare facing only.
                let differs = match slot.1 {
                    0x04 => our_position != position,
                    0x08 => (our_position & 0x5 != 0) != (position & 0x5 != 0),
                    _ => false,
                };
                if differs {
                    problems.push(format!("card {slot:?}: position {our_position} vs {position}"));
                }
            }
        }
    }
    for slot in ours.keys().filter(|s| !theirs.contains_key(s)) {
        problems.push(format!("extra card {slot:?}"));
    }
    for pile in snapshot["piles"].as_array().unwrap() {
        let (controller, location, count) = (n(&pile[0]), n(&pile[1]), n(&pile[2]));
        let ours = obs
            .pile_sizes
            .iter()
            .find(|(c, l, _)| *c as u64 == controller && location_bits(*l) == location)
            .map(|(_, _, size)| *size as u64)
            .unwrap_or_else(|| obs.cards.iter().filter(|c| c.at.controller as u64 == controller && location_bits(c.at.location) == location).count() as u64);
        if ours != count {
            problems.push(format!("pile {controller}/{location:#x}: {ours} vs {count}"));
        }
    }
    let chain: Vec<(u64, u64)> = obs.chain.iter().map(|l| (l.code as u64, l.controller as u64)).collect();
    let their_chain: Vec<(u64, u64)> = snapshot["chain"].as_array().unwrap().iter().map(|l| (n(&l[0]), n(&l[1]))).collect();
    if chain != their_chain {
        problems.push(format!("chain {chain:?} vs {their_chain:?}"));
    }
    problems
}

fn replay(path: &Path) {
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_string(&mut text).unwrap();
    let trace: Value = serde_json::from_str(&text).unwrap();
    let steps = trace["steps"].as_array().unwrap();
    for viewer in 0..2u8 {
        let db = Arc::new(MemoryCards::default());
        let mut seat = Seat::new(registry::create("blackwing", db.clone()).unwrap(), db, Some(viewer));
        let mut failures = Vec::new();
        let mut confirmed: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
        for (index, step) in steps.iter().enumerate() {
            for message in step["messages"].as_array().unwrap() {
                let mut bytes = base64(message.as_str().unwrap());
                if bytes.first() == Some(&4) {
                    bytes[1] = viewer; // MSG_START names the receiving seat
                }
                // Track what MSG_CONFIRM_CARDS showed (codes, per hand owner) until MSG_SHUFFLE_HAND.
                match bytes[0] {
                    31 => {
                        let n = u32::from_le_bytes(bytes[2..6].try_into().unwrap()) as usize;
                        for k in 0..n {
                            let entry = &bytes[6 + 10 * k..16 + 10 * k];
                            if entry[5] == 0x02 {
                                confirmed[entry[4] as usize].push(u32::from_le_bytes(entry[0..4].try_into().unwrap()) as u64);
                            }
                        }
                    }
                    33 => confirmed[bytes[1] as usize].clear(),
                    _ => {}
                }
                if let Err(error) = seat.feed(&bytes) {
                    panic!("{}: step {index}: message {} rejected: {error}", path.display(), bytes[0]);
                }
            }
            let problems = compare(&seat.observation().unwrap(), &step["snapshots"][viewer as usize], viewer, &confirmed);
            if !problems.is_empty() {
                failures.push(format!("step {index} (viewer {viewer}): {}", problems.join("; ")));
            }
        }
        assert!(
            failures.is_empty(),
            "{}: {} of {} decisions differ, first ones:\n{}",
            path.display(),
            failures.len(),
            steps.len(),
            failures.iter().take(12).cloned().collect::<Vec<_>>().join("\n")
        );
    }
}

#[test]
fn projection_matches_ocgcore_snapshots() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut traces: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".json.gz"))
        .collect();
    traces.sort();
    assert!(!traces.is_empty(), "no traces in {}", dir.display());
    for trace in traces {
        replay(&trace);
    }
}

#[test]
fn all_registered_policies_fork_with_independent_history_and_random_state() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/monarch-Emperor,_Arise!-9100.json.gz");
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_string(&mut text).unwrap();
    let trace: Value = serde_json::from_str(&text).unwrap();
    let messages: Vec<Vec<u8>> = trace["steps"].as_array().unwrap().iter()
        .flat_map(|step| step["messages"].as_array().unwrap())
        .map(|message| base64(message.as_str().unwrap())).collect();
    for entry in registry::POLICIES {
        for viewer in 0..2u8 {
            let db = Arc::new(MemoryCards::default());
            let mut seat = Seat::new(registry::create_seeded(entry.id, db.clone(), 9161).unwrap(), db, Some(viewer));
            let mut checkpoint = None;
            let mut suffix = Vec::new();
            for (index, raw) in messages.iter().enumerate() {
                let mut raw = raw.clone();
                if raw[0] == 4 { raw[1] = viewer; }
                let answer = seat.feed(&raw).unwrap();
                if checkpoint.is_some() {
                    suffix.push((raw, answer));
                } else if index > messages.len() / 3 && answer.is_some() {
                    let copy = seat.fork().expect(entry.id);
                    assert_eq!(copy.observation(), seat.observation());
                    assert_eq!(copy.last_answer().unwrap().responses, seat.last_answer().unwrap().responses);
                    checkpoint = Some(copy);
                }
            }
            let checkpoint = checkpoint.expect("a decision to copy");
            assert!(!suffix.is_empty());
            // The original ran ahead; two copies of the earlier checkpoint must
            // independently reproduce its continuation, including random choices.
            for _ in 0..2 {
                let mut copy = checkpoint.fork().unwrap();
                for (raw, expected) in &suffix {
                    assert_eq!(&copy.feed(raw).unwrap(), expected, "{} viewer {viewer}", entry.id);
                }
                assert_eq!(copy.observation(), seat.observation());
            }
        }
    }
}
