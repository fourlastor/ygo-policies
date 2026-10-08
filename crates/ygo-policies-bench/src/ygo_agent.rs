//! The released model's HTTP seat. It translates the engine's actual prompts,
//! rather than the policy's tactical choices, and records every fallback.
use crate::engine::{Core, Deck, DuelOptions, PlayOptions, PolicyLibrary, Recorded, Result};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::Path,
    sync::Arc,
    time::Duration,
};
use ygo_policies::{
    cards::CardDatabase,
    model::{ChoiceKind, Location, Observation, Phase},
};
use ygo_policies_duel::{Duel, LibrarySeat, State};
use ygo_policies_ocgcore::{
    decision::{self, Prompt, Step},
    message::{self, Message, Offered},
    query::{read_location, RawRecord},
    wire::{location, query, Loc},
    Seat, SqliteCards,
};

struct Http {
    address: String,
    prefix: String,
}
impl Http {
    fn new(url: &str) -> Result<Self> {
        let url = url
            .strip_prefix("http://")
            .ok_or("the kit server uses http://HOST:PORT")?;
        let (address, prefix) = url.split_once('/').unwrap_or((url, ""));
        if address.is_empty() || address.contains(['\r', '\n']) || prefix.contains(['\r', '\n']) {
            return Err("invalid server URL".into());
        }
        Ok(Self {
            address: if address.contains(':') {
                address.into()
            } else {
                format!("{address}:80")
            },
            prefix: if prefix.is_empty() {
                String::new()
            } else {
                format!("/{}", prefix.trim_end_matches('/'))
            },
        })
    }
    fn request(&self, method: &str, path: &str, body: &Value) -> Result<Value> {
        let addr = self
            .address
            .to_socket_addrs()
            .map_err(|e| e.to_string())?
            .next()
            .ok_or("no server address")?;
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(30))
            .map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| e.to_string())?;
        let body = body.to_string();
        write!(stream, "{method} {}{path} HTTP/1.0\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.prefix, self.address, body.len()).map_err(|e| e.to_string())?;
        let mut response = Vec::new();
        stream
            .take(16 * 1024 * 1024)
            .read_to_end(&mut response)
            .map_err(|e| e.to_string())?;
        let split = response
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or("malformed HTTP response")?;
        let header = String::from_utf8_lossy(&response[..split]);
        let status = header
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("");
        if status == "204" {
            return Ok(Value::Null);
        }
        let answer: Value = serde_json::from_slice(&response[split + 4..])
            .map_err(|e| format!("HTTP {status}: {e}"))?;
        if status != "200" {
            return Err(format!("HTTP {status}: {answer}"));
        }
        Ok(answer)
    }
}

fn controller(owner: u8, me: u8) -> &'static str {
    if owner == me {
        "me"
    } else {
        "opponent"
    }
}
fn location_name(loc: u8) -> &'static str {
    match loc & 0x7f {
        1 => "deck",
        2 => "hand",
        4 => "mzone",
        8 => "szone",
        16 => "grave",
        32 => "removed",
        64 => "extra",
        _ => "deck",
    }
}
fn model_location(loc: Location) -> u8 {
    match loc {
        Location::Deck => 1,
        Location::Hand => 2,
        Location::MonsterZone => 4,
        Location::SpellTrapZone => 8,
        Location::Graveyard => 16,
        Location::Banished => 32,
        Location::Extra => 64,
        _ => 0,
    }
}
fn at(loc: Loc, me: u8) -> Value {
    json!({"controller": controller(loc.controller, me), "location": location_name(loc.location),
        "sequence": loc.sequence, "overlay_sequence": if loc.location & 0x80 != 0 { loc.position as i64 } else { -1 }})
}
fn description(value: u64) -> u64 {
    if value < 10000 {
        value
    } else {
        (value >> 20) * 16 + (value & 0xfffff)
    }
}
fn field_u32(record: &RawRecord, flag: u32) -> u32 {
    record
        .get(flag)
        .filter(|b| b.len() >= 4)
        .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))
        .unwrap_or(0)
}
fn attribute(value: u32) -> &'static str {
    match value {
        1 => "earth",
        2 => "water",
        4 => "fire",
        8 => "wind",
        16 => "light",
        32 => "dark",
        64 => "divine",
        _ => "none",
    }
}
const RACES: &[&str] = &[
    "warrior",
    "spellcaster",
    "fairy",
    "fiend",
    "zombie",
    "machine",
    "aqua",
    "pyro",
    "rock",
    "windbeast",
    "plant",
    "insect",
    "thunder",
    "dragon",
    "beast",
    "beast_warrior",
    "dinosaur",
    "fish",
    "sea_serpent",
    "reptile",
    "psycho",
    "devine",
    "creator_god",
    "wyrm",
    "cyberse",
    "illusion",
];
const TYPES: &[(u32, &str)] = &[
    (1, "monster"),
    (2, "spell"),
    (4, "trap"),
    (0x10, "normal"),
    (0x20, "effect"),
    (0x40, "fusion"),
    (0x80, "ritual"),
    (0x100, "trap_monster"),
    (0x200, "spirit"),
    (0x400, "union"),
    (0x800, "dual"),
    (0x1000, "tuner"),
    (0x2000, "synchro"),
    (0x4000, "token"),
    (0x10000, "quick_play"),
    (0x20000, "continuous"),
    (0x40000, "equip"),
    (0x80000, "field"),
    (0x100000, "counter"),
    (0x200000, "flip"),
    (0x400000, "toon"),
    (0x800000, "xyz"),
    (0x1000000, "pendulum"),
    (0x2000000, "special"),
    (0x4000000, "link"),
];
fn position(value: u32, kind: u32) -> &'static str {
    if kind & 6 != 0 {
        return if value & 5 != 0 { "faceup" } else { "facedown" };
    }
    match value {
        1 => "faceup_attack",
        2 => "facedown_attack",
        4 => "faceup_defense",
        8 => "facedown_defense",
        5 => "faceup",
        10 => "facedown",
        _ => "none",
    }
}

fn card(record: &RawRecord, owner: u8, loc: u8, sequence: usize, me: u8) -> Value {
    card_visible(record, owner, loc, sequence, me, false)
}
fn card_visible(
    record: &RawRecord,
    owner: u8,
    loc: u8,
    sequence: usize,
    me: u8,
    revealed: bool,
) -> Value {
    let pos = record.position().unwrap_or(0);
    let known = owner == me
        || revealed
        || ygo_policies_ocgcore::redact::visible(record, owner, loc, Some(me));
    let code = if known {
        field_u32(record, query::CODE)
    } else {
        0
    };
    let kind = if known {
        field_u32(record, query::TYPE)
    } else {
        0
    };
    let race = if known {
        field_u32(record, query::RACE)
    } else {
        0
    };
    let pile = loc == location::DECK || loc == location::HAND || loc == location::EXTRA;
    let face = if pile && !known {
        "none"
    } else if pile && owner == me {
        "facedown"
    } else {
        position(pos, kind)
    };
    json!({"code": code, "controller": controller(owner, me), "location": location_name(loc),
        "sequence": sequence, "overlay_sequence": -1, "position": face,
        "attribute": attribute(if known { field_u32(record, query::ATTRIBUTE) } else { 0 }),
        "race": if race.is_power_of_two() { RACES.get(race.trailing_zeros() as usize).copied().unwrap_or("none") } else { "none" },
        "level": if !known { 0 } else if kind & 0x4000000 != 0 { field_u32(record, query::LINK) }
            else if kind & 0x800000 != 0 { field_u32(record, query::RANK) } else { field_u32(record, query::LEVEL) },
        "attack": if known { field_u32(record, query::ATTACK) as i32 } else { 0 },
        "defense": if kind & 0x4000000 != 0 {
            record.get(query::LINK).filter(|b| b.len()>=8)
                .map(|b|u32::from_le_bytes(b[4..8].try_into().unwrap())).unwrap_or(0) as i32
        } else if known { field_u32(record, query::DEFENSE) as i32 } else { 0 },
        "counter": if known { record.get(query::COUNTERS).filter(|b| b.len() >= 8)
            .map(|b| u32::from_le_bytes(b[4..8].try_into().unwrap()) >> 16).unwrap_or(0) } else { 0 },
        "negated": known && field_u32(record, query::STATUS) & 0x4000001 != 0,
        "types": TYPES.iter().filter(|(bit, _)| kind & bit != 0).map(|(_, name)| *name).collect::<Vec<_>>()})
}

fn table(duel: &Duel, me: u8, message: &Message, revealed: &[Loc]) -> Result<Vec<Value>> {
    let flags = query::RECOMMENDED
        | query::TYPE
        | query::ATTRIBUTE
        | query::RACE
        | query::STATUS
        | query::LINK
        | query::OVERLAY_CARD;
    let mut cards = Vec::new();
    for owner in [me, 1 - me] {
        for loc in [1u8, 2, 4, 8, 16, 32, 64] {
            let bytes = duel.query_location(flags, owner, loc as u32);
            let records = read_location(&bytes).map_err(|e| e.to_string())?;
            let mut pile = Vec::new();
            for (sequence, record) in records.iter().enumerate() {
                let Some(record) = record else { continue };
                let item = if revealed.iter().any(|r| {
                    r.controller == owner && r.location == loc && r.sequence == sequence as u32
                }) {
                    card_visible(record, owner, loc, sequence, me, true)
                } else {
                    card(record, owner, loc, sequence, me)
                };
                pile.push(item);
                if let Some(materials) = record.get(query::OVERLAY_CARD) {
                    for (overlay, bytes) in materials[4..].chunks_exact(4).enumerate() {
                        let code = u32::from_le_bytes(bytes.try_into().unwrap());
                        let data = duel
                            .printed(code)
                            .ok_or_else(|| format!("unknown material {code}"))?;
                        pile.push(json!({"code": code, "controller": controller(owner, me),
                            "location": location_name(loc), "sequence": sequence, "overlay_sequence": overlay,
                            "position": "faceup", "attribute": attribute(data.attribute),
                            "race": RACES.get(data.race.trailing_zeros() as usize).copied().unwrap_or("none"),
                            "level": data.level, "attack": data.attack,
                            "defense": if data.kind & 0x4000000!=0 {duel.link_markers(code).unwrap_or(0) as i32} else {data.defense},
                            "counter": 0, "negated": false,
                            "types": TYPES.iter().filter(|(bit,_)| data.kind & bit != 0).map(|(_,n)| *n).collect::<Vec<_>>()}));
                    }
                }
            }
            // Only the remaining inventory is known, never the shuffle order.
            // A Deck selection temporarily names candidates by engine sequence.
            if owner == me && loc == location::DECK {
                pile.sort_by_key(|c| c["code"].as_u64().unwrap());
                let offered: &[Offered] = match message {
                    Message::SelectCard(c)
                    | Message::SelectTribute(c)
                    | Message::SelectUnselect(c) => &c.cards,
                    Message::SelectSum { cards, .. } => &cards.cards,
                    _ => &[],
                };
                let mut assigned = Vec::new();
                for c in offered
                    .iter()
                    .filter(|c| c.loc.controller == me && c.loc.location == location::DECK)
                {
                    let k = c.loc.sequence as usize;
                    if k >= pile.len() {
                        return Err("Deck candidate outside its pile".into());
                    }
                    let n = (k..pile.len())
                        .chain(0..k)
                        .find(|&n| (!assigned.contains(&n) || n == k) && pile[n]["code"] == c.code)
                        .ok_or("Deck candidate missing from inventory")?;
                    pile.swap(k, n);
                    assigned.push(k);
                }
                for (i, c) in pile.iter_mut().enumerate() {
                    c["sequence"] = json!(i);
                }
            }
            cards.extend(pile);
        }
    }
    if cards.len() > 160 {
        return Err("model table exceeds 160 cards".into());
    }
    Ok(cards)
}

/// A protocol prompt and the exact engine response for each returned position.
fn prompt(message: &Message, p: &Prompt, me: u8) -> Result<(Value, Vec<Vec<u8>>)> {
    let mut responses = p.responses.clone();
    let selectable = |offered: &[Offered]| {
        offered
            .iter()
            .enumerate()
            .map(|(i, c)| json!({"location": at(c.loc, me), "response": i}))
            .collect::<Vec<_>>()
    };
    let data = match message {
        Message::SelectIdle(_) | Message::SelectBattle(_) => {
            let battle = matches!(message, Message::SelectBattle(_));
            let mut commands = Vec::new();
            responses.clear();
            for (i, c) in p.decision.choices.iter().enumerate() {
                let kind = match c.kind {
                    ChoiceKind::NormalSummon => "summon",
                    ChoiceKind::SpecialSummon => "sp_summon",
                    ChoiceKind::ChangePosition => "reposition",
                    ChoiceKind::SetMonster => "mset",
                    ChoiceKind::SetSpellTrap => "set",
                    ChoiceKind::Activate => "activate",
                    ChoiceKind::Attack => "attack",
                    ChoiceKind::EnterBattle => "to_bp",
                    ChoiceKind::EnterMain2 => "to_m2",
                    ChoiceKind::EndTurn => "to_ep",
                    _ => continue,
                };
                let mut command = json!({"cmd_type": kind});
                if let Some(m) = c.card {
                    command["data"] = json!({"card_info": {"code": m.code.unwrap_or(0) & 0x7fffffff,
                        "controller": controller(m.at.controller, me), "location": location_name(model_location(m.at.location)),
                        "sequence": m.at.sequence}, "effect_description": description(c.description),
                        "direct_attackable": m.value != 0, "response": commands.len()});
                }
                commands.push(command);
                responses.push(p.responses[i].clone());
            }
            if battle {
                json!({"msg_type":"select_battlecmd", "battle_cmds":commands})
            } else {
                json!({"msg_type":"select_idlecmd", "idle_cmds":commands})
            }
        }
        Message::SelectChain { forced, chains, .. } => {
            json!({"msg_type":"select_chain", "forced":forced,
            "chains": chains.iter().enumerate().map(|(i,c)| json!({"code":c.code & 0x7fffffff,
                "location":at(c.loc,me), "effect_description":description(c.description), "response":i})).collect::<Vec<_>>()})
        }
        Message::SelectEffectYesNo {
            code,
            loc,
            description: desc,
            ..
        } => json!({"msg_type":"select_effectyn",
            "code":code, "location":at(*loc,me), "effect_description":description(*desc)}),
        Message::SelectYesNo {
            description: desc, ..
        } => json!({"msg_type":"select_yesno", "effect_description":description(*desc)}),
        Message::SelectOption { options, .. } => json!({"msg_type":"select_option",
            "options":options.iter().enumerate().map(|(i,n)| json!({"code":description(*n), "response":i})).collect::<Vec<_>>()}),
        Message::SelectPosition {
            code, positions, ..
        } => json!({"msg_type":"select_position", "code":code,
            "positions":([1u32,2,4,8].into_iter().filter(|b| *positions as u32 & b != 0)
                .map(|b| position(b,1)).collect::<Vec<_>>())}),
        Message::SelectPlace { count, disable, .. } => {
            json!({"msg_type":if *disable { "select_disfield" } else { "select_place" },
            "count":(*count).max(1), "places":p.decision.choices.iter().map(|c| {
                let at=c.place.unwrap(); json!({"controller":controller(at.controller,me),
                    "location":location_name(model_location(at.location)), "sequence":at.sequence})
            }).collect::<Vec<_>>()})
        }
        Message::SelectCard(c) | Message::SelectTribute(c) => {
            let tribute = matches!(message, Message::SelectTribute(_));
            let mut data = json!({"msg_type":if tribute {"select_tribute"} else {"select_card"},
                "cancelable":c.cancelable,"min":c.min,"max":c.max,"cards":selectable(&c.cards),"selected":[]});
            if tribute {
                for (i, item) in data["cards"].as_array_mut().unwrap().iter_mut().enumerate() {
                    item["level"] = json!(c.cards[i].value);
                }
            }
            data
        }
        Message::SelectUnselect(c) => {
            // Their model does not unselect, and ignores cancellation unless finishable.
            responses.truncate(c.cards.len() + usize::from(c.finishable));
            json!({"msg_type":"select_unselect_card","cancelable":c.cancelable,"finishable":c.finishable,
                "min":c.min,"max":c.max,"selectable_cards":selectable(&c.cards),"selected_cards":selectable(&c.fixed)})
        }
        Message::SelectSum {
            cards: c,
            exact,
            target,
        } => {
            let sum_cards = |cards: &[Offered]| {
                cards
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        json!({"location":at(c.loc,me),
                "level1":c.value & 0xffff,"level2":c.value >> 16,"response":i})
                    })
                    .collect::<Vec<_>>()
            };
            json!({"msg_type":"select_sum","overflow":!exact,"level_sum":target,"min":c.min,"max":c.max,
                "cards":sum_cards(&c.cards),"must_cards":sum_cards(&c.fixed),"selected":[]})
        }
        Message::AnnounceAttribute {
            count, available, ..
        } if *count == 1 => json!({"msg_type":"announce_attrib","count":count,
            "attributes":(0..7).map(|i| 1u32<<i).filter(|b| available & b != 0)
                .enumerate().map(|(i,b)| json!({"attribute":attribute(b),"response":i})).collect::<Vec<_>>()}),
        Message::AnnounceNumber { values, .. } => json!({"msg_type":"announce_number","count":1,
            "numbers":values.iter().enumerate().map(|(i,n)| json!({"number":n,"response":i})).collect::<Vec<_>>()}),
        _ => {
            return Err(format!(
                "unsupported engine prompt {}",
                message_name(message)
            ))
        }
    };
    Ok((data, responses))
}
fn message_name(m: &Message) -> String {
    match m {
        Message::SelectCard(_) => "select_card",
        Message::SelectTribute(_) => "select_tribute",
        Message::SelectSum { .. } => "select_sum",
        Message::SelectUnselect(_) => "select_unselect_card",
        Message::SelectIdle(_) => "select_idlecmd",
        Message::SelectBattle(_) => "select_battlecmd",
        Message::SelectChain { .. } => "select_chain",
        Message::SelectEffectYesNo { .. } => "select_effectyn",
        Message::SelectYesNo { .. } => "select_yesno",
        Message::SelectPlace { .. } => "select_place",
        Message::SelectPosition { .. } => "select_position",
        Message::SelectOption { .. } => "select_option",
        Message::AnnounceAttribute { .. } => "announce_attrib",
        Message::AnnounceNumber { .. } => "announce_number",
        _ => return format!("{m:?}").split(" {").next().unwrap().into(),
    }
    .into()
}
fn global(obs: &Observation) -> Value {
    json!({"my_lp":obs.life_points[obs.me as usize],"op_lp":obs.life_points[1-obs.me as usize],
    "turn":obs.turn,"is_first":obs.me==0,"is_my_turn":obs.turn_player==Some(obs.me),
    "phase":match obs.phase.unwrap_or(Phase::Draw) {
        Phase::Draw=>"draw",Phase::Standby=>"standby",Phase::Main1=>"main1",
        Phase::BattleStart=>"battle_start",Phase::BattleStep=>"battle_step",Phase::Damage=>"damage",
        Phase::DamageCalculation=>"damage_calculation",Phase::Battle=>"battle",Phase::Main2=>"main2",Phase::End=>"end"
    }})
}

struct ModelSeat {
    http: Http,
    id: String,
    index: u64,
    previous: usize,
    me: u8,
    first: Seat,
    db: Arc<dyn CardDatabase>,
    asked: usize,
    server_errors: usize,
    fallbacks: BTreeMap<String, usize>,
    values: Vec<Option<f64>>,
    trace: Vec<Value>,
    trace_enabled: bool,
    revealed: Vec<Loc>,
}
impl ModelSeat {
    fn new(url: &str, me: u8, cards: &Path, trace_enabled: bool) -> Result<Self> {
        let http = Http::new(url)?;
        let answer = http.request("POST", "/v0/duels", &json!({}))?;
        let id = answer["duelId"]
            .as_str()
            .ok_or("server did not return duelId")?
            .to_owned();
        let db: Arc<dyn CardDatabase> =
            Arc::new(SqliteCards::open(cards).map_err(|e| e.to_string())?);
        Ok(Self {
            http,
            id,
            index: 0,
            previous: 0,
            me,
            first: Seat::new(
                Box::new(ygo_policies::reference::First),
                db.clone(),
                Some(me),
            ),
            db,
            asked: 0,
            server_errors: 0,
            fallbacks: BTreeMap::new(),
            values: Vec::new(),
            trace: Vec::new(),
            trace_enabled,
            revealed: Vec::new(),
        })
    }
    fn fallback(&mut self, reason: String) {
        *self.fallbacks.entry(reason).or_default() += 1;
    }
    fn predict(&mut self, input: &Value) -> Result<Option<(usize, Value)>> {
        let request = json!({"input":input,"index":self.index,"prev_action_idx":self.previous});
        self.asked += 1;
        // A lost HTTP response leaves the server's index unknown. Abort that game;
        // guessing its recurrent state would silently invalidate later decisions.
        let answer =
            self.http
                .request("POST", &format!("/v0/duels/{}/predict", self.id), &request)?;
        if let Some(error) = answer["error"].as_str() {
            self.server_errors += 1;
            self.values.push(None);
            self.fallback(format!(
                "{}: {}",
                input["action_msg"]["data"]["msg_type"]
                    .as_str()
                    .unwrap_or("unknown"),
                error
            ));
            if self.trace_enabled {
                self.trace
                    .push(json!({"request":request,"answer":answer,"fallback":true}));
            }
            return Ok(None);
        }
        let next = answer["index"]
            .as_u64()
            .ok_or("server did not return index")?;
        if next != self.index + 1 {
            return Err("server index did not advance once".into());
        }
        let predictions = answer["predict_results"]["action_preds"]
            .as_array()
            .ok_or("server did not return action_preds")?;
        let data = &input["action_msg"]["data"];
        let list = |key: &str| data[key].as_array().map_or(0, Vec::len);
        let expected = match data["msg_type"].as_str().unwrap_or("") {
            "select_idlecmd" => list("idle_cmds"),
            "select_battlecmd" => list("battle_cmds"),
            "select_chain" => list("chains") + usize::from(data["forced"] != true),
            "select_effectyn" | "select_yesno" => 2,
            "select_position" => list("positions"),
            "select_place" | "select_disfield" => list("places"),
            "select_option" => list("options"),
            "select_card" | "select_tribute" => list("cards") + 1,
            "select_sum" => list("cards"),
            "select_unselect_card" => {
                list("selectable_cards") + usize::from(data["finishable"] == true)
            }
            "announce_attrib" => list("attributes"),
            "announce_number" => list("numbers"),
            _ => return Err("unknown model prompt".into()),
        };
        if predictions.len() != expected {
            return Err(format!(
                "server returned {} options; expected {expected}",
                predictions.len()
            ));
        }
        let mut best = None;
        for (i, p) in predictions.iter().enumerate() {
            let prob = p["prob"].as_f64().ok_or("invalid model probability")?;
            if prob == -1.0 {
                continue;
            }
            if !(0.0..=1.0).contains(&prob) {
                return Err("invalid model probability".into());
            }
            if best.map_or(true, |(_, score)| prob > score) {
                best = Some((i, prob));
            }
        }
        let chosen = best.ok_or("server offered no model action")?.0;
        let value = answer["predict_results"]["win_rate"]
            .as_f64()
            .ok_or("missing win_rate")?;
        // Its value head is linear: the server sometimes emits values just
        // outside [0,1]. Keep them verbatim; -1 alone denotes a single option.
        self.values.push(Some(value));
        self.index = next;
        self.previous = chosen;
        if self.trace_enabled {
            self.trace
                .push(json!({"request":request,"answer":answer,"chosen":chosen}));
        }
        Ok(Some((chosen, predictions[chosen].clone())))
    }
    fn feed(&mut self, bytes: &[u8], duel: &Duel) -> Result<Option<Vec<u8>>> {
        if bytes.first() == Some(&ygo_policies_ocgcore::wire::msg::CONFIRM_CARDS) {
            if let Some(visible) = ygo_policies_ocgcore::redact::redact(bytes, Some(self.me))
                .map_err(|e| e.to_string())?
            {
                if let Message::ConfirmCards { cards, .. } =
                    message::parse(&visible).map_err(|e| e.to_string())?
                {
                    self.revealed.extend(
                        cards
                            .into_iter()
                            .filter(|(code, _)| *code != 0)
                            .map(|(_, loc)| loc),
                    );
                }
            }
        } else if bytes.first() == Some(&ygo_policies_ocgcore::wire::msg::CHAIN_SOLVED) {
            self.revealed.clear();
        }
        let fallback = self.first.feed(bytes).map_err(|e| e.to_string())?;
        let Some(fallback) = fallback else {
            return Ok(None);
        };
        let visible = ygo_policies_ocgcore::redact::redact(bytes, Some(self.me))
            .map_err(|e| e.to_string())?
            .ok_or("our prompt was redacted")?;
        let message = message::parse(&visible).map_err(|e| e.to_string())?;
        if matches!(&message,Message::SelectChain{chains,..} if chains.is_empty()) {
            return Ok(Some(fallback));
        }
        let obs = self.first.observation().ok_or("seat has no observation")?;
        let p = decision::prompt(&message, &obs, 0, &*self.db)
            .map_err(|e| e.to_string())?
            .ok_or("missing prompt")?;
        let (mut data, responses) = match prompt(&message, &p, self.me) {
            Ok(p) => p,
            Err(e) => {
                self.fallback(e);
                return Ok(Some(fallback));
            }
        };
        let table = table(duel, self.me, &message, &self.revealed)?;
        let mut input = json!({"global":global(&obs),"cards":table,"action_msg":{"data":data}});
        if let Some(sequential) = p.sequential {
            let mut selected = Vec::new();
            loop {
                selected.sort_unstable();
                data["selected"] = json!(selected);
                input["action_msg"]["data"] = data.clone();
                let prediction = self.predict(&input)?;
                let (chosen, finish) = if let Some((chosen, entry)) = prediction {
                    // Returned positions include skipped cards; never reinterpret
                    // them as positions in a reduced list of legal candidates.
                    (chosen, entry["can_finish"] == true)
                } else {
                    let (d, steps) = sequential.step(&selected);
                    match steps[ygo_policies::reference::first(&d)] {
                        Step::Pick(k) => (k, false),
                        Step::Finish => break,
                        Step::Cancel => return Ok(Some(decision::Sequential::cancel_response())),
                    }
                };
                if chosen == sequential.candidates.len() {
                    if sequential.step(&selected).1.contains(&Step::Finish) {
                        break;
                    }
                    return Err("model finished an incomplete selection".into());
                }
                if !sequential.picks(&selected).contains(&chosen) {
                    return Err(format!("model chose illegal selection candidate {chosen}"));
                }
                selected.push(chosen);
                let (_, steps) = sequential.step(&selected);
                if selected.len() >= sequential.maximum || finish {
                    if !steps.contains(&Step::Finish) {
                        return Err("model finished an illegal sum".into());
                    }
                    break;
                }
            }
            return Ok(Some(sequential.response(&selected)));
        }
        Ok(Some(match self.predict(&input)? {
            Some((i, _)) => responses
                .get(i)
                .ok_or("model response outside engine choices")?
                .clone(),
            None => fallback,
        }))
    }
    fn stats(&self) -> Value {
        json!({"asked":self.asked,"server_errors":self.server_errors,"fallbacks":self.fallbacks.values().sum::<usize>(),
            "fallback_reasons":self.fallbacks,"win_rates":self.values,"trace":self.trace})
    }
}
impl Drop for ModelSeat {
    fn drop(&mut self) {
        let _ = self
            .http
            .request("DELETE", &format!("/v0/duels/{}", self.id), &Value::Null);
    }
}
enum Player<'a> {
    Model(Box<ModelSeat>),
    Pilot(LibrarySeat<'a>),
}
impl Player<'_> {
    fn feed(&mut self, bytes: &[u8], duel: &Duel) -> Result<Option<Vec<u8>>> {
        match self {
            Self::Model(m) => m.feed(bytes, duel),
            Self::Pilot(p) => p.feed(bytes),
        }
    }
}
pub fn play(
    core: &Core,
    decks: &[Deck; 2],
    library: &PolicyLibrary,
    names: [&str; 2],
    cards: &Path,
    run: PlayOptions,
    server: &str,
) -> Result<Value> {
    let options = DuelOptions {
        life_points: run.life_points,
        flags: run.flags,
        ..DuelOptions::seeded(run.seed)
    };
    let mut duel = core.deal(&options, decks)?;
    let mut recorded = run.record.then(|| Recorded::dealt(&options, decks));
    let mut seats = Vec::new();
    for (p, name) in names.iter().enumerate() {
        seats.push(if *name == "ygo-agent" {
            Player::Model(Box::new(ModelSeat::new(server, p as u8, cards, run.trace)?))
        } else {
            Player::Pilot(library.seat(name, cards, p as i32, run.seed + p as u64)?)
        });
        seats[p].feed(&duel.start_message(p as u8), &duel)?;
    }
    let (mut decisions, mut turns, mut digest) = (0usize, 0u32, 0xcbf29ce484222325u64);
    let mut activations = BTreeMap::<u32, usize>::new();
    loop {
        let step = duel.step()?;
        let mut response = None;
        for sent in &step.messages {
            if !sent.refresh {
                match message::parse(&sent.bytes).map_err(|e| e.to_string())? {
                    Message::Retry => return Err("Engine rejected response (MSG_RETRY)".into()),
                    Message::NewTurn { .. } => turns += 1,
                    Message::Chaining { code, .. } => *activations.entry(code).or_default() += 1,
                    _ => {}
                }
            }
            for (p, seat) in seats.iter_mut().enumerate() {
                if let Some(answer) = seat.feed(&sent.bytes, &duel)? {
                    if response.is_some() {
                        return Err("multiple answers in engine batch".into());
                    }
                    for byte in sent.bytes.iter().chain(&answer) {
                        digest = (digest ^ *byte as u64).wrapping_mul(0x100000001b3)
                    }
                    response = Some((p as u8, answer));
                }
            }
        }
        if matches!(step.state, State::Over(_)) || decisions >= run.limit {
            let (winner, reason) = match step.state {
                State::Over(Some(o)) => (Some(o.winner), Some(o.reason)),
                State::Over(None) => return Err("Engine ended without MSG_WIN".into()),
                _ => (None, None),
            };
            let model_seats: Vec<_> = seats
                .iter()
                .enumerate()
                .filter_map(|(p, s)| match s {
                    Player::Model(m) => Some(json!({"seat":p,"stats":m.stats()})),
                    _ => None,
                })
                .collect();
            let mut row = json!({"winner":winner,"reason":reason,"turns":turns,"decisions":decisions,
                "limit":winner.is_none(),"digest":format!("{digest:016x}"),"activations":activations,
                "model_seats":model_seats});
            if let Some(recorded) = recorded.as_mut() {
                row["record"] = recorded.to_json();
                row["record"]["players"] = json!(names);
            }
            return Ok(row);
        }
        if let Some((player, answer)) = response {
            duel.respond(&answer);
            if let Some(recorded) = recorded.as_mut() {
                recorded.responses.push((player, answer))
            }
            decisions += 1;
        } else if step.state == State::Awaiting {
            return Err("engine awaiting without an answer".into());
        }
    }
}

/// Outcomes, seats, fallback kinds and calibration; draws are always separate.
pub fn summary(rows: &[Value], metadata: &Value, path: &Path) -> Result<()> {
    let mut pairs = BTreeMap::<(String, String), Vec<&Value>>::new();
    for row in rows {
        pairs
            .entry((
                row["a"].as_str().ok_or("missing a")?.into(),
                row["b"].as_str().ok_or("missing b")?.into(),
            ))
            .or_default()
            .push(row);
    }
    let outcomes = |sample: &[&Value]| {
        let (mut wins, mut losses, mut draws, mut failed) = (0, 0, 0, 0);
        for row in sample {
            if row["game"]["error"].is_string() {
                failed += 1
            } else {
                match row["game"]["winner"].as_u64() {
                    Some(w) if w < 2 && w == row["seat"].as_u64().unwrap() => wins += 1,
                    Some(w) if w < 2 => losses += 1,
                    _ => draws += 1,
                }
            }
        }
        let n = (wins + losses + draws) as f64;
        let share = if n > 0.0 { Some(wins as f64 / n) } else { None };
        json!({"games":sample.len(),"wins":wins,"losses":losses,"draws":draws,"failures":failed,
            "win_share":share,"standard_error":share.map(|p|(p*(1.0-p)/n).sqrt()),
            "score":if n>0.0 {Some((wins as f64+0.5*draws as f64)/n)} else {None}})
    };
    let mut matchups = Vec::new();
    for ((a, b), sample) in pairs {
        let mut row = outcomes(&sample);
        row["a"] = json!(a);
        row["b"] = json!(b);
        row["by_seat"] = json!((0..2)
            .map(|seat| outcomes(
                &sample
                    .iter()
                    .copied()
                    .filter(|r| r["seat"] == seat)
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>());
        let (mut asked, mut errors, mut fallbacks) = (0usize, 0usize, 0usize);
        let mut reasons = BTreeMap::<String, u64>::new();
        let mut fallback_games = BTreeMap::<String, usize>::new();
        let mut bins = [[0.0f64; 4]; 10]; // decisions, wins, draws, predicted sum
        let (mut valid_models, mut second_wins, mut model_draws) = (0, 0, 0);
        for game in &sample {
            if game["game"]["error"].is_string() {
                continue;
            }
            if a == "ygo-agent" && b == "ygo-agent" {
                valid_models += 1;
                match game["game"]["winner"].as_u64() {
                    Some(1) => second_wins += 1,
                    Some(0) => {}
                    _ => model_draws += 1,
                }
            }
            if let Some(seats) = game["game"]["model_seats"].as_array() {
                let mut kinds = BTreeSet::new();
                for seat in seats {
                    let stats = &seat["stats"];
                    asked += stats["asked"].as_u64().unwrap_or(0) as usize;
                    errors += stats["server_errors"].as_u64().unwrap_or(0) as usize;
                    fallbacks += stats["fallbacks"].as_u64().unwrap_or(0) as usize;
                    if let Some(r) = stats["fallback_reasons"].as_object() {
                        for (k, v) in r {
                            *reasons.entry(k.clone()).or_default() += v.as_u64().unwrap_or(0);
                            kinds.insert(k.split(": ").next().unwrap().to_owned());
                        }
                    }
                    for value in stats["win_rates"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_f64)
                        .filter(|p| *p != -1.0)
                    {
                        let bin = &mut bins[((value.clamp(0.0, 1.0) * 10.0) as usize).min(9)];
                        bin[0] += 1.0;
                        bin[3] += value;
                        match game["game"]["winner"].as_u64() {
                            Some(w) if w < 2 && Some(w) == seat["seat"].as_u64() => bin[1] += 1.0,
                            Some(w) if w < 2 => {}
                            _ => bin[2] += 1.0,
                        }
                    }
                }
                for kind in kinds {
                    *fallback_games.entry(kind).or_default() += 1
                }
            }
        }
        row["model"] = json!({"asked":asked,"server_errors":errors,"fallbacks":fallbacks,"fallback_reasons":reasons,
            "fallback_games_by_kind":fallback_games,
            "server_error_rate":if asked>0 {Some(errors as f64/asked as f64)} else {None},
            "second_seat_win_share":if valid_models>0 {Some(second_wins as f64/valid_models as f64)} else {None},
            "mirror_draws":model_draws,
            "calibration":bins.iter().enumerate().map(|(i,b)|json!({"band":[i as f64/10.0,(i+1) as f64/10.0],
                "decisions":b[0] as usize,"wins":b[1] as usize,"draws":b[2] as usize,
                "observed_win_share":if b[0]>0.0 {Some(b[1]/b[0])} else {None},
                "mean_prediction":if b[0]>0.0 {Some(b[3]/b[0])} else {None}})).collect::<Vec<_>>()});
        matchups.push(row);
    }
    let failures = rows
        .iter()
        .filter(|r| r["game"]["error"].is_string())
        .count();
    let result =
        json!({"metadata":metadata,"games":rows.len(),"failures":failures,"matchups":matchups});
    std::fs::write(path, serde_json::to_string_pretty(&result).unwrap())
        .map_err(|e| e.to_string())?;
    for m in result["matchups"].as_array().unwrap() {
        println!(
            "{} vs {}: {} wins, {} losses, {} draws, {} failures; model {}/{} server errors",
            m["a"],
            m["b"],
            m["wins"],
            m["losses"],
            m["draws"],
            m["failures"],
            m["model"]["server_errors"],
            m["model"]["asked"]
        );
    }
    if failures > 0 {
        Err(format!("{failures} games failed; results are incomplete"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, path::PathBuf};
    use ygo_policies_ocgcore::message::{Effect, Idle};
    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }
    fn core() -> Core {
        Core::open(
            None,
            &root().join("vendor/BabelCdb/cards.cdb"),
            &root().join("vendor/CardScripts"),
        )
        .unwrap()
    }
    fn empty_obs() -> Observation {
        let db = Arc::new(ygo_policies::cards::MemoryCards::default());
        Seat::new(Box::new(ygo_policies::reference::First), db, Some(0))
            .observation()
            .unwrap()
    }
    fn field<'a>(fields: &'a [(u32, Vec<u8>)]) -> RawRecord<'a> {
        RawRecord {
            fields: fields.iter().map(|(f, v)| (*f, v.as_slice())).collect(),
        }
    }
    #[test]
    fn hidden_cards_keep_no_identity_or_statistics_and_descriptions_convert() {
        let fields: Vec<_> = [
            (query::CODE, 89631139u32),
            (query::TYPE, 0x11),
            (query::POSITION, 8),
            (query::ATTRIBUTE, 16),
            (query::RACE, 1 << 13),
            (query::ATTACK, 3000),
            (query::DEFENSE, 2500),
            (query::LEVEL, 8),
            (query::STATUS, 1),
        ]
        .map(|(f, v)| (f, v.to_le_bytes().to_vec()))
        .into();
        let record = field(&fields);
        let hidden = card(&record, 1, 4, 3, 0);
        assert_eq!(hidden["code"], 0);
        assert_eq!(hidden["types"], json!([]));
        assert_eq!(hidden["attack"], 0);
        assert_eq!(hidden["race"], "none");
        assert_eq!(hidden["position"], "facedown_defense");
        assert_eq!(card(&record, 1, 2, 0, 0)["position"], "none");
        let own = card(&record, 0, 1, 7, 0);
        assert_eq!(own["code"], 89631139);
        assert_eq!(own["position"], "facedown");
        assert_eq!(own["attack"], 3000);
        assert_eq!(own["negated"], true);
        assert_eq!(description((38517737u64 << 20) | 2), 38517737 * 16 + 2);
        assert_eq!(description(1050), 1050);
        assert_eq!(description(0), 0);
        let overlay = Loc {
            controller: 0,
            location: 0x84,
            sequence: 5,
            position: 2,
        };
        assert_eq!(
            at(overlay, 0),
            json!({"controller":"me","location":"mzone","sequence":5,"overlay_sequence":2})
        );
    }
    #[test]
    fn the_golden_idle_prompt_translates_in_the_same_order() {
        let requests: Vec<Value> = serde_json::from_str(include_str!(
            "../../../benchmarks/ygo-agent/kit/golden-requests.json"
        ))
        .unwrap();
        let expected = &requests[0]["input"]["action_msg"]["data"];
        let mut idle = Idle {
            player: 0,
            summon: vec![],
            special_summon: vec![],
            reposition: vec![],
            set_monster: vec![],
            set_spell_trap: vec![],
            activate: vec![],
            battle_phase: false,
            end_phase: true,
            shuffle_hand: false,
        };
        for command in expected["idle_cmds"].as_array().unwrap() {
            let info = &command["data"]["card_info"];
            if info.is_null() {
                continue;
            }
            let loc = Loc {
                controller: 0,
                location: 2,
                sequence: info["sequence"].as_u64().unwrap() as u32,
                position: 0,
            };
            let code = info["code"].as_u64().unwrap() as u32;
            if command["cmd_type"] == "set" {
                idle.set_spell_trap.push(Offered {
                    code,
                    loc,
                    value: 0,
                })
            } else {
                idle.activate.push(Effect {
                    code,
                    loc,
                    description: 0,
                })
            }
        }
        let message = Message::SelectIdle(idle);
        let obs = empty_obs();
        let db = ygo_policies::cards::MemoryCards::default();
        let p = decision::prompt(&message, &obs, 0, &db).unwrap().unwrap();
        let (got, responses) = prompt(&message, &p, 0).unwrap();
        for (mine, theirs) in got["idle_cmds"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["idle_cmds"].as_array().unwrap())
        {
            assert_eq!(mine["cmd_type"], theirs["cmd_type"]);
            assert_eq!(mine["data"]["card_info"], theirs["data"]["card_info"]);
            assert_eq!(
                mine["data"]["effect_description"],
                theirs["data"]["effect_description"]
            );
        }
        assert_eq!(responses[4], 5i32.to_le_bytes()); // first Trade-In activation
        assert_eq!(responses[7], 7i32.to_le_bytes()); // End Phase
    }
    #[test]
    fn deck_order_is_hidden_but_search_candidates_keep_their_engine_places() {
        let core = core();
        let options = DuelOptions {
            hand: 0,
            per_turn: 0,
            flags: DuelOptions::MASTER_RULE_5,
            ..DuelOptions::seeded(7)
        };
        let mut duel = core.duel(&options).unwrap();
        for (sequence, code) in [89631139, 38120068, 71039903].into_iter().enumerate() {
            duel.add(ygo_policies_duel::Placed {
                controller: 0,
                code,
                location: 1,
                sequence: sequence as u32,
                position: 8,
            });
        }
        duel.add(ygo_policies_duel::Placed {
            controller: 1,
            code: 38517737,
            location: 2,
            sequence: 0,
            position: 8,
        });
        let cards = table(&duel, 0, &Message::Other(0), &[]).unwrap();
        let codes: Vec<_> = cards
            .iter()
            .filter(|c| c["controller"] == "me")
            .map(|c| c["code"].as_u64().unwrap())
            .collect();
        assert_eq!(codes, vec![38120068, 71039903, 89631139]);
        assert_eq!(cards.last().unwrap()["code"], 0);
        let raw = duel.card(query::CODE, 0, 1, 0).unwrap();
        let record = ygo_policies_ocgcore::query::read_record(
            &mut ygo_policies_ocgcore::wire::Reader::new(&raw),
        )
        .unwrap()
        .unwrap();
        let code = field_u32(&record, query::CODE);
        let m = Message::SelectCard(message::Cards {
            player: 0,
            cancelable: false,
            finishable: false,
            min: 1,
            max: 1,
            cards: vec![Offered {
                code,
                loc: Loc {
                    controller: 0,
                    location: 1,
                    sequence: 0,
                    position: 0,
                },
                value: 0,
            }],
            fixed: vec![],
        });
        let cards = table(&duel, 0, &m, &[]).unwrap();
        assert_eq!(cards[0]["code"], code);
        assert_eq!(cards[0]["sequence"], 0);
    }
    #[test]
    fn staged_golden_table_matches_every_visible_field() {
        let requests: Vec<Value> = serde_json::from_str(include_str!(
            "../../../benchmarks/ygo-agent/kit/golden-requests.json"
        ))
        .unwrap();
        let expected = requests[0]["input"]["cards"].as_array().unwrap();
        let core = core();
        let options = DuelOptions {
            hand: 0,
            per_turn: 0,
            flags: DuelOptions::MASTER_RULE_5,
            ..DuelOptions::seeded(7)
        };
        let mut duel = core.duel(&options).unwrap();
        for c in expected {
            let loc = match c["location"].as_str().unwrap() {
                "deck" => 1,
                "hand" => 2,
                "mzone" => 4,
                "szone" => 8,
                "grave" => 16,
                "removed" => 32,
                "extra" => 64,
                _ => unreachable!(),
            };
            let code = c["code"].as_u64().unwrap() as u32;
            let code = if code == 0 {
                if loc == 64 {
                    40908371
                } else {
                    89631139
                }
            } else {
                code
            };
            let pos = if c["position"] == "faceup_attack" {
                1
            } else if c["position"] == "faceup" {
                5
            } else {
                8
            };
            duel.add(ygo_policies_duel::Placed {
                controller: u8::from(c["controller"] != "me"),
                code,
                location: loc,
                sequence: c["sequence"].as_u64().unwrap() as u32,
                position: pos,
            });
        }
        let got = table(&duel, 0, &Message::Other(0), &[]).unwrap();
        // Deck/Extra row order is immaterial to the attention encoder. For this
        // nonselection fixture their sequence does not enter the model input.
        let normalized = |cards: &[Value]| {
            let mut cards = cards.to_vec();
            for c in &mut cards {
                if c["location"] == "deck" || c["location"] == "extra" {
                    c["sequence"] = json!(0)
                }
            }
            let mut rows: Vec<_> = cards.iter().map(Value::to_string).collect();
            rows.sort();
            rows
        };
        let mut got = normalized(&got);
        for expected in normalized(expected) {
            let found = got.iter().position(|c| c == &expected);
            assert!(found.is_some(), "missing golden card {expected}");
            got.remove(found.unwrap());
        }
        assert!(got.is_empty(), "extra golden cards {got:?}");
    }
    #[test]
    fn server_indices_survive_errors_and_single_option_history() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let answers = [
                json!({"duelId":"fixture","index":0}),
                json!({"index":1,"predict_results":{"win_rate":0.4,"action_preds":[{"prob":-1.0},{"prob":0.5},{"prob":0.5}]}}),
                json!({"error":"fixture error"}),
                json!({"index":2,"predict_results":{"win_rate":-1.0,"action_preds":[{"prob":1.0}]}}),
                Value::Null,
            ];
            let mut requests = Vec::new();
            for answer in answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut bytes = Vec::new();
                let end = loop {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).unwrap();
                    bytes.extend_from_slice(&buf[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break end;
                    }
                };
                let headers = String::from_utf8_lossy(&bytes[..end]).into_owned();
                let length: usize = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("Content-Length: "))
                    .unwrap()
                    .parse()
                    .unwrap();
                while bytes.len() < end + 4 + length {
                    let mut b = [0u8; 4096];
                    let n = stream.read(&mut b).unwrap();
                    bytes.extend_from_slice(&b[..n]);
                }
                requests.push(serde_json::from_slice::<Value>(&bytes[end + 4..]).unwrap());
                let data = answer.to_string();
                write!(
                    stream,
                    "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{data}",
                    data.len()
                )
                .unwrap();
            }
            requests
        });
        let mut model =
            ModelSeat::new(&url, 0, &root().join("vendor/BabelCdb/cards.cdb"), true).unwrap();
        let input = json!({"action_msg":{"data":{"msg_type":"select_chain","forced":true,"chains":[{},{},{}]}}});
        assert_eq!(model.predict(&input).unwrap().unwrap().0, 1);
        assert!(model.predict(&input).unwrap().is_none());
        assert_eq!(model.index, 1);
        let single =
            json!({"action_msg":{"data":{"msg_type":"select_chain","forced":true,"chains":[{}]}}});
        assert_eq!(model.predict(&single).unwrap().unwrap().0, 0);
        assert_eq!(model.stats()["win_rates"], json!([0.4, null, -1.0]));
        drop(model);
        let requests = server.join().unwrap();
        assert_eq!(requests[2]["prev_action_idx"], 1);
        assert_eq!(requests[2]["index"], 1);
        assert_eq!(requests[3]["index"], 1);
    }
}
