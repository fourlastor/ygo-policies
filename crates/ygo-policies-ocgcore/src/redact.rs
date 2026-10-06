//! The information boundary: what one player may see of an engine message.
//!
//! OCGCore writes every message with full knowledge (the drawn card, the
//! monster Set face-down, the opponent's selection prompts).  A duel server
//! filters them per player before they reach a client; [`redact`] applies the
//! same rules as YGOPro / EDOPro servers, so a seat fed straight from OCGCore
//! sees exactly what an EDOPro client would.  Redacting an already filtered
//! stream (EDOPro's) changes nothing.
//!
//! A message is either dropped (not delivered to this player at all) or
//! delivered with the codes of cards the player cannot see set to 0.

use std::borrow::Cow;

use crate::message::is_selection;
use crate::query::{read_location, read_record, RawRecord};
use crate::wire::{error, hint, location, msg, position, query, Reader, Result};

/// `viewer` is the seat the message is for; `None` for a spectator.
/// Returns `None` when the message is not delivered to that seat.
pub fn redact(message: &[u8], viewer: Option<u8>) -> Result<Option<Cow<'_, [u8]>>> {
    let Some((&id, body)) = message.split_first() else { return Err(error("empty message")) };
    let is = |player: u8| viewer == Some(player);
    let mut out = Cow::Borrowed(message);
    match id {
        msg::HINT => {
            let (kind, player) = (byte(body, 0)?, byte(body, 1)?);
            let delivered = match kind {
                hint::EVENT | hint::MESSAGE | hint::SELECTMSG | hint::EFFECT => is(player),
                hint::OPSELECTED | hint::RACE | hint::ATTRIB | hint::CODE | hint::NUMBER | hint::ZONE => !is(player),
                hint::CARD => true,
                _ => is(player),
            };
            if !delivered {
                return Ok(None);
            }
        }
        id if is_selection(id) => {
            let player = byte(body, 0)?;
            if !is(player) {
                return Ok(None);
            }
            // Servers blank the codes of the other player's cards in a prompt.
            let buf = out.to_mut();
            let body = &mut buf[1..];
            match id {
                msg::SELECT_CARD => {
                    blank_list(body, 10, 14, 4, player)?;
                }
                msg::SELECT_TRIBUTE => {
                    blank_list(body, 10, 11, 4, player)?;
                }
                msg::SELECT_UNSELECT_CARD => {
                    let end = blank_list(body, 11, 14, 4, player)?;
                    blank_list(body, end, 14, 4, player)?;
                }
                msg::SELECT_SUM => {
                    let end = blank_list(body, 14, 18, 4, player)?;
                    blank_list(body, end, 18, 4, player)?;
                }
                msg::SELECT_COUNTER => {
                    blank_list(body, 5, 9, 4, player)?;
                }
                msg::SORT_CARD | msg::SORT_CHAIN => {
                    blank_list(body, 1, 13, 4, player)?;
                }
                _ => {}
            }
        }
        msg::MISSED_EFFECT => {
            if !is(byte(body, 0)?) {
                return Ok(None);
            }
        }
        // Tag duels and reconnection snapshots: not supported, and they carry
        // the full hidden state.
        msg::TAG_SWAP | msg::RELOAD_FIELD => return Ok(None),
        msg::DRAW => {
            let player = byte(body, 0)?;
            if !is(player) {
                let n = u32_at(body, 1)? as usize;
                let buf = out.to_mut();
                for k in 0..n {
                    let at = 1 + 5 + 8 * k;
                    if u32_at(buf, at + 4)? & position::FACEUP == 0 {
                        zero(buf, at)?;
                    }
                }
            }
        }
        msg::MOVE => {
            let (to_controller, to_location, to_position) = (byte(body, 14)?, byte(body, 15)?, u32_at(body, 20)?);
            let hidden = to_location & (location::GRAVE | location::OVERLAY) == 0
                && (to_location & (location::DECK | location::HAND) != 0 || to_position & position::FACEDOWN != 0);
            if hidden && !is(to_controller) {
                zero(out.to_mut(), 1)?;
            }
        }
        msg::SET => zero(out.to_mut(), 1)?,
        msg::SHUFFLE_HAND | msg::SHUFFLE_EXTRA => {
            if !is(byte(body, 0)?) {
                let n = u32_at(body, 1)? as usize;
                let buf = out.to_mut();
                for k in 0..n {
                    zero(buf, 1 + 5 + 4 * k)?;
                }
            }
        }
        msg::POS_CHANGE => {
            if !is(byte(body, 4)?) && byte(body, 8)? as u32 & position::FACEDOWN != 0 {
                zero(out.to_mut(), 1)?;
            }
        }
        msg::SWAP => {
            for at in [0usize, 14] {
                if !is(byte(body, at + 4)?) && u32_at(body, at + 10)? & position::FACEDOWN != 0 {
                    zero(out.to_mut(), 1 + at)?;
                }
            }
        }
        msg::DECK_TOP => {
            if u32_at(body, 9)? & position::FACEUP == 0 {
                zero(out.to_mut(), 1 + 5)?;
            }
        }
        msg::CONFIRM_CARDS => {
            // Cards confirmed from the Deck are shown to `player` alone.
            let n = u32_at(body, 1)?;
            if n > 0 && byte(body, 5 + 5)? == location::DECK && !is(byte(body, 0)?) {
                return Ok(None);
            }
        }
        msg::UPDATE_DATA => {
            let (player, loc) = (byte(body, 0)?, byte(body, 1)?);
            let records = read_location(&body[2..])?;
            let mut payload = Vec::new();
            for (sequence, record) in records.iter().enumerate() {
                write_visible(record.as_ref(), player, loc, sequence, viewer, &mut payload);
            }
            let mut rebuilt = vec![id, player, loc];
            rebuilt.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            rebuilt.extend_from_slice(&payload);
            out = Cow::Owned(rebuilt);
        }
        msg::UPDATE_CARD => {
            let (player, loc, sequence) = (byte(body, 0)?, byte(body, 1)?, byte(body, 2)?);
            let record = read_record(&mut Reader::new(&body[3..]))?;
            let mut rebuilt = vec![id, player, loc, sequence];
            write_visible(record.as_ref(), player, loc, sequence as usize, viewer, &mut rebuilt);
            out = Cow::Owned(rebuilt);
        }
        _ => {}
    }
    Ok(Some(out))
}

/// Whether `viewer` may see a card's identity and stats.
pub fn visible(record: &RawRecord, owner: u8, loc: u8, viewer: Option<u8>) -> bool {
    visible_at(record.position().unwrap_or(0), record.is_public(), owner, loc, viewer)
}

fn visible_at(position: u32, public: bool, owner: u8, loc: u8, viewer: Option<u8>) -> bool {
    let face_up = position & position::FACEUP != 0;
    if viewer == Some(owner) {
        return loc != location::DECK;
    }
    match loc {
        location::GRAVE | location::OVERLAY => true,
        location::MZONE | location::SZONE | location::REMOVED => face_up,
        location::HAND | location::EXTRA => public || (loc == location::EXTRA && face_up),
        _ => false,
    }
}

/// Filter a decoded update with the same visibility rules as the wire path.
/// No encoded intermediate buffer is needed before applying it to a projection.
pub fn filter_update(message: &mut crate::Message, viewer: Option<u8>) -> Result<()> {
    let hide = |card: &mut Option<crate::query::Query>, owner, loc| {
        if let Some(card) = card {
            if !visible_at(card.position.unwrap_or(0), card.is_public.unwrap_or(false), owner, loc, viewer) {
                *card = crate::query::Query { position: card.position, ..Default::default() };
            }
        }
    };
    match message {
        crate::Message::UpdateData { player, location, cards } => {
            for card in cards { hide(card, *player, *location); }
        }
        crate::Message::UpdateCard { player, location, card, .. } => hide(card, *player, *location),
        _ => return Err(error("expected a card update")),
    }
    Ok(())
}

fn write_visible(record: Option<&RawRecord>, owner: u8, loc: u8, _sequence: usize, viewer: Option<u8>, out: &mut Vec<u8>) {
    match record {
        None => out.extend_from_slice(&0u16.to_le_bytes()),
        Some(record) if visible(record, owner, loc, viewer) => record.write(|_| true, out),
        // A hidden card still shows where it is and which way it faces.
        Some(record) => record.write(|flag| flag == query::POSITION, out),
    }
}

fn byte(body: &[u8], at: usize) -> Result<u8> {
    body.get(at).copied().ok_or_else(|| error("message truncated"))
}

fn u32_at(body: &[u8], at: usize) -> Result<u32> {
    body.get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| error("message truncated"))
}

fn zero(buf: &mut [u8], at: usize) -> Result<()> {
    buf.get_mut(at..at + 4).ok_or_else(|| error("message truncated"))?.fill(0);
    Ok(())
}

/// Blank the codes of cards `viewer` does not control in a `u32`-counted
/// list at `count_at` (entries of `stride` bytes, controller at `controller_at`).
/// Returns the offset just past the list.
fn blank_list(body: &mut [u8], count_at: usize, stride: usize, controller_at: usize, viewer: u8) -> Result<usize> {
    let n = u32_at(body, count_at)? as usize;
    let start = count_at + 4;
    for k in 0..n {
        let entry = start + k * stride;
        if byte(body, entry + controller_at)? != viewer {
            zero(body, entry)?;
        }
    }
    Ok(start + n * stride)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{Loc, Writer};

    fn draw(player: u8, code: u32, position: u32) -> Vec<u8> {
        Writer::default().u8(msg::DRAW).u8(player).u32(1).u32(code).u32(position).0
    }

    fn moved(code: u32, to: Loc) -> Vec<u8> {
        let from = Loc { controller: to.controller, location: location::DECK, sequence: 0, position: position::FACEDOWN_DEFENSE };
        Writer::default().u8(msg::MOVE).u32(code).loc(from).loc(to).u32(0).0
    }

    fn code_at(message: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(message[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn typed_updates_match_wire_redaction_for_every_visibility_case() {
        for owner in 0..2 {
            for viewer in [None, Some(0), Some(1)] {
                for loc in [location::DECK, location::HAND, location::MZONE, location::SZONE,
                            location::GRAVE, location::REMOVED, location::EXTRA, location::OVERLAY] {
                    for pos in [0, position::FACEUP_ATTACK, position::FACEDOWN_DEFENSE] {
                        for public in [0u8, 1] {
                            let mut record = Vec::new();
                            for (flag, value) in [(query::CODE, 12345), (query::POSITION, pos), (query::ATTACK, 1800),
                                                   (query::DEFENSE, 1000), (query::LEVEL, 4), (query::RANK, 0)] {
                                record.extend_from_slice(&8u16.to_le_bytes());
                                record.extend_from_slice(&flag.to_le_bytes());
                                record.extend_from_slice(&value.to_le_bytes());
                            }
                            record.extend_from_slice(&5u16.to_le_bytes());
                            record.extend_from_slice(&query::IS_PUBLIC.to_le_bytes());
                            record.push(public);
                            record.extend_from_slice(&4u16.to_le_bytes());
                            record.extend_from_slice(&query::END.to_le_bytes());
                            let mut location_update = vec![msg::UPDATE_DATA, owner, loc];
                            location_update.extend_from_slice(&((record.len() + 2) as u32).to_le_bytes());
                            location_update.extend_from_slice(&record);
                            location_update.extend_from_slice(&0u16.to_le_bytes()); // empty slot
                            let mut card_update = vec![msg::UPDATE_CARD, owner, loc, 2];
                            card_update.extend_from_slice(&record);
                            for bytes in [location_update, card_update] {
                                let wire = redact(&bytes, viewer).unwrap().unwrap();
                                let expected = crate::message::parse(&wire).unwrap();
                                let mut typed = crate::message::parse(&bytes).unwrap();
                                filter_update(&mut typed, viewer).unwrap();
                                assert_eq!(typed, expected, "owner {owner}, viewer {viewer:?}, loc {loc}, pos {pos}, public {public}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_other_players_draws_are_hidden_unless_face_up() {
        let hidden = draw(1, 12345, position::FACEDOWN_DEFENSE);
        assert_eq!(code_at(&redact(&hidden, Some(1)).unwrap().unwrap(), 6), 12345);
        assert_eq!(code_at(&redact(&hidden, Some(0)).unwrap().unwrap(), 6), 0);
        assert_eq!(code_at(&redact(&hidden, None).unwrap().unwrap(), 6), 0);
        let public = draw(1, 12345, position::FACEUP_ATTACK);
        assert_eq!(code_at(&redact(&public, Some(0)).unwrap().unwrap(), 6), 12345);
    }

    #[test]
    fn cards_reaching_a_hidden_place_lose_their_code() {
        let to_hand = moved(777, Loc { controller: 1, location: location::HAND, sequence: 0, position: 0 });
        assert_eq!(code_at(&redact(&to_hand, Some(0)).unwrap().unwrap(), 1), 0);
        assert_eq!(code_at(&redact(&to_hand, Some(1)).unwrap().unwrap(), 1), 777);
        let set = moved(777, Loc { controller: 1, location: location::SZONE, sequence: 2, position: position::FACEDOWN_DEFENSE });
        assert_eq!(code_at(&redact(&set, Some(0)).unwrap().unwrap(), 1), 0);
        let to_grave = moved(777, Loc { controller: 1, location: location::GRAVE, sequence: 0, position: position::FACEUP_ATTACK });
        assert_eq!(code_at(&redact(&to_grave, Some(0)).unwrap().unwrap(), 1), 777);
    }

    #[test]
    fn prompts_reach_only_their_player() {
        let prompt = Writer::default().u8(msg::SELECT_YESNO).u8(1).u64(99).0;
        assert!(redact(&prompt, Some(0)).unwrap().is_none());
        assert!(redact(&prompt, Some(1)).unwrap().is_some());
        let selection_hint = Writer::default().u8(msg::HINT).u8(hint::SELECTMSG).u8(1).u64(506).0;
        assert!(redact(&selection_hint, Some(0)).unwrap().is_none());
    }

    #[test]
    fn prompts_blank_the_other_players_cards() {
        let theirs = Loc { controller: 0, location: location::MZONE, sequence: 1, position: position::FACEDOWN_DEFENSE };
        let mine = Loc { controller: 1, location: location::HAND, sequence: 0, position: 0 };
        let prompt =
            Writer::default().u8(msg::SELECT_CARD).u8(1).u8(0).u32(1).u32(1).u32(2).u32(4206964).loc(theirs).u32(5).loc(mine).0;
        let seen = redact(&prompt, Some(1)).unwrap().unwrap();
        assert_eq!(code_at(&seen, 15), 0, "the face-down monster was revealed");
        assert_eq!(code_at(&seen, 29), 5);
    }

    #[test]
    fn queries_of_hidden_cards_keep_only_their_position() {
        let mut record = Vec::new();
        for (flag, value) in [(query::CODE, 4206964u32), (query::POSITION, position::FACEDOWN_DEFENSE), (query::ATTACK, 1800)] {
            record.extend_from_slice(&8u16.to_le_bytes());
            record.extend_from_slice(&flag.to_le_bytes());
            record.extend_from_slice(&value.to_le_bytes());
        }
        record.extend_from_slice(&4u16.to_le_bytes());
        record.extend_from_slice(&query::END.to_le_bytes());
        let mut message = vec![msg::UPDATE_DATA, 0, location::MZONE];
        message.extend_from_slice(&((record.len() + 2) as u32).to_le_bytes());
        message.extend_from_slice(&record);
        message.extend_from_slice(&0u16.to_le_bytes()); // an empty zone
        let seen = redact(&message, Some(1)).unwrap().unwrap();
        let records = read_location(&seen[3..]).unwrap();
        assert_eq!(records.len(), 2);
        let hidden = records[0].as_ref().unwrap().parse();
        assert_eq!((hidden.code, hidden.attack, hidden.position), (None, None, Some(position::FACEDOWN_DEFENSE)));
        assert!(records[1].is_none());
        let own_view = redact(&message, Some(0)).unwrap().unwrap();
        let own = read_location(&own_view[3..]).unwrap();
        assert_eq!(own[0].as_ref().unwrap().parse().code, Some(4206964));
    }
}
