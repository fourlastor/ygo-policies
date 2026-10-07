//! The Quiet Grove: recruit Naturia tribute fodder, establish Bamboo Shoot,
//! and protect it with traps or independent Naturia Synchro negation.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "The Quiet Grove";
const BAMBOO: u32 = 20174189;
const CLIFF: u32 = 33866130;
const CHERRIES: u32 = 60668166;
const PUMPKIN: u32 = 96653775;
const BUTTERFLY: u32 = 42110434;
const BEANS: u32 = 44789585;
const ANTJAW: u32 = 99150062;
const BEAST: u32 = 33198837;
const BARKION: u32 = 2956282;
const DOUBLE: u32 = 43422537;
const RYKO: u32 = 21502796;
#[derive(Clone, Default)]
pub struct Naturia;
impl Strategy for Naturia {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            BAMBOO => 3700,
            BEAST => 3200,
            BARKION => 2900,
            CLIFF => 2100,
            CHERRIES => 1600,
            PUMPKIN => 1900,
            ANTJAW => 1600,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if ctx.in_hand(BAMBOO)
            && ctx.obs.summon_used
            && ctx
                .monsters(ctx.me)
                .iter()
                .any(|c| ctx.view_data(c).in_set(0x2a) && !ctx.is(c, BAMBOO))
        {
            if let Some(i) = t.activate(DOUBLE) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, BAMBOO)
                if !ctx.face_up_on_field(ctx.me, BAMBOO)
                    && ctx
                        .monsters(ctx.me)
                        .iter()
                        .any(|c| ctx.view_data(c).in_set(0x2a)) =>
            {
                Some(5500.0)
            }
            (_, BAMBOO) => None,
            (ChoiceKind::NormalSummon, PUMPKIN)
                if !ctx.monsters(ctx.opp).is_empty()
                    && ctx
                        .hand()
                        .iter()
                        .any(|c| ctx.view_data(c).in_set(0x2a) && !ctx.is(c, PUMPKIN)) =>
            {
                Some(3300.0)
            }
            (ChoiceKind::SetMonster, CHERRIES | BEANS | ANTJAW) => Some(2000.0),
            (ChoiceKind::NormalSummon, CHERRIES | BUTTERFLY)
                if support::body_score(self, &ctx, code) > 2000.0 =>
            {
                Some(3400.0)
            }
            (ChoiceKind::NormalSummon, CHERRIES | BEANS | ANTJAW) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        Some(match ctx.canonical(t.choice(i).code()?) {
            CLIFF | CHERRIES | PUMPKIN | ANTJAW | BEANS => Response::new(120.0),
            BEAST if t.hostile_top().matches(|l| ctx.data(l.code).is_spell()) => {
                Response::new(95.0)
            }
            BARKION if t.hostile_top().matches(|l| ctx.data(l.code).is_trap()) => {
                Response::new(95.0)
            }
            BUTTERFLY
                if ctx
                    .incoming_attack()
                    .map_or(false, |(a, b)| ctx.attack_hurts(a, b)) =>
            {
                Response::new(90.0)
            }
            BEAST | BARKION | BUTTERFLY => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        if matches!(t.decision.hint, Hint::Tribute | Hint::Release) {
            return Some(if code != BAMBOO && ctx.data(code).in_set(0x2a) {
                5000.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(match code {
                BAMBOO => 100.0,
                BUTTERFLY if ctx.face_up_on_field(ctx.me, BAMBOO) => 4500.0,
                CLIFF => 3000.0,
                CHERRIES
                    if ctx
                        .monsters(ctx.me)
                        .iter()
                        .any(|c| !ctx.view_data(c).is_tuner()) =>
                {
                    4000.0
                }
                _ => support::body_score(self, &ctx, code),
            });
        }
        if t.decision.hint == Hint::SynchroMaterial && code == BAMBOO {
            return Some(-20000.0);
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        if c.at().map(|a| a.location) == Some(Location::Extra)
            && ctx.face_up_on_field(ctx.me, BAMBOO)
        {
            let level = ctx.data(c.code()?).level;
            let other: Vec<_> = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| !ctx.is(c, BAMBOO) && c.position.face_up)
                .collect();
            if !other.iter().any(|a| {
                other.iter().any(|b| {
                    a.at != b.at
                        && ctx.view_data(a).is_tuner() != ctx.view_data(b).is_tuner()
                        && a.level + b.level == level
                })
            }) {
                return Some(false);
            }
        }
        support::extra_allowed(t, c)
    }
    fn yes_no(&self, t: &Turn) -> Option<bool> {
        (t.decision.subject == Some(RYKO)).then(|| {
            !t.ctx.monsters(t.ctx.opp).is_empty() || !t.ctx.spell_traps(t.ctx.opp).is_empty()
        })
    }
}
