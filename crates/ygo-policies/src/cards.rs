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

    /// Every card, for prompts that ask to name one ("declare a card name").
    fn all(&self) -> Box<dyn Iterator<Item = &CardData> + '_> {
        Box::new(std::iter::empty())
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
