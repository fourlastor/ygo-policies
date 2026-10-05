//! "Morphtronic - Straight Up": one effect in Attack Position, another in
//! Defense Position.
//!
//! Battle Position is the deck's main lever, not a combat afterthought:
//! Radion in Attack Position gives every Morphtronic +800 ATK, Boarden in
//! Attack Position lets them all attack directly (in Defense Position, the
//! others cannot be destroyed by battle), Magnen in Defense Position draws
//! every attack, Scopen in Attack Position Special Summons a Level 4 from the
//! hand and in Defense Position becomes Level 4, Boomboxen attacks twice.
//! The deck Synchro Summons Power Tool Dragon (Scopen/Remoten + a Level 4 or 3)
//! and dresses it in Double Tool C&D; Morphtronic Map grows with every
//! position change; Morphtronic Bind stops the opponent's Level 4+ attackers.

use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Phase, Position};

pub const DECK: &str = "01 Morphtronic - Straight Up";

const RADION: u32 = 55119278;
const SCOPEN: u32 = 10591919;
const MAGNEN: u32 = 29947751;
const CAMERAN: u32 = 28124263;
const BOARDEN: u32 = 48381268;
const REMOTEN: u32 = 57108202;
const CELFON: u32 = 93542102;
const BOOMBOXEN: u32 = 92720564;
const DOUBLE_TOOL: u32 = 63730624;
const REPAIR_UNIT: u32 = 90239723;
const CORD: u32 = 70423794;
const MAP: u32 = 56074358;
const JUNK_BOX: u32 = 37745919;
const ACCELERATOR: u32 = 44424095;
const MORPHTRANSITION: u32 = 92890308;
const BIND: u32 = 85101228;
const POWER_TOOL_DRAGON: u32 = 2403771;
const SET_MORPHTRONIC: u16 = 0x26;

#[derive(Default)]
pub struct Morphtronic;

impl Morphtronic {
    fn is_morph(ctx: &Ctx, card: &CardView) -> bool {
        card.code.map_or(false, |c| ctx.data(c).is_monster() && ctx.data(c).in_set(SET_MORPHTRONIC))
    }

    fn ours<'a>(ctx: &Ctx<'a>, code: u32) -> Option<&'a CardView> {
        ctx.monsters(ctx.me).into_iter().find(|c| c.position.face_up && ctx.is(c, code))
    }

    /// Our face-up Morphtronics that could still attack this turn.
    fn attackers(ctx: &Ctx) -> usize {
        ctx.monsters(ctx.me).iter().filter(|c| Self::is_morph(ctx, c) && ctx.can_attack(c)).count()
    }

    /// Where each Morphtronic wants to be now.
    fn wanted(ctx: &Ctx, code: u32) -> Option<Position> {
        let attacking = ctx.my_turn() && ctx.main1();
        let threatened = ctx.opp_best_attack() > ctx.my_best_attack();
        Some(match code {
            RADION => Position::FACE_UP_ATTACK,
            // Direct attacks for everyone, if anyone can use them.
            BOARDEN if attacking && Self::attackers(ctx) >= 2 && !ctx.monsters(ctx.opp).is_empty() => Position::FACE_UP_ATTACK,
            BOARDEN => Position::FACE_UP_DEFENSE,
            // Magnen soaks the attacks while we are behind.
            MAGNEN if threatened || !attacking => Position::FACE_UP_DEFENSE,
            MAGNEN => Position::FACE_UP_ATTACK,
            BOOMBOXEN if attacking => Position::FACE_UP_ATTACK,
            BOOMBOXEN => Position::FACE_UP_DEFENSE,
            // Cameran in Defense Position shields Morphtronics from targeting.
            CAMERAN if !attacking => Position::FACE_UP_DEFENSE,
            SCOPEN | REMOTEN | CELFON | CAMERAN => Position::FACE_UP_ATTACK,
            _ => return None,
        })
    }

    fn level4_morph_in_hand(ctx: &Ctx) -> bool {
        ctx.hand().iter().any(|c| Self::is_morph(ctx, c) && ctx.view_data(c).level == 4)
    }
}

impl Strategy for Morphtronic {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            POWER_TOOL_DRAGON => 2800,
            RADION => 2000,
            DOUBLE_TOOL | BIND => 1900,
            BOARDEN => 1800,
            SCOPEN | REMOTEN => 1700,
            BOOMBOXEN | ACCELERATOR => 1600,
            MAP | REPAIR_UNIT => 1500,
            MAGNEN | MORPHTRANSITION => 1400,
            JUNK_BOX => 1300,
            CAMERAN => 1200,
            CELFON => 1000,
            CORD => 700,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !ctx.face_up_on_field(ctx.me, MAP) {
            if let Some(i) = t.activate_from(MAP, Location::Hand) {
                return t.pick(i);
            }
        }
        // Put each Morphtronic in the position whose effect we want now.
        let turn_pos = t.find_where(|c| {
            if c.kind != ChoiceKind::ChangePosition {
                return false;
            }
            let Some(card) = c.at().and_then(|at| ctx.card(at)) else { return false };
            let Some(code) = card.code.map(|code| ctx.canonical(code)) else { return false };
            match Self::wanted(&ctx, code) {
                Some(want) => card.position.face_up && card.position.attack != want.attack,
                None => false,
            }
        });
        if let Some(i) = turn_pos {
            return t.pick(i);
        }
        // Effects that build the board.
        if let Some(scopen) = Self::ours(&ctx, SCOPEN) {
            if scopen.position.attack && Self::level4_morph_in_hand(&ctx) {
                if let Some(i) = t.activate_from(SCOPEN, Location::MonsterZone) {
                    return t.pick(i);
                }
            }
        }
        for code in [CELFON, REMOTEN, POWER_TOOL_DRAGON] {
            if let Some(i) = t.activate_from(code, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        // Double Tool C&D on Power Tool Dragon, else a Level 4+ Machine Morphtronic.
        let holder = Self::ours(&ctx, POWER_TOOL_DRAGON).or_else(|| Self::ours(&ctx, BOOMBOXEN));
        if let Some(holder) = holder {
            if let Some(i) = t.activate(DOUBLE_TOOL) {
                return t.pick_targeting(i, vec![holder.at]);
            }
        }
        // Accelerator: a spare Morphtronic for their best card and a draw.
        let target = ctx
            .monsters(ctx.opp)
            .into_iter()
            .chain(ctx.spell_traps(ctx.opp))
            .max_by_key(|c| if ctx.view_data(c).is_monster() || c.known() { ctx.threat(c) } else { 1600 });
        if let Some(target) = target {
            if let Some(i) = t.activate(ACCELERATOR) {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        let graveyard_morphs = ctx.graveyard(ctx.me).iter().filter(|c| Self::is_morph(&ctx, c)).count();
        if graveyard_morphs > 0 && ctx.free_monster_zones(ctx.me) > 0 {
            if let Some(i) = t.activate(REPAIR_UNIT) {
                return t.pick(i);
            }
            // Junk Box's monster dies in the End Phase: use it to Synchro or attack.
            if ctx.main1() {
                if let Some(i) = t.activate(JUNK_BOX) {
                    return t.pick(i);
                }
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let has_tuner = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).is_tuner());
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, RADION) => Some(2000.0),
            (ChoiceKind::NormalSummon, SCOPEN) if Self::level4_morph_in_hand(&ctx) => Some(1950.0),
            (ChoiceKind::NormalSummon, BOOMBOXEN) => Some(1750.0 + if has_tuner { 100.0 } else { 0.0 }),
            (ChoiceKind::NormalSummon, SCOPEN | REMOTEN) => Some(1650.0),
            (ChoiceKind::NormalSummon, BOARDEN) => Some(1550.0),
            (ChoiceKind::NormalSummon, CELFON) => Some(1300.0),
            (ChoiceKind::NormalSummon, MAGNEN | CAMERAN) => Some(1200.0),
            // Face-down, a Morphtronic has neither effect.
            (ChoiceKind::SetMonster, _) if ctx.data(code).in_set(SET_MORPHTRONIC) => None,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        Some(match code {
            MORPHTRANSITION => match ctx.incoming_attack() {
                Some((attacker, target)) if ctx.attack_hurts(attacker, target) => Response::new(70.0),
                _ => Response::no(),
            },
            // Lock the opponent's Level 4+ attackers before they swing.
            BIND => {
                let big = ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up && c.level >= 4);
                let before_battle = !ctx.my_turn() && matches!(ctx.phase(), Some(Phase::Standby | Phase::Main1 | Phase::BattleStart));
                if big && (before_battle || ctx.incoming_attack().is_some()) && ctx.monsters(ctx.me).iter().any(|c| Self::is_morph(&ctx, c)) {
                    Response::new(65.0)
                } else {
                    Response::no()
                }
            }
            CORD => Response::no(),
            _ => return None,
        })
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        Self::wanted(&t.ctx, t.ctx.canonical(code))
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        // Power Tool Dragon: the opponent picks among the three we show;
        // show the best Equip Spells.
        if t.decision.hint == Hint::Confirm && member.at.controller == ctx.me && member.at.location == Location::Deck {
            return member.code.map(|c| crate::agent::value(self, &ctx, Some(ctx.canonical(c)), None) as f64);
        }
        None
    }
}
