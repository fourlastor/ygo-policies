//! What the shared tactics know about particular cards beyond their printed
//! stats: battles they cannot lose, effects that cannot reach them, what
//! their owner gets back when battle destroys them.  Every deck plays around
//! them by default; a deck that wants the exception still decides it in its
//! own `Strategy` hooks.
//!
//! Facts are looked up by the code the viewer sees (`Ctx::facts`).  The
//! opponent's face-down cards have no code (the seat redacts them before the
//! policy looks), so nothing about a hidden card leaks: it stays an unknown
//! monster until it is flipped or revealed.

pub const THE_FOOL: u32 = 62892347;
pub const SPIRIT_REAPER: u32 = 23205979;
pub const BLACKWING_ARMOR_MASTER: u32 = 69031175;
pub const X_SABER_PASHUUL: u32 = 23093604;
pub const MARSHMALLON: u32 = 31305911;
pub const INFERNITY_GUARDIAN: u32 = 51566770;
pub const MORPHTRONIC_BOARDEN: u32 = 48381268;
pub const SET_MORPHTRONIC: u16 = 0x26;
pub const WHITE_NIGHT_DRAGON: u32 = 79473793;
pub const GIANT_RAT: u32 = 97017120;
pub const MYSTIC_TOMATO: u32 = 83011277;
pub const MASKED_DRAGON: u32 = 39191307;
pub const UFO_TURTLE: u32 = 60806437;
pub const SHINING_ANGEL: u32 = 95956346;
pub const PYRAMID_TURTLE: u32 = 77044671;
pub const XX_SABER_EMMERSBLADE: u32 = 42737833;
pub const KARAKURI_SOLDIER_NISAMU: u32 = 3846170;
pub const APPRENTICE_MAGICIAN: u32 = 9156135;
pub const MORPHTRONIC_CAMERAN: u32 = 28124263;
pub const BIRDFACE: u32 = 45547649;
pub const KOAKI_MEIRU_BOULDER: u32 = 6320631;
pub const OJAMA_BLUE: u32 = 64627453;
pub const COLOSSAL_FIGHTER: u32 = 23693634;
pub const MACHINA_FORTRESS: u32 = 5556499;
pub const JAIN: u32 = 96235275;
pub const X_SABER_GALAHAD: u32 = 50604950;
pub const DD_WARRIOR_LADY: u32 = 7572887;
pub const KREBONS: u32 = 59575539;
pub const MORPHTRONIC_BOOMBOXEN: u32 = 92720564;
pub const TOON_WORLD: u32 = 15259703;
pub const TOON_KINGDOM: u32 = 43175858;
pub const SET_TOON: u16 = 0x62;
pub const KARAKURI_MERCHANT_INASHICHI: u32 = 30230789;
pub const KARAKURI_BUSHI_MUZANICHIHA: u32 = 39118197;
pub const KARAKURI_STRATEGIST_NISHIPACHI: u32 = 66625883;
pub const KARAKURI_NINJA_SAZANK: u32 = 93724592;
pub const KARAKURI_NINJA_KUICK: u32 = 6276588;
pub const KARAKURI_WATCHDOG_SAIZAN: u32 = 70271583;

/// Toon World by name: Toon Kingdom is treated as it while in the Field Zone.
pub const TOON_WORLDS: &[u32] = &[TOON_WORLD, TOON_KINGDOM];

/// When battle cannot destroy a face-up monster.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BattleProof {
    #[default]
    No,
    Always,
    /// While its controller has no cards in hand (Infernity Guardian).
    EmptyHand,
}

/// The battle position a face-up monster switches to when it is selected as
/// an attack target (the Karakuri).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Switch {
    #[default]
    No,
    /// From Attack to Defense Position.
    ToDefense,
    /// To the other position, either way.
    Either,
}

/// Facts about one card, as long as it is face-up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub battle_proof: BattleProof,
    /// Its controller takes no battle damage from battles involving it.
    pub no_battle_damage: bool,
    /// Its coin decides whose effects that target it are negated and
    /// destroyed: heads its controller's, tails the opponent's (The Fool).
    pub coin_targeting_shield: bool,
    /// Negates and destroys the Spells/Traps that target it.
    pub spell_trap_shield: bool,
    /// Destroyed once an effect that targets it resolves.
    pub dies_when_targeted: bool,
    /// Roughly what its controller gets back when battle destroys it: a
    /// monster from the Deck, a search, or a card of ours destroyed.
    pub battle_payoff: i32,
    /// ATK it gains in the Damage Step when it attacks a monster.
    pub attack_bonus: i32,
    /// ATK it loses in the Damage Step when it is attacked.
    pub attacked_malus: i32,
    /// After damage calculation it can banish itself and the monster it battled.
    pub banishes_in_battle: bool,
    /// Its controller can negate an attack on it for this many LP.
    pub negates_attacks_for: Option<i32>,
    /// Switches its battle position when selected as an attack target.
    pub switches_when_attacked: Switch,
}

impl Facts {
    pub const NONE: Facts = Facts {
        battle_proof: BattleProof::No,
        no_battle_damage: false,
        coin_targeting_shield: false,
        spell_trap_shield: false,
        dies_when_targeted: false,
        battle_payoff: 0,
        attack_bonus: 0,
        attacked_malus: 0,
        banishes_in_battle: false,
        negates_attacks_for: None,
        switches_when_attacked: Switch::No,
    };
}

/// What we know about the card with this (canonical) code.
pub fn facts(code: u32) -> Facts {
    const NONE: Facts = Facts::NONE;
    match code {
        // Battle cannot destroy them.
        THE_FOOL => Facts { battle_proof: BattleProof::Always, coin_targeting_shield: true, ..NONE },
        SPIRIT_REAPER => Facts { battle_proof: BattleProof::Always, dies_when_targeted: true, ..NONE },
        BLACKWING_ARMOR_MASTER => Facts { battle_proof: BattleProof::Always, no_battle_damage: true, ..NONE },
        X_SABER_PASHUUL | MARSHMALLON => Facts { battle_proof: BattleProof::Always, ..NONE },
        INFERNITY_GUARDIAN => Facts { battle_proof: BattleProof::EmptyHand, ..NONE },
        WHITE_NIGHT_DRAGON => Facts { spell_trap_shield: true, ..NONE },
        // Destroyed by battle, they bring a monster from the Deck, hand or
        // Graveyard, a search, or destroy a card of ours.
        GIANT_RAT | MYSTIC_TOMATO | MASKED_DRAGON | UFO_TURTLE | SHINING_ANGEL | PYRAMID_TURTLE | XX_SABER_EMMERSBLADE
        | APPRENTICE_MAGICIAN | MORPHTRONIC_CAMERAN | COLOSSAL_FIGHTER | OJAMA_BLUE => {
            Facts { battle_payoff: 1000, ..NONE }
        }
        BIRDFACE | KOAKI_MEIRU_BOULDER => Facts { battle_payoff: 700, ..NONE },
        MACHINA_FORTRESS => Facts { battle_payoff: 1200, ..NONE },
        // The Damage Step.
        JAIN => Facts { attack_bonus: 300, ..NONE },
        X_SABER_GALAHAD => Facts { attack_bonus: 300, attacked_malus: 500, ..NONE },
        DD_WARRIOR_LADY => Facts { banishes_in_battle: true, ..NONE },
        KREBONS => Facts { negates_attacks_for: Some(800), ..NONE },
        // Selected as an attack target, they change battle position.
        KARAKURI_SOLDIER_NISAMU | KARAKURI_MERCHANT_INASHICHI | KARAKURI_BUSHI_MUZANICHIHA
        | KARAKURI_STRATEGIST_NISHIPACHI | KARAKURI_NINJA_SAZANK => {
            let payoff = if code == KARAKURI_SOLDIER_NISAMU { 1000 } else { 0 };
            Facts { switches_when_attacked: Switch::ToDefense, battle_payoff: payoff, ..NONE }
        }
        KARAKURI_NINJA_KUICK | KARAKURI_WATCHDOG_SAIZAN => Facts { switches_when_attacked: Switch::Either, ..NONE },
        _ => NONE,
    }
}

/// What a card is worth to the deck that plays it, as that deck's own policy
/// rates it (its `Strategy::value`): the Spells/Traps that stay on the field
/// and that a deck runs on (Toon World, Necrovalley, Gateway of the Six...),
/// and the monsters it values above their printed stats.  Seen face-up on
/// the opponent's field, such a card is one to remove first.  0 for the rest.
pub fn owner_worth(code: u32) -> i32 {
    match code {
        52687916 => 3300, // Trishula, Dragon of the Ice Barrier (quickdraw-plant)
        15259703 | // Toon World (toon)
        79229522 => 3000, // Chimeratech Fortress Dragon (machina)
        82732705 => 2800, // Skill Drain (rock-block)
        75782277 | // Harpies' Hunting Ground (harpie)
        53569894 | // Pyramid of Light (pyramid)
        83965310 | // Destiny HERO - Plasma (destiny-hero)
        64752646 | // Fire Princess (burn)
        3825890 => 2600, // Gravekeeper's Visionary (gravekeeper)
        27970830 | // Gateway of the Six (six-samurai)
        47355498 => 2500, // Necrovalley (gravekeeper)
        39910367 => 2400, // Magical Citadel of Endymion (spellcaster)
        34487429 | // Ancient City - Rainbow Ruins (crystal)
        62265044 => 2300, // Dragon Ravine (dragunity)
        51481927 => 2200, // Spell Absorption (burn)
        66957584 | // Infernity Launcher (infernity)
        82971335 => 2100, // Fortune Lady Earth (fortune-lady)
        33550694 | // Fusion Gate (heroes)
        90011152 | // Ojama Country (ojama)
        86780027 | // Solidarity (arcana)
        55586621 | // Fortune Lady Dark (fortune-lady)
        6924874 | // Harpie's Pet Baby Dragon (harpie)
        86197239 | // Infernity Mirage (infernity)
        55119278 | // Morphtronic Radion (morphtronic)
        64631466 => 2000, // Relinquished (toon)
        63730624 | // Double Tool C&D (morphtronic)
        61962135 | // Glorious Illusion (lightsworn)
        85101228 | // Morphtronic Bind (morphtronic)
        38992735 | // Wave-Motion Cannon (burn)
        54578613 | // Dragunity Legionnaire (dragunity)
        95503687 => 1900, // Lumina, Lightsworn Summoner (lightsworn)
        59755122 => 1850, // Dragunity Phalanx (dragunity)
        77778835 | // Hysteric Party (harpie)
        72345736 | // Six Samurai United (six-samurai)
        32710364 | // Crystal Beast Ruby Carbuncle (crystal)
        28355718 | // Destiny HERO - Double Dude (destiny-hero)
        82693917 | // Fortune Lady Wind (fortune-lady)
        5975022 | // Gladiator Beast Murmillo (gladiator)
        49080532 | // Infernity Beetle (infernity)
        37132349 => 1800, // Ojama Red (ojama)
        47408488 | // Crystal Tree (crystal)
        93016201 | // Royal Oppression (rock-block)
        29088922 => 1700, // Fortune Lady Water (fortune-lady)
        81896771 => 1650, // Wattpheasant (watt)
        22751868 | // Karakuri Showdown Castle (karakuri)
        36870345 | // Dragunity Aklys (dragunity)
        81962318 | // Dragunity Tribus (dragunity)
        71870152 => 1600, // Fortune Lady Fire (fortune-lady)
        91351370 | // Black Whirlwind (blackwing)
        77565204 | // Future Fusion (heroes)
        85541675 | // Karakuri Anatomy (karakuri)
        56074358 | // Morphtronic Map (morphtronic)
        90239723 | // Morphtronic Repair Unit (morphtronic)
        15341821 | // Dandylion (quickdraw-plant)
        41613948 | // Destiny HERO - Doom Lord (destiny-hero)
        9411399 | // Destiny HERO - Malicious (destiny-hero)
        34471458 | // Fortune Lady Light (fortune-lady)
        45141844 | // Old Vindictive Magician (spellcaster)
        92373006 | // Test Tiger (gladiator)
        98777036 => 1500, // Tragoedia (monarch)
        85087012 | // Card Trooper (quickdraw-plant)
        2326738 | // Des Lacooda (pyramid)
        67441435 | // Glow-Up Bulb (quickdraw-plant)
        29947751 | // Morphtronic Magnen (morphtronic)
        23205979 | // Spirit Reaper (pyramid)
        11747708 => 1400, // Spore (quickdraw-plant)
        19665973 | // Battle Fader (monarch)
        54455664 | // Dragunity Brandistock (dragunity)
        33420078 | // Plaguespreader Zombie (tele-dad)
        21502796 | // Ryko, Lightsworn Hunter (lightsworn)
        33883834 => 1300, // Shien's Squire (six-samurai)
        82099401 | // Crystal Seer (spellcaster)
        55461064 | // Destiny HERO - Blade Master (destiny-hero)
        57421866 | // Level Eater (quickdraw-plant)
        78349103 | // Machina Peacekeeper (machina)
        65685470 | // Spirit of the Six Samurai (six-samurai)
        24996659 => 1200, // Wattkiwi (watt)
        5554990 | // Wattberyx (watt)
        96099959 => 1100, // X-Saber Palomuro (x-saber)
        85475641 | // Infernity Avenger (infernity)
        93542102 | // Morphtronic Celfon (morphtronic)
        32548609 => 1000, // Wattmole (watt)
        23093604 => 900, // X-Saber Pashuul (x-saber)
        _ => 0,
    }
}
