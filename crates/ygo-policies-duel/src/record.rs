//! A duel as it was played, and playing it again.
use std::path::Path;

use serde_json::{json, Value};
use ygo_policies_ocgcore::message::{self, Message};

use crate::core::Core;
use crate::deck::Deck;
use crate::duel::{Duel, DuelOptions, Placed, State};
use crate::policy::PolicyLibrary;
use crate::Result;

/// A duel as it was played: all it takes to play it again.  Beat Claudi-oh
/// keeps its duels this way, and `policy-bench search --record true` writes
/// the duels it plays in the same form ([`Recorded::to_json`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
    /// The engine's random seed.
    pub seed: [u64; 4],
    /// OCGCore's duel flags.
    pub flags: u64,
    /// Starting Life Points of seat 0, who went first, and of seat 1.
    pub life_points: [u32; 2],
    /// Cards in the opening hand.
    pub hand: u32,
    /// Cards drawn each turn.
    pub per_turn: u32,
    /// Each player's Deck in the order it was loaded, already shuffled: the
    /// engine is given the last card first.
    pub decks: [Deck; 2],
    /// Who answered each of the engine's questions, and with which bytes.
    pub responses: Vec<(u8, Vec<u8>)>,
}

/// What a replayed duel shows its watcher.
pub enum Replayed<'a> {
    /// Every engine message, in order, as the engine wrote it: nothing hidden.
    Message(&'a Message),
    /// The recorded answer to the selection just shown, and who gave it.
    Answer(u8, &'a [u8]),
    /// Before a recorded answer that the policy asked alongside
    /// ([`Asked`]) would not give: its own (as `policy-bench --trace`
    /// writes a decision), and the recorded one.
    Otherwise(&'a Value, &'a [u8]),
}

/// A policy to ask, at every decision one seat took in a recorded duel,
/// what it would answer there.
pub struct Asked<'a> {
    /// The library the policy is in.
    pub library: &'a PolicyLibrary,
    /// Its id.
    pub policy: &'a str,
    /// The card database it reads.
    pub cards: &'a Path,
    /// The seat it is asked for.
    pub seat: u8,
}

const DECK: u32 = 0x01;
const EXTRA: u32 = 0x40;
const FACE_DOWN_DEFENSE: u32 = 0x8;

impl Recorded {
    /// The options the recorded duel was started with.
    pub fn options(&self) -> DuelOptions {
        DuelOptions {
            seed: self.seed,
            flags: self.flags,
            life_points: self.life_points,
            hand: self.hand,
            per_turn: self.per_turn,
            shuffle: None,
        }
    }

    /// The recorded duel on `core`, started and not yet stepped: step it and
    /// give it [`Recorded::responses`] in order to play it again
    /// ([`Core::replay`] does).
    pub fn duel<'a>(&self, core: &'a Core) -> Result<Duel<'a>> {
        if let Some(code) = core.missing(self.decks.iter().flat_map(|d| d.main.iter().chain(&d.extra))) {
            return Err(format!("Deck card {code} is absent from the database"));
        }
        let mut duel = core.duel(&self.options())?;
        for (p, deck) in self.decks.iter().enumerate() {
            for (location, pile) in [(DECK, &deck.main), (EXTRA, &deck.extra)] {
                for code in pile.iter().rev() {
                    duel.add(Placed { controller: p as u8, code: *code, location, sequence: 0, position: FACE_DOWN_DEFENSE });
                }
            }
        }
        duel.start();
        Ok(duel)
    }

    /// The record of a duel [`Core::deal`] started from `decks` with
    /// `options`, before any answer: `responses` is empty.
    pub fn dealt(options: &DuelOptions, decks: &[Deck; 2]) -> Self {
        let mut rng = options.shuffle;
        let dealt = |deck: &Deck, rng: &mut Option<u64>| {
            let mut main = deck.main.clone();
            if let Some(state) = rng {
                crate::deck::shuffle(&mut main, state);
            }
            // `deal` gives the engine the first card first, a record the last.
            main.reverse();
            Deck { main, extra: deck.extra.iter().rev().copied().collect() }
        };
        let first = dealt(&decks[0], &mut rng);
        let second = dealt(&decks[1], &mut rng);
        Self {
            seed: options.seed,
            flags: options.flags,
            life_points: options.life_points,
            hand: options.hand,
            per_turn: options.per_turn,
            decks: [first, second],
            responses: Vec::new(),
        }
    }

    /// Read a record from its JSON form: `seed` (four numbers), `flags`,
    /// `start_lp` (one number, or one for each player), `start_hand`,
    /// `draw_count`, `decks` (`main` and `extra` of each player) and
    /// `responses` (each `[player, "bytes in hex"]`).  Numbers may be
    /// written as strings.
    pub fn from_json(replay: &Value) -> Result<Self> {
        let number = |value: &Value| value.as_u64().or_else(|| value.as_str()?.parse().ok());
        let field = |key: &str| number(&replay[key]).ok_or_else(|| format!("no {key} in the replay"));
        let seed: Vec<u64> = replay["seed"].as_array().map(|a| a.iter().filter_map(number).collect()).unwrap_or_default();
        let seed: [u64; 4] = seed.try_into().map_err(|_| "the replay's seed is not four numbers".to_owned())?;
        let codes = |value: &Value| -> Vec<u32> { value.as_array().map(|a| a.iter().filter_map(|c| Some(c.as_u64()? as u32)).collect()).unwrap_or_default() };
        let deck = |index: usize| Deck { main: codes(&replay["decks"][index]["main"]), extra: codes(&replay["decks"][index]["extra"]) };
        let mut responses = Vec::new();
        for answer in replay["responses"].as_array().ok_or("no responses in the replay")? {
            let player = answer[0].as_u64().ok_or("a response without its player")? as u8;
            let hex = answer[1].as_str().ok_or("a response without its bytes")?;
            let bytes = (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)).collect::<std::result::Result<Vec<u8>, _>>();
            responses.push((player, bytes.map_err(|e| e.to_string())?));
        }
        let life_points = match replay["start_lp"].as_array().map(|both| both.iter().filter_map(number).collect::<Vec<u64>>()) {
            Some(both) if both.len() == 2 => [both[0] as u32, both[1] as u32],
            _ => [field("start_lp")? as u32; 2],
        };
        Ok(Self {
            seed,
            flags: field("flags")?,
            life_points,
            hand: field("start_hand")? as u32,
            per_turn: field("draw_count")? as u32,
            decks: [deck(0), deck(1)],
            responses,
        })
    }

    /// The JSON form [`Recorded::from_json`] reads.  `start_lp` is one
    /// number when both players start with the same Life Points.
    pub fn to_json(&self) -> Value {
        let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let start_lp = if self.life_points[0] == self.life_points[1] { json!(self.life_points[0]) } else { json!(self.life_points) };
        json!({
            "seed": self.seed,
            "flags": self.flags,
            "start_lp": start_lp,
            "start_hand": self.hand,
            "draw_count": self.per_turn,
            "decks": self.decks.iter().map(|deck| json!({"main": deck.main, "extra": deck.extra})).collect::<Vec<_>>(),
            "responses": self.responses.iter().map(|(player, bytes)| json!([player, hex(bytes)])).collect::<Vec<_>>(),
        })
    }
}

impl Core {
    /// Play a recorded duel again: its seed and Decks, and the recorded
    /// answers in order.  `watch` sees every message and every answer.
    /// A policy can be `asked` alongside: it is fed the duel as its seat saw
    /// it and answers each of that seat's decisions, the duel going on with
    /// the recorded answer whatever it says.
    /// Returns how many recorded answers were left when the duel ended.
    pub fn replay(&self, record: &Recorded, asked: Option<&Asked>, watch: &mut dyn FnMut(Replayed)) -> Result<usize> {
        let mut duel = record.duel(self)?;
        let seat = match asked {
            Some(asked) => {
                let seat = asked.library.seat(asked.policy, asked.cards, asked.seat as i32, record.seed[0])?;
                seat.feed(&duel.start_message(asked.seat))?;
                Some(seat)
            }
            None => None,
        };
        let mut answers = record.responses.iter();
        let mut responder = None;
        loop {
            let step = duel.step()?;
            let mut own = None;
            for sent in &step.messages {
                if sent.refresh {
                    if let Some(seat) = &seat {
                        seat.feed(&sent.bytes)?;
                    }
                    continue;
                }
                let parsed = message::parse(&sent.bytes).map_err(|e| e.to_string())?;
                // A rejected answer: the same player is asked again.
                if let Some(player) = parsed.responder() {
                    responder = Some(player);
                }
                if let Some(seat) = &seat {
                    if let Some(answer) = seat.feed(&sent.bytes)? {
                        own = Some(answer);
                    }
                }
                watch(Replayed::Message(&parsed));
            }
            match step.state {
                State::Over(_) => return Ok(answers.len()),
                State::Running => {}
                State::Awaiting => {
                    let (player, answer) = answers.next().ok_or("The record ends before the duel does")?;
                    if responder.is_some_and(|p| p != *player) {
                        return Err(format!("The record has player {player} answering a question for player {}: not the same duel", responder.unwrap()));
                    }
                    if let (Some(seat), Some(own)) = (&seat, own) {
                        if own != *answer {
                            watch(Replayed::Otherwise(&seat.last_answer()?, answer));
                        }
                    }
                    watch(Replayed::Answer(*player, answer));
                    duel.respond(answer);
                }
            }
        }
    }
}
