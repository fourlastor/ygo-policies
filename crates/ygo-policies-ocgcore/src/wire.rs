//! OCGCore's little-endian wire primitives and constants.
//!
//! Layouts follow the edo9300 `ygopro-core` used by EDOPro (`ocgapi.h`):
//! OCGCore writes them, EDOPro's server forwards them to its clients.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolError(pub String);

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ProtocolError {}

pub type Result<T> = std::result::Result<T, ProtocolError>;

pub fn error(message: impl Into<String>) -> ProtocolError {
    ProtocolError(message.into())
}

pub mod msg {
    pub const RETRY: u8 = 1;
    pub const HINT: u8 = 2;
    pub const WAITING: u8 = 3;
    pub const START: u8 = 4;
    pub const WIN: u8 = 5;
    pub const UPDATE_DATA: u8 = 6;
    pub const UPDATE_CARD: u8 = 7;
    pub const SELECT_BATTLECMD: u8 = 10;
    pub const SELECT_IDLECMD: u8 = 11;
    pub const SELECT_EFFECTYN: u8 = 12;
    pub const SELECT_YESNO: u8 = 13;
    pub const SELECT_OPTION: u8 = 14;
    pub const SELECT_CARD: u8 = 15;
    pub const SELECT_CHAIN: u8 = 16;
    pub const SELECT_PLACE: u8 = 18;
    pub const SELECT_POSITION: u8 = 19;
    pub const SELECT_TRIBUTE: u8 = 20;
    pub const SORT_CHAIN: u8 = 21;
    pub const SELECT_COUNTER: u8 = 22;
    pub const SELECT_SUM: u8 = 23;
    pub const SELECT_DISFIELD: u8 = 24;
    pub const SORT_CARD: u8 = 25;
    pub const SELECT_UNSELECT_CARD: u8 = 26;
    pub const CONFIRM_DECKTOP: u8 = 30;
    pub const CONFIRM_CARDS: u8 = 31;
    pub const SHUFFLE_DECK: u8 = 32;
    pub const SHUFFLE_HAND: u8 = 33;
    pub const REFRESH_DECK: u8 = 34;
    pub const SWAP_GRAVE_DECK: u8 = 35;
    pub const SHUFFLE_SET_CARD: u8 = 36;
    pub const REVERSE_DECK: u8 = 37;
    pub const DECK_TOP: u8 = 38;
    pub const SHUFFLE_EXTRA: u8 = 39;
    pub const NEW_TURN: u8 = 40;
    pub const NEW_PHASE: u8 = 41;
    pub const CONFIRM_EXTRATOP: u8 = 42;
    pub const MOVE: u8 = 50;
    pub const POS_CHANGE: u8 = 53;
    pub const SET: u8 = 54;
    pub const SWAP: u8 = 55;
    pub const FIELD_DISABLED: u8 = 56;
    pub const SUMMONING: u8 = 60;
    pub const SUMMONED: u8 = 61;
    pub const SPSUMMONING: u8 = 62;
    pub const SPSUMMONED: u8 = 63;
    pub const FLIPSUMMONING: u8 = 64;
    pub const FLIPSUMMONED: u8 = 65;
    pub const CHAINING: u8 = 70;
    pub const CHAINED: u8 = 71;
    pub const CHAIN_SOLVING: u8 = 72;
    pub const CHAIN_SOLVED: u8 = 73;
    pub const CHAIN_END: u8 = 74;
    pub const CHAIN_NEGATED: u8 = 75;
    pub const CHAIN_DISABLED: u8 = 76;
    pub const CARD_SELECTED: u8 = 80;
    pub const RANDOM_SELECTED: u8 = 81;
    pub const BECOME_TARGET: u8 = 83;
    pub const DRAW: u8 = 90;
    pub const DAMAGE: u8 = 91;
    pub const RECOVER: u8 = 92;
    pub const EQUIP: u8 = 93;
    pub const LPUPDATE: u8 = 94;
    pub const UNEQUIP: u8 = 95;
    pub const CARD_TARGET: u8 = 96;
    pub const CANCEL_TARGET: u8 = 97;
    pub const PAY_LPCOST: u8 = 100;
    pub const ADD_COUNTER: u8 = 101;
    pub const REMOVE_COUNTER: u8 = 102;
    pub const ATTACK: u8 = 110;
    pub const BATTLE: u8 = 111;
    pub const ATTACK_DISABLED: u8 = 112;
    pub const DAMAGE_STEP_START: u8 = 113;
    pub const DAMAGE_STEP_END: u8 = 114;
    pub const MISSED_EFFECT: u8 = 120;
    pub const BE_CHAIN_TARGET: u8 = 121;
    pub const CREATE_RELATION: u8 = 122;
    pub const RELEASE_RELATION: u8 = 123;
    pub const TOSS_COIN: u8 = 130;
    pub const TOSS_DICE: u8 = 131;
    pub const ROCK_PAPER_SCISSORS: u8 = 132;
    pub const HAND_RES: u8 = 133;
    pub const ANNOUNCE_RACE: u8 = 140;
    pub const ANNOUNCE_ATTRIB: u8 = 141;
    pub const ANNOUNCE_CARD: u8 = 142;
    pub const ANNOUNCE_NUMBER: u8 = 143;
    pub const CARD_HINT: u8 = 160;
    pub const TAG_SWAP: u8 = 161;
    pub const RELOAD_FIELD: u8 = 162;
    pub const AI_NAME: u8 = 163;
    pub const SHOW_HINT: u8 = 164;
    pub const PLAYER_HINT: u8 = 165;
    pub const MATCH_KILL: u8 = 170;
    pub const CUSTOM_MSG: u8 = 180;
    pub const REMOVE_CARDS: u8 = 190;
}

pub mod location {
    pub const DECK: u8 = 0x01;
    pub const HAND: u8 = 0x02;
    pub const MZONE: u8 = 0x04;
    pub const SZONE: u8 = 0x08;
    pub const GRAVE: u8 = 0x10;
    pub const REMOVED: u8 = 0x20;
    pub const EXTRA: u8 = 0x40;
    pub const OVERLAY: u8 = 0x80;
}

pub mod position {
    pub const FACEUP_ATTACK: u32 = 0x1;
    pub const FACEDOWN_ATTACK: u32 = 0x2;
    pub const FACEUP_DEFENSE: u32 = 0x4;
    pub const FACEDOWN_DEFENSE: u32 = 0x8;
    pub const FACEUP: u32 = FACEUP_ATTACK | FACEUP_DEFENSE;
    pub const FACEDOWN: u32 = FACEDOWN_ATTACK | FACEDOWN_DEFENSE;
}

pub mod query {
    pub const CODE: u32 = 0x1;
    pub const POSITION: u32 = 0x2;
    pub const ALIAS: u32 = 0x4;
    pub const TYPE: u32 = 0x8;
    pub const LEVEL: u32 = 0x10;
    pub const RANK: u32 = 0x20;
    pub const ATTRIBUTE: u32 = 0x40;
    pub const RACE: u32 = 0x80;
    pub const ATTACK: u32 = 0x100;
    pub const DEFENSE: u32 = 0x200;
    pub const BASE_ATTACK: u32 = 0x400;
    pub const BASE_DEFENSE: u32 = 0x800;
    pub const REASON: u32 = 0x1000;
    pub const REASON_CARD: u32 = 0x2000;
    pub const EQUIP_CARD: u32 = 0x4000;
    pub const TARGET_CARD: u32 = 0x8000;
    pub const OVERLAY_CARD: u32 = 0x10000;
    pub const COUNTERS: u32 = 0x20000;
    pub const OWNER: u32 = 0x40000;
    pub const STATUS: u32 = 0x80000;
    pub const IS_PUBLIC: u32 = 0x100000;
    pub const LSCALE: u32 = 0x200000;
    pub const RSCALE: u32 = 0x400000;
    pub const LINK: u32 = 0x800000;
    pub const IS_HIDDEN: u32 = 0x1000000;
    pub const COVER: u32 = 0x2000000;
    pub const END: u32 = 0x80000000;

    /// What a front end should ask `OCG_DuelQueryLocation` for when it
    /// forwards `MSG_UPDATE_DATA` (the fields the projection uses).
    pub const RECOMMENDED: u32 = CODE | POSITION | LEVEL | RANK | ATTACK | DEFENSE | COUNTERS | IS_PUBLIC;
}

pub mod hint {
    pub const EVENT: u8 = 1;
    pub const MESSAGE: u8 = 2;
    pub const SELECTMSG: u8 = 3;
    pub const OPSELECTED: u8 = 4;
    pub const EFFECT: u8 = 5;
    pub const RACE: u8 = 6;
    pub const ATTRIB: u8 = 7;
    pub const CODE: u8 = 8;
    pub const NUMBER: u8 = 9;
    pub const CARD: u8 = 10;
    pub const ZONE: u8 = 11;
}

/// OCGCore's `loc_info`: controller, location, sequence, position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Loc {
    pub controller: u8,
    pub location: u8,
    pub sequence: u32,
    pub position: u32,
}

impl Loc {
    pub const SIZE: usize = 10;

    pub fn is_none(&self) -> bool {
        self.location == 0
    }

    pub fn face_up(&self) -> bool {
        self.position & position::FACEUP != 0
    }

    pub fn face_down(&self) -> bool {
        self.position & position::FACEDOWN != 0
    }
}

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(error(format!("message truncated: wanted {n} bytes at offset {}", self.pos)));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }

    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }

    pub fn loc(&mut self) -> Result<Loc> {
        Ok(Loc { controller: self.u8()?, location: self.u8()?, sequence: self.u32()?, position: self.u32()? })
    }

    /// A `u32` element count, rejected when it could not possibly fit.
    pub fn count(&mut self, element_size: usize) -> Result<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(element_size) > self.remaining() {
            return Err(error(format!("implausible element count {n}")));
        }
        Ok(n)
    }
}

/// Splits an `OCG_DuelGetMessage` buffer (`u32` length + message, repeated).
pub fn split_messages(buffer: &[u8]) -> Result<Vec<&[u8]>> {
    let mut reader = Reader::new(buffer);
    let mut messages = Vec::new();
    while reader.remaining() > 0 {
        let length = reader.u32()? as usize;
        messages.push(reader.bytes(length)?);
    }
    Ok(messages)
}

/// Little-endian response writer.
#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    pub fn u8(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    pub fn u16(mut self, v: u16) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u32(mut self, v: u32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn i32(mut self, v: i32) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u64(mut self, v: u64) -> Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn loc(self, l: Loc) -> Self {
        self.u8(l.controller).u8(l.location).u32(l.sequence).u32(l.position)
    }
}
