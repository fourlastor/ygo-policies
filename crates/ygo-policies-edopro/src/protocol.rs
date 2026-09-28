//! EDOPro's client/server packets (the YGOPro network protocol).
//!
//! Every packet is `[u16 length][u8 opcode][payload]`, `length` counting the
//! opcode.  During a duel the server forwards OCGCore messages in
//! `STOC_GAME_MSG` (already filtered for this player) and expects OCGCore
//! responses in `CTOS_RESPONSE`.

use std::io::{self, Read, Write};

pub mod ctos {
    pub const RESPONSE: u8 = 0x01;
    pub const UPDATE_DECK: u8 = 0x02;
    pub const HAND_RESULT: u8 = 0x03;
    pub const TP_RESULT: u8 = 0x04;
    pub const PLAYER_INFO: u8 = 0x10;
    pub const JOIN_GAME: u8 = 0x12;
    pub const TIME_CONFIRM: u8 = 0x15;
    pub const HS_READY: u8 = 0x22;
}

pub mod stoc {
    pub const GAME_MSG: u8 = 0x01;
    pub const ERROR_MSG: u8 = 0x02;
    pub const SELECT_HAND: u8 = 0x03;
    pub const SELECT_TP: u8 = 0x04;
    pub const HAND_RESULT: u8 = 0x05;
    pub const CHANGE_SIDE: u8 = 0x07;
    pub const WAITING_SIDE: u8 = 0x08;
    pub const JOIN_GAME: u8 = 0x12;
    pub const TYPE_CHANGE: u8 = 0x13;
    pub const LEAVE_GAME: u8 = 0x14;
    pub const DUEL_START: u8 = 0x15;
    pub const DUEL_END: u8 = 0x16;
    pub const REPLAY: u8 = 0x17;
    pub const TIME_LIMIT: u8 = 0x18;
    pub const CHAT: u8 = 0x19;
}

/// EDOPro 41.0 with OCGCore 11.0: `ClientVersion` as a little-endian `u32`
/// (client major, client minor, core major, core minor).
pub const DEFAULT_VERSION: u32 = 41 | 11 << 16;

pub fn write_packet(out: &mut impl Write, opcode: u8, payload: &[u8]) -> io::Result<()> {
    let length = u16::try_from(payload.len() + 1).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "packet too long"))?;
    let mut packet = Vec::with_capacity(payload.len() + 3);
    packet.extend_from_slice(&length.to_le_bytes());
    packet.push(opcode);
    packet.extend_from_slice(payload);
    out.write_all(&packet)?;
    out.flush()
}

pub fn read_packet(input: &mut impl Read) -> io::Result<(u8, Vec<u8>)> {
    let mut header = [0u8; 2];
    input.read_exact(&mut header)?;
    let length = u16::from_le_bytes(header) as usize;
    if length == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty packet"));
    }
    let mut packet = vec![0u8; length];
    input.read_exact(&mut packet)?;
    let opcode = packet.remove(0);
    Ok((opcode, packet))
}

/// A NUL-padded UTF-16 string of `characters` code units.
pub fn utf16(text: &str, characters: usize) -> Vec<u8> {
    let mut out: Vec<u8> = text.encode_utf16().take(characters - 1).flat_map(u16::to_le_bytes).collect();
    out.resize(characters * 2, 0);
    out
}

pub fn player_info(name: &str) -> Vec<u8> {
    utf16(name, 20)
}

pub fn join_game(version: u32, room_id: u32, password: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(version as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&room_id.to_le_bytes());
    out.extend_from_slice(&utf16(password, 20));
    out.extend_from_slice(&version.to_le_bytes());
    out
}

pub fn update_deck(main: &[u32], extra: &[u32], side: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&((main.len() + extra.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(side.len() as u32).to_le_bytes());
    for code in main.iter().chain(extra).chain(side) {
        out.extend_from_slice(&code.to_le_bytes());
    }
    out
}

/// A readable `STOC_ERROR_MSG`.
pub fn describe_error(payload: &[u8]) -> String {
    match payload {
        // VERERROR2: error type, three padding bytes, the server's ClientVersion.
        [5, _, _, _, client_major, client_minor, core_major, core_minor, ..] => format!(
            "EDOPro wants client {client_major}.{client_minor} with core {core_major}.{core_minor} \
             (pass --version 0x{:08x})",
            u32::from_le_bytes([*client_major, *client_minor, *core_major, *core_minor])
        ),
        [1, ..] => "cannot join the room (wrong password, room full or unknown room)".into(),
        [2, ..] => "the server rejected the deck".into(),
        _ => format!("server error {payload:02x?}"),
    }
}
