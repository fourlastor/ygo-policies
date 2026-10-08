//! Eclipse Without End: supported 4000-ATK summons, Skill Drain and field protection.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Eclipse Without End";
const CYBER: u32 = 1710476;
const STAR: u32 = 36521459;
const BLUE: u32 = 9433350;
const TRUTH: u32 = 37115575;
const GEAR: u32 = 74509280;
const PARADOX: u32 = 8310162;
const WORLD: u32 = 27564031;
const VALLEY: u32 = 47355498;
const DRAIN: u32 = 82732705;
const CLAW: u32 = 53063039;
const BARBAROS: u32 = 78651105;
#[derive(Clone, Default)]
pub struct Malefic;
impl Malefic {
    fn supported(ctx: &Ctx) -> bool {
        ctx.effects_drained()
            || [ctx.me, ctx.opp].iter().any(|&p| {
                ctx.spell_traps(p)
                    .iter()
                    .any(|c| c.position.face_up && ctx.view_data(c).is(crate::cards::types::FIELD))
            })
    }
}
impl Strategy for Malefic {
    fn value(&self, _: &Ctx, k: u32) -> Option<i32> {
        Some(match k {
            CYBER | PARADOX => 4400,
            TRUTH => 5200,
            STAR => 3300,
            BLUE => 3400,
            WORLD | VALLEY => 3000,
            DRAIN => 3300,
            _ => return support::extra_value(k),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !Self::supported(&ctx) {
            for k in [WORLD, VALLEY] {
                if let Some(i) = t.activate(k) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.activate(38120068) {
            if ctx
                .hand()
                .iter()
                .any(|c| matches!(c.code, Some(89631139) | Some(BLUE)))
                || ctx.hand().iter().filter(|c| c.level == 8).count() >= 2
            {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(DRAIN) {
            if ctx.my_lp() > 1000 && !ctx.effects_drained() {
                return t.pick(i);
            }
        }
        for k in [CYBER, BLUE, STAR] {
            if Self::supported(&ctx) {
                if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(k), Some(Location::Hand)) {
                    return t.pick(i);
                }
            }
        }
        if let (Some(i), Some(c)) = (t.activate(CLAW), support::target(&ctx, CLAW, true)) {
            if c.at.location == Location::MonsterZone {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let k = c.code()?;
        Some(match (c.kind, k) {
            (ChoiceKind::NormalSummon, GEAR)
                if !t.ctx.effects_drained()
                    && t.ctx.face_up_on_field(t.ctx.me, WORLD)
                    && t.ctx
                        .hand()
                        .iter()
                        .any(|c| c.level == 8 && t.ctx.view_data(c).in_set(0x23)) =>
            {
                Some(6000.0)
            }
            (ChoiceKind::NormalSummon, BARBAROS) => Some(if t.ctx.effects_drained() {
                4000.0
            } else {
                2300.0
            }),
            (_, GEAR | 89631139 | CYBER | STAR | BLUE | TRUTH) => None,
            _ => return None,
        })
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        match c.code()? {
            CYBER | STAR | BLUE => Some(Self::supported(&t.ctx)),
            PARADOX => Some(t.ctx.face_up_on_field(t.ctx.me, WORLD) || t.ctx.effects_drained()),
            _ => support::extra_allowed(t, c),
        }
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        Some(match t.choice(i).code()? {
            TRUTH if Self::supported(&ctx) => Response::new(130.0),
            PARADOX => Response::new(130.0),
            WORLD
                if t.choice(i).at().map(|a| a.location) == Some(Location::SpellTrapZone)
                    && !ctx
                        .hand()
                        .iter()
                        .any(|c| [CYBER, STAR, BLUE].contains(&c.code.unwrap_or(0))) =>
            {
                Response::new(120.0)
            }
            DRAIN
                if ctx.my_lp() > 1000
                    && !ctx.effects_drained()
                    && (!ctx.my_turn() || ctx.face_up_on_field(ctx.me, BARBAROS)) =>
            {
                Response::new(90.0)
            }
            CLAW => ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| ctx.reaches(c, CLAW, true, true))
                .max_by_key(|c| ctx.threat(c))
                .map(|c| Response::targeting(80.0, vec![c.at]))
                .unwrap_or_else(Response::no),
            WORLD | VALLEY | DRAIN | TRUTH => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if m.at.controller != t.ctx.me {
            return None;
        }
        let k = m.code?;
        if t.decision.hint == Hint::AddToHand {
            return Some(match k {
                CYBER => 8000.0,
                STAR => 6500.0,
                BLUE => 6000.0,
                WORLD if !Self::supported(&t.ctx) => 9000.0,
                VALLEY => 4000.0,
                _ => 0.0,
            });
        }
        if t.decision.hint == Hint::Discard {
            return Some(match k {
                89631139 => 6000.0,
                BLUE => 4000.0,
                TRUTH => 3000.0,
                _ => 0.0,
            });
        }
        None
    }
}
