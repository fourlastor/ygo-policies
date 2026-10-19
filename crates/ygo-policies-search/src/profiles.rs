//! The effect profiles stored for the World Championship 2011 card pool
//! (`data/wc2011-effect-profiles.txt`, which the duel crate's
//! `effect_profiles` example writes): what `Core::effect_profiles` reads from
//! the script of each card `data/wc2011.lflist.conf` lists and of each other
//! print of one, kept so that a seat describes its own cards to its pilot
//! with no script run, on whatever engine its host plays.
use std::sync::OnceLock;

use ygo_policies::cards::{EffectDescriptor, EffectProfiles};

use crate::Result;

const STORED: &str = include_str!("../../../data/wc2011-effect-profiles.txt");

/// Every stored profile, by code.
pub fn stored() -> Result<&'static EffectProfiles> {
    static PARSED: OnceLock<Result<EffectProfiles>> = OnceLock::new();
    PARSED.get_or_init(|| parse(STORED).map_err(|error| format!("data/wc2011-effect-profiles.txt: {error}")))
        .as_ref()
        .map_err(Clone::clone)
}

/// Profiles in the stored layout: one line per effect, `code category
/// effect_type range event description flags has_cost card_target` (`0x`
/// for hex), a card that registers no effect a line of its code alone, and
/// `#` comments.
pub fn parse(text: &str) -> Result<EffectProfiles> {
    let mut profiles = EffectProfiles::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let malformed = || format!("line {}: {line:?}", number + 1);
        let fields = line.split_whitespace().map(|field| match field.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16).ok(),
            None => field.parse().ok(),
        }).collect::<Option<Vec<u64>>>().ok_or_else(malformed)?;
        let effects = profiles.entry(narrow(fields[0]).ok_or_else(malformed)?).or_default();
        match fields[1..] {
            [] => {}
            [category, effect_type, range, event, description, flags, has_cost, card_target] => {
                effects.push((|| Some(EffectDescriptor {
                    category, effect_type: narrow(effect_type)?, range: narrow(range)?, event: narrow(event)?,
                    description, flags, has_cost: flag(has_cost)?, card_target: flag(card_target)?,
                }))().ok_or_else(malformed)?);
            }
            _ => return Err(malformed()),
        }
    }
    Ok(profiles)
}

fn narrow<T: TryFrom<u64>>(value: u64) -> Option<T> {
    value.try_into().ok()
}

fn flag(value: u64) -> Option<bool> {
    (value <= 1).then_some(value == 1)
}

/// The stored profiles of `codes`, keyed by code as `Core::effect_profiles`
/// returns them, and the codes there is none for.
pub fn profiles_of(codes: &[u32]) -> Result<(serde_json::Value, Vec<u32>)> {
    let stored = stored()?;
    let (mut found, mut missing) = (EffectProfiles::new(), Vec::new());
    for &code in codes {
        match stored.get(&code) {
            Some(effects) => {
                found.insert(code, effects.clone());
            }
            None => missing.push(code),
        }
    }
    missing.sort_unstable();
    missing.dedup();
    Ok((serde_json::to_value(found).map_err(|error| error.to_string())?, missing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stored_layout_reads_effects_and_cards_without_any() {
        let profiles = parse("# a comment\n\n27551 0x200 0x1a 0x10a 1002 0 0x11 0 1\n27551 0x0 0x809 0x0 1015 0 0x1 0 0\n46986414\n").unwrap();
        assert!(profiles[&46986414].is_empty());
        assert_eq!(profiles[&27551], [
            EffectDescriptor { category: 0x200, effect_type: 0x1a, range: 0x10a, event: 1002, description: 0, flags: 0x11,
                has_cost: false, card_target: true },
            EffectDescriptor { effect_type: 0x809, event: 1015, flags: 1, ..EffectDescriptor::default() },
        ]);
        for malformed in ["27551 0x200", "27551 0x200 0x1a 0x10a 1002 0 0x11 0 2", "27551 0x200 0x10000 0x10a 1002 0 0x11 0 1", "x"] {
            assert!(parse(malformed).is_err(), "{malformed}");
        }
    }

    #[test]
    fn cards_are_looked_up_by_their_own_code_other_prints_too() {
        // Polymerization, a print of it with a code of its own, Dark Magician
        // (no effect) and a code nothing is stored for.
        let (profiles, missing) = profiles_of(&[24094653, 27847700, 46986414, 99999999, 24094653]).unwrap();
        assert!(!profiles["24094653"].as_array().unwrap().is_empty());
        assert!(!profiles["27847700"].as_array().unwrap().is_empty());
        assert_eq!(profiles["46986414"], serde_json::json!([]));
        assert_eq!(missing, [99999999]);
        assert!(profiles.get("99999999").is_none());
    }

    /// The guard on the stored file: every card the pool lists and every
    /// other print of one, profiled as the pinned scripts and card database
    /// profile it.
    #[test]
    fn the_stored_profiles_are_those_of_the_pinned_scripts_for_every_card_of_the_pool() {
        use ygo_policies::cards::CardDatabase;
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let pool = std::fs::read_to_string(root.join("data/wc2011.lflist.conf")).unwrap();
        let listed: std::collections::HashSet<u32> =
            pool.lines().filter_map(|line| line.split_whitespace().next()?.parse().ok()).collect();
        let cards = ygo_policies_ocgcore::SqliteCards::open(root.join("vendor/BabelCdb/cards.cdb")).unwrap();
        let prints = cards.all().filter(|card| listed.contains(&card.alias)).map(|card| card.code);
        let mut codes: Vec<u32> = listed.iter().copied().chain(prints).collect();
        codes.sort_unstable();
        codes.dedup();
        let stored = stored().unwrap();
        let mut kept: Vec<u32> = stored.keys().copied().collect();
        kept.sort_unstable();
        let core = ygo_policies_duel::Core::open(None, &root.join("vendor/BabelCdb/cards.cdb"), &root.join("vendor/CardScripts"))
            .unwrap();
        let read = core.effect_profiles(&codes).unwrap();
        let expected = serde_json::to_value(stored).unwrap();
        let changed: Vec<_> = codes.iter().filter(|code| read[code.to_string()] != expected[code.to_string()]).collect();
        let regenerate = "regenerate data/wc2011-effect-profiles.txt (the duel crate's effect_profiles example)";
        assert_eq!(kept, codes, "the pool lists other cards: {regenerate}");
        assert!(changed.is_empty(), "other profiles for {:?} and {} more: {regenerate}", changed.first(), changed.len().saturating_sub(1));
    }
}
