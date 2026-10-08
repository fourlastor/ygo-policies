//! Full Charge: Micro-Cell and Charger create the first batteries, Fuel
//! Cell supplies the third, and Short Circuit clears the way for an attack.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Position};
pub const DECK: &str = "Full Charge";
const AA: u32 = 63142001;
const CHARGER: u32 = 83446909;
const MICRO: u32 = 56839613;
const FUEL: u32 = 74730899;
const INDUSTRIAL: u32 = 19441018;
const CIRCUIT: u32 = 75967082;
const QUICK: u32 = 49479374;
const REVIVE: u32 = 61181383;
const PACK: u32 = 61840587;
const INFERNO: u32 = 12247206;
const ANGEL: u32 = 95956346;
#[derive(Clone, Default)]
pub struct Batteryman;
impl Strategy for Batteryman {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            CHARGER => 2600,
            FUEL => 2400,
            INDUSTRIAL => 3200,
            MICRO => 2100,
            AA => 1700,
            CIRCUIT => 2800,
            PACK | REVIVE => 2300,
            QUICK => 1700,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = support::flip(t, &[56839613]) {
            return Some(i);
        }
        let ctx = t.ctx;
        for code in [FUEL, INDUSTRIAL] {
            if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(code), Some(Location::Hand)) {
                return t.pick(i);
            }
        }
        if !ctx.monsters(ctx.opp).is_empty() || !ctx.spell_traps(ctx.opp).is_empty() {
            if let Some(i) = t.activate(CIRCUIT) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(INDUSTRIAL) {
            let mon = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| ctx.reaches(c, INDUSTRIAL, true, true))
                .max_by_key(|c| ctx.threat(c));
            let spell = ctx
                .spell_traps(ctx.opp)
                .into_iter()
                .filter(|c| ctx.reaches(c, INDUSTRIAL, true, true))
                .max_by_key(|c| ctx.threat(c));
            if let (Some(a), Some(b)) = (mon, spell) {
                return t.pick_targeting(i, vec![a.at, b.at]);
            }
        }
        if ctx.my_lp() > 1200 {
            if let Some(i) = t.activate(REVIVE) {
                return t.pick(i);
            }
        }
        if ctx.free_monster_zones(ctx.me) >= 2 {
            if let Some(i) = t.activate(PACK) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(QUICK) {
            return t.pick(i);
        }
        if let Some(i) = t.activate(FUEL) {
            if let Some(target) = support::target(&ctx, FUEL, false) {
                let cost = ctx
                    .monsters(ctx.me)
                    .into_iter()
                    .filter(|c| !ctx.is(c, FUEL) && ctx.view_data(c).in_set(0x28))
                    .min_by_key(|c| value(self, &ctx, c.code, Some(c)));
                if let Some(cost) = cost {
                    if ctx.threat(target) > value(self, &ctx, cost.code, Some(cost)) + 300 {
                        return t.pick_targeting(i, vec![cost.at, target.at]);
                    }
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
        Some(match (c.kind, ctx.canonical(c.code()?)) {
            (ChoiceKind::NormalSummon, CHARGER) => Some(4500.0),
            (ChoiceKind::SetMonster, MICRO) => Some(3000.0),
            (ChoiceKind::NormalSummon, AA)
                if ctx.face_up_on_field(ctx.me, AA) || ctx.in_hand(FUEL) =>
            {
                Some(2600.0)
            }
            (ChoiceKind::NormalSummon, AA) => Some(800.0),
            (_, FUEL | INDUSTRIAL | support::HONEST) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        Some(match ctx.canonical(t.choice(i).code()?) {
            MICRO | CHARGER | ANGEL => Response::new(130.0),
            INFERNO => Response::new(120.0),
            PACK if !ctx.my_turn() && ctx.monsters(ctx.me).len() < 2 => Response::new(70.0),
            FUEL | INDUSTRIAL | CIRCUIT | QUICK | REVIVE | PACK => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        if t.decision.hint == Hint::SpecialSummon {
            return Some(match code {
                AA if ctx.in_hand(INFERNO) || ctx.face_up_on_field(ctx.me, AA) => 6000.0,
                FUEL => 5000.0,
                CHARGER => 4000.0,
                AA => 3500.0,
                MICRO => 2500.0,
                _ => 0.0,
            });
        }
        if t.decision.hint == Hint::Banish {
            return Some(if m.at.location != Location::Graveyard {
                -10000.0
            } else if code == AA && ctx.in_hand(REVIVE) {
                -2000.0
            } else {
                1000.0
            });
        }
        None
    }
    fn position(&self, _: &Turn, code: u32) -> Option<Position> {
        (code == AA).then_some(Position::FACE_UP_ATTACK)
    }
    fn attack_trick(&self, ctx: &Ctx, c: &CardView) -> i32 {
        support::honest_trick(ctx, c)
    }
}
