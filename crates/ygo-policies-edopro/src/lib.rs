//! EDOPro client: a policy joins an EDOPro room as a player.
//!
//! EDOPro's server forwards OCGCore's messages to each client, already
//! filtered for that player, and takes OCGCore responses back, so the policy
//! seat is exactly the one `ygo-policies-ocgcore` provides: this crate only
//! speaks the lobby protocol around it (join, deck, rock-paper-scissors,
//! turn order) and pipes the duel messages through.

pub mod log;
pub mod protocol;
pub mod ydk;

use std::io::{self, Read, Write};

use ygo_policies_ocgcore::message::{is_selection, parse, Message};
use ygo_policies_ocgcore::wire::msg;
use ygo_policies_ocgcore::Seat;

use protocol::{ctos, stoc, write_packet};

#[derive(Clone, Debug)]
pub struct Config {
    pub name: String,
    pub room_id: u32,
    pub password: String,
    pub version: u32,
    pub deck: ydk::Deck,
    /// Answer to "go first?" when we win rock-paper-scissors.
    pub go_first: bool,
}

/// Sees everything that crosses the wire (logging, traces).
pub trait Observer {
    fn received(&mut self, _message: &[u8]) {}
    fn answered(&mut self, _seat: &Seat, _response: &[u8]) {}
    fn note(&mut self, _text: &str) {}
}

impl Observer for () {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// `MSG_WIN`: `winner` is a duel seat (2 for a draw); `seat` is ours.
    Finished { seat: Option<u8>, winner: u8, reason: u8 },
    /// The server ended the session without a result.
    Ended(String),
}

fn hand(entropy: u64) -> u8 {
    (entropy % 3) as u8 + 1
}

/// Play one duel on a connected stream.  `new_seat` builds a fresh policy
/// seat (with no seat number: EDOPro's `MSG_START` names it).
pub fn play(
    stream: &mut (impl Read + Write),
    config: &Config,
    new_seat: &mut dyn FnMut() -> Seat,
    observer: &mut dyn Observer,
) -> io::Result<Outcome> {
    write_packet(stream, ctos::PLAYER_INFO, &protocol::player_info(&config.name))?;
    write_packet(stream, ctos::JOIN_GAME, &protocol::join_game(config.version, config.room_id, &config.password))?;
    let mut role = 0u8;
    let mut seat: Option<Seat> = None;
    let entropy = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.as_nanos() as u64);
    loop {
        let (opcode, payload) = match protocol::read_packet(stream) {
            Ok(packet) => packet,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return Ok(Outcome::Ended("EDOPro closed the connection".into()))
            }
            Err(error) => return Err(error),
        };
        match opcode {
            stoc::JOIN_GAME => {
                let deck = &config.deck;
                write_packet(stream, ctos::UPDATE_DECK, &protocol::update_deck(&deck.main, &deck.extra, &deck.side))?;
                observer.note("joined the room; deck registered");
            }
            stoc::TYPE_CHANGE => {
                role = payload.first().copied().unwrap_or(0) & 0x0f;
                write_packet(stream, ctos::HS_READY, &[])?;
                observer.note(&format!("seated in slot {role}; ready"));
            }
            stoc::CHANGE_SIDE => {
                // Between the duels of a match: keep the same deck.
                let deck = &config.deck;
                write_packet(stream, ctos::UPDATE_DECK, &protocol::update_deck(&deck.main, &deck.extra, &deck.side))?;
            }
            stoc::SELECT_HAND => write_packet(stream, ctos::HAND_RESULT, &[hand(entropy)])?,
            stoc::SELECT_TP => write_packet(stream, ctos::TP_RESULT, &[config.go_first as u8])?,
            stoc::TIME_LIMIT => {
                if payload.first() == Some(&role) {
                    write_packet(stream, ctos::TIME_CONFIRM, &[])?;
                }
            }
            stoc::DUEL_START => observer.note("duel started"),
            stoc::GAME_MSG => {
                let Some(&id) = payload.first() else { continue };
                observer.received(&payload);
                if id == msg::START {
                    seat = Some(new_seat());
                }
                if id == msg::WIN {
                    if let Ok(Message::Win { player, reason }) = parse(&payload) {
                        let ours = seat.as_ref().and_then(Seat::seat);
                        return Ok(Outcome::Finished { seat: ours, winner: player, reason });
                    }
                }
                let Some(seat) = seat.as_mut() else { continue };
                // EDOPro only sends a client its own prompts.
                if is_selection(id) && payload.len() > 1 && seat.seat() != Some(payload[1]) {
                    observer.note(&format!("prompt for seat {} while seated as {:?}: switching", payload[1], seat.seat()));
                    seat.set_seat(payload[1]);
                }
                match seat.feed(&payload) {
                    Ok(Some(response)) => {
                        write_packet(stream, ctos::RESPONSE, &response)?;
                        observer.answered(seat, &response);
                    }
                    Ok(None) => {}
                    Err(error) => observer.note(&format!("message {id} not understood: {error}")),
                }
            }
            stoc::ERROR_MSG => return Ok(Outcome::Ended(protocol::describe_error(&payload))),
            stoc::DUEL_END => return Ok(Outcome::Ended("the duel ended without a result".into())),
            _ => {}
        }
    }
}
