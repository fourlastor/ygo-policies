//! A Bitter Cure: resolve Nurse/Simochi before giving LP to the opponent.
//! Gift Card supplies burst damage; recruitment and shields protect Nurse.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Member};
pub const DECK: &str = "A Bitter Cure";
const NURSE: u32 = 67316075;
const SIMOCHI: u32 = 40633297;
const GIFT: u32 = 39526584;
const UPSTART: u32 = 70368879;
const RAIN: u32 = 66719324;
const SOUL: u32 = 81510157;
const PATHS: u32 = 50470982;
const EYE: u32 = 34694160;
const RECKLESS: u32 = 37576645;
const ANGEL: u32 = 95956346;
const CROW: u32 = 18964575;
const MARSH: u32 = 31305911;
#[derive(Clone, Default)]
pub struct Nurse;
impl Nurse {
    fn active(ctx: &Ctx) -> bool {
        (!ctx.effects_drained() && ctx.face_up_on_field(ctx.me, NURSE))
            || (ctx.face_up_on_field(ctx.me, SIMOCHI)
                && !ctx.obs.chain.iter().any(|l| l.code == SIMOCHI)
                && !ctx.face_up_on_field(ctx.me, 51452091)
                && !ctx.face_up_on_field(ctx.opp, 51452091))
    }
}
impl Strategy for Nurse {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            NURSE | SIMOCHI => 3500,
            GIFT => 3000,
            RAIN | UPSTART => 1700,
            SOUL => 2400,
            EYE => 1800,
            ANGEL => 2200,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !Self::active(&ctx) {
            if let Some(i) = t.activate(SIMOCHI) {
                return t.pick(i);
            }
        }
        if Self::active(&ctx) {
            for code in [UPSTART, RAIN, EYE] {
                if let Some(i) = t.activate(code) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.activate(SOUL) {
            if let Some(c) = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| c.position.face_up && ctx.reaches(c, SOUL, true, true))
                .max_by_key(|c| ctx.threat(c))
            {
                if Self::active(&ctx) || ctx.threat(c) > 2000 {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        Some(match (c.kind, ctx.canonical(c.code()?)) {
            (ChoiceKind::NormalSummon, NURSE) => {
                Some(if Self::active(&ctx) { 2200.0 } else { 5000.0 })
            }
            (ChoiceKind::NormalSummon, ANGEL) => Some(2300.0),
            (ChoiceKind::SetMonster, MARSH) => Some(1900.0),
            (_, CROW) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        Some(match ctx.canonical(t.choice(i).code()?) {
            SIMOCHI if !Self::active(&ctx) => Response::new(125.0),
            GIFT if Self::active(&ctx) => Response::new(120.0),
            PATHS if Self::active(&ctx) && ctx.my_lp() > 2000 => Response::new(115.0),
            EYE if Self::active(&ctx) => Response::new(110.0),
            RECKLESS if ctx.deck_size(ctx.me) > 2 => Response::new(120.0),
            ANGEL => Response::new(120.0),
            SIMOCHI | GIFT | PATHS | EYE | RECKLESS | UPSTART | RAIN | SOUL => Response::no(),
            _ => return support::stall_chain(t, i),
        })
    }
    fn allow_repeated_chain(&self, t: &Turn, i: usize) -> bool {
        matches!(t.choice(i).code(), Some(GIFT | PATHS | RECKLESS))
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if m.at.controller == t.ctx.me
            && t.decision.hint == Hint::SpecialSummon
            && m.code == Some(NURSE)
        {
            return Some(6000.0);
        }
        None
    }
    fn allow_staple(&self, t: &Turn, code: u32) -> bool {
        code != 42703248 || !t.ctx.face_up_on_field(t.ctx.me, SIMOCHI)
    }
    fn wants_battle(&self, t: &Turn) -> Option<bool> {
        Some(
            !t.ctx
                .monsters(t.ctx.me)
                .iter()
                .all(|c| t.ctx.is(c, NURSE) || t.ctx.is(c, MARSH)),
        )
    }
}
