//! Parsed OCGCore messages: the ones the projection and the decisions use.
//! Everything else parses as [`Message::Other`].

use crate::query::{read_location, read_record, Query};
use crate::wire::{error, msg, Loc, Reader, Result};

/// A card offered by a selection message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offered {
    pub code: u32,
    pub loc: Loc,
    /// Tribute value, sum parameter or counters available, depending on the prompt.
    pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effect {
    pub code: u32,
    pub loc: Loc,
    pub description: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Idle {
    pub player: u8,
    pub summon: Vec<Offered>,
    pub special_summon: Vec<Offered>,
    pub reposition: Vec<Offered>,
    pub set_monster: Vec<Offered>,
    pub set_spell_trap: Vec<Offered>,
    pub activate: Vec<Effect>,
    pub battle_phase: bool,
    pub end_phase: bool,
    pub shuffle_hand: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Battle {
    pub player: u8,
    pub activate: Vec<Effect>,
    /// `value` is 1 when the monster may attack directly.
    pub attack: Vec<Offered>,
    pub main2: bool,
    pub end_phase: bool,
}

/// `MSG_SELECT_CARD`, `MSG_SELECT_TRIBUTE`, `MSG_SELECT_UNSELECT_CARD`, `MSG_SELECT_SUM`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cards {
    pub player: u8,
    pub cancelable: bool,
    pub finishable: bool,
    pub min: u32,
    pub max: u32,
    pub cards: Vec<Offered>,
    /// Already selected (unselect prompts) or required (sum prompts) cards.
    pub fixed: Vec<Offered>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Retry,
    Hint { kind: u8, player: u8, data: u64 },
    /// Not produced by OCGCore itself: EDOPro's server (and any front end that
    /// wants deck counts / its seat from the stream) sends it before the duel.
    Start { seat: u8, observer: bool, life_points: [u32; 2], deck: [u16; 2], extra: [u16; 2] },
    Win { player: u8, reason: u8 },
    UpdateData { player: u8, location: u8, cards: Vec<Option<Query>> },
    UpdateCard { player: u8, location: u8, sequence: u8, card: Option<Query> },

    SelectIdle(Idle),
    SelectBattle(Battle),
    SelectEffectYesNo { player: u8, code: u32, loc: Loc, description: u64 },
    SelectYesNo { player: u8, description: u64 },
    SelectOption { player: u8, options: Vec<u64> },
    SelectCard(Cards),
    SelectTribute(Cards),
    SelectUnselect(Cards),
    SelectSum { cards: Cards, exact: bool, target: u32 },
    /// `triggers`: only triggered effects are offered (the engine is building
    /// a chain of triggers), not a free chain window.
    SelectChain { player: u8, forced: bool, triggers: bool, chains: Vec<Effect> },
    SelectPlace { player: u8, count: u8, blocked: u32, disable: bool },
    SelectPosition { player: u8, code: u32, positions: u8 },
    SelectCounter { player: u8, kind: u16, count: u16, cards: Vec<Offered> },
    Sort { player: u8, cards: Vec<Offered>, chain: bool },
    AnnounceRace { player: u8, count: u8, available: u64 },
    AnnounceAttribute { player: u8, count: u8, available: u32 },
    AnnounceCard { player: u8, filter: Vec<u64> },
    AnnounceNumber { player: u8, values: Vec<u64> },
    RockPaperScissors { player: u8 },

    ConfirmCards { player: u8, cards: Vec<(u32, Loc)> },
    ShuffleHand { player: u8, codes: Vec<u32> },
    ShuffleExtra { player: u8, codes: Vec<u32> },
    ShuffleSetCard { location: u8, from: Vec<Loc>, to: Vec<Loc> },
    NewTurn { player: u8 },
    NewPhase { phase: u16 },
    Move { code: u32, from: Loc, to: Loc, reason: u32 },
    PosChange { code: u32, controller: u8, location: u8, sequence: u8, previous: u8, current: u8 },
    Set { code: u32, loc: Loc },
    Swap { first: (u32, Loc), second: (u32, Loc) },
    Summoning { code: u32, loc: Loc },
    SpecialSummoning { code: u32, loc: Loc },
    FlipSummoning { code: u32, loc: Loc },
    Summoned,
    Chaining { code: u32, loc: Loc, controller: u8, location: u8, sequence: u32, description: u64, size: u32 },
    Chained { link: u8 },
    ChainSolving { link: u8 },
    ChainSolved { link: u8 },
    ChainNegated { link: u8 },
    ChainEnd,
    BecomeTarget { cards: Vec<Loc> },
    Draw { player: u8, cards: Vec<(u32, u32)> },
    Damage { player: u8, amount: u32 },
    Recover { player: u8, amount: u32 },
    PayLpCost { player: u8, amount: u32 },
    LpUpdate { player: u8, life_points: u32 },
    Counter { kind: u16, controller: u8, location: u8, sequence: u8, count: u16, add: bool },
    Attack { attacker: Loc, target: Option<Loc> },
    BattleResult { attacker: Loc, attack: u32, defense: u32, target: Option<Loc>, target_attack: u32, target_defense: u32 },
    AttackDisabled,
    DamageStepEnd,
    Other(u8),
}

impl Message {
    /// The player that must answer, for messages that expect a response.
    pub fn responder(&self) -> Option<u8> {
        use Message::*;
        Some(match self {
            SelectIdle(m) => m.player,
            SelectBattle(m) => m.player,
            SelectCard(m) | SelectTribute(m) | SelectUnselect(m) => m.player,
            SelectSum { cards, .. } => cards.player,
            SelectEffectYesNo { player, .. }
            | SelectYesNo { player, .. }
            | SelectOption { player, .. }
            | SelectChain { player, .. }
            | SelectPlace { player, .. }
            | SelectPosition { player, .. }
            | SelectCounter { player, .. }
            | Sort { player, .. }
            | AnnounceRace { player, .. }
            | AnnounceAttribute { player, .. }
            | AnnounceCard { player, .. }
            | AnnounceNumber { player, .. }
            | RockPaperScissors { player } => *player,
            _ => return None,
        })
    }
}

/// Whether a message id expects a response (and only goes to its player).
pub fn is_selection(id: u8) -> bool {
    matches!(
        id,
        msg::SELECT_BATTLECMD..=msg::SELECT_UNSELECT_CARD
            | msg::ROCK_PAPER_SCISSORS
            | msg::ANNOUNCE_RACE..=msg::ANNOUNCE_NUMBER
    )
}

fn effect(r: &mut Reader, with_position: bool) -> Result<Effect> {
    let code = r.u32()?;
    let loc = if with_position {
        r.loc()?
    } else {
        Loc { controller: r.u8()?, location: r.u8()?, sequence: r.u32()?, position: 0 }
    };
    let description = r.u64()?;
    let _client_mode = r.u8()?;
    Ok(Effect { code, loc, description })
}

/// `[code u32][controller u8][location u8][sequence u32|u8]` idle/battle entries.
fn short_card(r: &mut Reader, byte_sequence: bool) -> Result<Offered> {
    let code = r.u32()?;
    let controller = r.u8()?;
    let location = r.u8()?;
    let sequence = if byte_sequence { r.u8()? as u32 } else { r.u32()? };
    Ok(Offered { code, loc: Loc { controller, location, sequence, position: 0 }, value: 0 })
}

fn list<T>(r: &mut Reader, size: usize, mut item: impl FnMut(&mut Reader) -> Result<T>) -> Result<Vec<T>> {
    let n = r.count(size)?;
    (0..n).map(|_| item(r)).collect()
}

fn offered(r: &mut Reader) -> Result<Offered> {
    Ok(Offered { code: r.u32()?, loc: r.loc()?, value: 0 })
}

fn optional(loc: Loc) -> Option<Loc> {
    (!loc.is_none()).then_some(loc)
}

pub fn parse(message: &[u8]) -> Result<Message> {
    let (&id, body) = message.split_first().ok_or_else(|| error("empty message"))?;
    let mut r = Reader::new(body);
    let r = &mut r;
    use Message::*;
    Ok(match id {
        msg::RETRY => Retry,
        msg::HINT => Hint { kind: r.u8()?, player: r.u8()?, data: r.u64()? },
        msg::START => {
            let kind = r.u8()?;
            let life_points = [r.u32()?, r.u32()?];
            let (deck0, extra0, deck1, extra1) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?);
            Start { seat: kind & 0x0f, observer: kind & 0xf0 != 0, life_points, deck: [deck0, deck1], extra: [extra0, extra1] }
        }
        msg::WIN => Win { player: r.u8()?, reason: r.u8()? },
        msg::UPDATE_DATA => {
            let player = r.u8()?;
            let location = r.u8()?;
            let records = read_location(r.bytes(r.remaining())?)?;
            UpdateData { player, location, cards: records.iter().map(|c| c.as_ref().map(|c| c.parse())).collect() }
        }
        msg::UPDATE_CARD => {
            let player = r.u8()?;
            let location = r.u8()?;
            let sequence = r.u8()?;
            let record = read_record(r)?;
            UpdateCard { player, location, sequence, card: record.map(|c| c.parse()) }
        }
        msg::SELECT_IDLECMD => SelectIdle(Idle {
            player: r.u8()?,
            summon: list(r, 10, |r| short_card(r, false))?,
            special_summon: list(r, 10, |r| short_card(r, false))?,
            reposition: list(r, 7, |r| short_card(r, true))?,
            set_monster: list(r, 10, |r| short_card(r, false))?,
            set_spell_trap: list(r, 10, |r| short_card(r, false))?,
            activate: list(r, 19, |r| effect(r, false))?,
            battle_phase: r.bool()?,
            end_phase: r.bool()?,
            shuffle_hand: r.bool()?,
        }),
        msg::SELECT_BATTLECMD => SelectBattle(Battle {
            player: r.u8()?,
            activate: list(r, 19, |r| effect(r, false))?,
            attack: list(r, 8, |r| {
                let mut card = short_card(r, true)?;
                card.value = r.u8()? as u32;
                Ok(card)
            })?,
            main2: r.bool()?,
            end_phase: r.bool()?,
        }),
        msg::SELECT_EFFECTYN => {
            SelectEffectYesNo { player: r.u8()?, code: r.u32()?, loc: r.loc()?, description: r.u64()? }
        }
        msg::SELECT_YESNO => SelectYesNo { player: r.u8()?, description: r.u64()? },
        msg::SELECT_OPTION => {
            let player = r.u8()?;
            let n = r.u8()?;
            SelectOption { player, options: (0..n).map(|_| r.u64()).collect::<Result<_>>()? }
        }
        msg::SELECT_CARD => {
            let player = r.u8()?;
            let cancelable = r.bool()?;
            let (min, max) = (r.u32()?, r.u32()?);
            let cards = list(r, 14, offered)?;
            SelectCard(Cards { player, cancelable, finishable: false, min, max, cards, fixed: Vec::new() })
        }
        msg::SELECT_TRIBUTE => {
            let player = r.u8()?;
            let cancelable = r.bool()?;
            let (min, max) = (r.u32()?, r.u32()?);
            let cards = list(r, 11, |r| {
                let mut card = short_card(r, false)?;
                card.value = r.u8()? as u32;
                Ok(card)
            })?;
            SelectTribute(Cards { player, cancelable, finishable: false, min, max, cards, fixed: Vec::new() })
        }
        msg::SELECT_UNSELECT_CARD => {
            let player = r.u8()?;
            let finishable = r.bool()?;
            let cancelable = r.bool()?;
            let (min, max) = (r.u32()?, r.u32()?);
            let cards = list(r, 14, offered)?;
            let fixed = list(r, 14, offered)?;
            SelectUnselect(Cards { player, cancelable, finishable, min, max, cards, fixed })
        }
        msg::SELECT_SUM => {
            let player = r.u8()?;
            // 0: pick between min and max cards that hit `target` exactly;
            // 1: pick cards whose sum reaches `target` with nothing superfluous.
            let exact = r.u8()? == 0;
            let target = r.u32()?;
            let (min, max) = (r.u32()?, r.u32()?);
            let summed = |r: &mut Reader| -> Result<Offered> {
                let mut card = offered(r)?;
                card.value = r.u32()?;
                Ok(card)
            };
            let fixed = list(r, 18, summed)?;
            let cards = list(r, 18, summed)?;
            SelectSum {
                cards: Cards { player, cancelable: false, finishable: false, min, max, cards, fixed },
                exact,
                target,
            }
        }
        msg::SELECT_CHAIN => {
            let player = r.u8()?;
            // OCGCore writes 0x7f here when it asks which trigger goes on the chain.
            let triggers = r.u8()? == 0x7f;
            let forced = r.bool()?;
            let _hint_timing = (r.u32()?, r.u32()?);
            SelectChain { player, forced, triggers, chains: list(r, 23, |r| effect(r, true))? }
        }
        msg::SELECT_PLACE | msg::SELECT_DISFIELD => SelectPlace {
            player: r.u8()?,
            count: r.u8()?,
            blocked: r.u32()?,
            disable: id == msg::SELECT_DISFIELD,
        },
        msg::SELECT_POSITION => SelectPosition { player: r.u8()?, code: r.u32()?, positions: r.u8()? },
        msg::SELECT_COUNTER => {
            let player = r.u8()?;
            let kind = r.u16()?;
            let count = r.u16()?;
            let cards = list(r, 9, |r| {
                let mut card = short_card(r, true)?;
                card.value = r.u16()? as u32;
                Ok(card)
            })?;
            SelectCounter { player, kind, count, cards }
        }
        msg::SORT_CARD | msg::SORT_CHAIN => {
            let player = r.u8()?;
            let cards = list(r, 13, |r| {
                let code = r.u32()?;
                let controller = r.u8()?;
                let location = r.u32()? as u8;
                let sequence = r.u32()?;
                Ok(Offered { code, loc: Loc { controller, location, sequence, position: 0 }, value: 0 })
            })?;
            Sort { player, cards, chain: id == msg::SORT_CHAIN }
        }
        msg::ANNOUNCE_RACE => AnnounceRace { player: r.u8()?, count: r.u8()?, available: r.u64()? },
        msg::ANNOUNCE_ATTRIB => AnnounceAttribute { player: r.u8()?, count: r.u8()?, available: r.u32()? },
        msg::ANNOUNCE_CARD | msg::ANNOUNCE_NUMBER => {
            let player = r.u8()?;
            let n = r.u8()?;
            let values = (0..n).map(|_| r.u64()).collect::<Result<Vec<_>>>()?;
            if id == msg::ANNOUNCE_CARD {
                AnnounceCard { player, filter: values }
            } else {
                AnnounceNumber { player, values }
            }
        }
        msg::ROCK_PAPER_SCISSORS => RockPaperScissors { player: r.u8()? },

        msg::CONFIRM_CARDS => {
            let player = r.u8()?;
            let cards = list(r, 10, |r| {
                let code = r.u32()?;
                let controller = r.u8()?;
                let location = r.u8()?;
                let sequence = r.u32()?;
                Ok((code, Loc { controller, location, sequence, position: 0 }))
            })?;
            ConfirmCards { player, cards }
        }
        msg::SHUFFLE_HAND | msg::SHUFFLE_EXTRA => {
            let player = r.u8()?;
            let codes = list(r, 4, |r| r.u32())?;
            if id == msg::SHUFFLE_HAND {
                ShuffleHand { player, codes }
            } else {
                ShuffleExtra { player, codes }
            }
        }
        msg::SHUFFLE_SET_CARD => {
            let location = r.u8()?;
            let n = r.u8()?;
            let from = (0..n).map(|_| r.loc()).collect::<Result<_>>()?;
            let to = (0..n).map(|_| r.loc()).collect::<Result<_>>()?;
            ShuffleSetCard { location, from, to }
        }
        msg::NEW_TURN => NewTurn { player: r.u8()? },
        msg::NEW_PHASE => NewPhase { phase: r.u16()? },
        msg::MOVE => Move { code: r.u32()?, from: r.loc()?, to: r.loc()?, reason: r.u32()? },
        msg::POS_CHANGE => PosChange {
            code: r.u32()?,
            controller: r.u8()?,
            location: r.u8()?,
            sequence: r.u8()?,
            previous: r.u8()?,
            current: r.u8()?,
        },
        msg::SET => Set { code: r.u32()?, loc: r.loc()? },
        msg::SWAP => Swap { first: (r.u32()?, r.loc()?), second: (r.u32()?, r.loc()?) },
        msg::SUMMONING => Summoning { code: r.u32()?, loc: r.loc()? },
        msg::SPSUMMONING => SpecialSummoning { code: r.u32()?, loc: r.loc()? },
        msg::FLIPSUMMONING => FlipSummoning { code: r.u32()?, loc: r.loc()? },
        msg::SUMMONED | msg::SPSUMMONED | msg::FLIPSUMMONED => Summoned,
        msg::CHAINING => Chaining {
            code: r.u32()?,
            loc: r.loc()?,
            controller: r.u8()?,
            location: r.u8()?,
            sequence: r.u32()?,
            description: r.u64()?,
            size: r.u32()?,
        },
        msg::CHAINED => Chained { link: r.u8()? },
        msg::CHAIN_SOLVING => ChainSolving { link: r.u8()? },
        msg::CHAIN_SOLVED => ChainSolved { link: r.u8()? },
        msg::CHAIN_NEGATED | msg::CHAIN_DISABLED => ChainNegated { link: r.u8()? },
        msg::CHAIN_END => ChainEnd,
        msg::BECOME_TARGET => BecomeTarget { cards: list(r, Loc::SIZE, |r| r.loc())? },
        msg::DRAW => {
            let player = r.u8()?;
            Draw { player, cards: list(r, 8, |r| Ok((r.u32()?, r.u32()?)))? }
        }
        msg::DAMAGE => Damage { player: r.u8()?, amount: r.u32()? },
        msg::RECOVER => Recover { player: r.u8()?, amount: r.u32()? },
        msg::PAY_LPCOST => PayLpCost { player: r.u8()?, amount: r.u32()? },
        msg::LPUPDATE => LpUpdate { player: r.u8()?, life_points: r.u32()? },
        msg::ADD_COUNTER | msg::REMOVE_COUNTER => Counter {
            kind: r.u16()?,
            controller: r.u8()?,
            location: r.u8()?,
            sequence: r.u8()?,
            count: r.u16()?,
            add: id == msg::ADD_COUNTER,
        },
        msg::ATTACK => Attack { attacker: r.loc()?, target: optional(r.loc()?) },
        msg::BATTLE => {
            let attacker = r.loc()?;
            let (attack, defense, _) = (r.u32()?, r.u32()?, r.u8()?);
            let target = optional(r.loc()?);
            let (target_attack, target_defense) = (r.u32()?, r.u32()?);
            BattleResult { attacker, attack, defense, target, target_attack, target_defense }
        }
        msg::ATTACK_DISABLED => AttackDisabled,
        msg::DAMAGE_STEP_END => DamageStepEnd,
        other => Other(other),
    })
}

/// `MSG_START` as EDOPro's server writes it, for front ends that drive
/// OCGCore directly: `seat` is the receiving player.
pub fn start_message(seat: u8, life_points: [u32; 2], deck: [u16; 2], extra: [u16; 2]) -> Vec<u8> {
    let mut out = vec![msg::START, seat];
    out.extend_from_slice(&life_points[0].to_le_bytes());
    out.extend_from_slice(&life_points[1].to_le_bytes());
    for player in 0..2 {
        out.extend_from_slice(&deck[player].to_le_bytes());
        out.extend_from_slice(&extra[player].to_le_bytes());
    }
    out
}
