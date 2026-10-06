//! A recorded duel, read back move by move.
//!
//! Beat Claudi-oh keeps every finished duel in SQLite with what it takes to
//! play it again: the seed, both Decks as they were loaded, and every answer
//! either player gave.  `policy-bench replay` runs that through the engine
//! and writes what happened with nothing hidden: both hands, the Set cards,
//! every activation and what it did, and what a player could have chained
//! and did not.  The policy that played can be asked again along the way:
//! the story then says where, as it is built now, it would answer otherwise.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;
use ygo_policies_ocgcore::{message::Message, wire::Loc};

use crate::engine::{Asked, Core, Recorded, Replayed, Result};

const HAND: u8 = 0x02;
const MONSTER_ZONE: u8 = 0x04;
const SPELL_TRAP_ZONE: u8 = 0x08;
const GRAVEYARD: u8 = 0x10;
const BANISHED: u8 = 0x20;
const FIELD: u8 = MONSTER_ZONE | SPELL_TRAP_ZONE;

mod reason {
    pub const DESTROY: u32 = 0x1;
    pub const RELEASE: u32 = 0x2;
    pub const MATERIAL: u32 = 0x8;
    pub const BATTLE: u32 = 0x20;
    pub const COST: u32 = 0x80;
    pub const RULE: u32 = 0x400;
    pub const DISCARD: u32 = 0x4000;
}

/// A duel to play again, and who played it.
pub struct Record {
    pub recorded: Recorded,
    pub players: [String; 2],
    pub title: String,
    /// The policy that played against the duelist and its seat, when the
    /// record says (a database row does, the bare replay does not).
    pub policy: Option<(String, u8)>,
}

/// Read a duel from Beat Claudi-oh's database (`duel`: its id, default the
/// last one), from a file holding the `replay` column's JSON, or from the
/// rows of a `policy-bench` run made with `--record true` (`duel`: the row,
/// the first being 0, default the last; `which`: the duel of that row).
pub fn load(input: &Path, duel: Option<i64>, which: Option<&str>) -> Result<Record> {
    let failed = |e: &dyn std::fmt::Display| format!("{}: {e}", input.display());
    if input.extension().is_some_and(|e| e == "jsonl") {
        return load_row(input, duel, which);
    }
    let (title, text, policy) = if input.extension().is_some_and(|e| e == "json") {
        (input.display().to_string(), std::fs::read_to_string(input).map_err(|e| failed(&e))?, None)
    } else {
        let db = rusqlite::Connection::open_with_flags(input, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| failed(&e))?;
        let columns = "id, player_deck_name, opponent_deck, result, turns, replay, opponent, player_went_first";
        let row = |row: &rusqlite::Row| {
            let (id, deck, opponent, result, turns): (i64, String, String, String, i64) = (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?);
            // Seat 0 goes first.
            let policy = (row.get::<_, String>(6)?, row.get::<_, bool>(7)? as u8);
            Ok((format!("duel {id}: {deck} against {opponent}, {turns} turns, won by the {result}"), row.get::<_, String>(5)?, Some(policy)))
        };
        match duel {
            Some(id) => db.query_row(&format!("select {columns} from duels where id = ?"), [id], row),
            None => db.query_row(&format!("select {columns} from duels order by id desc limit 1"), [], row),
        }
        .map_err(|e| failed(&e))?
    };
    let replay: Value = serde_json::from_str(&text).map_err(|e| failed(&e))?;
    let recorded = Recorded::from_json(&replay).map_err(|e| failed(&e))?;
    let name = |index: usize| replay["players"][index].as_str().map_or_else(|| format!("Player {}", index + 1), str::to_owned);
    Ok(Record { recorded, players: [name(0), name(1)], title, policy })
}

/// A duel of one row of a run made with `--record true`.  A search row holds
/// two: the pilot's own (`baseline`) and the searched one (`search`).
fn load_row(input: &Path, row: Option<i64>, which: Option<&str>) -> Result<Record> {
    let failed = |e: &dyn std::fmt::Display| format!("{}: {e}", input.display());
    let text = std::fs::read_to_string(input).map_err(|e| failed(&e))?;
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    let index = match row {
        Some(row) => usize::try_from(row).ok().filter(|row| *row < lines.len()).ok_or_else(|| failed(&format!("no row {row}: it has {} rows", lines.len())))?,
        None => lines.len().checked_sub(1).ok_or_else(|| failed(&"no rows"))?,
    };
    let row: Value = serde_json::from_str(lines[index]).map_err(|e| failed(&e))?;
    let which = match which {
        Some(which) => which,
        None => ["search", "game", "candidate"].into_iter().find(|key| row[*key].is_object()).ok_or_else(|| failed(&"no duel in this row"))?,
    };
    let played = &row[which];
    if !played["record"].is_object() {
        return Err(failed(&format!("row {index} has no record of its {which} duel: was the run made with --record true?")));
    }
    let recorded = Recorded::from_json(&played["record"]).map_err(|e| failed(&e))?;
    let name = |index: usize| played["record"]["players"][index].as_str().map_or_else(|| format!("Player {}", index + 1), str::to_owned);
    // The row's own policy and its seat: the one that searched, in a search row.
    let seat = row["seat"].as_u64().unwrap_or(0) as u8;
    let policy = row["a"].as_str().map(|policy| (policy.to_owned(), seat));
    let ending = match played["winner"].as_u64() {
        Some(winner) if winner < 2 => format!("won by {}", name(winner as usize)),
        Some(_) => "a draw".to_owned(),
        None => "stopped at the decision limit".to_owned(),
    };
    let title = format!(
        "row {index}, the {which} duel: {} against {}, seed {}, {} turns, {ending}",
        name(0),
        name(1),
        row["seed"],
        played["turns"]
    );
    Ok(Record { recorded, players: [name(0), name(1)], title, policy })
}

/// Card names, for the story.
pub fn names(cards: &Path) -> Result<HashMap<u32, String>> {
    let db = rusqlite::Connection::open_with_flags(cards, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
    let mut statement = db.prepare("select id, name from texts").map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
    rows.collect::<std::result::Result<_, _>>().map_err(|e| e.to_string())
}

/// What each player holds when a turn ends.
struct Holding {
    turn: u32,
    hand: [usize; 2],
    set: [usize; 2],
    face_up: [usize; 2],
    monsters: [usize; 2],
    life_points: [i64; 2],
}

struct Story<'a> {
    names: &'a HashMap<u32, String>,
    players: &'a [String; 2],
    life_points: [i64; 2],
    /// Cards on the field by zone: their code and position.
    field: HashMap<(u8, u8, u32), (u32, u32)>,
    hand: [Vec<u32>; 2],
    turn: u32,
    /// The effect a player is being asked about.
    question: Option<u32>,
    /// What a player is being offered to chain.
    offered: Option<(u8, Vec<u32>)>,
    /// What each player last chose not to chain, and in which turn: said once.
    declined: [Option<(u32, Vec<u32>)>; 2],
    /// Where the last such line is in the text, until something else is
    /// told: it goes if the player only waited for the chain to resolve.
    waited: Option<(std::ops::Range<usize>, u8, Vec<u32>)>,
    text: String,
    holdings: Vec<Holding>,
}

impl Story<'_> {
    fn name(&self, code: u32) -> String {
        match self.names.get(&(code & 0x7fff_ffff)) {
            Some(name) => name.clone(),
            None if code == 0 => "a card".into(),
            None => format!("card {code}"),
        }
    }

    fn at(&self, loc: &Loc) -> String {
        match self.field.get(&(loc.controller, loc.location, loc.sequence)) {
            Some((code, _)) => self.name(*code),
            None => format!("a card in {}", place(loc.location)),
        }
    }

    fn say(&mut self, line: impl AsRef<str>) {
        self.waited = None;
        let _ = writeln!(self.text, "   {}", line.as_ref());
    }

    fn zone(&self, player: u8, location: u8) -> Vec<String> {
        let mut cards: Vec<_> = self.field.iter().filter(|((p, l, _), _)| *p == player && *l == location).collect();
        cards.sort_by_key(|((_, _, sequence), _)| *sequence);
        cards
            .into_iter()
            .map(|(_, (code, position))| {
                let face_down = position & 0x5 == 0;
                let state = match (location, face_down, position & 0x1 != 0) {
                    (_, true, _) => " (Set)",
                    (MONSTER_ZONE, _, true) => " (ATK)",
                    (MONSTER_ZONE, _, false) => " (DEF)",
                    _ => "",
                };
                format!("{}{state}", self.name(*code))
            })
            .collect()
    }

    fn board(&self, player: u8) -> String {
        let list = |cards: Vec<String>| if cards.is_empty() { "-".into() } else { cards.join(", ") };
        format!(
            "{} ({} LP): hand [{}]; monsters [{}]; Spells/Traps [{}]",
            self.players[player as usize],
            self.life_points[player as usize],
            list(self.hand[player as usize].iter().map(|c| self.name(*c)).collect()),
            list(self.zone(player, MONSTER_ZONE)),
            list(self.zone(player, SPELL_TRAP_ZONE)),
        )
    }

    fn hold(&mut self) {
        let count = |player: u8, location: u8, face_down: Option<bool>| {
            self.field.iter().filter(|((p, l, _), (_, position))| *p == player && *l == location && face_down.map_or(true, |f| (position & 0x5 == 0) == f)).count()
        };
        let each = |f: &dyn Fn(u8) -> usize| [f(0), f(1)];
        self.holdings.push(Holding {
            turn: self.turn,
            hand: each(&|p| self.hand[p as usize].len()),
            set: each(&|p| count(p, SPELL_TRAP_ZONE, Some(true))),
            face_up: each(&|p| count(p, SPELL_TRAP_ZONE, Some(false))),
            monsters: each(&|p| count(p, MONSTER_ZONE, None)),
            life_points: self.life_points,
        });
    }

    fn moved(&mut self, code: u32, from: &Loc, to: &Loc, why: u32) {
        if from.location & FIELD != 0 {
            self.field.remove(&(from.controller, from.location, from.sequence));
        }
        if from.location == HAND {
            let hand = &mut self.hand[from.controller as usize];
            if let Some(index) = hand.iter().position(|c| *c == code) {
                hand.remove(index);
            }
        }
        if to.location & FIELD != 0 {
            self.field.insert((to.controller, to.location, to.sequence), (code, to.position));
        }
        if to.location == HAND {
            self.hand[to.controller as usize].push(code);
        }
        // Summons, Sets and activations tell of what reaches the field.
        if to.location & FIELD != 0 || from.location == 0 {
            return;
        }
        let owner = &self.players[from.controller as usize];
        let name = self.name(code);
        let line = if from.location & FIELD != 0 {
            let fate = if why & reason::DESTROY != 0 && why & reason::BATTLE != 0 {
                "is destroyed by battle".into()
            } else if why & reason::DESTROY != 0 {
                "is destroyed".into()
            } else if why & reason::RELEASE != 0 {
                "is Tributed".into()
            } else if why & reason::MATERIAL != 0 {
                "is used as material".into()
            } else if to.location == GRAVEYARD && why & reason::RULE != 0 && from.location == SPELL_TRAP_ZONE {
                // A Spell or Trap that has done its work.
                return;
            } else if to.location == GRAVEYARD && why & reason::COST != 0 {
                "is sent to the Graveyard as a cost".into()
            } else {
                format!("goes to {}", place(to.location))
            };
            format!("{owner}'s {name} {fate}")
        } else if from.location == HAND && to.location == GRAVEYARD {
            format!("{owner} {} {name}", if why & reason::DISCARD != 0 { "discards" } else { "sends from the hand to the Graveyard" })
        } else {
            format!("{owner}'s {name}: {} to {}", place(from.location), place(to.location))
        };
        self.say(line);
    }

    fn see(&mut self, message: &Message) {
        use Message::*;
        match message {
            NewTurn { player } => {
                if self.turn > 0 {
                    self.hold();
                }
                self.turn += 1;
                let _ = writeln!(self.text, "\n== Turn {}: {} ==", self.turn, self.players[*player as usize]);
                for p in [*player, 1 - *player] {
                    let board = self.board(p);
                    self.say(board);
                }
            }
            NewPhase { phase } => {
                let name = match phase {
                    0x02 => "Standby Phase",
                    0x04 => "Main Phase 1",
                    0x08 => "Battle Phase",
                    0x100 => "Main Phase 2",
                    0x200 => "End Phase",
                    _ => return,
                };
                let _ = writeln!(self.text, " - {name}");
            }
            Draw { player, cards } => {
                let drawn: Vec<u32> = cards.iter().map(|(code, _)| code & 0x7fff_ffff).collect();
                let names: Vec<String> = drawn.iter().map(|c| self.name(*c)).collect();
                self.hand[*player as usize].extend(drawn);
                self.say(format!("{} draws {}", self.players[*player as usize], names.join(", ")));
            }
            ShuffleHand { player, codes } if !codes.is_empty() && codes.iter().all(|c| *c != 0) => {
                self.hand[*player as usize] = codes.clone();
            }
            Move { code, from, to, reason } => self.moved(*code, from, to, *reason),
            PosChange { code, controller, location, sequence, current, .. } => {
                if let Some(card) = self.field.get_mut(&(*controller, *location, *sequence as u32)) {
                    card.1 = *current as u32;
                }
                // A Spell or Trap turned face-up: its activation follows.
                if *location != MONSTER_ZONE {
                    return;
                }
                let to = match current {
                    0x1 => "Attack Position",
                    0x4 => "Defense Position",
                    _ => "face-down Defense Position",
                };
                self.say(format!("{}'s {} changes to {to}", self.players[*controller as usize], self.name(*code)));
            }
            Set { loc, .. } => {
                if let Some(card) = self.field.get_mut(&(loc.controller, loc.location, loc.sequence)) {
                    card.1 = loc.position;
                }
                self.say(format!("{} Sets {}", self.players[loc.controller as usize], self.at(loc)));
            }
            Summoning { code, loc } => self.say(format!("{} Normal Summons {}", self.players[loc.controller as usize], self.name(*code))),
            SpecialSummoning { code, loc } => self.say(format!("{} Special Summons {}", self.players[loc.controller as usize], self.name(*code))),
            FlipSummoning { code, loc } => {
                if let Some(card) = self.field.get_mut(&(loc.controller, loc.location, loc.sequence)) {
                    card.1 = 0x1;
                }
                self.say(format!("{} Flip Summons {}", self.players[loc.controller as usize], self.name(*code)));
            }
            Chaining { code, loc, controller, size, .. } => {
                // Passed on a moment ago, activated now: no chance was let go.
                if let Some((line, player, codes)) = self.waited.take() {
                    if player == *controller && codes.contains(code) {
                        self.text.replace_range(line, "");
                        self.declined[player as usize] = None;
                    }
                }
                let link = if *size > 1 { format!(" (chain link {size})") } else { String::new() };
                self.say(format!("{} activates {} [{}]{link}", self.players[*controller as usize], self.name(*code), place(loc.location)));
            }
            BecomeTarget { cards } => {
                let targets: Vec<String> = cards.iter().map(|loc| format!("{}'s {}", self.players[loc.controller as usize], self.at(loc))).collect();
                self.say(format!("    targeting {}", targets.join(", ")));
            }
            ChainNegated { link } => self.say(format!("    chain link {link} is negated")),
            Attack { attacker, target } => {
                let target = target.map_or_else(|| "directly".into(), |loc| self.at(&loc));
                self.say(format!("{}'s {} attacks {target}", self.players[attacker.controller as usize], self.at(attacker)));
            }
            AttackDisabled => self.say("    the attack is negated"),
            Damage { player, amount } => {
                self.life_points[*player as usize] -= *amount as i64;
                self.say(format!("    {} takes {amount} damage: {} LP", self.players[*player as usize], self.life_points[*player as usize]));
            }
            Recover { player, amount } => {
                self.life_points[*player as usize] += *amount as i64;
                self.say(format!("    {} gains {amount}: {} LP", self.players[*player as usize], self.life_points[*player as usize]));
            }
            PayLpCost { player, amount } => {
                self.life_points[*player as usize] -= *amount as i64;
                self.say(format!("    {} pays {amount}: {} LP", self.players[*player as usize], self.life_points[*player as usize]));
            }
            LpUpdate { player, life_points } => self.life_points[*player as usize] = *life_points as i64,
            SelectEffectYesNo { code, .. } => self.question = Some(*code),
            SelectChain { player, chains, .. } if !chains.is_empty() => {
                let mut codes: Vec<u32> = chains.iter().map(|effect| effect.code).collect();
                codes.sort_unstable();
                codes.dedup();
                self.offered = Some((*player, codes));
            }
            Retry => self.say("    (the engine rejects that answer and asks again)"),
            Win { player, reason } => {
                self.hold();
                let how = match reason {
                    1 => "Life Points".into(),
                    2 => "the other Deck ran out".into(),
                    4 => "surrender".into(),
                    other => format!("a card's own win condition, reason {other}"),
                };
                let winner = self.players.get(*player as usize).map_or("Nobody", |p| p.as_str());
                let _ = writeln!(self.text, "\n{winner} wins: {how}.");
            }
            _ => {}
        }
        if !matches!(message, SelectEffectYesNo { .. }) && message.responder().is_some() {
            self.question = None;
        }
        if !matches!(message, SelectChain { .. }) && message.responder().is_some() {
            self.offered = None;
        }
    }

    fn answered(&mut self, player: u8, bytes: &[u8]) {
        if let Some(code) = self.question.take() {
            let yes = bytes.first().is_some_and(|b| *b != 0);
            self.say(format!("{}: use {}? {}", self.players[player as usize], self.name(code), if yes { "yes" } else { "no" }));
        }
        // A chain passed on: what could have been activated.
        if let Some((who, codes)) = self.offered.take() {
            let said = Some((self.turn, codes));
            if who == player && bytes == (-1i32).to_le_bytes() && self.declined[player as usize] != said {
                let names: Vec<String> = said.iter().flat_map(|(_, codes)| codes).map(|c| self.name(*c)).collect();
                let start = self.text.len();
                self.say(format!("({} could activate {} and does not)", self.players[player as usize], names.join(", ")));
                self.waited = said.clone().map(|(_, codes)| (start..self.text.len(), player, codes));
                self.declined[player as usize] = said;
            }
        }
    }

    /// A choice of a policy's decision, in words.
    fn choice(&self, choice: &Value) -> String {
        let name = |card: &Value| card["code"].as_u64().map_or_else(|| "a card".into(), |code| self.name(code as u32));
        let card = name(&choice["card"]);
        match choice["kind"].as_str() {
            Some("Pass") => "pass".into(),
            Some("Activate") => format!("activate {card}"),
            Some("NormalSummon") => format!("Normal Summon {card}"),
            Some("SpecialSummon") => format!("Special Summon {card}"),
            Some("SetMonster" | "SetSpellTrap") => format!("Set {card}"),
            Some("ChangePosition") => format!("change the position of {card}"),
            Some("Attack") => format!("attack with {card}"),
            Some("EnterBattle") => "enter the Battle Phase".into(),
            Some("EnterMain2") => "go to Main Phase 2".into(),
            Some("EndTurn") => "end the turn".into(),
            Some("Yes") => "say yes".into(),
            Some("No") => "say no".into(),
            Some("Cancel") => "pick nothing".into(),
            Some("Cards") => {
                let picked: Vec<String> = choice["members"].as_array().into_iter().flatten().map(name).collect();
                format!("pick {}", picked.join(", "))
            }
            Some("Option") => format!("take option {}", choice["description"]),
            Some(other) => other.to_lowercase(),
            None => match (choice["kind"]["Position"]["face_up"].as_bool(), choice["kind"]["Position"]["attack"].as_bool()) {
                (Some(true), Some(true)) => "choose Attack Position".into(),
                (Some(true), Some(false)) => "choose Defense Position".into(),
                (Some(false), _) => "choose face-down Defense Position".into(),
                _ => choice["kind"].to_string(),
            },
        }
    }

    /// The policy asked alongside would not give the recorded answer.
    fn otherwise(&mut self, own: &Value, recorded: &[u8]) {
        // Which zone, and in what order: no difference worth telling.
        if matches!(own["decision"]["kind"].as_str(), Some("Place" | "Sort")) {
            return;
        }
        let choices = &own["decision"]["choices"];
        let now = self.choice(&choices[own["choice"].as_u64().unwrap_or(0) as usize]);
        let bytes = |response: &Value| -> Vec<u8> { response.as_array().into_iter().flatten().filter_map(|b| Some(b.as_u64()? as u8)).collect() };
        let then = own["responses"].as_array().and_then(|all| all.iter().position(|r| bytes(r) == recorded)).map(|index| self.choice(&choices[index]));
        let player = &self.players[own["observation"]["me"].as_u64().unwrap_or(0) as usize];
        match then {
            // Another copy of the same card.
            Some(then) if then == now => {}
            Some(then) => self.say(format!(">> as built now, {player} would {now} here, not {then}")),
            None => self.say(format!(">> as built now, {player} would {now} here")),
        }
    }

    fn holdings(&self) -> String {
        let mut table = format!("\nCards when each turn ends (hand / Set / face-up Spells and Traps / monsters / LP):\n turn  {:<28} {}\n", self.players[0], self.players[1]);
        for h in &self.holdings {
            let cell = |p: usize| format!("{} / {} / {} / {} / {}", h.hand[p], h.set[p], h.face_up[p], h.monsters[p], h.life_points[p]);
            let _ = writeln!(table, " {:>4}  {:<28} {}", h.turn, cell(0), cell(1));
        }
        table
    }
}

fn place(location: u8) -> &'static str {
    match location {
        0x01 => "the Deck",
        HAND => "the hand",
        MONSTER_ZONE => "the Monster Zone",
        SPELL_TRAP_ZONE => "the Spell & Trap Zone",
        GRAVEYARD => "the Graveyard",
        BANISHED => "the banished pile",
        0x40 => "the Extra Deck",
        _ => "elsewhere",
    }
}

/// Play the record again and tell it; with a policy `asked` alongside, also
/// where it would answer otherwise (lines starting with `>>`).
pub fn narrate(core: &mut Core, record: &Record, names: &HashMap<u32, String>, asked: Option<&Asked>) -> Result<String> {
    let mut story = Story {
        names,
        players: &record.players,
        life_points: record.recorded.life_points.map(i64::from),
        field: HashMap::new(),
        hand: [Vec::new(), Vec::new()],
        turn: 0,
        question: None,
        offered: None,
        declined: [None, None],
        waited: None,
        text: format!("{}\n", record.title),
        holdings: Vec::new(),
    };
    let unused = core.replay(&record.recorded, asked, &mut |step| match step {
        Replayed::Message(message) => story.see(message),
        Replayed::Answer(player, bytes) => story.answered(player, bytes),
        Replayed::Otherwise(own, recorded) => story.otherwise(own, recorded),
    })?;
    if unused > 0 {
        let _ = writeln!(story.text, "({unused} recorded answers were not needed.)");
    }
    let holdings = story.holdings();
    Ok(story.text + &holdings)
}
