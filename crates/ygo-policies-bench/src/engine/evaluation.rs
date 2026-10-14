//! Deck-independent values for completed search transactions.
//!
//! Fixed, hand-written Rust rules combine observable resources and threats.
//! Features contain no deck identifiers or card-code embeddings.
//! Printed data and public rule facts describe cards; optional native effect
//! profiles describe the same generic categories for previously unseen cards.
//! Profiles advertise possible effects, never prove that their conditions or
//! costs can currently be met. Search must resolve those through the engine.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::Value;
use ygo_policies::agent::Outcome;
use ygo_policies::cards::{
    effect_categories as category, effect_locations as range, effect_types as kind,
    types, CardDatabase, EffectDescriptor as Effect, EffectProfiles, ProfiledCards,
};
use ygo_policies::ctx::Ctx;
use ygo_policies::generic::alternate;
use ygo_policies::generic::knowledge::{self as card_knowledge, CounterRecipients, CounterTiming, DamageShield};
use ygo_policies::generic::requirements;
use ygo_policies::knowledge::{Burn, Fate, ALWAYS};
use ygo_policies::model::{CardRef, CardView, Coin, CoinEffect, Location, Observation, Phase, Position};
use ygo_policies::tactics;
use ygo_policies_ocgcore::SqliteCards;

pub const FEATURE_NAMES: [&str; 13] = [
    "accessible_cards", "stranded_hand", "body_power", "dominant_attack",
    "reusable_grave", "interaction", "effect_access", "battle_card_gain",
    "battle_damage", "immediate_lethal", "life_buffer", "deck_reserve",
    "alternate_win_progress",
];

/// Fixed heuristic coefficients, in `FEATURE_NAMES` order. Accessible cards
/// anchor resource value; imminent lethal pressure outweighs ordinary board
/// improvements. Explicit alternate-win progress is capped at five units.
/// Engine-confirmed outcomes remain outside this estimate.
pub const HEURISTIC_COEFFICIENTS: [f64; 13] = [0.8, 0.5, 0.2, 0.45, 0.6, 0.8, 0.45, 0.6, 0.45, 2.5, 0.55, 0.7, 1.0];

// Stable OCGCore category/type/location flags. These are facts about generic
// effect semantics, not a lookup of benchmark cards or deck families.
const REMOVE: u64 = category::DESTROY | category::REMOVE | category::TO_DECK | category::CONTROL;
const ACCESS: u64 = category::TO_HAND | category::DRAW | category::SEARCH;
const SUMMON: u64 = category::SUMMON | category::SPECIAL_SUMMON | category::TOKEN;
const INTERACT: u64 = category::DISABLE | category::DISABLE_SUMMON | category::NEGATE;
const ACTIVATED: u16 = kind::ACTION;
const QUICK: u16 = kind::QUICK;
const IGNITION: u16 = kind::IGNITION;
const HAND: u16 = range::HAND;
const MONSTER: u16 = range::MONSTER;
const BACKROW: u16 = range::SPELL_TRAP;
const GRAVE: u16 = range::GRAVEYARD;

/// Cheap cloneable evaluator. The printed database is loaded once per path;
/// loading profiles is separate so callers can reuse their native-core cache.
#[derive(Clone)]
pub struct Evaluator {
    cards: Arc<dyn CardDatabase>,
    profiles: Option<Arc<ProfiledCards>>,
}

impl Evaluator {
    pub fn open(path: &Path) -> Result<Self, String> {
        static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<SqliteCards>>>> = OnceLock::new();
        let path = std::fs::canonicalize(path).map_err(|error| error.to_string())?;
        let mut cache = CACHE.get_or_init(Default::default).lock().map_err(|_| "evaluator card cache poisoned")?;
        let cards = match cache.get(&path) {
            Some(cards) => cards.clone(),
            None => {
                let cards = Arc::new(SqliteCards::open(&path).map_err(|error| error.to_string())?);
                cache.insert(path, cards.clone());
                cards
            }
        };
        Ok(Self::new(cards))
    }

    pub fn new(cards: Arc<dyn CardDatabase>) -> Self {
        Self { cards, profiles: None }
    }

    /// Set a code-keyed object of native effect descriptor arrays. Reading an
    /// effect's range/category does not read any live duel or hidden card.
    pub fn set_profiles(&mut self, profiles: &Value) -> Result<(), String> {
        let source = profiles.as_object().ok_or("effect profiles must be an object")?;
        let mut parsed = EffectProfiles::new();
        for (code, descriptors) in source {
            let code = code.parse::<u32>().map_err(|_| "invalid effect-profile card code")?;
            let descriptors = descriptors.as_array().ok_or("effect profile must be an array")?;
            let mut effects = Vec::with_capacity(descriptors.len());
            for effect in descriptors {
                effects.push(Effect {
                    category: effect["category"].as_u64().ok_or("effect category missing")?,
                    effect_type: u16::try_from(effect["effect_type"].as_u64().ok_or("effect type missing")?)
                        .map_err(|_| "invalid effect type")?,
                    range: u16::try_from(effect["range"].as_u64().ok_or("effect range missing")?)
                        .map_err(|_| "invalid effect range")?,
                    event: u32::try_from(effect["event"].as_u64().unwrap_or(0))
                        .map_err(|_| "invalid effect event")?,
                    description: effect["description"].as_u64().unwrap_or(0),
                    flags: effect["flags"].as_u64().unwrap_or(0),
                    has_cost: effect["has_cost"].as_bool().unwrap_or(false),
                    card_target: effect["card_target"].as_bool().unwrap_or(false),
                });
            }
            parsed.insert(code, effects);
        }
        // Reuse the shared exact-first/near-art alias rules. Distant aliases
        // share a printed name, not necessarily an effect implementation.
        self.profiles = Some(Arc::new(ProfiledCards::new(self.cards.clone(), parsed)));
        Ok(())
    }

    pub fn value(&self, observation: &Value, viewer: u8) -> f64 {
        let features = self.features(observation, viewer);
        let score: f64 = features.iter().zip(HEURISTIC_COEFFICIENTS).map(|(feature, coefficient)| feature * coefficient).sum();
        // Proven engine wins/losses are +/-1. Even an extreme estimated board
        // must stay strictly inside those outcomes.
        (score / 6.0).tanh() * 0.95
    }

    /// Difference features in the requested seat's favor. The input remains
    /// anchored to its own `me` visibility boundary when changing perspective.
    pub fn features(&self, observation: &Value, viewer: u8) -> [f64; 13] {
        let observation = visible_observation(observation, viewer);
        let ctx = Ctx::new(&observation, self.cards.as_ref());
        let profiles = self.profiles.as_deref().map(|cards| cards as &dyn CardDatabase).unwrap_or(self.cards.as_ref());
        let protection = BattleProtection::from_public_board(&ctx);
        let mut sides = [[0.0; 13]; 2];
        for player in 0..2u8 {
            let side = &mut sides[player as usize];
            let bodies: Vec<_> = observation.pile(player, Location::MonsterZone).collect();
            let body_count = bodies.len();
            let hand_size = observation.pile_size(player, Location::Hand);
            let mut victory_pieces = 0u8;
            for card in observation.cards.iter().filter(|card| card.at.controller == player) {
                let data = ctx.view_data(card);
                let effects = card.code.map(|code| profiles.effects(code)).unwrap_or(&[]);
                let code = card.code.map(|code| ctx.canonical(code)).unwrap_or(0);
                let available = || available_effects(effects, code, hand_size);
                match card.at.location {
                    Location::Hand => {
                        side[0] += 1.0;
                        if let Some(piece) = card.code.and_then(|code| alternate::exodia_piece(ctx.canonical(code))) {
                            victory_pieces |= 1 << piece;
                        }
                        if data.is_monster() && data.tributes() as usize > body_count {
                            let self_summon = available().any(|effect| effect.range & HAND != 0 && effect.category & SUMMON != 0);
                            if !self_summon { side[1] -= 0.65; }
                        }
                        side[6] += effect_access(available(), HAND, None, data.is_spell() || data.is_trap());
                    }
                    Location::MonsterZone => {
                        side[0] += if data.is(types::TOKEN) { 0.3 } else { 1.0 };
                        side[2] += f64::from(card.attack.max(card.defense).max(0).min(5000)) / 2500.0;
                        if card.position.face_up {
                            let facts = ctx.facts(card);
                            let pays = !facts.negate_discards || observation.pile_size(player, Location::Hand) > 0;
                            if pays {
                                side[5] += 0.3 * f64::from(facts.negates_any.count_ones());
                                side[5] += 0.1 * f64::from((facts.negates_aimed | facts.negates_destruction).count_ones());
                            }
                            side[5] += 0.1 * f64::from((facts.immune | facts.unaffected | facts.untargetable).count_ones());
                            if ctx.battle_proof(card) { side[5] += 0.45; }
                            if !ctx.effects_drained() {
                                side[6] += effect_access(available(), MONSTER, Some(card.at.sequence), false);
                                side[5] += quick_interaction(available(), MONSTER, card.at.sequence);
                            }
                        }
                    }
                    Location::SpellTrapZone => {
                        side[0] += 1.0;
                        let native_access = effect_access(available(), BACKROW, Some(card.at.sequence), true);
                        let known_access = known_backrow_access(&ctx, card);
                        side[6] += native_access.max(known_access);
                        side[5] += quick_interaction(available(), BACKROW, card.at.sequence);
                    }
                    Location::Graveyard => {
                        // Ordinary spent cards are not another hand. A small
                        // pool of monster targets is useful, but graveyard
                        // activation ranges are substantially more informative.
                        if data.is_monster() { side[4] += 0.025; }
                        let reusable = available().filter(|effect| effect.range & GRAVE != 0 && effect.effect_type & ACTIVATED != 0
                            && effect.category & (REMOVE | ACCESS | SUMMON | INTERACT | category::DAMAGE | category::RECOVER) != 0)
                            .map(|effect| (if effect.effect_type & IGNITION != 0 { 0.35 } else { 0.2 })
                                * if effect.has_cost { 0.6 } else { 1.0 })
                            .fold(0.0, f64::max);
                        side[4] += reusable;
                    }
                    _ => {}
                }
            }
            side[4] = side[4].min(2.0);
            side[5] = side[5].min(4.0);
            side[6] = side[6].min(3.0);
            // Each different piece advances the same explicit five-card hand
            // condition. Duplicates and pieces outside the hand do not advance
            // it. Hidden opposing identities were removed before this loop.
            let distinct = f64::from(victory_pieces.count_ones());
            side[12] = distinct * distinct / 5.0;
            let strongest = bodies.iter().filter(|card| card.position.face_up)
                .map(|card| card.attack.max(0)).max().unwrap_or(0);
            let opposing = observation.pile(1 - player, Location::MonsterZone)
                .map(|card| if card.position.face_up { card.attack.max(card.defense) } else { 1500 })
                .max().unwrap_or(0).max(0);
            side[3] = f64::from((strongest - opposing).clamp(0, 3000)) / 1500.0;

            let current = observation.turn_player == Some(player);
            let battle_soon = matches!(observation.phase, Some(Phase::Main1 | Phase::BattleStart | Phase::BattleStep | Phase::Battle));
            // A future turn's attacks are less reliable than attacks which the
            // engine says remain available now. Main Phase 2 cannot inflict a
            // fresh battle's damage just by keeping its monsters in attack.
            let discount = if current { if battle_soon { 1.0 } else { 0.0 } } else { 0.55 };
            let battle = battle_projection(&ctx, player, current, protection);
            side[7] = battle.cards * discount;
            side[8] = (battle.damage / 2500.0).clamp(-4.0, 4.0) * discount;
            let opposing_lp = f64::from(observation.life_points[(1 - player) as usize].max(1));
            let backrow = observation.pile_size(1 - player, Location::SpellTrapZone) as f64;
            if current && battle_soon && battle.damage >= opposing_lp {
                side[9] = 1.0 / (1.0 + 0.25 * backrow);
            }
            let life = f64::from(observation.life_points[player as usize].max(0));
            side[10] = ((life.min(24000.0) + 1000.0) / 9000.0).ln();
            if let Some((_, _, deck)) = observation.pile_sizes.iter().find(|(owner, location, _)| *owner == player && *location == Location::Deck) {
                side[11] = if *deck == 0 { -2.0 } else { -1.0 / f64::from(*deck) };
            }
        }
        let me = usize::from(viewer.min(1));
        std::array::from_fn(|index| sides[me][index] - sides[1 - me][index])
    }
}

/// Unknown requirements remain optimistic priors. Known conditions apply to
/// their precise effect slot, never to every effect printed on the same card.
fn available_effects(effects: &[Effect], code: u32, hand_size: u32) -> impl Iterator<Item = &Effect> {
    effects.iter().filter(move |effect| requirements::hand_condition_allows(code, effect, hand_size))
}

fn known_backrow_access(ctx: &Ctx<'_>, card: &CardView) -> f64 {
    if !card.position.face_up { return 0.0; }
    let Some(code) = card.code.map(|code| ctx.canonical(code)) else { return 0.0; };
    // Heart advertises CATEGORY_DRAW only while targeting, not among its
    // initial descriptors. This prices an optional future draw, not income.
    if alternate::heart_of_the_underdog(code) && ctx.obs.pile_size(card.at.controller, Location::Deck) > 1 { return 0.35; }
    let Some(support) = card_knowledge::profile(code).counter_support else { return 0.0; };
    if support.timing != CounterTiming::OwnStandby || support.recipients != CounterRecipients::EveryFaceUpMonster
        || ctx.effects_drained() { return 0.0; }
    // A symmetric counter generator has some option value if more of our live
    // engines can use that counter than theirs. No displayed counter totals
    // are read: they do not reveal types or prove an effect is payable.
    let mut users = [0usize; 2];
    for monster in ctx.obs.cards.iter().filter(|monster| monster.at.location == Location::MonsterZone && monster.position.face_up) {
        if monster.code.and_then(|code| card_knowledge::profile(ctx.canonical(code)).counter_engine)
            .is_some_and(|engine| engine.counter == support.counter && (engine.spend.is_some() || engine.attack_per_counter_on_field > 0)) {
            users[monster.at.controller as usize] += 1;
        }
    }
    if users[card.at.controller as usize] > users[(1 - card.at.controller) as usize] { 0.2 } else { 0.0 }
}

fn effect_access<'a>(effects: impl IntoIterator<Item = &'a Effect>, location: u16, sequence: Option<u32>, activate_here: bool) -> f64 {
    effects.into_iter().filter(|effect| effect.effect_type & ACTIVATED != 0
        && (sequence.map_or_else(|| range::normalize(effect.range) & location != 0,
            |sequence| range::matches_at(effect.range, location, sequence))
            || (activate_here && effect.effect_type & kind::ACTIVATE != 0)))
        .map(|effect| {
            let mut worth: f64 = 0.0;
            if effect.category & ACCESS != 0 { worth += 0.35; }
            if effect.category & SUMMON != 0 { worth += 0.2; }
            if effect.category & REMOVE != 0 { worth += 0.25; }
            if effect.category & INTERACT != 0 { worth += 0.25; }
            worth.min(0.6) * if effect.has_cost { 0.6 } else { 1.0 }
        }).fold(0.0, f64::max)
}

fn quick_interaction<'a>(effects: impl IntoIterator<Item = &'a Effect>, location: u16, sequence: u32) -> f64 {
    effects.into_iter().filter(|effect| effect.effect_type & QUICK != 0 && range::matches_at(effect.range, location, sequence))
        .map(|effect| if effect.category & (REMOVE | INTERACT) != 0 {
            if effect.has_cost { 0.25 } else { 0.4 }
        } else { 0.0 }).fold(0.0, f64::max)
}

#[derive(Clone, Copy, Default)]
struct BattleEstimate { cards: f64, damage: f64 }

impl BattleEstimate {
    fn score(self) -> f64 { self.cards + self.damage / 2500.0 }
}

#[derive(Clone, Copy)]
struct BattleChoice {
    target: Option<usize>,
    destroys_target: bool,
    loses_attacker: bool,
    value: BattleEstimate,
}

/// Public continuous protections are collected once, outside the attack-pair
/// loops. Conditions tied to surviving monsters remain dynamic within the
/// projected battle sequence.
#[derive(Clone, Copy, Default)]
struct BattleProtection {
    while_controls_monster: [bool; 2],
    protected_races: u32,
}

impl BattleProtection {
    fn from_public_board(ctx: &Ctx<'_>) -> Self {
        let mut result = Self::default();
        let drained = ctx.effects_drained();
        // Exact disable ordering is not observable. A visible suppressor is
        // enough to avoid claiming guaranteed protection from a field trap.
        let field_traps_suppressed = ctx.obs.cards.iter().any(|source| {
            let data = ctx.view_data(source);
            let active_zone = data.is_monster() && source.at.location == Location::MonsterZone
                || (data.is_spell() || data.is_trap()) && source.at.location == Location::SpellTrapZone;
            active_zone && source.position.face_up
                && !(source.at.location == Location::MonsterZone && drained)
                && source.code.is_some_and(|code| card_knowledge::profile(ctx.canonical(code)).negates_field_traps)
        });
        for source in ctx.obs.cards.iter().filter(|source| source.at.location == Location::SpellTrapZone && source.position.face_up) {
            let data = ctx.view_data(source);
            if field_traps_suppressed && data.is_trap() { continue; }
            if data.is(types::FIELD) && source.at.sequence != 5 { continue; }
            let Some(code) = source.code else { continue; };
            match card_knowledge::profile(ctx.canonical(code)).damage_shield {
                Some(DamageShield::ControllerWhileControlsMonster) => result.while_controls_monster[source.at.controller as usize] = true,
                Some(DamageShield::BattleInvolvingRace { race }) => result.protected_races |= race,
                None => {}
            }
        }
        result
    }

    fn prevents(self, ctx: &Ctx<'_>, player: u8, battler: Option<&CardView>, remaining_bodies: usize) -> bool {
        self.while_controls_monster[player as usize] && remaining_bodies > 0
            || battler.is_some_and(|card| card.at.controller == player && ctx.view_data(card).race & self.protected_races != 0)
    }
}

/// A cheap two-order greedy exchange estimate. Real search resolves actual
/// battles; this only prices the remaining tactical exposure at a safe leaf.
fn battle_projection(ctx: &Ctx<'_>, player: u8, current: bool, protection: BattleProtection) -> BattleEstimate {
    let mut attackers: Vec<_> = ctx.obs.pile(player, Location::MonsterZone)
        .filter(|card| card.position.face_up && card.position.attack
            && (!current || !ctx.obs.can_attack_known || card.can_attack)).collect();
    if attackers.is_empty() { return BattleEstimate::default(); }
    // The shared rules distinguish which controller a known attack lock stops.
    let locked = if player == ctx.me { !ctx.attack_locks().is_empty() } else { !ctx.own_attack_locks().is_empty() };
    if locked { return BattleEstimate::default(); }
    attackers.sort_by_key(|card| card.attack);
    let mut best = BattleEstimate::default();
    for reversed in [false, true] {
        let mut targets: Vec<_> = ctx.obs.pile(1 - player, Location::MonsterZone).collect();
        let mut remaining_own_bodies = ctx.obs.pile(player, Location::MonsterZone).count();
        let mut estimate = BattleEstimate::default();
        let ordered: Vec<_> = if reversed { attackers.iter().rev().copied().collect() } else { attackers.clone() };
        for attacker in ordered {
            let striking = ctx.facts(attacker).striking;
            let mut selected = if targets.is_empty() || striking.direct {
                let damage = if protection.prevents(ctx, 1 - player, None, targets.len()) {
                    0.0
                } else { f64::from(attacker.attack.max(0)) };
                Some(BattleChoice { target: None, destroys_target: false, loses_attacker: false,
                    value: BattleEstimate { cards: 0.0, damage } })
            } else { None };
            for (index, target) in targets.iter().enumerate() {
                // An available repeatable attack negation is not a free kill.
                // Its exact payments and exhaustion belong to engine search.
                if ctx.attack_negatable(target) { continue; }
                let result = tactics::default_outcome(ctx, attacker, target, 0);
                let met = ctx.attack_meets(Some(attacker), target);
                let attack = met.facts.attacker_atk.apply(attacker.attack + striking.bonus);
                let destroyed = matches!(result, Outcome::Win { .. } | Outcome::Trade);
                let lost = matches!(result, Outcome::Lose | Outcome::Trade);
                let mut cards = f64::from(destroyed as u8) - f64::from(lost as u8);
                if destroyed {
                    cards -= f64::from(met.facts.payoff.max(0)) / 1500.0;
                    cards -= if met.facts.collateral == ALWAYS { attackers.len().saturating_sub(1) as f64 }
                        else { f64::from(met.facts.collateral) };
                }
                let mut damage = 0.0;
                let before = met.facts.before_damage && met.facts.attacker != Fate::Unharmed
                    || striking.before_damage && ctx.strike_removes(attacker, target).is_some();
                if !before {
                    if !met.facts.no_damage && (!met.defending || striking.piercing)
                        && !protection.prevents(ctx, 1 - player, Some(target), targets.len()) {
                        damage += f64::from((attack - met.stat).max(0));
                    }
                    let own_protected = protection.prevents(ctx, player, Some(attacker), remaining_own_bodies);
                    if !own_protected { damage -= f64::from((met.stat - attack).max(0)); }
                    damage -= f64::from(match met.facts.burn {
                        Burn::None => 0,
                        Burn::Fixed(amount) => amount,
                        Burn::AttackerAtk => attack,
                        Burn::Reflected => if !own_protected && (!met.defending || striking.piercing) { (attack - met.stat).max(0) } else { 0 },
                    });
                }
                let value = BattleEstimate { cards, damage };
                if value.score() > selected.map_or(0.0, |choice| choice.value.score()) {
                    selected = Some(BattleChoice { target: Some(index), destroys_target: destroyed,
                        loses_attacker: lost, value });
                }
            }
            if let Some(choice) = selected {
                estimate.cards += choice.value.cards;
                estimate.damage += choice.value.damage;
                if choice.loses_attacker { remaining_own_bodies = remaining_own_bodies.saturating_sub(1); }
                if let Some(index) = choice.target.filter(|_| choice.destroys_target) { targets.remove(index); }
            }
        }
        if estimate.score() > best.score() { best = estimate; }
    }
    best
}

fn location(value: &Value) -> Location {
    match value.as_str().unwrap_or("") {
        "Deck" => Location::Deck, "Hand" => Location::Hand, "MonsterZone" => Location::MonsterZone,
        "SpellTrapZone" => Location::SpellTrapZone, "Graveyard" => Location::Graveyard,
        "Banished" => Location::Banished, "Extra" => Location::Extra, "Overlay" => Location::Overlay,
        _ => Location::Other,
    }
}

fn phase(value: &Value) -> Option<Phase> {
    Some(match value.as_str()? {
        "Draw" => Phase::Draw, "Standby" => Phase::Standby, "Main1" => Phase::Main1,
        "BattleStart" => Phase::BattleStart, "BattleStep" => Phase::BattleStep, "Damage" => Phase::Damage,
        "DamageCalculation" => Phase::DamageCalculation, "Battle" => Phase::Battle,
        "Main2" => Phase::Main2, "End" => Phase::End, _ => return None,
    })
}

fn visible_observation(value: &Value, viewer: u8) -> Observation {
    let me = value["me"].as_u64().filter(|seat| *seat < 2).unwrap_or(u64::from(viewer.min(1))) as u8;
    let cards = value["cards"].as_array().into_iter().flatten().filter_map(|card| {
        let controller = card["at"]["controller"].as_u64().filter(|seat| *seat < 2)? as u8;
        let location = location(&card["at"]["location"]);
        let face_up = card["position"]["face_up"].as_bool().unwrap_or(false);
        let hidden = controller != me && (location == Location::Hand || location == Location::Deck
            || location == Location::Extra || location.is_field() && !face_up);
        let read_stat = |name: &str| card[name].as_i64().unwrap_or(0).clamp(0, 100_000) as i32;
        let coin_effect = if hidden { None } else {
            card["coin_effect"]["code"].as_u64().and_then(|code| Some(CoinEffect {
                code: code as u32,
                result: match card["coin_effect"]["result"].as_str()? {
                    "Heads" => Coin::Heads, "Tails" => Coin::Tails, _ => return None,
                },
            }))
        };
        Some(CardView {
            at: CardRef { controller, location, sequence: card["at"]["sequence"].as_u64().unwrap_or(0) as u32 },
            code: if hidden { None } else { card["code"].as_u64().filter(|code| *code > 0).map(|code| code as u32) },
            position: Position { face_up, attack: card["position"]["attack"].as_bool().unwrap_or(false) },
            attack: if hidden { 1500 } else { read_stat("attack") },
            defense: if hidden { 1500 } else { read_stat("defense") },
            level: if hidden { 0 } else { card["level"].as_u64().unwrap_or(0) as u32 },
            can_attack: !hidden && card["can_attack"].as_bool().unwrap_or(false),
            battles: card["battles"].as_u64().unwrap_or(0) as u32,
            counters: if hidden { 0 } else { card["counters"].as_u64().unwrap_or(0) as u32 },
            coin_effect,
        })
    }).collect();
    let pile_sizes = value["pile_sizes"].as_array().into_iter().flatten().filter_map(|pile| {
        Some((pile[0].as_u64().filter(|player| *player < 2)? as u8, location(&pile[1]), pile[2].as_u64()? as u32))
    }).collect();
    Observation {
        me, cards, pile_sizes, turn: value["turn"].as_u64().unwrap_or(0) as u32,
        turn_player: value["turn_player"].as_u64().filter(|player| *player < 2).map(|player| player as u8),
        phase: phase(&value["phase"]),
        life_points: std::array::from_fn(|player| value["life_points"][player].as_i64().unwrap_or(8000).clamp(0, i64::from(i32::MAX)) as i32),
        chain: Vec::new(), battle_attacker: None, battle_target: None, event_cards: Vec::new(),
        summon_used: value["summon_used"].as_bool().unwrap_or(false), chain_known: true,
        can_attack_known: value["can_attack_known"].as_bool().unwrap_or(false), coin_toss: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use ygo_policies::cards::{CardData, MemoryCards};

    fn evaluator() -> Evaluator {
        let mut cards = MemoryCards::default();
        for (code, attack, level, kind) in [(1, 1800, 4, types::MONSTER), (2, 1900, 6, types::MONSTER | types::FUSION),
            (3, 0, 0, types::SPELL), (4, 2600, 8, types::MONSTER)] {
            cards.0.insert(code, CardData { code, attack, defense: attack, level, kind, ..Default::default() });
        }
        Evaluator::new(Arc::new(cards))
    }

    fn card(player: u8, location: &str, code: u32, attack: i32) -> Value {
        json!({"at":{"controller":player,"location":location,"sequence":code},"code":code,
            "position":{"face_up":true,"attack":true},"attack":attack,"defense":attack,
            "level":4,"can_attack":true,"battles":0})
    }

    fn observation(cards: Vec<Value>) -> Value {
        json!({"me":0,"cards":cards,"life_points":[8000,8000],"turn_player":0,"phase":"Main2",
            "can_attack_known":true,"pile_sizes":[[0,"Deck",30],[1,"Deck",30]]})
    }

    #[test]
    fn hidden_identity_stats_and_profiles_cannot_change_value() {
        let mut evaluator = evaluator();
        evaluator.set_profiles(&json!({"4":[{"category":0x10000000u64,"effect_type":0x100,"range":4,"has_cost":false}]})).unwrap();
        let mut value = observation(vec![card(1,"MonsterZone",1,1000),card(1,"Hand",3,0)]);
        value["cards"][0]["position"]["face_up"] = json!(false);
        let baseline = evaluator.features(&value,0);
        value["cards"][0]["code"] = json!(4);
        value["cards"][0]["attack"] = json!(99999);
        value["cards"][0]["defense"] = json!(99999);
        value["cards"][1]["code"] = json!(4);
        assert_eq!(evaluator.features(&value,0),baseline);
    }

    #[test]
    fn resource_negative_fusion_is_not_rewarded_for_large_printed_stats() {
        let evaluator = evaluator();
        let before = observation(vec![card(0,"MonsterZone",1,1800),card(0,"Hand",1,1800),card(0,"Hand",3,0)]);
        let after = observation(vec![card(0,"MonsterZone",2,1900),card(0,"Graveyard",1,1800),card(0,"Graveyard",1,1800),card(0,"Graveyard",3,0)]);
        assert!(evaluator.value(&before,0) > evaluator.value(&after,0));
    }

    #[test]
    fn sacrificing_the_only_large_body_to_destroy_a_small_body_loses_value() {
        let evaluator = evaluator();
        // Worm King's script can tribute the King itself for targeted removal.
        // Compare complete transactions, after both the cost and removal, using
        // ordinary card facts so this stays a general material-exchange rule.
        let before = observation(vec![card(0,"MonsterZone",4,2700),card(1,"MonsterZone",1,1200)]);
        let after = observation(vec![card(0,"Graveyard",4,2700),card(1,"Graveyard",1,1200)]);
        assert!(evaluator.value(&before,0) > evaluator.value(&after,0));
    }

    #[test]
    fn already_spent_attacks_do_not_create_immediate_lethal() {
        let evaluator = evaluator();
        let mut value = observation(vec![card(0,"MonsterZone",4,2600)]);
        value["phase"] = json!("Main1"); value["life_points"][1] = json!(2000);
        assert!(evaluator.features(&value,0)[9] > 0.0);
        value["cards"][0]["can_attack"] = json!(false);
        assert_eq!(evaluator.features(&value,0)[9],0.0);
        value["cards"][0]["can_attack"] = json!(true);
        value["phase"] = json!("Main2");
        assert_eq!(evaluator.features(&value,0)[9],0.0);
    }

    #[test]
    fn native_grave_range_adds_bounded_reuse_without_counting_spent_spells_as_hand() {
        let mut evaluator = evaluator();
        let value = observation(vec![card(0,"Graveyard",3,0)]);
        let plain = evaluator.value(&value,0);
        evaluator.set_profiles(&json!({"3":[{"category":0x10000,"effect_type":0x40,"range":0x10,"has_cost":true}]})).unwrap();
        assert!(evaluator.value(&value,0) > plain);
        assert!(evaluator.value(&value,0) < evaluator.value(&observation(vec![card(0,"Hand",3,0)]),0));
    }

    #[test]
    fn profile_loading_preserves_effect_slot_and_condition_metadata() {
        let mut evaluator = evaluator();
        let effect = Effect { category: category::SPECIAL_SUMMON, effect_type: IGNITION,
            range: MONSTER, event: 1102, description: (3u64 << 20) | 1,
            flags: 0x4000, has_cost: true, card_target: true };
        evaluator.set_profiles(&json!({"3":[{"category":effect.category,"effect_type":effect.effect_type,
            "range":effect.range,"event":effect.event,"description":effect.description,"flags":effect.flags,
            "has_cost":effect.has_cost,"card_target":effect.card_target}]})).unwrap();
        assert_eq!(evaluator.profiles.as_ref().unwrap().effects(3), &[effect]);
    }

    #[test]
    fn known_empty_hand_requirement_filters_only_the_unavailable_effect_prior() {
        let mut evaluator = evaluator();
        let mirage = 86197239u32;
        evaluator.set_profiles(&json!({mirage.to_string():[{
            "category":category::SPECIAL_SUMMON,"effect_type":IGNITION,"range":MONSTER,
            "description":u64::from(mirage) << 20,"event":0,"has_cost":true
        }]})).unwrap();
        let mut value = observation(vec![card(0,"MonsterZone",mirage,0),card(0,"Hand",3,0)]);
        assert_eq!(evaluator.features(&value,0)[6],0.0);
        value["cards"].as_array_mut().unwrap().pop();
        assert_eq!(evaluator.features(&value,0)[6],0.12,
            "empty hand satisfies the known requirement, without proving targets or costs");

        // Unknown conditions still retain their usual prior; an unrelated
        // description is not evidence for this exact card's documented slot.
        value["cards"].as_array_mut().unwrap().push(card(0,"Hand",3,0));
        evaluator.set_profiles(&json!({mirage.to_string():[{
            "category":category::SPECIAL_SUMMON,"effect_type":IGNITION,"range":MONSTER,
            "description":(u64::from(mirage) << 20) | 1,"event":0,"has_cost":true
        }]})).unwrap();
        assert_eq!(evaluator.features(&value,0)[6],0.12);
    }

    #[test]
    fn launcher_installation_and_hand_dump_are_not_gated_with_its_revival_slot() {
        let mut evaluator = evaluator();
        let launcher = 66957584u32;
        let slot = u64::from(launcher) << 20;
        evaluator.set_profiles(&json!({launcher.to_string():[
            {"category":0,"effect_type":kind::ACTIVATE,"range":0,"description":0,"event":1002},
            {"category":category::TO_GRAVE,"effect_type":IGNITION,"range":BACKROW,"description":slot,"event":0},
            {"category":category::SPECIAL_SUMMON,"effect_type":IGNITION,"range":BACKROW,"description":slot | 1,"event":0,"has_cost":true}
        ]})).unwrap();
        let effects = evaluator.profiles.as_ref().unwrap().effects(launcher);
        let with_hand: Vec<_> = available_effects(effects,launcher,1).map(|effect| effect.description).collect();
        assert_eq!(with_hand,vec![0,slot]);
        assert_eq!(available_effects(effects,launcher,0).count(),3);
        let mut value = observation(vec![card(0,"SpellTrapZone",launcher,0),card(0,"Hand",3,0)]);
        assert_eq!(evaluator.features(&value,0)[6],0.0);
        value["cards"].as_array_mut().unwrap().pop();
        assert_eq!(evaluator.features(&value,0)[6],0.12);
    }

    #[test]
    fn grave_revival_requirement_does_not_blanket_gate_archfiends_draw_trigger() {
        let mut evaluator = evaluator();
        let avenger = 85475641u32;
        let archfiend = 99177923u32;
        evaluator.set_profiles(&json!({
            avenger.to_string():[{"category":category::SPECIAL_SUMMON,"effect_type":0x82,"range":GRAVE,
                "description":u64::from(avenger) << 20,"event":1140}],
            archfiend.to_string():[{"category":category::SPECIAL_SUMMON,"effect_type":0x82,"range":HAND,
                "description":u64::from(archfiend) << 20,"event":1110,"has_cost":true}]
        })).unwrap();
        let mut value = observation(vec![card(0,"Graveyard",avenger,0),card(0,"Hand",archfiend,0)]);
        assert_eq!(evaluator.features(&value,0)[4],0.0);
        assert_eq!(evaluator.features(&value,0)[6],0.12,
            "the hand-draw condition concerns the previous hand and is not currently-empty");
        value["cards"].as_array_mut().unwrap().pop();
        assert_eq!(evaluator.features(&value,0)[4],0.2);
    }

    #[test]
    fn opposing_effect_requirements_use_public_hand_count_without_private_identities() {
        let mut evaluator = evaluator();
        let mirage = 86197239u32;
        evaluator.set_profiles(&json!({mirage.to_string():[{
            "category":category::SPECIAL_SUMMON,"effect_type":IGNITION,"range":MONSTER,
            "description":u64::from(mirage) << 20,"event":0,"has_cost":true
        }]})).unwrap();
        let mut value = observation(vec![card(1,"MonsterZone",mirage,0),card(1,"Hand",3,0)]);
        assert_eq!(evaluator.features(&value,0)[6],0.0);
        let before = evaluator.features(&value,0);
        value["cards"][1]["code"] = json!(4);
        assert_eq!(evaluator.features(&value,0),before);
        value["cards"].as_array_mut().unwrap().pop();
        assert_eq!(evaluator.features(&value,0)[6],-0.12);
    }

    #[test]
    fn symbolic_field_ranges_preserve_access_and_quick_interaction_priors() {
        for (base, symbolic, sequence) in [(BACKROW, range::FIELD, 5), (BACKROW, range::PENDULUM, 0),
            (BACKROW, range::SPELL_TRAP_MAIN, 0), (MONSTER, range::MONSTER_MAIN, 0),
            (MONSTER, range::MONSTER_EXTRA, 5)] {
            let ordinary = Effect { category: category::DESTROY | category::DRAW,
                effect_type: QUICK, range: base, ..Effect::default() };
            let restricted = Effect { range: symbolic, ..ordinary.clone() };
            assert!(effect_access(&[ordinary.clone()], base, None, false) > 0.0);
            assert_eq!(effect_access(&[ordinary.clone()], base, None, false), effect_access(&[restricted.clone()], base, None, false));
            assert_eq!(effect_access(&[ordinary.clone()], base, Some(sequence), false), effect_access(&[restricted.clone()], base, Some(sequence), false));
            assert_eq!(quick_interaction(&[ordinary], base, sequence), quick_interaction(&[restricted.clone()], base, sequence));
            assert_eq!(effect_access(&[restricted.clone()], GRAVE, None, false), 0.0);
            assert_eq!(effect_access(&[restricted.clone()], base, Some(99), false), 0.0);
            let other_zone = if base == BACKROW { MONSTER } else { BACKROW };
            assert_eq!(quick_interaction(&[restricted], other_zone, sequence), 0.0);
        }
    }

    #[test]
    fn public_alternate_art_uses_shared_metadata_without_copying_distant_name_aliases() {
        let (base, art, namesake) = (900_003_000, 900_003_001, 900_004_000);
        let mut cards = MemoryCards::default();
        for code in [base, art, namesake] {
            cards.0.insert(code, CardData { code, alias: if code == base { 0 } else { base },
                kind: types::MONSTER | types::EFFECT, attack: 1000, defense: 1000, level: 4, ..Default::default() });
        }
        let mut evaluator = Evaluator::new(Arc::new(cards));
        let profile = |category| json!([{"category":category,"effect_type":IGNITION,"range":MONSTER}]);
        let board = |code| observation(vec![card(0,"MonsterZone",code,1000)]);
        evaluator.set_profiles(&json!({base.to_string():profile(category::DRAW)})).unwrap();
        assert_eq!(evaluator.features(&board(art),0), evaluator.features(&board(base),0));
        assert!(evaluator.features(&board(base),0)[6] > evaluator.features(&board(namesake),0)[6]);

        evaluator.set_profiles(&json!({art.to_string():profile(category::DRAW)})).unwrap();
        assert_eq!(evaluator.features(&board(art),0), evaluator.features(&board(base),0));
        evaluator.set_profiles(&json!({base.to_string():profile(category::DRAW),art.to_string():profile(category::DESTROY)})).unwrap();
        assert!(evaluator.features(&board(base),0)[6] > evaluator.features(&board(art),0)[6]);
    }

    #[test]
    fn public_battle_immunity_prevents_a_fictitious_clear_and_direct_attack() {
        let evaluator = evaluator();
        let mut wall = card(1,"MonsterZone",ygo_policies::knowledge::THE_FOOL,0);
        wall["position"]["attack"] = json!(false);
        let mut value = observation(vec![card(0,"MonsterZone",4,2600),card(0,"MonsterZone",1,1800),wall]);
        value["phase"] = json!("Main1"); value["life_points"][1] = json!(1000);
        assert_eq!(evaluator.features(&value,0)[9],0.0);
        assert_eq!(evaluator.features(&value,0)[7],0.0);
        value["cards"][2]["code"] = json!(1);
        assert!(evaluator.features(&value,0)[9] > 0.0);
    }

    fn protection_evaluator() -> Evaluator {
        let mut cards = MemoryCards::default();
        for (code, kind, race) in [
            (1, types::MONSTER, 0), (4, types::MONSTER, ygo_policies::cards::races::FAIRY),
            (16197610, types::MONSTER | types::EFFECT, ygo_policies::cards::races::FAIRY),
            (53239672, types::TRAP | types::CONTINUOUS, 0),
            (56433456, types::SPELL | types::FIELD, 0),
            (51452091, types::TRAP | types::CONTINUOUS, 0),
            (77585513, types::MONSTER | types::EFFECT, 0),
            (69750546, types::MONSTER | types::EFFECT, 0),
        ] {
            cards.0.insert(code, CardData { code, kind, race, ..Default::default() });
        }
        Evaluator::new(Arc::new(cards))
    }

    #[test]
    fn spirit_barrier_blocks_damage_with_a_body_and_expires_when_the_last_body_leaves() {
        let evaluator = protection_evaluator();
        let mut value = observation(vec![card(0,"MonsterZone",4,2600),card(0,"MonsterZone",1,1800),
            card(1,"MonsterZone",16197610,800),card(1,"SpellTrapZone",53239672,0)]);
        value["phase"] = json!("Main1"); value["life_points"][1] = json!(2700);
        let shielded = evaluator.features(&value,0);
        assert_eq!(shielded[8],0.0, "battle-immune body maintains Barrier through both attacks");
        assert_eq!(shielded[9],0.0);
        value["cards"][3]["position"]["face_up"] = json!(false);
        assert!(evaluator.features(&value,0)[9] > 0.0, "a set trap is not an active shield");
        value["cards"][3]["position"]["face_up"] = json!(true);
        value["cards"][2]["code"] = json!(1);
        assert_eq!(evaluator.features(&value,0)[8],2600.0 / 2500.0,
            "the first attacker clears the body without damage, then the other hits directly");
        assert_eq!(evaluator.features(&value,0)[9],0.0);
        value["life_points"][1] = json!(2500);
        assert!(evaluator.features(&value,0)[9] > 0.0);
    }

    #[test]
    fn sanctuary_protects_the_battling_fairy_on_either_side_but_not_direct_hits() {
        let evaluator = protection_evaluator();
        let mut value = observation(vec![card(0,"MonsterZone",1,2600),
            card(1,"MonsterZone",16197610,800),card(0,"SpellTrapZone",56433456,0)]);
        value["cards"][2]["at"]["sequence"] = json!(5);
        value["phase"] = json!("Main1");
        assert_eq!(evaluator.features(&value,0)[8],0.0, "Sanctuary also protects the opposing Fairy");
        value["cards"][2]["at"]["sequence"] = json!(0);
        assert!(evaluator.features(&value,0)[8] > 0.0, "a Field Spell outside the Field Zone has no field-range shield");
        value["cards"][2]["at"]["sequence"] = json!(5);
        value["cards"][1]["code"] = json!(1);
        assert!(evaluator.features(&value,0)[8] > 0.0, "a non-Fairy receives battle damage");
        value["cards"].as_array_mut().unwrap().remove(1);
        assert_eq!(evaluator.features(&value,0)[8],2600.0 / 2500.0);
    }

    #[test]
    fn visible_field_trap_suppression_prevents_claiming_barrier_immunity() {
        let evaluator = protection_evaluator();
        let protected = |value: &Value| {
            let obs = visible_observation(value, 0);
            let ctx = Ctx::new(&obs, evaluator.cards.as_ref());
            BattleProtection::from_public_board(&ctx).prevents(&ctx, 1, None, 1)
        };
        let mut value = observation(vec![card(0,"MonsterZone",1,2600),
            card(1,"MonsterZone",16197610,800),card(1,"SpellTrapZone",53239672,0)]);
        value["phase"] = json!("Main1");
        assert_eq!(evaluator.features(&value,0)[8],0.0);
        value["cards"].as_array_mut().unwrap().push(card(0,"SpellTrapZone",51452091,0));
        assert!(evaluator.features(&value,0)[8] > 0.0);
        value["cards"][3]["at"]["location"] = json!("MonsterZone");
        assert!(protected(&value), "a trap treated as a monster has no backrow suppression");
        value["cards"][3]["code"] = json!(77585513);
        assert!(!protected(&value), "Jinzo suppresses traps in the Monster Zone");
        value["cards"][3]["at"]["location"] = json!("SpellTrapZone");
        assert!(protected(&value), "equipped Jinzo has no monster-zone suppression");
    }

    #[test]
    fn battle_shields_do_not_remove_scattershots_effect_damage() {
        let evaluator = protection_evaluator();
        let mut value = observation(vec![card(0,"MonsterZone",1,2600),card(0,"SpellTrapZone",53239672,0),
            card(1,"MonsterZone",69750546,500)]);
        value["phase"] = json!("Main1");
        // The target takes 2100 battle damage; its grave trigger deals 500
        // effect damage through our Barrier. The script is c69750546.lua.
        assert_eq!(evaluator.features(&value,0)[8],1600.0 / 2500.0);
    }

    #[test]
    fn victory_progress_requires_unique_visible_pieces_in_hand() {
        let evaluator = evaluator();
        let mut value = observation(vec![card(0,"Hand",33396948,0),card(0,"Hand",70903634,0)]);
        assert_eq!(evaluator.features(&value,0)[12],0.8);
        value["cards"].as_array_mut().unwrap().push(card(0,"Hand",70903634,0));
        assert_eq!(evaluator.features(&value,0)[12],0.8, "duplicates do not advance the condition");
        value["cards"].as_array_mut().unwrap().push(card(0,"Graveyard",7902349,0));
        assert_eq!(evaluator.features(&value,0)[12],0.8);
        value["cards"].as_array_mut().unwrap().push(card(1,"Hand",8124921,0));
        assert_eq!(evaluator.features(&value,0)[12],0.8, "opponent private pieces are ignored");
        value["cards"][0]["at"]["location"] = json!("MonsterZone");
        assert_eq!(evaluator.features(&value,0)[12],0.2);
        assert_eq!(evaluator.features(&value,0)[12],-evaluator.features(&value,1)[12]);
    }

    #[test]
    fn live_uncategorized_draw_engine_has_a_prior_without_double_counting_native_metadata() {
        let mut evaluator = evaluator();
        let mut value = observation(vec![card(0,"SpellTrapZone",35762283,0)]);
        value["cards"][0]["at"]["sequence"] = json!(0);
        let without_native = evaluator.features(&value,0)[6];
        assert_eq!(without_native,0.35);
        evaluator.set_profiles(&json!({"35762283":[{"category":category::DRAW,"effect_type":kind::TRIGGER,"range":BACKROW}]})).unwrap();
        assert_eq!(evaluator.features(&value,0)[6],without_native);
        evaluator.set_profiles(&json!({})).unwrap();
        value["cards"][0]["position"]["face_up"] = json!(false);
        assert_eq!(evaluator.features(&value,0)[6],0.0);
        value["cards"][0]["position"]["face_up"] = json!(true);
        value["pile_sizes"][0][2] = json!(0);
        assert_eq!(evaluator.features(&value,0)[6],0.0, "empty deck has no draw-engine option value");
    }

    #[test]
    fn symmetric_counter_support_needs_live_beneficiaries_and_never_reads_counter_totals() {
        let evaluator = evaluator();
        let mut value = observation(vec![card(0,"SpellTrapZone",90135989,0)]);
        assert_eq!(evaluator.features(&value,0)[6],0.0);
        value["cards"].as_array_mut().unwrap().push(card(0,"MonsterZone",16197610,800));
        assert_eq!(evaluator.features(&value,0)[6],0.2);
        value["cards"][1]["counters"] = json!(10000);
        assert_eq!(evaluator.features(&value,0)[6],0.2, "aggregate counts are not typed resource value");
        value["cards"].as_array_mut().unwrap().push(card(1,"MonsterZone",16197610,800));
        assert_eq!(evaluator.features(&value,0)[6],0.0, "equal opposing benefit cancels the support prior");
    }

    #[test]
    fn shared_observation_is_antisymmetric_and_terminal_outcomes_dominate() {
        let evaluator = evaluator();
        let mut value = observation(vec![card(0,"MonsterZone",4,2600),card(1,"MonsterZone",1,1800)]);
        value["phase"] = json!("Main1");
        assert_eq!(evaluator.value(&value,0),-evaluator.value(&value,1));
        value["life_points"] = json!([i32::MAX,0]);
        assert!(evaluator.value(&value,0).is_finite());
        assert!(evaluator.value(&value,0).abs() < 1.0);
    }
}
