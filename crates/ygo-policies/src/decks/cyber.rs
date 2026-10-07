//! Power Surge: develop Cyber names before fusing, use Power Bond only
//! with a Battle Phase and an LP cushion, and finish with Overload/Limiter.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Power Surge";
const CYBER: u32 = 70095154;
const ZWEI: u32 = 5373478;
const PROTO: u32 = 26439287;
const PHOENIX: u32 = 3370104;
const VALLEY: u32 = 3657444;
const ANGEL: u32 = 95956346;
const TROOPER: u32 = 85087012;
const BOND: u32 = 37630732;
const OVERLOAD: u32 = 3659803;
const FUTURE: u32 = 77565204;
const POLY: u32 = 24094653;
const LIMITER: u32 = 23171610;
const TWIN: u32 = 74157028;
const END: u32 = 1546123;
const OVERDRAGON: u32 = 64599569;
#[derive(Clone, Default)]
pub struct Cyber;
impl Strategy for Cyber {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            TWIN => 4000,
            END => 3600,
            OVERDRAGON => 3100,
            CYBER => 2200,
            ZWEI => 1800,
            PROTO => 1400,
            PHOENIX => 1700,
            BOND | OVERLOAD => 2600,
            FUTURE => 2400,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(CYBER), Some(Location::Hand)) {
            return t.pick(i);
        }
        if let Some(i) = t.activate_from(ZWEI, Location::MonsterZone) {
            return t.pick(i);
        }
        if ctx.deck_size(ctx.me) > 6 {
            if let Some(i) = t.activate_from(TROOPER, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(FUTURE) {
            return t.pick(i);
        }
        if ctx.main1() && t.has(ChoiceKind::EnterBattle) {
            if ctx.my_lp() > 3000 {
                if let Some(i) = t.activate(BOND) {
                    return t.pick(i);
                }
            }
            // Overdragon sends the rest of our board away. Build it from the GY
            // only when it improves the attacking board, not over a live Twin/End.
            let n = ctx
                .graveyard(ctx.me)
                .iter()
                .filter(|c| ctx.view_data(c).race & races::MACHINE != 0)
                .count();
            if n >= 3 && ctx.field_strength(ctx.me) < (n as i32) * 800 {
                if let Some(i) = t.activate(OVERLOAD) {
                    return t.pick(i);
                }
            }
            if ctx.monsters(ctx.opp).is_empty() {
                let damage: i32 = ctx
                    .monsters(ctx.me)
                    .iter()
                    .filter(|c| ctx.can_attack(c))
                    .map(|c| {
                        let doubled = if ctx.view_data(c).race & races::MACHINE != 0 {
                            2 * c.attack
                        } else {
                            c.attack
                        };
                        doubled * if ctx.is(c, TWIN) { 2 } else { 1 }
                    })
                    .sum();
                if damage >= ctx.opp_lp() {
                    if let Some(i) = t.activate(LIMITER) {
                        return t.pick(i);
                    }
                }
            }
        }
        if !ctx.face_up_on_field(ctx.me, TWIN) && !ctx.face_up_on_field(ctx.me, END) {
            if let Some(i) = t.activate(POLY) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, ZWEI | PROTO) if ctx.in_hand(BOND) || ctx.in_hand(POLY) => {
                Some(3300.0)
            }
            (ChoiceKind::NormalSummon, TROOPER) => Some(2400.0),
            (ChoiceKind::NormalSummon, VALLEY) => Some(1200.0),
            (_, CYBER) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        let c = t.choice(i);
        Some(match ctx.canonical(c.code()?) {
            PHOENIX | ANGEL => Response::new(120.0),
            VALLEY
                if ctx.incoming_attack().is_some() && c.description == ((VALLEY as u64) << 20) =>
            {
                Response::new(85.0)
            }
            FUTURE if c.at().map(|a| a.location) == Some(Location::SpellTrapZone) => {
                Response::new(120.0)
            }
            BOND | OVERLOAD | POLY | ZWEI | FUTURE | VALLEY | LIMITER => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let code = ctx.canonical(m.code?);
        if m.at.controller != ctx.me {
            return None;
        }
        let src = t.memory.last_activated.map(|k| ctx.canonical(k));
        if m.at.location == Location::Extra
            && matches!(
                t.decision.hint,
                Hint::SpecialSummon | Hint::Confirm | Hint::None
            )
        {
            return Some(if src == Some(FUTURE) {
                if code == OVERDRAGON {
                    9000.0
                } else {
                    1000.0
                }
            } else {
                value(self, &ctx, Some(code), None) as f64
            });
        }
        if matches!(
            t.decision.hint,
            Hint::FusionMaterial | Hint::ToGraveyard | Hint::Banish
        ) {
            if src == Some(FUTURE) && m.at.location == Location::Deck {
                return Some(if t.decision.selected.len() < 5 {
                    if code == ZWEI || code == CYBER {
                        4000.0
                    } else {
                        2500.0
                    }
                } else {
                    -1000.0
                });
            }
            if src == Some(OVERLOAD) && m.at.location == Location::Graveyard {
                return Some(4000.0);
            }
            return Some(
                -value(self, &ctx, Some(code), None) as f64
                    - if ctx.data(code).is_extra() {
                        5000.0
                    } else {
                        0.0
                    },
            );
        }
        if t.decision.hint == Hint::SpecialSummon && src == Some(ANGEL) {
            return Some(if code == PROTO {
                3500.0
            } else if code == VALLEY {
                2500.0
            } else {
                1000.0
            });
        }
        None
    }
}
