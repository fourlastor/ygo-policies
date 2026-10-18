//! Volcanic Aftershock: Shell provides renewable ammunition, Scattershot
//! clears boards through Accelerator, and Doomfire/Rocket finish battles.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Volcanic Aftershock";
const ROCKET: u32 = 76459806;
const SHELL: u32 = 33365932;
const SCATTER: u32 = 69750546;
const COUNTER: u32 = 66436257;
const SLICER: u32 = 17415895;
const GUARDS: u32 = 54040221;
const DOOM: u32 = 32543380;
const MONK: u32 = 423585;
const BLAZE: u32 = 69537999;
const TRI: u32 = 21420702;
const FOOLISH: u32 = 81439173;
const BREAK: u32 = 4178474;
#[derive(Clone, Default)]
pub struct Volcanic;
impl Volcanic {
    fn removal(t: &Turn) -> Response {
        let ctx = t.ctx;
        let cheap = ctx.in_hand(SHELL) || (ctx.in_hand(COUNTER) && !ctx.monsters_banished());
        support::target(&ctx, BREAK, true)
            .filter(|c| ctx.threat(c) >= if cheap { 1200 } else { 2300 })
            .map(|c| Response::targeting(70.0, vec![c.at]))
            .unwrap_or_else(Response::no)
    }
}
impl Strategy for Volcanic {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            ROCKET => 2500,
            SHELL => 1600,
            SCATTER => 1900,
            DOOM => 3700,
            GUARDS => 2300,
            BLAZE => 2400,
            TRI => 1600,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(DOOM), Some(Location::Hand)) {
            return t.pick(i);
        }
        if ctx.my_lp() > 1200 {
            if let Some(i) = t.activate_from(SHELL, Location::Graveyard) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate_from(MONK, Location::MonsterZone) {
            return t.pick(i);
        }
        if ctx.count_in(ctx.me, Location::Graveyard, SHELL) == 0 {
            if let Some(i) = t.activate(FOOLISH) {
                return t.pick(i);
            }
        }
        if !ctx.face_up_on_field(ctx.me, BLAZE) {
            if let Some(i) = t.activate_from(BLAZE, Location::Hand) {
                return t.pick(i);
            }
        }
        if ctx.in_hand(DOOM) && !ctx.face_up_on_field(ctx.me, TRI) {
            if let Some(i) = t.activate(TRI) {
                return t.pick(i);
            }
        }
        // Accelerator forbids all attacks for the turn. Prefer it for a board
        // we cannot beat in battle, or a multi-monster Scattershot wipe.
        let ours = ctx
            .monsters(ctx.me)
            .iter()
            .map(|c| c.attack)
            .max()
            .unwrap_or(0);
        let should_fire = ctx.opp_best_attack() >= ours
            || ours == 0
            || (ctx.in_hand(SCATTER) && ctx.monsters(ctx.opp).len() >= 2);
        if should_fire {
            for code in [BLAZE, TRI] {
                if let Some(i) = t.activate_from(code, Location::SpellTrapZone) {
                    if let Some(c) = ctx
                        .monsters(ctx.opp)
                        .into_iter()
                        .filter(|c| ctx.reaches(c, code, true, true))
                        .max_by_key(|c| ctx.threat(c))
                    {
                        return t.pick_targeting(i, vec![c.at]);
                    }
                }
            }
        }
        if !ctx.main1() || ctx.opp_best_attack() >= 1800 {
            if let Some(i) = t.activate(SLICER) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(BREAK) {
            let r = Self::removal(t);
            if r.score > 0.0 {
                return t.pick_targeting(i, r.intent);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        Some(match (c.kind, ctx.canonical(c.code()?)) {
            (ChoiceKind::NormalSummon, ROCKET) => Some(3000.0),
            (ChoiceKind::NormalSummon, GUARDS)
                if ctx
                    .graveyard(ctx.me)
                    .iter()
                    .filter(|c| ctx.view_data(c).race & crate::cards::races::PYRO != 0)
                    .count()
                    >= 4 =>
            {
                Some(4000.0)
            }
            (ChoiceKind::NormalSummon, MONK) => Some(2600.0),
            (_, SHELL | SCATTER | COUNTER | DOOM) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.ctx.canonical(t.choice(i).code()?) {
            ROCKET | SCATTER | COUNTER | GUARDS | DOOM => Response::new(130.0),
            BREAK => Self::removal(t),
            SHELL | MONK | BLAZE | TRI | FOOLISH | SLICER => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn allow_repeated_chain(&self, t: &Turn, i: usize) -> bool {
        t.choice(i).code() == Some(SCATTER)
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let src = t.memory.last_activated.map(|k| ctx.canonical(k));
        if t.decision.hint == Hint::AddToHand && src == Some(ROCKET) {
            return Some(
                if code == BLAZE && !ctx.in_hand(BLAZE) && !ctx.face_up_on_field(ctx.me, BLAZE) {
                    6000.0
                } else if code == TRI && ctx.in_hand(DOOM) {
                    5000.0
                } else {
                    1000.0
                },
            );
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(if code == ROCKET {
                5500.0
            } else if code == DOOM {
                6500.0
            } else {
                ctx.data(code).attack as f64
            });
        }
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(if code == SHELL {
                5000.0
            } else if code == COUNTER {
                3500.0
            } else {
                0.0
            });
        }
        if t.decision.hint == Hint::ToGraveyard
            && matches!(src, Some(BLAZE | TRI))
            && m.at.location == Location::Hand
        {
            return Some(match code {
                SCATTER if ctx.monsters(ctx.opp).len() >= 2 => 7000.0,
                SHELL => 6000.0,
                SCATTER => 5000.0,
                _ => 0.0,
            });
        }
        if t.decision.hint == Hint::Discard {
            return Some(if code == SHELL {
                5000.0
            } else if code == COUNTER && !ctx.monsters_banished() {
                4000.0
            } else if code == SCATTER {
                // A discard is not an Accelerator wipe; preserve the trio.
                if ctx.opp_lp() <= 500 && !ctx.monsters_banished() {
                    6000.0
                } else {
                    -3500.0
                }
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::ToDeck && src == Some(GUARDS) {
            return Some(if code == SCATTER {
                5000.0
            } else if code == SHELL {
                4000.0
            } else {
                1000.0
            });
        }
        None
    }
}
