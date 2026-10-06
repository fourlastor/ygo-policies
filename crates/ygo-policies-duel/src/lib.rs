//! Duels on OCGCore, for any program that wants to run them.
//!
//! This crate compiles the engine of `vendor/ocgcore` and its Lua, and holds
//! what it takes to play a duel on it:
//!
//! * [`Core`]: the engine with a card database and the card scripts.  One
//!   per thread; it plays any number of duels.
//! * [`Deck`]: a `.ydk` list.
//! * [`DuelOptions`]: how a duel starts.  [`DuelOptions::seeded`] is the duel
//!   `policy-bench` plays, from one number.
//! * [`Duel`]: one duel, stepped by the caller.  [`Duel::step`] returns the
//!   engine's messages and says whether the engine now waits for a response
//!   or the duel is over; the caller hands the response back with
//!   [`Duel::respond`].  [`Duel::snapshot`], [`Duel::restore`] and
//!   [`Duel::swap`] are what a search is built on.
//! * [`Recorded`]: a duel as it was played, and [`Core::replay`] to play it
//!   again.
//! * [`PolicyLibrary`]: the policies of a `libygo_policies` build, seated
//!   through its C ABI.
//!
//! The seats are the caller's.  A seat is anything that is fed the messages
//! and answers the selections addressed to it: a
//! [`ygo_policies_ocgcore::Seat`] around a policy, a [`LibrarySeat`], or one
//! of each.
//!
//! ```no_run
//! # fn main() -> Result<(), String> {
//! use std::path::Path;
//! use ygo_policies_duel::{Core, Deck, DuelOptions, PolicyLibrary, State};
//!
//! let cards = Path::new("vendor/BabelCdb/cards.cdb");
//! let core = Core::open(None, cards, Path::new("vendor/CardScripts"))?;
//! let library = PolicyLibrary::open(Path::new("target/release/libygo_policies.so"))?;
//! let decks = [Deck::load(Path::new("decks/Blackwing Assassin.ydk"))?, Deck::load(Path::new("decks/Emperor, Arise!.ydk"))?];
//!
//! let mut duel = core.deal(&DuelOptions::seeded(7), &decks)?;
//! let seats = [library.seat("blackwing", cards, 0, 7)?, library.seat("monarch", cards, 1, 8)?];
//! for (player, seat) in seats.iter().enumerate() {
//!     seat.feed(&duel.start_message(player as u8))?;
//! }
//! loop {
//!     let step = duel.step()?;
//!     let mut response = None;
//!     for sent in &step.messages {
//!         for seat in &seats {
//!             if let Some(answer) = seat.feed(&sent.bytes)? {
//!                 response = Some(answer);
//!             }
//!         }
//!     }
//!     match step.state {
//!         State::Over(outcome) => {
//!             println!("{outcome:?}");
//!             break;
//!         }
//!         State::Awaiting => duel.respond(&response.ok_or("nobody answered")?),
//!         State::Running => {}
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # The engine's memory
//!
//! The engine keeps each duel in an arena of its own, which is what makes a
//! snapshot a plain copy.  It does so by replacing the global C++ `operator
//! new` and `operator delete`: memory asked for while one of its calls runs
//! comes from that duel's arena.  Linked into a program, as cargo links a
//! Rust dependency, this needs nothing.  A shared library that holds this
//! crate and is loaded with `dlopen` must be linked with
//! `-Wl,-Bsymbolic-functions`, or the host's own `operator new` takes the
//! engine's place and restored snapshots go wrong
//! (`vendor/ocgcore/docs/snapshot-restore-prototype.md`).

#![warn(missing_docs)]

mod core;
mod deck;
mod duel;
mod policy;
mod record;

pub use crate::core::{Core, Printed};
pub use crate::deck::Deck;
pub use crate::duel::{Duel, DuelOptions, Monster, Outcome, Placed, Sent, Snapshot, State, Step};
pub use crate::policy::{LibrarySeat, PolicyLibrary};
pub use crate::record::{Asked, Recorded, Replayed};

/// Everything here fails with a message.
pub type Result<T> = std::result::Result<T, String>;
