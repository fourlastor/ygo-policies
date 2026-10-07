//! Ashes to Inferno: Firedog develops Tuners, Laval mills stock the GY,
//! and Rekindling converts multiple 200-DEF FIRE monsters into Synchros.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Ashes to Inferno";
const DOG: u32 = 23297235;
const MAGICIAN: u32 = 95621257;
const POUN: u32 = 28332833;
const MILLER: u32 = 89893715;
const MAID: u32 = 2407147;
const WARRIOR: u32 = 52786469;
const CANNON: u32 = 38492752;
const LORD: u32 = 123709;
const BABY: u32 = 13761956;
const REKINDLE: u32 = 74845897;
const FOOLISH: u32 = 81439173;
const ONE: u32 = 2295440;
const GREATER: u32 = 12986807;
const DEITY: u32 = 26304459;
#[derive(Clone, Default)]
pub struct Flamvell;
impl Strategy for Flamvell {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            REKINDLE => 3400,
            DOG => 2300,
            MAGICIAN => 1900,
            MILLER => 1700,
            MAID => 1200,
            CANNON => 2000,
            LORD => 2500,
            GREATER => 2800,
            DEITY => 3000,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.activate(FOOLISH) {
            return t.pick(i);
        }
        if ctx.free_monster_zones(ctx.me) >= 2
            && ctx
                .graveyard(ctx.me)
                .iter()
                .filter(|c| ctx.view_data(c).defense == 200)
                .count()
                >= 2
        {
            if let Some(i) = t.activate(REKINDLE) {
                return t.pick(i);
            }
        }
        if !ctx.monsters(ctx.me).is_empty() {
            if let Some(i) = t.activate(ONE) {
                return t.pick(i);
            }
        }
        if ctx.obs.summon_used {
            if let Some(i) = t.activate_from(BABY, Location::Hand) {
                if let Some(c) = ctx
                    .monsters(ctx.me)
                    .into_iter()
                    .filter(|c| {
                        c.position.face_up
                            && ctx.view_data(c).attribute & crate::cards::attributes::FIRE != 0
                    })
                    .max_by_key(|c| c.attack)
                {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, DOG) => Some(2600.0),
            (ChoiceKind::NormalSummon, CANNON)
                if ctx
                    .banished(ctx.me)
                    .iter()
                    .any(|c| ctx.view_data(c).in_set(0x39)) =>
            {
                Some(4000.0)
            }
            (ChoiceKind::NormalSummon, MAGICIAN | MAID | BABY) => {
                Some(support::body_score(self, &ctx, code))
            }
            (ChoiceKind::NormalSummon, LORD) => Some(2600.0),
            (ChoiceKind::SetMonster, MILLER | POUN) => Some(1900.0),
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.ctx.canonical(t.choice(i).code()?) {
            DOG | POUN | MILLER | MAID | CANNON | LORD | DEITY | GREATER => Response::new(120.0),
            REKINDLE | FOOLISH | ONE | BABY => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn allow_repeated_chain(&self, t: &Turn, i: usize) -> bool {
        t.choice(i).code() == Some(MAID)
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let src = t.memory.last_activated.map(|k| ctx.canonical(k));
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            let other = ctx
                .graveyard(ctx.me)
                .iter()
                .any(|c| ctx.view_data(c).in_set(0x39) && !ctx.is(c, MAID))
                || t.decision.selected.iter().any(|a| {
                    t.decision
                        .choices
                        .iter()
                        .flat_map(|c| c.members.iter())
                        .any(|m| {
                            m.at == *a
                                && m.code
                                    .map_or(false, |k| ctx.data(k).in_set(0x39) && k != MAID)
                        })
                });
            return Some(match code {
                MAID if other => 6000.0,
                WARRIOR => 5000.0,
                LORD => 4500.0,
                MAID => 2000.0,
                _ => 0.0,
            });
        }
        if t.decision.hint == Hint::SpecialSummon {
            if src == Some(DOG) && code == MAGICIAN {
                return Some(6000.0);
            }
            let picked_tuner = t.decision.selected.iter().any(|a| {
                t.decision
                    .choices
                    .iter()
                    .flat_map(|c| c.members.iter())
                    .any(|m| m.at == *a && ctx.data(m.code.unwrap_or(0)).is_tuner())
            });
            return Some(
                support::body_score(self, &ctx, code)
                    + if ctx.data(code).is_tuner() && !picked_tuner {
                        2200.0
                    } else {
                        0.0
                    },
            );
        }
        if matches!(t.decision.hint, Hint::Discard | Hint::ToGraveyard)
            && m.at.location == Location::Hand
            && code == MAID
            && !ctx.monsters_banished()
        {
            return Some(4000.0);
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
}
