//! Visitors from Beyond: Ammonite revives a Level 4 Alien for Gol'gar;
//! reusable Swords/Ruins supply counters for removal and further revival.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Visitors from Beyond";
const AMMONITE: u32 = 652362;
const DOG: u32 = 15475415;
const WARRIOR: u32 = 98719226;
const TELEPATH: u32 = 91070115;
const GREY: u32 = 62437709;
const OVERLORD: u32 = 63253763;
const GOLGAR: u32 = 68319538;
const RUINS: u32 = 99342953;
const SWORDS: u32 = 72302403;
const BURDEN: u32 = 44947065;
const FOOLISH: u32 = 81439173;
const REVERSE: u32 = 27551;
#[derive(Clone, Default)]
pub struct Alien;
impl Alien {
    fn level4(ctx: &Ctx) -> bool {
        ctx.graveyard(ctx.me)
            .iter()
            .any(|c| ctx.view_data(c).in_set(0xc) && ctx.view_data(c).level == 4)
    }
}
impl Strategy for Alien {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            GOLGAR => 3600,
            AMMONITE => 2400,
            DOG => 1600,
            OVERLORD => 2400,
            RUINS => 2300,
            WARRIOR => 1900,
            SWORDS => 2400,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = support::flip(t, &[62437709]) {
            return Some(i);
        }
        let ctx = t.ctx;
        if let Some(i) = t.activate(RUINS) {
            return t.pick(i);
        }
        if !ctx.effects_drained() {
            if let Some(i) = t.find_where(|c| {
                c.kind == ChoiceKind::Activate
                    && c.code() == Some(GOLGAR)
                    && c.description == ((GOLGAR as u64) << 20) + 1
            }) {
                if let Some(c) = support::target(&ctx, GOLGAR, true) {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
            if let Some(i) = t.find_where(|c| {
                c.kind == ChoiceKind::Activate
                    && c.code() == Some(GOLGAR)
                    && c.description == (GOLGAR as u64) << 20
            }) {
                let targets: Vec<_> = ctx
                    .spell_traps(ctx.opp)
                    .into_iter()
                    .filter(|c| c.position.face_up && ctx.reaches(c, GOLGAR, true, false))
                    .map(|c| c.at)
                    .chain(
                        ctx.spell_traps(ctx.me)
                            .into_iter()
                            .filter(|c| {
                                c.position.face_up
                                    && (ctx.is(c, SWORDS) || (ctx.is(c, RUINS) && c.counters == 0))
                            })
                            .map(|c| c.at),
                    )
                    .collect();
                if !targets.is_empty() {
                    return t.pick_targeting(i, targets);
                }
            }
            if !ctx.monsters(ctx.opp).is_empty() {
                if let Some(i) = t.activate(OVERLORD) {
                    return t.pick(i);
                }
            }
            if let Some(i) = t.activate(TELEPATH) {
                if let Some(c) = ctx
                    .spell_traps(ctx.opp)
                    .into_iter()
                    .filter(|c| ctx.reaches(c, TELEPATH, true, true))
                    .max_by_key(|c| ctx.threat(c))
                {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
        }
        if !ctx.face_up_on_field(ctx.me, BURDEN) {
            if let Some(i) = t.activate(BURDEN) {
                return t.pick(i);
            }
        }
        if !Self::level4(&ctx) {
            if let Some(i) = t.activate(FOOLISH) {
                return t.pick(i);
            }
        }
        if ctx.graveyard(ctx.me).iter().any(|c| ctx.is(c, AMMONITE))
            && ctx
                .monsters(ctx.me)
                .iter()
                .any(|c| ctx.view_data(c).in_set(0xc) && c.level == 4)
        {
            if let Some(i) = t.activate(REVERSE) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, AMMONITE) if Self::level4(&ctx) => Some(5000.0),
            (ChoiceKind::NormalSummon, AMMONITE) => Some(support::body_score(self, &ctx, code)),
            (ChoiceKind::SetMonster, GREY) => Some(1700.0),
            (_, OVERLORD) => None,
            (ChoiceKind::NormalSummon, _) if ctx.data(code).in_set(0xc) => Some(
                support::body_score(self, &ctx, code) + if ctx.in_hand(DOG) { 800.0 } else { 0.0 },
            ),
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.ctx.canonical(t.choice(i).code()?) {
            AMMONITE | DOG | WARRIOR | GREY => Response::new(120.0),
            GOLGAR | RUINS | BURDEN | FOOLISH | TELEPATH | OVERLORD | REVERSE => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let code = ctx.canonical(m.code?);
        let src = t.memory.last_activated.map(|k| ctx.canonical(k));
        if src == Some(GOLGAR) && t.decision.hint == Hint::ReturnToHand && m.at.controller == ctx.me
        {
            return Some(if code == SWORDS || code == RUINS {
                3000.0
            } else {
                -3000.0
            });
        }
        if m.at.controller != ctx.me {
            return None;
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(support::body_score(self, &ctx, code));
        }
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(if ctx.data(code).in_set(0xc) && ctx.data(code).level == 4 {
                5000.0
            } else {
                0.0
            });
        }
        if t.decision.hint == Hint::SynchroMaterial && code == GOLGAR {
            return Some(-9000.0);
        }
        if src == Some(GOLGAR) && matches!(t.decision.hint, Hint::Other(_)) {
            return Some(-1000.0);
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        if c.code() == Some(GOLGAR) {
            Some(true)
        } else {
            support::extra_allowed(t, c)
        }
    }
}
