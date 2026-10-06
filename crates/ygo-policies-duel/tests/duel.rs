//! Duels stepped through the public API, on the vendored engine, cards,
//! scripts and Decks, with two policies seated in process.
use std::{path::PathBuf, sync::Arc};

use ygo_policies::{cards::CardDatabase, registry};
use ygo_policies_duel::{Core, Deck, Duel, DuelOptions, Outcome, Recorded, Replayed, State};
use ygo_policies_ocgcore::{Message, Seat, SqliteCards};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn core() -> Core {
    let root = root();
    Core::open(None, &root.join("vendor/BabelCdb/cards.cdb"), &root.join("vendor/CardScripts")).unwrap()
}

fn decks() -> [Deck; 2] {
    let deck = |name: &str| Deck::load(&root().join("decks").join(name)).unwrap();
    [deck("Blackwing Assassin.ydk"), deck("Emperor, Arise!.ydk")]
}

/// Blackwing in seat 0 and Monarch in seat 1, each fed the start of `duel`.
fn seats(duel: &Duel, seed: u64) -> [Seat; 2] {
    let db: Arc<dyn CardDatabase> = Arc::new(SqliteCards::open(root().join("vendor/BabelCdb/cards.cdb")).unwrap());
    let mut seats = ["blackwing", "monarch"].map(|id| Seat::new(registry::create_seeded(id, db.clone(), seed).unwrap(), db.clone(), None));
    for (player, seat) in seats.iter_mut().enumerate() {
        seat.feed(&duel.start_message(player as u8)).unwrap();
    }
    seats
}

/// Step `duel` to its next selection, or its end: the answer and who gave it.
fn advance(duel: &mut Duel, seats: &mut [Seat; 2]) -> Result<(u8, Vec<u8>), Option<Outcome>> {
    loop {
        let step = duel.step().unwrap();
        let mut response = None;
        for sent in &step.messages {
            for (player, seat) in seats.iter_mut().enumerate() {
                if let Some(answer) = seat.feed(&sent.bytes).unwrap() {
                    response = Some((player as u8, answer));
                }
            }
        }
        match step.state {
            State::Over(outcome) => return Err(outcome),
            State::Awaiting => return Ok(response.expect("a seat answers every selection")),
            State::Running => {}
        }
    }
}

#[test]
fn a_stepped_duel_ends_and_its_record_plays_it_again() {
    let core = core();
    let decks = decks();
    let options = DuelOptions::seeded(7);
    let mut duel = core.deal(&options, &decks).unwrap();
    let mut seats = seats(&duel, 7);
    let mut record = Recorded::dealt(&options, &decks);
    let outcome = loop {
        match advance(&mut duel, &mut seats) {
            Ok((player, answer)) => {
                duel.respond(&answer);
                record.responses.push((player, answer));
                assert!(record.responses.len() < 4096, "the duel does not end");
            }
            Err(outcome) => break outcome.expect("the engine names a winner"),
        }
    };
    assert!(outcome.winner <= 2);
    assert!(!record.responses.is_empty());

    // The record, through its JSON form, plays the same duel again.
    let read = Recorded::from_json(&record.to_json()).unwrap();
    assert_eq!(read, record);
    let (mut won, mut answers) = (None, 0);
    let left = core
        .replay(&read, None, &mut |shown| match shown {
            Replayed::Message(Message::Win { player, .. }) => won = Some(*player),
            Replayed::Answer(..) => answers += 1,
            _ => {}
        })
        .unwrap();
    assert_eq!(left, 0);
    assert_eq!(answers, record.responses.len());
    assert_eq!(won, Some(outcome.winner));
}

#[test]
fn a_snapshot_brings_the_duel_back_and_a_swap_changes_it() {
    let core = core();
    assert!(core.has_snapshots());
    let decks = decks();
    let mut duel = core.deal(&DuelOptions::seeded(11), &decks).unwrap();
    let mut seats = seats(&duel, 11);
    // A few decisions in, at a selection.
    let mut answer = Vec::new();
    for _ in 0..6 {
        if !answer.is_empty() {
            duel.respond(&answer);
        }
        answer = advance(&mut duel, &mut seats).expect("the duel is not over yet").1;
    }
    let snapshot = duel.snapshot().unwrap();
    let field = duel.field();
    duel.respond(&answer);
    let first = duel.step().unwrap();

    // Back at the selection: the same answer brings the same messages.
    duel.restore(&snapshot).unwrap();
    assert_eq!(duel.field(), field);
    duel.respond(&answer);
    let second = duel.step().unwrap();
    assert_eq!(first.messages, second.messages);
    assert_eq!(first.state, second.state);

    // Two cards of seat 1's Deck change places; the same slot twice is refused.
    duel.restore(&snapshot).unwrap();
    const DECK: u32 = 0x01;
    let size = duel.count(1, DECK).unwrap();
    assert!(size >= 2);
    let code = |duel: &Duel, sequence: u32| duel.card(0x1, 1, DECK, sequence).unwrap();
    let (top, below) = (code(&duel, size - 1), code(&duel, size - 2));
    assert!(!duel.swap(1, (DECK, size - 1), (DECK, size - 1)).unwrap());
    if top != below {
        assert!(duel.swap(1, (DECK, size - 1), (DECK, size - 2)).unwrap());
        assert_eq!(code(&duel, size - 1), below);
        assert_eq!(code(&duel, size - 2), top);
    }
    // And the snapshot still brings back the duel as it was.
    duel.restore(&snapshot).unwrap();
    assert_eq!(code(&duel, size - 1), top);
    assert_eq!(duel.field(), field);
}
