//! "Dragunity Flight": small Dragons equipped to Winged Beasts, then Synchro.
//!
//! The Dragunity Dragons (Level 1-3 Tuners) spend most of the duel as Equip
//! Spells: Dux / Legionnaire / Mystletainn / Leyvaten equip one from the
//! Graveyard, Aklys equips itself.  Equipped, they are a resource: Phalanx
//! Special Summons itself back (Tuner + host = Synchro), Legionnaire sends one
//! to the Graveyard to destroy a monster, Aklys destroys a card on its way,
//! Brandistock gives a second attack, Militum summons them from the Spell &
//! Trap Zone.  Dragon Ravine and Tribus stock the Graveyard (Phalanx first).
//!
//! Synchros: Vajrayana (6), Barcha (8, equips every Dragunity Dragon),
//! Trishula (9, banishes 3), Mist Wurm (9, bounces 3), Scrap Dragon,
//! Gaia Knight.

use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};

pub const DECK: &str = "Dragunity Flight";

const MYSTLETAINN: u32 = 876330;
const LEYVATEN: u32 = 63487632;
const LIGHT_AND_DARKNESS: u32 = 47297616;
const AKLYS: u32 = 36870345;
const JAVELIN: u32 = 80549379;
const DUX: u32 = 28183605;
const TRIBUS: u32 = 81962318;
const PHALANX: u32 = 59755122;
const DARKSPEAR: u32 = 13361027;
const BRANDISTOCK: u32 = 54455664;
const MILITUM: u32 = 81661951;
const LEGIONNAIRE: u32 = 54578613;
const RELEASE_RESTRAINT_WAVE: u32 = 98847704;
const FISSURE: u32 = 66788016;
const CARDS_OF_CONSONANCE: u32 = 39701395;
const TERRAFORMING: u32 = 73628505;
const DRAGON_RAVINE: u32 = 62265044;
const CATASTOR: u32 = 26593852;
const SCRAP_DRAGON: u32 = 76774528;
const TRIDENT_DRAGION: u32 = 39402797;
const BARCHA: u32 = 25682811;
const VAJRAYANA: u32 = 21249921;
const TRISHULA: u32 = 52687916;
const BRIONAC: u32 = 50321796;
const MIST_WURM: u32 = 27315304;
const BLACK_ROSE_DRAGON: u32 = 73580471;
const SET_DRAGUNITY: u16 = 0x29;

#[derive(Clone, Default)]
pub struct Dragunity;

impl Dragunity {
    fn is_dragunity(ctx: &Ctx, card: &CardView) -> bool {
        card.code.map_or(false, |c| ctx.data(c).in_set(SET_DRAGUNITY))
    }

    /// Dragunity cards in our Spell & Trap Zone (equipped Dragons).
    fn equipped(ctx: &Ctx) -> usize {
        ctx.spell_traps(ctx.me).iter().filter(|c| c.position.face_up && Self::is_dragunity(ctx, c)).count()
    }

    fn in_graveyard(ctx: &Ctx, code: u32) -> bool {
        ctx.graveyard(ctx.me).iter().any(|c| ctx.is(c, code))
    }

    fn winged_host_in_hand(ctx: &Ctx) -> bool {
        [DUX, LEGIONNAIRE].iter().any(|c| ctx.in_hand(*c))
    }

    /// How good each small Dragon is as an equip (or in the Graveyard waiting to be one).
    fn equip_score(code: u32) -> f64 {
        match code {
            PHALANX => 1900.0,  // Special Summons itself: a Tuner for the Synchro
            AKLYS => 1700.0,    // destroys a card when sent from equip
            BRANDISTOCK => 1500.0,
            DARKSPEAR => 1200.0,
            JAVELIN => 1000.0,
            _ => 500.0,
        }
    }

    fn synchro_score(ctx: &Ctx, code: u32) -> f64 {
        let their_cards = (ctx.monsters(ctx.opp).len() + ctx.spell_traps(ctx.opp).len()) as f64;
        let equips_in_graveyard =
            ctx.graveyard(ctx.me).iter().filter(|c| Self::is_dragunity(ctx, c) && ctx.view_data(c).level <= 3).count() as f64;
        match code {
            TRISHULA => 2700.0 + 250.0 * their_cards.min(2.0) + 300.0,
            MIST_WURM => 2500.0 + 200.0 * their_cards.min(3.0),
            BARCHA => 2000.0 + 300.0 * equips_in_graveyard + 400.0,
            SCRAP_DRAGON => 2800.0,
            TRIDENT_DRAGION => 2600.0,
            BRIONAC => 2300.0 + 100.0 * their_cards.min(3.0),
            CATASTOR => 2200.0,
            VAJRAYANA => 2100.0,
            _ => ctx.data(code).attack as f64,
        }
    }
}

impl Strategy for Dragunity {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            TRISHULA | SCRAP_DRAGON | TRIDENT_DRAGION | MIST_WURM | BARCHA | BRIONAC | CATASTOR | VAJRAYANA => {
                Self::synchro_score(ctx, code) as i32
            }
            LIGHT_AND_DARKNESS => 2800,
            LEYVATEN => 2600,
            DRAGON_RAVINE => 2300,
            MYSTLETAINN => 2100,
            DUX | LEGIONNAIRE => 1900,
            PHALANX => 1850,
            MILITUM | TERRAFORMING => 1700,
            AKLYS | TRIBUS => 1600,
            CARDS_OF_CONSONANCE | FISSURE => 1400,
            BRANDISTOCK | DARKSPEAR => 1300,
            RELEASE_RESTRAINT_WAVE => 1200,
            JAVELIN => 1000,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !ctx.face_up_on_field(ctx.me, DRAGON_RAVINE) {
            if let Some(i) = t.activate_from(DRAGON_RAVINE, Location::Hand) {
                return t.pick(i);
            }
            if let Some(i) = t.activate(TERRAFORMING) {
                return t.pick(i);
            }
        }
        // Dragon Ravine: discard 1 to stock the Graveyard / find a host.
        if ctx.hand_size(ctx.me) >= 2 {
            if let Some(i) = t.activate_from(DRAGON_RAVINE, Location::SpellTrapZone) {
                return t.pick(i);
            }
        }
        // Equipped Dragons come back out as Synchro material.
        if ctx.free_monster_zones(ctx.me) > 0 && !ctx.monsters(ctx.me).is_empty() {
            if let Some(i) = t.activate_from(PHALANX, Location::SpellTrapZone) {
                return t.pick(i);
            }
            if Self::equipped(&ctx) > 0 {
                if let Some(i) = t.activate_from(MILITUM, Location::MonsterZone) {
                    return t.pick(i);
                }
            }
        }
        // Legionnaire: an equipped Dragon for their best face-up monster.
        if Self::equipped(&ctx) > 0 {
            let target = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| c.position.face_up)
                .max_by_key(|c| ctx.threat(c))
                .filter(|c| ctx.threat(c) >= 1500);
            if let Some(target) = target {
                if let Some(i) = t.activate_from(LEGIONNAIRE, Location::MonsterZone) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
        }
        if ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up) {
            if let Some(i) = t.activate(FISSURE) {
                return t.pick(i);
            }
        }
        // Release Restraint Wave: one equip for every Set card they have.
        if ctx.set_backrow(ctx.opp).len() >= 2 && Self::equipped(&ctx) > 0 {
            if let Some(i) = t.activate(RELEASE_RESTRAINT_WAVE) {
                return t.pick(i);
            }
        }
        // Cards of Consonance: a spare Dragon Tuner (ideally Phalanx, which
        // wants the Graveyard anyway) for two cards.
        let tuners_in_hand = ctx.hand().iter().filter(|c| ctx.view_data(c).is_tuner()).count();
        if tuners_in_hand >= 1 && (tuners_in_hand >= 2 || ctx.in_hand(PHALANX)) {
            if let Some(i) = t.activate(CARDS_OF_CONSONANCE) {
                return t.pick(i);
            }
        }
        // Darkspear: a Dragon for a Winged Beast back from the Graveyard.
        if ctx.graveyard(ctx.me).iter().any(|c| ctx.is(c, DUX) || ctx.is(c, LEGIONNAIRE) || ctx.is(c, MILITUM)) {
            if let Some(i) = t.activate_from(DARKSPEAR, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let equip_ready = [PHALANX, AKLYS, BRANDISTOCK, DARKSPEAR, JAVELIN].iter().any(|c| Self::in_graveyard(&ctx, *c));
        Some(match (choice.kind, code) {
            // Hosts that equip a Dragon from the Graveyard.
            (ChoiceKind::NormalSummon, DUX) if equip_ready => Some(2200.0),
            (ChoiceKind::NormalSummon, LEGIONNAIRE) if equip_ready => Some(2150.0),
            // Aklys brings a Dragunity from the hand and equips itself to it.
            (ChoiceKind::NormalSummon, AKLYS) if ctx.hand().iter().any(|c| Self::is_dragunity(&ctx, c) && !ctx.is(c, AKLYS)) => Some(2100.0),
            // Tribus stocks the Graveyard for the next host.
            (ChoiceKind::NormalSummon, TRIBUS) if !equip_ready && Self::winged_host_in_hand(&ctx) => Some(2050.0),
            (ChoiceKind::NormalSummon, DUX | LEGIONNAIRE | MILITUM) => Some(1800.0),
            (ChoiceKind::NormalSummon, TRIBUS) => Some(1300.0),
            (ChoiceKind::NormalSummon, PHALANX | BRANDISTOCK | JAVELIN | DARKSPEAR) => Some(900.0),
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code().unwrap_or(0));
        Some(match code {
            // Costs a face-up Dragunity: fine when it is a spent small one.
            MYSTLETAINN => ctx.monsters(ctx.me).iter().any(|c| Self::is_dragunity(&ctx, c) && c.attack <= 1200),
            // Banishes an equipped monster: when no Synchro is on the way.
            LEYVATEN => !ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).is_tuner()),
            // Destroys every card on the field, ours included: only when losing badly.
            BLACK_ROSE_DRAGON => ctx.field_strength(ctx.opp) > ctx.field_strength(ctx.me) + 2500,
            _ if choice.at().map(|a| a.location) == Some(Location::Extra) => true,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        Some(match code {
            // Negates any activation, shrinking by 500 each time.
            LIGHT_AND_DARKNESS if t.hostile_top().matches(|_| true) => Response::new(60.0),
            LIGHT_AND_DARKNESS => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let code = member.code.map(|c| ctx.canonical(c))?;
        if member.at.controller != ctx.me {
            return None;
        }
        match t.decision.hint {
            // Which Dragon to equip.
            Hint::Equip => Some(Self::equip_score(code)),
            // Tribus / Dragon Ravine send from the Deck: the next equip.
            Hint::ToGraveyard if member.at.location == Location::Deck => Some(Self::equip_score(code)),
            // Dragon Ravine search: a host if we have Dragons to equip, else Tribus.
            Hint::AddToHand if member.at.location == Location::Deck => Some(match code {
                DUX | LEGIONNAIRE if Self::equipped(&ctx) + ctx.graveyard(ctx.me).len() > 0 => 2000.0,
                TRIBUS => 1700.0,
                other => ctx.data(other).attack as f64,
            }),
            // The Synchro (Extra Deck) to summon.
            Hint::SpecialSummon if member.at.location == Location::Extra => Some(Self::synchro_score(&ctx, code)),
            // Costs from the Spell & Trap Zone (Legionnaire, Release Restraint
            // Wave): Aklys first, its trigger destroys another card.
            Hint::ToGraveyard | Hint::Destroy if member.at.location == Location::SpellTrapZone => {
                Some(if code == AKLYS { 100.0 } else { -Self::equip_score(code) })
            }
            _ => None,
        }
    }

    fn option(&self, t: &Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Dragon Ravine: search a Dragunity (1) or send a Dragon to the Graveyard (2).
        let graveyard_dragon = [PHALANX, AKLYS, BRANDISTOCK].iter().any(|c| Self::in_graveyard(&ctx, *c));
        let wanted = if !graveyard_dragon && Self::winged_host_in_hand(&ctx) { 2 } else { 1 };
        t.choices().find(|(_, c)| c.description & 0xf == wanted).map(|(i, _)| i).or(Some(0))
    }
}
