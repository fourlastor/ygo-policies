//! Clockwork Current: Undine setup, reusable Normal Summons and Machine Synchros.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Clockwork Current";
const UNDINE: u32 = 4904812;
const CONTROLLER: u32 = 68505803;
const BIRD: u32 = 64034255;
const FROG: u32 = 12538374;
const CAIUS: u32 = 9748752;
const NEUTRON: u32 = 19182751;
const CRUSHER: u32 = 27827903;
const DURADARK: u32 = 68450517;
#[derive(Clone, Default)]
pub struct Genex;
impl Strategy for Genex {
    fn value(&self, _: &Ctx, k: u32) -> Option<i32> {
        Some(match k {
            UNDINE => 2200,
            NEUTRON => 2200,
            BIRD => 2200,
            38354937 => 3600,
            52709508 | 66165755 => 2900,
            _ => return support::extra_value(k),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.activate(DURADARK) {
            if let Some(c) = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| {
                    c.position.face_up
                        && c.position.attack
                        && ctx.view_data(c).attribute & crate::cards::attributes::DARK != 0
                        && ctx.reaches(c, DURADARK, true, true)
                })
                .max_by_key(|c| ctx.threat(c))
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if let Some(i) = t.activate(BIRD) {
            let bodies = ctx.monsters(ctx.me);
            let bounce = bodies
                .iter()
                .filter(|c| {
                    c.position.face_up
                        && [UNDINE, NEUTRON, 30399511, FROG].contains(&c.code.unwrap_or(0))
                })
                .min_by_key(|c| c.attack);
            if let Some(c) = bounce {
                if bodies.len() >= 2 || (!ctx.obs.summon_used && c.level <= 4) {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
        }
        for code in [52709508, 66165755, 17760003] {
            if let Some(i) = t.activate(code) {
                return t.pick(i);
            }
        }
        if ctx.graveyard(ctx.me).iter().all(|c| c.code != Some(FROG)) {
            if let Some(i) = t.activate(81439173) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let code = c.code()?;
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, CAIUS) => {
                support::target(&t.ctx, CAIUS, false).map(|_| 3500.0)
            }
            (ChoiceKind::NormalSummon, UNDINE) => Some(2700.0),
            (ChoiceKind::NormalSummon, NEUTRON) => Some(2400.0),
            (ChoiceKind::NormalSummon, CONTROLLER | BIRD) => {
                Some(support::body_score(self, &t.ctx, code))
            }
            (_, FROG) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.choice(i).code()? {
            UNDINE | NEUTRON | FROG | 30399511 | 67483216 | 38354937 | CRUSHER => {
                Response::new(130.0)
            }
            CAIUS => support::target(&t.ctx, CAIUS, false)
                .map(|c| Response::targeting(120.0, vec![c.at]))
                .unwrap_or_else(Response::no),
            BIRD | DURADARK | 52709508 | 66165755 | 17760003 => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if m.at.controller != t.ctx.me {
            return None;
        }
        let code = m.code?;
        let src = t.memory.last_activated;
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(
                if code == FROG
                    && !t
                        .ctx
                        .graveyard(t.ctx.me)
                        .iter()
                        .any(|c| c.code == Some(FROG))
                {
                    8000.0
                } else {
                    0.0
                },
            );
        }
        if t.decision.hint == Hint::AddToHand {
            return Some(match code {
                BIRD if !t.ctx.in_hand(BIRD) => 6000.0,
                CONTROLLER if !t.ctx.in_hand(CONTROLLER) => 5000.0,
                _ => value(self, &t.ctx, Some(code), None) as f64,
            });
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(
                support::body_score(self, &t.ctx, code)
                    + if src == Some(66165755) {
                        t.ctx.data(code).attack as f64
                    } else {
                        0.0
                    },
            );
        }
        if t.decision.hint == Hint::Discard && code == FROG {
            return Some(5000.0);
        }
        support::material_score(&t.ctx, m, t.decision.hint)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
}
