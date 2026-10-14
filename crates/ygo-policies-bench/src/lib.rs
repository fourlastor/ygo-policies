//! Engine-backed policy runners for arbitrary caller-supplied decks.
//!
//! [`engine::play_fast`] accepts two [`engine::Deck`] values independently of
//! policy catalog entries. A catalog is only needed by the benchmark CLI.
pub mod engine;
