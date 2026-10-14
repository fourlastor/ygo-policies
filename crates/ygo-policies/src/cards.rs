//! Static (printed) card facts.  The bit layouts follow the standard card
//! database format shared by YGOPro-family engines.

pub mod types {
    pub const MONSTER: u32 = 0x1;
    pub const SPELL: u32 = 0x2;
    pub const TRAP: u32 = 0x4;
    pub const NORMAL: u32 = 0x10;
    pub const EFFECT: u32 = 0x20;
    pub const FUSION: u32 = 0x40;
    pub const RITUAL: u32 = 0x80;
    pub const SPIRIT: u32 = 0x200;
    pub const UNION: u32 = 0x400;
    pub const GEMINI: u32 = 0x800;
    pub const TUNER: u32 = 0x1000;
    pub const SYNCHRO: u32 = 0x2000;
    pub const TOKEN: u32 = 0x4000;
    pub const QUICKPLAY: u32 = 0x10000;
    pub const CONTINUOUS: u32 = 0x20000;
    pub const EQUIP: u32 = 0x40000;
    pub const FIELD: u32 = 0x80000;
    pub const COUNTER: u32 = 0x100000;
    pub const FLIP: u32 = 0x200000;
    pub const XYZ: u32 = 0x800000;
    pub const LINK: u32 = 0x4000000;
    pub const EXTRA: u32 = FUSION | SYNCHRO | XYZ | LINK;
}

pub mod races {
    pub const WARRIOR: u32 = 0x1;
    pub const SPELLCASTER: u32 = 0x2;
    pub const FAIRY: u32 = 0x4;
    pub const FIEND: u32 = 0x8;
    pub const ZOMBIE: u32 = 0x10;
    pub const MACHINE: u32 = 0x20;
    pub const AQUA: u32 = 0x40;
    pub const PYRO: u32 = 0x80;
    pub const ROCK: u32 = 0x100;
    pub const WINGED_BEAST: u32 = 0x200;
    pub const PLANT: u32 = 0x400;
    pub const INSECT: u32 = 0x800;
    pub const THUNDER: u32 = 0x1000;
    pub const DRAGON: u32 = 0x2000;
    pub const BEAST: u32 = 0x4000;
    pub const BEAST_WARRIOR: u32 = 0x8000;
    pub const DINOSAUR: u32 = 0x10000;
    pub const FISH: u32 = 0x20000;
    pub const SEA_SERPENT: u32 = 0x40000;
    pub const REPTILE: u32 = 0x80000;
    pub const PSYCHIC: u32 = 0x100000;
}

pub mod attributes {
    pub const EARTH: u32 = 0x1;
    pub const WATER: u32 = 0x2;
    pub const FIRE: u32 = 0x4;
    pub const WIND: u32 = 0x8;
    pub const LIGHT: u32 = 0x10;
    pub const DARK: u32 = 0x20;
}

/// OCG effect categories describe operations, not whether their outcome helps
/// the player. These are distinct from the client database's category tags.
pub mod effect_categories {
    pub const DESTROY: u64 = 0x1;
    pub const RELEASE: u64 = 0x2;
    pub const REMOVE: u64 = 0x4;
    pub const TO_HAND: u64 = 0x8;
    pub const TO_DECK: u64 = 0x10;
    pub const TO_GRAVE: u64 = 0x20;
    pub const SUMMON: u64 = 0x100;
    pub const SPECIAL_SUMMON: u64 = 0x200;
    pub const TOKEN: u64 = 0x400;
    pub const CONTROL: u64 = 0x2000;
    pub const DISABLE: u64 = 0x4000;
    pub const DISABLE_SUMMON: u64 = 0x8000;
    pub const DRAW: u64 = 0x10000;
    pub const SEARCH: u64 = 0x20000;
    pub const EQUIP: u64 = 0x40000;
    pub const DAMAGE: u64 = 0x80000;
    pub const RECOVER: u64 = 0x100000;
    pub const ATK_CHANGE: u64 = 0x200000;
    pub const DEF_CHANGE: u64 = 0x400000;
    pub const NEGATE: u64 = 0x10000000;
    pub const FUSION_SUMMON: u64 = 0x40000000;
}

pub mod effect_types {
    pub const ACTIVATE: u16 = 0x10;
    pub const IGNITION: u16 = 0x40;
    pub const TRIGGER: u16 = 0x80 | 0x200;
    pub const QUICK: u16 = 0x100 | 0x400;
    pub const ACTION: u16 = ACTIVATE | IGNITION | TRIGGER | QUICK | 0x20;
}

pub mod effect_locations {
    pub const HAND: u16 = 0x2;
    pub const MONSTER: u16 = 0x4;
    pub const SPELL_TRAP: u16 = 0x8;
    pub const GRAVEYARD: u16 = 0x10;
    pub const BANISHED: u16 = 0x20;
    pub const EXTRA: u16 = 0x40;
    pub const FIELD: u16 = 0x100;
    pub const PENDULUM: u16 = 0x200;
    pub const SPELL_TRAP_MAIN: u16 = 0x400;
    pub const MONSTER_MAIN: u16 = 0x800;
    pub const MONSTER_EXTRA: u16 = 0x1000;

    /// Zone families for option-value estimates where a card has no slot yet.
    pub fn normalize(mask: u16) -> u16 {
        mask | if mask & (FIELD | PENDULUM | SPELL_TRAP_MAIN) != 0 { SPELL_TRAP } else { 0 }
            | if mask & (MONSTER_MAIN | MONSTER_EXTRA) != 0 { MONSTER } else { 0 }
    }

    /// Match the engine's symbolic subzones against a known board slot.
    pub fn matches_at(mask: u16, location: u16, sequence: u32) -> bool {
        if mask & location != 0 {
            return true;
        }
        match location {
            SPELL_TRAP => mask & FIELD != 0 && sequence == 5
                || mask & SPELL_TRAP_MAIN != 0 && sequence < 5
                // Pendulum zones are separate slots in older rules and the
                // outer main backrow slots in newer rules.
                || mask & PENDULUM != 0 && matches!(sequence, 0 | 4 | 6 | 7),
            MONSTER => mask & MONSTER_MAIN != 0 && sequence < 5
                || mask & MONSTER_EXTRA != 0 && matches!(sequence, 5 | 6),
            _ => false,
        }
    }
}

pub mod effect_events {
    pub const TO_GRAVE: u32 = 1014;
    pub const SUMMON_SUCCESS: u32 = 1100;
    pub const FLIP_SUMMON_SUCCESS: u32 = 1101;
    pub const SPECIAL_SUMMON_SUCCESS: u32 = 1102;
}

/// Public, initial script metadata. Conditions, costs and operations remain
/// opaque: categories are useful priors but do not prove an action is good.
/// Profiles must come from isolated card definitions, never hidden live state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct EffectDescriptor {
    pub category: u64,
    pub effect_type: u16,
    pub range: u16,
    pub event: u32,
    pub description: u64,
    pub flags: u64,
    pub has_cost: bool,
    pub card_target: bool,
}

pub type EffectProfiles = std::collections::HashMap<u32, Vec<EffectDescriptor>>;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct CardData {
    pub code: u32,
    pub alias: u32,
    pub kind: u32,
    pub attack: i32,
    pub defense: i32,
    pub level: u32,
    pub race: u32,
    pub attribute: u32,
    pub setcodes: Vec<u16>,
}

impl CardData {
    pub fn is(&self, kind: u32) -> bool {
        self.kind & kind != 0
    }
    pub fn is_monster(&self) -> bool {
        self.is(types::MONSTER)
    }
    pub fn is_spell(&self) -> bool {
        self.is(types::SPELL)
    }
    pub fn is_trap(&self) -> bool {
        self.is(types::TRAP)
    }
    pub fn is_extra(&self) -> bool {
        self.is(types::EXTRA)
    }
    pub fn is_tuner(&self) -> bool {
        self.is(types::TUNER)
    }
    pub fn in_set(&self, set: u16) -> bool {
        self.setcodes.iter().any(|s| s & 0x0fff == set & 0x0fff && (set & 0xf000 == 0 || s & set == set))
    }
    /// Tributes needed for a Normal Summon.
    pub fn tributes(&self) -> u32 {
        match self.level {
            0..=4 => 0,
            5 | 6 => 1,
            _ => 2,
        }
    }
}

/// Printed card data, keyed by passcode.
pub trait CardDatabase: Send + Sync {
    fn card(&self, code: u32) -> Option<&CardData>;

    /// Optional initial script descriptors for a known card. Absence carries
    /// no negative evidence: the caller may not have loaded this definition.
    fn effects(&self, _code: u32) -> &[EffectDescriptor] {
        &[]
    }

    /// Every card, for prompts that ask to name one ("declare a card name").
    fn all(&self) -> Box<dyn Iterator<Item = &CardData> + '_> {
        Box::new(std::iter::empty())
    }
}

/// Add reusable script metadata without changing existing database providers.
pub struct ProfiledCards {
    inner: std::sync::Arc<dyn CardDatabase>,
    profiles: EffectProfiles,
}

impl ProfiledCards {
    pub fn new(inner: std::sync::Arc<dyn CardDatabase>, mut profiles: EffectProfiles) -> Self {
        // Some callers canonicalize alternate-art passcodes before lookup.
        // Preserve exact profiles first, then add missing canonical entries.
        // Distant name aliases are different cards and must stay separate.
        let mut aliases: Vec<_> = profiles.keys().copied().filter_map(|code| {
            let alias = inner.card(code)?.alias;
            (alias != 0 && code.abs_diff(alias) < 20).then_some((code, alias))
        }).collect();
        aliases.sort_unstable();
        for (code, alias) in aliases {
            if !profiles.contains_key(&alias) {
                profiles.insert(alias, profiles[&code].clone());
            }
        }
        Self { inner, profiles }
    }
}

impl CardDatabase for ProfiledCards {
    fn card(&self, code: u32) -> Option<&CardData> {
        self.inner.card(code)
    }

    fn effects(&self, code: u32) -> &[EffectDescriptor] {
        self.profiles.get(&code).or_else(|| {
            let alias = self.inner.card(code)?.alias;
            (alias != 0 && code.abs_diff(alias) < 20).then(|| self.profiles.get(&alias)).flatten()
        }).map_or_else(|| self.inner.effects(code), Vec::as_slice)
    }

    fn all(&self) -> Box<dyn Iterator<Item = &CardData> + '_> {
        self.inner.all()
    }
}

/// Static in-memory database (handy for tests).
#[derive(Default)]
pub struct MemoryCards(pub std::collections::HashMap<u32, CardData>);

impl CardDatabase for MemoryCards {
    fn card(&self, code: u32) -> Option<&CardData> {
        self.0.get(&code)
    }

    fn all(&self) -> Box<dyn Iterator<Item = &CardData> + '_> {
        Box::new(self.0.values())
    }
}

#[cfg(test)]
mod effect_profile_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn alternate_art_profiles_work_from_exact_and_canonical_codes() {
        let base = 900002000;
        let art = base + 1;
        let namesake = base + 500;
        let inner = Arc::new(MemoryCards([
            (base, CardData { code: base, ..CardData::default() }),
            (art, CardData { code: art, alias: base, ..CardData::default() }),
            (namesake, CardData { code: namesake, alias: base, ..CardData::default() }),
        ].into()));
        let effect = EffectDescriptor { category: effect_categories::DRAW, description: 987, ..EffectDescriptor::default() };
        for source in [base, art] {
            let db = ProfiledCards::new(inner.clone(), [(source, vec![effect.clone()])].into());
            assert_eq!(db.effects(base), &[effect.clone()]);
            assert_eq!(db.effects(art), &[effect.clone()]);
            assert!(db.effects(namesake).is_empty());
        }
    }

    #[test]
    fn symbolic_ranges_keep_their_actual_board_subzones() {
        use effect_locations as r;
        assert!(r::matches_at(r::FIELD, r::SPELL_TRAP, 5));
        assert!(!r::matches_at(r::FIELD, r::SPELL_TRAP, 0));
        assert!(r::matches_at(r::SPELL_TRAP_MAIN, r::SPELL_TRAP, 0));
        assert!(!r::matches_at(r::SPELL_TRAP_MAIN, r::SPELL_TRAP, 5));
        assert!(r::matches_at(r::MONSTER_EXTRA, r::MONSTER, 5));
        assert!(!r::matches_at(r::MONSTER_EXTRA, r::MONSTER, 0));
        assert!(r::matches_at(r::MONSTER_MAIN, r::MONSTER, 0));
        assert!(!r::matches_at(r::MONSTER_MAIN, r::MONSTER, 5));
        for symbol in [r::FIELD, r::SPELL_TRAP_MAIN, r::PENDULUM] {
            assert_ne!(r::normalize(symbol) & r::SPELL_TRAP, 0);
            assert_eq!(r::normalize(symbol) & r::GRAVEYARD, 0);
        }
    }
}
