//! Second Bloom: Lonefire reaches Gigaplant; Supervise/second summons
//! unlock revival, while Spark and Chevalier convert Gemini bodies/equips.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::types;
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
use crate::tactics;
pub const DECK: &str = "Second Bloom";
const GIGA: u32 = 53257892;
const CHEVALIER: u32 = 96872283;
const BUTTERFLY: u32 = 16984449;
const LONEFIRE: u32 = 48686504;
const SPORE: u32 = 11747708;
const BULB: u32 = 67441435;
const DANDY: u32 = 15341821;
const COPY: u32 = 66457407;
const POISON: u32 = 40320754;
const SUPERVISE: u32 = 95750695;
const SPARK: u32 = 33846209;
const SWING: u32 = 96765646;
const FOOLISH: u32 = 81439173;
#[derive(Clone, Default)]
pub struct Gemini;
impl Gemini {
    fn spark(&self, t: &Turn) -> Response {
        let ctx = t.ctx;
        let ours = ctx
            .monsters(ctx.me)
            .into_iter()
            .filter(|c| c.position.face_up && c.level == 4 && ctx.view_data(c).is(types::GEMINI))
            .min_by_key(|c| value(self, &ctx, c.code, Some(c)));
        if let (Some(ours), Some(theirs)) = (ours, support::target(&ctx, SPARK, true)) {
            if ctx.threat(theirs) > 1200 {
                return Response::targeting(65.0, vec![ours.at, theirs.at]);
            }
        }
        Response::no()
    }
}
impl Strategy for Gemini {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            GIGA => 2900,
            LONEFIRE => 2500,
            CHEVALIER => 2100,
            BUTTERFLY => 1700,
            SUPERVISE => 2400,
            SPARK => 2000,
            SWING => 1800,
            POISON => 1600,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = support::copy_plant(self, t) {
            return Some(i);
        }
        let ctx = t.ctx;
        if ctx.free_monster_zones(ctx.me) > 0 {
            for code in [LONEFIRE, GIGA, BUTTERFLY, SWING] {
                if let Some(i) = t.activate(code) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.activate(SUPERVISE) {
            let best = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| c.position.face_up && ctx.view_data(c).is(types::GEMINI))
                .max_by_key(|c| {
                    if ctx.is(c, GIGA) {
                        5000
                    } else {
                        value(self, &ctx, c.code, Some(c))
                    }
                });
            if let Some(c) = best {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if let Some(i) = t.activate(CHEVALIER) {
            if let Some(c) = support::target(&ctx, CHEVALIER, true) {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if let Some(i) = t.activate(SPARK) {
            let r = self.spark(t);
            if r.score > 0.0 {
                return t.pick_targeting(i, r.intent);
            }
        }
        if let Some(i) = t.activate(FOOLISH) {
            return t.pick(i);
        }
        if tactics::synchro_with_tuner(self, &ctx, 1).is_some() {
            if let Some(i) = t.activate_from(BULB, Location::Graveyard) {
                return t.pick(i);
            }
        }
        if ctx
            .monsters(ctx.me)
            .iter()
            .any(|c| c.position.face_up && !ctx.view_data(c).is_tuner())
        {
            if let Some(i) = t.activate_from(SPORE, Location::Graveyard) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, GIGA | BUTTERFLY | CHEVALIER)
                if c.at().map(|a| a.location) == Some(Location::MonsterZone) =>
            {
                Some(if code == GIGA { 5000.0 } else { 2700.0 })
            }
            (ChoiceKind::NormalSummon, LONEFIRE) => Some(4000.0),
            (ChoiceKind::NormalSummon, GIGA) => Some(if ctx.in_hand(SUPERVISE) {
                3300.0
            } else {
                2200.0
            }),
            (ChoiceKind::NormalSummon, CHEVALIER | BUTTERFLY) => Some(
                support::body_score(self, &ctx, code)
                    + if ctx.in_hand(SUPERVISE) { 1000.0 } else { 0.0 },
            ),
            (ChoiceKind::NormalSummon, SPORE | BULB | COPY)
                if tactics::synchro_with_tuner(self, &ctx, 1).is_some() =>
            {
                Some(3800.0)
            }
            (ChoiceKind::NormalSummon, SPORE | BULB | COPY) => None,
            (ChoiceKind::SetMonster, POISON | DANDY) => Some(1700.0),
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let c = t.choice(i);
        Some(match t.ctx.canonical(c.code()?) {
            DANDY | POISON => Response::new(120.0),
            SUPERVISE if c.at().map(|a| a.location) == Some(Location::Graveyard) => {
                Response::new(120.0)
            }
            SPARK => self.spark(t),
            GIGA | BUTTERFLY | LONEFIRE | SUPERVISE | CHEVALIER | FOOLISH | SWING | SPORE
            | BULB | COPY => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if let Some(score) = support::spore_cost(self, t, m) {
            return Some(score);
        }
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let src = t.memory.last_activated.map(|k| ctx.canonical(k));
        if t.decision.hint == Hint::SpecialSummon {
            return Some(match code {
                GIGA => 5500.0,
                LONEFIRE => 4500.0,
                SPORE | BULB | COPY => support::body_score(self, &ctx, code),
                _ => value(self, &ctx, Some(code), None) as f64,
            });
        }
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(match code {
                GIGA if ctx.in_hand(SWING) || ctx.face_up_on_field(ctx.me, BUTTERFLY) => 5500.0,
                DANDY => 5000.0,
                BULB => 4000.0,
                SPORE => 3000.0,
                _ => 0.0,
            });
        }
        if t.decision.hint == Hint::Release && src == Some(LONEFIRE) {
            return Some(if code == LONEFIRE || ctx.data(code).attack == 0 {
                2000.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::ToGraveyard && src == Some(CHEVALIER) && code == SUPERVISE {
            return Some(5000.0);
        }
        if t.decision.hint == Hint::SynchroMaterial
            && code == GIGA
            && !ctx.face_up_on_field(ctx.me, SUPERVISE)
        {
            return Some(-6000.0);
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
}
