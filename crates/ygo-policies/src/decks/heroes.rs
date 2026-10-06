//! "Fusion Heroes": Elemental HERO Fusion toolbox.
//!
//! The deck turns small HEROes into Level 8 Fusions through five routes:
//! Polymerization (hand/field), Miracle Fusion (banishing from the Graveyard,
//! so it costs one card), Fusion Gate (a Field Spell: fuse without a spell),
//! Super Polymerization (discard 1, fuse with the *opponent's* monsters: a
//! removal spell) and Future Fusion (mill the materials now, summon two turns
//! later).  Prisma and Future Fusion fill the Graveyard for Miracle Fusion,
//! Woodsman / King of the Swamp find Polymerization, Stratos and Emergency
//! Call find HEROes, Ocean recycles them.
//!
//! Which Fusion depends on the board: Great Tornado halves the opponent's
//! monsters, Absolute Zero wipes their field when it leaves, The Shining
//! grows with banished HEROes (Miracle Fusion banishes), Gaia steals ATK.
//! Materials are spent cheapest first: Graveyard cards before hand/field ones,
//! and the opponent's monsters first of all when Super Polymerization allows.

use crate::agent::{value, Outcome, Response, Strategy, Turn};
use crate::cards::attributes;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Phase};
use crate::tactics::default_outcome;

pub const DECK: &str = "Fusion Heroes";

const ARMORED_BEE: u32 = 86915847;
const BIRDMAN: u32 = 64034255;
const NEOS_ALIUS: u32 = 69884162;
const STRATOS: u32 = 40044918;
const OCEAN: u32 = 37195861;
const WOODSMAN: u32 = 75434695;
const PRISMA: u32 = 89312388;
const VOLTIC: u32 = 9327502;
const SNOWMAN_EATER: u32 = 91133740;
const DEBRIS_DRAGON: u32 = 14943837;
const KING_OF_THE_SWAMP: u32 = 79109599;
const PENGUIN_SOLDIER: u32 = 93920745;
const MIST_VALLEY_SOLDIER: u32 = 22837504;
const EMERGENCY_CALL: u32 = 213326;
const WARRIOR_RETURNING: u32 = 95281259;
const REINFORCEMENT: u32 = 32807846;
const SUPER_POLYMERIZATION: u32 = 48130397;
const PARALLEL_WORLD_FUSION: u32 = 54283059;
const FUSION_GATE: u32 = 33550694;
const FUTURE_FUSION: u32 = 77565204;
const MIRACLE_SYNCHRO_FUSION: u32 = 36484016;
const MIRACLE_FUSION: u32 = 45906428;
/// The deck's Polymerization is an alternate artwork whose alias is too far
/// from its passcode for `Ctx::canonical`: name both.
const POLYMERIZATION: u32 = 24094653;
const POLYMERIZATION_ART: u32 = 27847700;
const STARLIGHT_ROAD: u32 = 58120309;
const ABSOLUTE_ZERO: u32 = 40854197;
const GAIA: u32 = 16304628;
const GREAT_TORNADO: u32 = 3642509;
const THE_SHINING: u32 = 22061412;
const TERRA_FIRMA: u32 = 74711057;
const NOVA_MASTER: u32 = 1945387;
const DRACO_EQUISTE: u32 = 14017402;
const STARDUST_DRAGON: u32 = 44508094;
const BLACK_ROSE_DRAGON: u32 = 73580471;
const SET_ELEMENTAL_HERO: u16 = 0x3008;
const SET_HERO: u16 = 0x8;

#[derive(Clone, Default)]
pub struct Heroes;

impl Heroes {
    fn is_poly(code: u32) -> bool {
        matches!(code, POLYMERIZATION | POLYMERIZATION_ART)
    }

    fn is_hero(ctx: &Ctx, code: u32) -> bool {
        let data = ctx.data(code);
        data.is_monster() && data.in_set(SET_HERO)
    }

    fn fusion_spells_in_hand(ctx: &Ctx) -> usize {
        ctx.hand()
            .iter()
            .filter_map(|c| c.code.map(|code| ctx.canonical(code)))
            .filter(|code| Self::is_poly(*code) || matches!(*code, MIRACLE_FUSION | SUPER_POLYMERIZATION))
            .count()
    }

    fn water_on_field(ctx: &Ctx) -> usize {
        (0..2)
            .flat_map(|p| ctx.monsters(p))
            .filter(|c| c.position.face_up && c.code.is_some() && ctx.view_data(c).attribute & attributes::WATER != 0)
            .filter(|c| !ctx.is(c, ABSOLUTE_ZERO))
            .count()
    }

    fn banished_heroes(ctx: &Ctx) -> usize {
        ctx.banished(ctx.me)
            .iter()
            .filter(|c| c.code.map_or(false, |code| ctx.data(code).in_set(SET_ELEMENTAL_HERO) && ctx.data(code).is_monster()))
            .count()
    }

    /// How much we want this Fusion Monster on the field right now.
    fn fusion_score(ctx: &Ctx, code: u32) -> f64 {
        let opp_face_up: Vec<&CardView> = ctx.monsters(ctx.opp).into_iter().filter(|c| c.position.face_up).collect();
        let opp_best = opp_face_up.iter().map(|c| c.attack).max().unwrap_or(0) as f64;
        match ctx.canonical(code) {
            // Halving every face-up opponent monster is removal for a board.
            GREAT_TORNADO => 2800.0 + opp_face_up.iter().map(|c| c.attack.max(c.defense)).sum::<i32>() as f64 / 4.0,
            // Leaving the field destroys the opponent's monsters: they cannot
            // afford to remove it, and it attacks without fear.
            ABSOLUTE_ZERO => 3500.0 + 500.0 * Self::water_on_field(ctx) as f64 + 150.0 * ctx.monsters(ctx.opp).len() as f64,
            THE_SHINING => 2600.0 + 300.0 * Self::banished_heroes(ctx) as f64,
            GAIA => 2200.0 + opp_best / 2.0,
            DRACO_EQUISTE => 3200.0,
            TERRA_FIRMA => 2500.0,
            NOVA_MASTER => 2600.0,
            other => ctx.data(other).attack as f64,
        }
    }

    /// Worth spending two cards on a Fusion now?
    fn want_fusion(ctx: &Ctx) -> bool {
        // Preserve a winning Fusion instead of repeatedly spending it as material.
        if ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).is(crate::cards::types::FUSION))
            && ctx.my_best_attack() > ctx.opp_best_attack() { return false; }
        match ctx.phase() {
            Some(Phase::Main1) => true,
            // After battle, only to put a wall up.
            _ => ctx.monsters(ctx.me).is_empty() || ctx.opp_best_attack() > ctx.my_best_attack(),
        }
    }

    /// The opponent monster Super Polymerization should absorb, if any is worth it.
    fn super_poly_target<'a>(ctx: &Ctx<'a>) -> Option<&'a CardView> {
        ctx.monsters(ctx.opp)
            .into_iter()
            .filter(|c| c.position.face_up)
            .max_by_key(|c| ctx.threat(c))
            .filter(|c| ctx.threat(c) >= 1800 || c.attack > ctx.my_best_attack())
    }
}

impl Strategy for Heroes {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        if Self::is_poly(code) {
            return Some(1900);
        }
        Some(match code {
            DRACO_EQUISTE => 3200,
            ABSOLUTE_ZERO => 3000,
            GREAT_TORNADO => 2800,
            THE_SHINING | NOVA_MASTER => 2600,
            TERRA_FIRMA | STARDUST_DRAGON => 2500,
            BLACK_ROSE_DRAGON => 2400,
            GAIA => 2300,
            SUPER_POLYMERIZATION => 2300,
            MIRACLE_FUSION => 2200,
            STRATOS => 2100,
            FUSION_GATE => 2000,
            NEOS_ALIUS => 1900,
            PRISMA => 1850,
            OCEAN | STARLIGHT_ROAD => 1750,
            WOODSMAN => 1700,
            EMERGENCY_CALL | REINFORCEMENT => 1650,
            FUTURE_FUSION => 1500,
            KING_OF_THE_SWAMP => 1450,
            MIST_VALLEY_SOLDIER | ARMORED_BEE | SNOWMAN_EATER => 1300,
            PARALLEL_WORLD_FUSION | PENGUIN_SOLDIER | WARRIOR_RETURNING => 1200,
            VOLTIC | DEBRIS_DRAGON => 1100,
            MIRACLE_SYNCHRO_FUSION => 1000,
            BIRDMAN => 800,
            _ => return (ctx.data(code).is_extra()).then(|| Self::fusion_score(ctx, code) as i32),
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Searches and setup first: they feed everything else.
        let field_gate = ctx.face_up_on_field(ctx.me, FUSION_GATE);
        if !field_gate {
            if let Some(i) = t.activate_from(FUSION_GATE, Location::Hand) {
                return t.pick(i);
            }
        }
        for code in [EMERGENCY_CALL, REINFORCEMENT, WARRIOR_RETURNING] {
            if let Some(i) = t.activate(code) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate_from(PRISMA, Location::MonsterZone) {
            return t.pick(i);
        }
        if !ctx.face_up_on_field(ctx.me, FUTURE_FUSION) && ctx.deck_size(ctx.me) > 10 {
            if let Some(i) = t.activate_from(FUTURE_FUSION, Location::Hand) {
                return t.pick(i);
            }
        }
        // Take the summon trigger before a Fusion can consume Stratos from hand.
        if let Some(i) = t.find(ChoiceKind::NormalSummon, Some(STRATOS), Some(Location::Hand)) { return t.pick(i); }
        // Super Polymerization as removal: fuse their best monster away.
        if let Some(target) = Self::super_poly_target(&ctx) {
            if let Some(i) = t.activate(SUPER_POLYMERIZATION) {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        if Self::want_fusion(&ctx) {
            // Miracle Fusion costs a single card: always worth it.
            if let Some(i) = t.activate(MIRACLE_FUSION) {
                return t.pick(i);
            }
            // Fusion Gate's own effect costs no spell.
            if let Some(i) = t.activate_from(FUSION_GATE, Location::SpellTrapZone) {
                return t.pick(i);
            }
            if let Some(i) = t.activate(POLYMERIZATION_ART).or_else(|| t.activate(POLYMERIZATION)) {
                return t.pick(i);
            }
            if let Some(i) = t.activate(MIRACLE_SYNCHRO_FUSION) {
                return t.pick(i);
            }
        }
        // Armored Bee: halve what our attackers could not otherwise get past.
        if let Some(i) = t.activate_from(ARMORED_BEE, Location::MonsterZone) {
            let ours = ctx.my_best_attack();
            let target = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| c.position.face_up && c.position.attack && c.attack >= ours && c.attack / 2 < ours)
                .max_by_key(|c| c.attack);
            if let Some(target) = target {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        // King of the Swamp: trade itself for Polymerization when we have none.
        if Self::fusion_spells_in_hand(&ctx) == 0 && ctx.hand_size(ctx.me) >= 2 {
            if let Some(i) = t.activate_from(KING_OF_THE_SWAMP, Location::Hand) {
                return t.pick(i);
            }
        }
        None
    }

    fn main_phase_late(&mut self, t: &mut Turn) -> Option<usize> {
        // Parallel World Fusion forbids other Special Summons this turn: last.
        if Self::want_fusion(&t.ctx) {
            if let Some(i) = t.activate(PARALLEL_WORLD_FUSION) {
                return t.pick(i);
            }
        }
        None
    }

    fn allow_staple(&self, t: &Turn, code: u32) -> bool {
        // Keep Giant Trunade from bouncing our own Fusion Gate / Future Fusion.
        !(code == crate::staples::GIANT_TRUNADE
            && (t.ctx.face_up_on_field(t.ctx.me, FUSION_GATE) || t.ctx.face_up_on_field(t.ctx.me, FUTURE_FUSION)))
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let small_revival = ctx
            .graveyard(ctx.me)
            .iter()
            .any(|c| c.code.map_or(false, |g| ctx.data(g).is_monster() && ctx.data(g).attack <= 500));
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, STRATOS) => Some(2300.0),
            (ChoiceKind::NormalSummon, NEOS_ALIUS) => Some(1900.0),
            (ChoiceKind::NormalSummon, PRISMA) => Some(1850.0),
            (ChoiceKind::NormalSummon, MIST_VALLEY_SOLDIER) => Some(1700.0),
            (ChoiceKind::NormalSummon, ARMORED_BEE) => Some(1650.0),
            (ChoiceKind::NormalSummon, OCEAN) => Some(1600.0),
            (ChoiceKind::NormalSummon, DEBRIS_DRAGON) if small_revival => Some(1500.0),
            (ChoiceKind::SetMonster, SNOWMAN_EATER) if !ctx.monsters(ctx.opp).is_empty() => Some(1500.0),
            (ChoiceKind::SetMonster, WOODSMAN) => Some(1450.0),
            (ChoiceKind::SetMonster, PENGUIN_SOLDIER) => Some(1400.0),
            (ChoiceKind::NormalSummon, VOLTIC) => Some(1100.0),
            (ChoiceKind::NormalSummon, SNOWMAN_EATER | PENGUIN_SOLDIER | KING_OF_THE_SWAMP) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // Returns one of our monsters to the hand for a 1400 body.
            BIRDMAN => false,
            // Destroys every card on the field, ours included: only when losing badly.
            BLACK_ROSE_DRAGON => ctx.field_strength(ctx.opp) > ctx.field_strength(ctx.me) + 2500,
            _ => return None,
        })
    }

    fn attack_outcome(&self, ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
        if !ctx.is(attacker, ABSOLUTE_ZERO) {
            return None;
        }
        // If Absolute Zero falls, every monster they control goes with it.
        Some(match default_outcome(ctx, attacker, target, 0) {
            Outcome::Lose | Outcome::Trade if ctx.monsters(ctx.opp).len() >= 2 => Outcome::Win { trick: false },
            other => other,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let hostile = t.hostile_top();
        Some(match code {
            // Only offered when a destruction effect hits 2+ of our cards.
            STARLIGHT_ROAD => Response::new(95.0),
            STARDUST_DRAGON if hostile.matches(|_| true) => Response::new(90.0),
            STARDUST_DRAGON => Response::no(),
            // Quick-play and unanswerable: absorb the attacker.
            SUPER_POLYMERIZATION => match ctx.incoming_attack() {
                Some((attacker, target)) if attacker.position.face_up && ctx.attack_hurts(attacker, target) => {
                    Response::targeting(85.0, vec![attacker.at])
                }
                _ => Response::no(),
            },
            // Terra Firma's Tribute boost is not worth a HERO.
            TERRA_FIRMA => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let code = member.code.map(|c| ctx.canonical(c));
        let mine = member.at.controller == ctx.me;
        match t.decision.hint {
            // The Fusion Monster to summon, or the one Future Fusion / Prisma reveals.
            Hint::SpecialSummon | Hint::Confirm if member.at.location == Location::Extra => {
                code.map(|c| Self::fusion_score(&ctx, c))
            }
            // Materials: theirs first (Super Polymerization), then what is
            // already spent (Graveyard, banished), then the cheapest in hand/field.
            Hint::FusionMaterial | Hint::SynchroMaterial if mine => {
                let worth = value(self, &ctx, code, ctx.card(member.at).filter(|v| v.known())) as f64;
                Some(match member.at.location {
                    Location::Graveyard | Location::Banished => -worth * 0.1,
                    _ => -worth,
                })
            }
            // Ocean returns a HERO from our Graveyard: a gain, not a bounce.
            Hint::ReturnToHand if mine && member.at.location == Location::Graveyard => {
                code.map(|c| value(self, &ctx, Some(c), None) as f64)
            }
            // Prisma / Future Fusion send materials from the Deck to fuel Miracle Fusion:
            // prefer HEROes (Miracle Fusion can banish them).
            Hint::ToGraveyard if mine && member.at.location == Location::Deck => {
                code.map(|c| if Self::is_hero(&ctx, c) { 1000.0 } else { 0.0 })
            }
            _ => None,
        }
    }

    fn option(&self, t: &Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Stratos: destroy Spells/Traps (1) or search a HERO (2).
        let wreck = ctx.spell_traps(ctx.opp).len() >= 1;
        let wanted = if wreck { 1 } else { 2 };
        t.choices().find(|(_, c)| c.description & 0xf == wanted).map(|(i, _)| i).or(Some(0))
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        match t.ctx.canonical(code) {
            // Super Polymerization stays in hand: it cannot be answered anyway,
            // and Setting it would telegraph it.
            SUPER_POLYMERIZATION => Some(false),
            MIRACLE_SYNCHRO_FUSION => Some(true),
            _ => None,
        }
    }
}
