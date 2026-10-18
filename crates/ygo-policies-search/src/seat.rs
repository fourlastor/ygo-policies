//! What the search needs from a seat, and from a source of seats.
use std::path::Path;

use ygo_policies_duel::{LibrarySeat, PolicyLibrary};

use crate::Result;

/// A player the search can play out, copy and redirect: what a policy
/// library's C ABI offers through [`LibrarySeat`].  Feed it every message of
/// the duel in order, as a `ygo_policies_ocgcore::Seat`.
pub trait PilotSeat: Sized {
    /// One engine message; the response when the message asks this seat.
    fn feed(&self, message: &[u8]) -> Result<Option<Vec<u8>>>;
    /// One refresh for this seat and `other`, decoded once.
    fn feed_update_pair(&self, other: &Self, message: &[u8]) -> Result<()>;
    /// Messages in `OCG_DuelGetMessage`'s layout: `u32` length, message.
    fn feed_buffer(&self, buffer: &[u8]) -> Result<()>;
    /// An independent copy, memory and random state included.
    fn fork(&self) -> Result<Self>;
    /// Let [`PilotSeat::override_response`] replace answers; before the
    /// first question.
    fn enable_response_overrides(&self) -> Result<()>;
    /// Answer the pending question with another offered response, and
    /// remember that one.
    fn override_response(&self, response: &[u8]) -> Result<()>;
    /// The pending question and its alternatives, as the C ABI describes them.
    fn alternative_responses(&self, maximum: usize) -> Result<serde_json::Value>;
    /// The last question, what was answered and how it was seen.
    fn last_answer(&self) -> Result<serde_json::Value>;
    /// What the seat sees now.
    fn observation(&self) -> Result<serde_json::Value>;
    /// The effects of its deck's cards (`Core::effect_profiles`).
    fn set_effect_profiles(&self, profiles: &serde_json::Value) -> Result<()>;
}

/// Where the seats of the other player's model come from.
pub trait Pilots {
    type Seat: PilotSeat;
    /// Policy `name` seated as `player`, breaking ties with `seed`.
    fn seat(&self, name: &str, player: u8, seed: u64) -> Result<Self::Seat>;
}

impl PilotSeat for LibrarySeat<'_> {
    fn feed(&self, message: &[u8]) -> Result<Option<Vec<u8>>> {
        LibrarySeat::feed(self, message)
    }
    fn feed_update_pair(&self, other: &Self, message: &[u8]) -> Result<()> {
        LibrarySeat::feed_update_pair(self, other, message)
    }
    fn feed_buffer(&self, buffer: &[u8]) -> Result<()> {
        LibrarySeat::feed_buffer(self, buffer)
    }
    fn fork(&self) -> Result<Self> {
        LibrarySeat::fork(self)
    }
    fn enable_response_overrides(&self) -> Result<()> {
        LibrarySeat::enable_response_overrides(self)
    }
    fn override_response(&self, response: &[u8]) -> Result<()> {
        LibrarySeat::override_response(self, response)
    }
    fn alternative_responses(&self, maximum: usize) -> Result<serde_json::Value> {
        LibrarySeat::alternative_responses(self, maximum)
    }
    fn last_answer(&self) -> Result<serde_json::Value> {
        LibrarySeat::last_answer(self)
    }
    fn observation(&self) -> Result<serde_json::Value> {
        LibrarySeat::observation(self)
    }
    fn set_effect_profiles(&self, profiles: &serde_json::Value) -> Result<()> {
        LibrarySeat::set_effect_profiles(self, profiles)
    }
}

/// The policies of a library loaded from a file, reading the card
/// database `cards`.
#[derive(Clone, Copy)]
pub struct LibraryPilots<'a> {
    pub library: &'a PolicyLibrary,
    pub cards: &'a Path,
}

impl<'a> Pilots for LibraryPilots<'a> {
    type Seat = LibrarySeat<'a>;
    fn seat(&self, name: &str, player: u8, seed: u64) -> Result<LibrarySeat<'a>> {
        self.library.seat(name, self.cards, player as i32, seed)
    }
}
