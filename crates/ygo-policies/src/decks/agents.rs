//! Heaven's Dispatch: Earth/Venus into Synchros, Hyperion removal and Kristya.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::{races, types};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
use crate::{staples, tactics};
pub const DECK: &str = "Heaven's Dispatch";
const EARTH: u32 = 91188343;
const VENUS: u32 = 64734921;
const BALL: u32 = 39552864;
const HYPERION: u32 = 55794644;
const KRISTYA: u32 = 59509952;
const ORANGE: u32 = 17266660;
const HECATRICE: u32 = 74968065;
const VALHALLA: u32 = 1353770;
const SANCTUARY: u32 = 56433456;
#[derive(Clone, Default)]
pub struct Agents;
impl Strategy for Agents {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            HYPERION => 3100,
            KRISTYA => 3200,
            EARTH => 2000,
            VENUS => 2100,
            BALL => 500,
            ORANGE | support::HONEST => 1800,
            HECATRICE | VALHALLA => 1500,
            SANCTUARY => 1200,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.find(
            ChoiceKind::SpecialSummon,
            Some(HYPERION),
            Some(Location::Hand),
        ) {
            return t.pick(i);
        }
        if let Some(i) = t.activate_from(HYPERION, Location::MonsterZone) {
            if let Some(c) = support::target(&ctx, HYPERION, true) {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if !ctx.face_up_on_field(ctx.me, SANCTUARY)
            && (ctx.in_hand(EARTH) || ctx.face_up_on_field(ctx.me, HYPERION))
        {
            if let Some(i) = t.activate(SANCTUARY) {
                return t.pick(i);
            }
        }
        if !ctx.in_hand(VALHALLA) && !ctx.face_up_on_field(ctx.me, VALHALLA) {
            if let Some(i) = t.activate_from(HECATRICE, Location::Hand) {
                return t.pick(i);
            }
        }
        if ctx.monsters(ctx.me).is_empty()
            && (ctx.in_hand(HYPERION) || ctx.in_hand(KRISTYA) || ctx.in_hand(VENUS))
        {
            if let Some(i) = t.activate(VALHALLA) {
                return t.pick(i);
            }
            if !ctx.in_hand(VALHALLA) && !ctx.face_up_on_field(ctx.me, VALHALLA) {
                if let Some(i) = t.activate_from(HECATRICE, Location::Hand) {
                    return t.pick(i);
                }
            }
        }
        if ctx.my_lp() > 1500 && ctx.free_monster_zones(ctx.me) > 0 {
            let tuner = ctx
                .monsters(ctx.me)
                .iter()
                .any(|c| c.position.face_up && ctx.view_data(c).is_tuner());
            let needs_tributes = ctx.in_hand(KRISTYA) && ctx.monsters(ctx.me).len() < 2;
            if tuner || needs_tributes || ctx.monsters(ctx.opp).is_empty() {
                if let Some(i) = t.activate_from(VENUS, Location::MonsterZone) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.activate_from(support::HONEST, Location::MonsterZone) {
            return t.pick(i);
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, EARTH) => Some(3000.0),
            (ChoiceKind::NormalSummon, VENUS) => Some(
                if ctx
                    .monsters(ctx.me)
                    .iter()
                    .any(|c| ctx.view_data(c).is_tuner())
                {
                    3500.0
                } else {
                    2400.0
                },
            ),
            (ChoiceKind::NormalSummon, KRISTYA)
                if ctx
                    .monsters(ctx.me)
                    .iter()
                    .filter(|c| ctx.is(c, BALL))
                    .count()
                    >= 2 =>
            {
                Some(3100.0)
            }
            (ChoiceKind::SetMonster, support::HONEST) if ctx.monsters(ctx.me).is_empty() => {
                Some(1000.0)
            }
            (_, HYPERION | KRISTYA | support::HONEST) => None,
            (ChoiceKind::NormalSummon, ORANGE)
                if tactics::synchro_with_tuner(self, &ctx, 2).is_some() =>
            {
                Some(2100.0)
            }
            (ChoiceKind::NormalSummon, BALL)
                if ctx
                    .monsters(ctx.me)
                    .iter()
                    .any(|c| ctx.view_data(c).is_tuner()) =>
            {
                Some(1000.0)
            }
            (ChoiceKind::NormalSummon, BALL) if ctx.monsters(ctx.opp).is_empty() => Some(500.0),
            (ChoiceKind::SetMonster, BALL) if ctx.monsters(ctx.me).is_empty() => Some(800.0),
            (_, ORANGE | BALL) => None,
            _ => return None,
        })
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        if t.ctx.canonical(c.code()?) == KRISTYA {
            return Some(!t.choices().any(|(_, c)| {
                c.kind == ChoiceKind::SpecialSummon
                    && c.at().map(|a| a.location) == Some(Location::Extra)
            }));
        }
        support::extra_allowed(t, c)
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let c = t.choice(i);
        Some(match t.ctx.canonical(c.code()?) {
            EARTH | KRISTYA => Response::new(120.0),
            ORANGE if t.hostile_top().matches(|l| t.ctx.data(l.code).is_monster()) => {
                Response::new(90.0)
            }
            ORANGE | VENUS | HYPERION | HECATRICE | VALHALLA | SANCTUARY => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let source = t.memory.last_activated.map(|k| ctx.canonical(k));
        if t.decision.hint == Hint::AddToHand {
            return Some(match code {
                VENUS if !ctx.in_hand(VENUS) && !ctx.face_up_on_field(ctx.me, VENUS) => 5000.0,
                HYPERION if !ctx.in_hand(HYPERION) => 4500.0,
                ORANGE => 2500.0,
                _ => value(self, &ctx, Some(code), None) as f64,
            });
        }
        if t.decision.hint == Hint::SpecialSummon && source == Some(VALHALLA) {
            return Some(if code == HYPERION {
                6000.0
            } else if code == KRISTYA {
                5500.0
            } else {
                0.0
            });
        }
        if matches!(
            t.decision.hint,
            Hint::Banish | Hint::Tribute | Hint::Release | Hint::ToGraveyard
        ) {
            return Some(if code == BALL {
                4000.0
            } else if m.at.location == Location::Graveyard {
                2000.0 - ctx.data(code).attack as f64 / 10.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn attack_trick(&self, ctx: &Ctx, c: &CardView) -> i32 {
        support::honest_trick(ctx, c)
    }
    fn allow_staple(&self, t: &Turn, code: u32) -> bool {
        code != staples::POT_OF_AVARICE
            || !t.ctx.in_hand(KRISTYA)
            || t.ctx
                .graveyard(t.ctx.me)
                .iter()
                .filter(|c| t.ctx.view_data(c).race & races::FAIRY != 0)
                .count()
                != 4
    }
    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        Some(t.ctx.data(code).is_trap() || t.ctx.data(code).is(types::QUICKPLAY))
    }
}
