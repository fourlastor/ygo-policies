//! Herald's Veto: assemble Herald with Dawn/Advanced Ritual Art, retain
//! Fairy ammunition, recover it with Dark Factory, and protect with Decree.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Herald's Veto";
const HERALD: u32 = 44665365;
const DAWN: u32 = 27383110;
const ART: u32 = 46052429;
const MANJU: u32 = 95492061;
const SENJU: u32 = 23401839;
const PREPARATION: u32 = 96729612;
const ORANGE: u32 = 17266660;
const KRISTYA: u32 = 59509952;
const SKELENGEL: u32 = 60694662;
const FACTORY: u32 = 90928333;
const BECKONING: u32 = 16255442;
const DECREE: u32 = 51452091;
#[derive(Clone, Default)]
pub struct Herald;
impl Herald {
    fn established(ctx: &Ctx) -> bool {
        !ctx.effects_drained() && ctx.face_up_on_field(ctx.me, HERALD)
    }
    fn fairy_fuel(ctx: &Ctx) -> usize {
        ctx.hand()
            .iter()
            .filter(|c| ctx.view_data(c).race & races::FAIRY != 0)
            .count()
    }
    fn beckon(ctx: &Ctx) -> bool {
        let fairies = ctx
            .graveyard(ctx.me)
            .iter()
            .filter(|c| ctx.view_data(c).race & races::FAIRY != 0)
            .count();
        Self::established(ctx)
            && ctx.hand_size(ctx.me) >= 2
            && fairies >= ctx.hand_size(ctx.me) as usize
            && Self::fairy_fuel(ctx) + 1 < ctx.hand_size(ctx.me) as usize
    }
}
impl Strategy for Herald {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            HERALD => {
                if Self::established(ctx) {
                    1200
                } else {
                    3500
                }
            }
            DAWN | ART => {
                if Self::established(ctx) {
                    600
                } else {
                    3000
                }
            }
            MANJU => 2500,
            SENJU | PREPARATION => 2200,
            KRISTYA => 2900,
            ORANGE | support::HONEST => 1900,
            FACTORY => {
                if Self::established(ctx) {
                    2400
                } else {
                    1100
                }
            }
            DECREE => 2300,
            BECKONING => 1500,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        for code in [PREPARATION, FACTORY] {
            if let Some(i) = t.activate(code) {
                return t.pick(i);
            }
        }
        if !ctx.face_up_on_field(ctx.me, HERALD) {
            for code in [ART, DAWN] {
                if let Some(i) = t.activate_from(code, Location::Hand) {
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
            (ChoiceKind::NormalSummon, MANJU) => Some(3200.0),
            (ChoiceKind::NormalSummon, SENJU) => {
                Some(if ctx.in_hand(HERALD) { 1600.0 } else { 2900.0 })
            }
            (ChoiceKind::SetMonster, SKELENGEL) if !Self::established(&ctx) => Some(1800.0),
            (_, HERALD | KRISTYA | ORANGE | support::HONEST) => None,
            (ChoiceKind::NormalSummon, _)
                if ctx.data(code).attack >= 1700
                    && (!Self::established(&ctx) || Self::fairy_fuel(&ctx) >= 3) =>
            {
                Some(1700.0)
            }
            _ => None,
        })
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        let code = t.ctx.canonical(c.code()?);
        if code == KRISTYA {
            return Some(Self::established(&t.ctx) || !t.ctx.in_hand(HERALD));
        }
        if c.at().map(|a| a.location) == Some(Location::Extra) && Self::established(&t.ctx) {
            return Some(false);
        }
        support::extra_allowed(t, c)
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let code = t.ctx.canonical(t.choice(i).code()?);
        Some(match code {
            HERALD if t.hostile_top().matches(|_| true) => Response::new(98.0),
            ORANGE if t.hostile_top().matches(|l| t.ctx.data(l.code).is_monster()) => {
                Response::new(85.0)
            }
            MANJU | SENJU | KRISTYA | SKELENGEL => Response::new(120.0),
            DAWN if t.choice(i).at().map(|a| a.location) == Some(Location::Graveyard) => {
                Response::new(120.0)
            }
            DECREE
                if !t.ctx.face_up_on_field(t.ctx.me, DECREE)
                    && (t.hostile_top().matches(|l| t.ctx.data(l.code).is_trap())
                        || !t.ctx.my_turn()) =>
            {
                Response::new(70.0)
            }
            BECKONING if Self::beckon(&t.ctx) => Response::new(40.0),
            HERALD | ORANGE | DAWN | ART | PREPARATION | FACTORY | BECKONING | DECREE => {
                Response::no()
            }
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
        if matches!(t.decision.hint, Hint::AddToHand | Hint::ReturnToHand) {
            if source == Some(DAWN) || source == Some(BECKONING) {
                return Some(if code == MANJU {
                    3500.0
                } else if code == ORANGE {
                    3000.0
                } else {
                    ctx.data(code).attack as f64
                });
            }
            return Some(match code {
                HERALD if !ctx.in_hand(HERALD) && !Self::established(&ctx) => 6000.0,
                DAWN | ART
                    if !ctx.in_hand(DAWN) && !ctx.in_hand(ART) && !Self::established(&ctx) =>
                {
                    5500.0
                }
                _ => value(self, &ctx, Some(code), None) as f64,
            });
        }
        if matches!(
            t.decision.hint,
            Hint::Release | Hint::Tribute | Hint::ToGraveyard
        ) {
            let protect = code == HERALD && m.at.location == Location::MonsterZone;
            return Some(if protect {
                -20000.0
            } else if m.at.location == Location::MonsterZone {
                1000.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn attack_trick(&self, ctx: &Ctx, c: &CardView) -> i32 {
        support::honest_trick(ctx, c)
    }
}
