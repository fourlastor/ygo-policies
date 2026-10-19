//! The generic policy's staged search, shared by the bench and the policies.
//!
//! At a decision of its seat, the search snapshots the duel, deals again in
//! each world what the seat cannot see (its own Deck order, the other
//! player's Deck, hand, Set cards and unknown Set monsters), plays every
//! alternative out to the end with both players played by a pilot, and
//! restores the duel.  It leaves its pilot's answer only for an alternative
//! clearly ahead over the same worlds (see [`StagedOptions`]).
//!
//! Seats are anything that plays like a [`PilotSeat`]: a policy of a library
//! loaded from a file (the bench compares builds this way) or one in process.
pub mod local;
pub mod memory;
pub mod profiles;
mod search_seat;
mod seat;
mod staged;
pub mod worlds;
pub use local::{LocalPilots, LocalSeat};
pub use search_seat::{SearchOptions, SearchSeat, SeatStats};
pub use seat::{LibraryPilots, PilotSeat, Pilots};
pub use staged::*;
pub use ygo_policies_duel::Result;
