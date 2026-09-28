//! Duel logs: a readable account of the duel from the bot's seat, and a raw
//! trace (every message received and every response sent) that
//! `edopro-bot --replay` feeds back into a policy to reproduce a duel.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufWriter, Write};
use std::path::Path;

use ygo_policies::model::{Choice, ChoiceKind, Decision, DecisionKind};
use ygo_policies_ocgcore::message::{parse, Message};
use ygo_policies_ocgcore::projection::phase;
use ygo_policies_ocgcore::Seat;

use crate::Observer;

/// Card names from a `cards.cdb`.
#[derive(Default)]
pub struct Names(HashMap<u32, String>);

impl Names {
    pub fn open(path: &Path) -> rusqlite::Result<Names> {
        let db = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut statement = db.prepare("select id, name from texts")?;
        let names = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)? as u32, row.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Names(names))
    }

    pub fn name(&self, code: Option<u32>) -> String {
        match code {
            None | Some(0) => "(hidden)".into(),
            Some(code) => self.0.get(&code).cloned().unwrap_or_else(|| format!("#{code}")),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(text: &str) -> Option<Vec<u8>> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

/// Describe a chosen answer.
pub fn describe(names: &Names, decision: &Decision, choice: &Choice) -> String {
    let card = || choice.card.map(|c| names.name(c.code)).unwrap_or_default();
    let members = || choice.members.iter().map(|m| names.name(m.code)).collect::<Vec<_>>().join(", ");
    let what = match choice.kind {
        ChoiceKind::NormalSummon => format!("Normal Summon {}", card()),
        ChoiceKind::SpecialSummon => format!("Special Summon {}", card()),
        ChoiceKind::ChangePosition => format!("change the position of {}", card()),
        ChoiceKind::SetMonster => format!("Set {}", card()),
        ChoiceKind::SetSpellTrap => format!("Set {}", card()),
        ChoiceKind::Activate => format!("activate {}", card()),
        ChoiceKind::Attack => format!("attack with {}", card()),
        ChoiceKind::EnterBattle => "enter the Battle Phase".into(),
        ChoiceKind::EnterMain2 => "enter Main Phase 2".into(),
        ChoiceKind::EndTurn => "end the turn".into(),
        ChoiceKind::Pass => "pass".into(),
        ChoiceKind::Yes => "yes".into(),
        ChoiceKind::No => "no".into(),
        ChoiceKind::Option => format!("option {}", choice.description),
        ChoiceKind::Position(p) => format!(
            "{} {}",
            if p.face_up { "face-up" } else { "face-down" },
            if p.attack { "Attack Position" } else { "Defense Position" }
        ),
        ChoiceKind::Cards | ChoiceKind::Toggle | ChoiceKind::Sort | ChoiceKind::Counter => {
            let listed = members();
            if listed.is_empty() { card() } else { listed }
        }
        ChoiceKind::Place => "a zone".into(),
        ChoiceKind::Announce => format!("declare {}", if choice.card.is_some() { card() } else { choice.description.to_string() }),
        other => format!("{other:?}"),
    };
    let prompt = match decision.kind {
        DecisionKind::SelectCards | DecisionKind::SelectToggle | DecisionKind::SelectSum => format!("select ({:?})", decision.hint),
        DecisionKind::Chain { .. } => "chain".into(),
        other => format!("{other:?}").to_lowercase(),
    };
    format!("{prompt}: {what}")
}

pub struct DuelLog {
    log: BufWriter<File>,
    trace: BufWriter<File>,
    names: Names,
    seat: Option<u8>,
}

impl DuelLog {
    pub fn create(stem: &Path, names: Names) -> io::Result<DuelLog> {
        Ok(DuelLog {
            log: BufWriter::new(File::create(stem.with_extension("log"))?),
            trace: BufWriter::new(File::create(stem.with_extension("trace"))?),
            names,
            seat: None,
        })
    }

    fn who(&self, player: u8) -> &'static str {
        match self.seat {
            Some(seat) if seat == player => "bot",
            Some(_) => "opponent",
            None => "?",
        }
    }

    fn line(&mut self, text: String) {
        let _ = writeln!(self.log, "{text}");
        let _ = self.log.flush();
    }
}

impl Observer for DuelLog {
    fn received(&mut self, message: &[u8]) {
        let _ = writeln!(self.trace, "< {}", hex(message));
        let Ok(parsed) = parse(message) else { return };
        let name = |code: u32| self.names.name(Some(code));
        let text = match parsed {
            Message::Start { seat, life_points, .. } => {
                self.seat = Some(seat);
                format!("duel start: the bot is player {seat} ({} LP each)", life_points[0])
            }
            Message::NewTurn { player } => format!("\n=== turn of the {} ===", self.who(player)),
            Message::NewPhase { phase: bits } => format!("-- {:?}", phase(bits).map_or("?".into(), |p| format!("{p:?}"))),
            Message::Draw { player, cards } => format!(
                "{} draws {}",
                self.who(player),
                cards.iter().map(|(code, _)| self.names.name(Some(*code))).collect::<Vec<_>>().join(", ")
            ),
            Message::Summoning { code, loc } => format!("{} Normal Summons {}", self.who(loc.controller), name(code)),
            Message::SpecialSummoning { code, loc } => format!("{} Special Summons {}", self.who(loc.controller), name(code)),
            Message::FlipSummoning { code, loc } => format!("{} Flip Summons {}", self.who(loc.controller), name(code)),
            Message::Set { loc, .. } => format!("{} Sets a card", self.who(loc.controller)),
            Message::Chaining { code, controller, .. } => format!("{} activates {}", self.who(controller), name(code)),
            Message::Attack { attacker, target } => format!(
                "{} attacks {}",
                self.who(attacker.controller),
                if target.is_some() { "a monster" } else { "directly" }
            ),
            Message::Damage { player, amount } | Message::PayLpCost { player, amount } => {
                format!("{} loses {amount} LP", self.who(player))
            }
            Message::Recover { player, amount } => format!("{} gains {amount} LP", self.who(player)),
            Message::Win { player, .. } => match player {
                2 => "=== draw ===".into(),
                p => format!("=== the {} wins ===", self.who(p)),
            },
            _ => return,
        };
        self.line(text);
    }

    fn answered(&mut self, seat: &Seat, response: &[u8]) {
        let _ = writeln!(self.trace, "> {}", hex(response));
        let _ = self.trace.flush();
        if let Some(answer) = seat.last_answer() {
            let choice = &answer.decision.choices[answer.choice];
            // Unforced windows with nothing to do are noise.
            if answer.decision.choices.len() == 1 && choice.kind == ChoiceKind::Pass {
                return;
            }
            let text = format!("   bot -> {}", describe(&self.names, &answer.decision, choice));
            self.line(text);
        }
    }

    fn note(&mut self, text: &str) {
        let _ = writeln!(self.trace, "# {text}");
        self.line(format!("[{text}]"));
    }
}

/// A recorded trace: messages received (`<`) and responses sent (`>`).
pub enum Entry {
    Received(Vec<u8>),
    Sent(Vec<u8>),
}

pub fn read_trace(path: &Path) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for line in io::BufReader::new(File::open(path)?).lines() {
        let line = line?;
        let (tag, data) = line.split_at(line.len().min(2));
        let bytes = || unhex(data).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("bad trace line {line:?}")));
        match tag {
            "< " => entries.push(Entry::Received(bytes()?)),
            "> " => entries.push(Entry::Sent(bytes()?)),
            _ => {}
        }
    }
    Ok(entries)
}
