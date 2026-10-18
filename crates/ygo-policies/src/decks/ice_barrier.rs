//! Winter Parliament: distinct names for Triangle, Gantala recovery and WATER Synchros.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member, Position};
pub const DECK: &str = "Winter Parliament";
const MEDAL: u32 = 84206435;
const TRIANGLE: u32 = 64990807;
const STRATEGIST: u32 = 50032342;
const GANTALA: u32 = 53921056;
const DEFENDER: u32 = 82498947;
const PRINCESS: u32 = 59546528;
const SPELLBREAKER: u32 = 73061465;
#[derive(Clone, Default)]
pub struct IceBarrier;
impl Strategy for IceBarrier {
    fn value(&self, _: &Ctx, k: u32) -> Option<i32> {
        Some(match k {
            GANTALA => 3500,
            9056100 => 3000,
            81275309 => 2600,
            DEFENDER => 1900,
            65749035 => 3100,
            _ => return support::extra_value(k),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = t.activate(96947648) {
            return t.pick(i);
        }
        if let Some(i) = t.activate(MEDAL) {
            return t.pick(i);
        }
        if let (Some(i), Some(c)) = (
            t.activate(TRIANGLE),
            support::target(&t.ctx, TRIANGLE, true),
        ) {
            return t.pick_targeting(i, vec![c.at]);
        }
        if let Some(i) = t.activate(PRINCESS) {
            if !t.ctx.set_backrow(t.ctx.opp).is_empty() {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(STRATEGIST) {
            return t.pick(i);
        }
        if let Some(i) = t.activate(65749035) {
            if support::target(&t.ctx, 65749035, true).is_some() {
                return t.pick(i);
            }
        }
        None
    }
    fn main_phase_late(&mut self, t: &mut Turn) -> Option<usize> {
        if !t.ctx.in_hand(TRIANGLE) && t.ctx.monsters(t.ctx.me).iter().any(|c| c.attack >= 2200) {
            if let Some(i) = t.activate(SPELLBREAKER) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let k = c.code()?;
        if c.kind == ChoiceKind::NormalSummon && t.ctx.data(k).in_set(0x2f) {
            return Some(Some(match k {
                STRATEGIST => 2700.0,
                GANTALA => 3500.0,
                _ => support::body_score(self, &t.ctx, k),
            }));
        }
        None
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.choice(i).code()? {
            GANTALA | 60161788 | 41090784 => Response::new(130.0),
            MEDAL | TRIANGLE | STRATEGIST | PRINCESS | SPELLBREAKER | 65749035 | 70583986 => {
                Response::no()
            }
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if m.at.controller != t.ctx.me {
            return None;
        }
        let ctx = t.ctx;
        let k = m.code?;
        if t.decision.hint == Hint::AddToHand
            || (t.decision.hint == Hint::ReturnToHand && t.memory.last_activated == Some(96947648))
        {
            let triangle = ctx.in_hand(TRIANGLE);
            return Some(if triangle && !ctx.in_hand(k) {
                7000.0 + ctx.data(k).attack as f64
            } else if k == STRATEGIST && !ctx.in_hand(k) {
                6000.0
            } else {
                value(self, &ctx, Some(k), None) as f64
            });
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(match k {
                GANTALA => 7000.0,
                9056100 => 6000.0,
                DEFENDER if ctx.monsters(ctx.me).len() > 0 && ctx.opp_best_attack() >= 1600 => {
                    4500.0
                }
                _ => support::body_score(self, &ctx, k),
            });
        }
        if matches!(t.decision.hint, Hint::ToGraveyard | Hint::Discard)
            && m.at.location == Location::Hand
        {
            let copies = ctx.hand().iter().filter(|c| c.code == Some(k)).count();
            return Some(if copies > 1 {
                4000.0
            } else if ctx.data(k).level >= 5 && !ctx.in_hand(TRIANGLE) {
                3000.0
            } else {
                -(value(self, &ctx, Some(k), None) as f64)
            });
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn position(&self, _: &Turn, k: u32) -> Option<Position> {
        (k == DEFENDER || k == 41090784).then_some(Position::FACE_UP_DEFENSE)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
}
