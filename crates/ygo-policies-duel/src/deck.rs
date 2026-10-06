//! A Deck list, and the shuffle a duel deals it with.
use std::path::Path;

use crate::Result;

/// A Main Deck and an Extra Deck, as card codes.  The Side Deck is not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deck {
    /// The Main Deck, in the order of the list.
    pub main: Vec<u32>,
    /// The Extra Deck, in the order of the list.
    pub extra: Vec<u32>,
}

impl Deck {
    /// Read a `.ydk` file.
    pub fn load(path: &Path) -> Result<Self> {
        let mut deck = Deck {
            main: vec![],
            extra: vec![],
        };
        let mut section = "";
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        for line in text.lines().map(str::trim) {
            match line {
                "#main" | "#extra" | "!side" => section = line,
                _ => {
                    if let Ok(code) = line.parse() {
                        match section {
                            "#main" => deck.main.push(code),
                            "#extra" => deck.extra.push(code),
                            _ => {}
                        }
                    }
                }
            }
        }
        if deck.main.is_empty() {
            return Err("Empty main deck".into());
        }
        Ok(deck)
    }
}

/// Shuffle `cards` with a SplitMix64 stream that goes on from `state`: both
/// players' Decks are shuffled from one seed, one after the other.
pub(crate) fn shuffle(cards: &mut [u32], state: &mut u64) {
    for i in (1..cards.len()).rev() {
        *state = state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        cards.swap(i, z as usize % (i + 1));
    }
}
