//! Eye of the Storm: keep Cloudians in Attack Position, prevent battle
//! damage with Sanctuary/Barrier, and spend Fog Counters on control effects.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Position};
pub const DECK: &str = "Eye of the Storm";
const TURB: u32 = 16197610;
const SMOKE: u32 = 80825553;
const ALTUS: u32 = 79703905;
const ACID: u32 = 17810268;
const CIRRO: u32 = 43318266;
const STORM: u32 = 13474291;
const NIMBUS: u32 = 20003527;
const SQUALL: u32 = 90135989;
const SANCTUARY: u32 = 56433456;
const BARRIER: u32 = 53239672;
const TERRAFORM: u32 = 73628505;
const GRIZZLY: u32 = 57839750;
#[derive(Clone, Default)]
pub struct Cloudian;
impl Strategy for Cloudian {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            TURB => 2300,
            ALTUS => 2500,
            ACID | CIRRO => 2200,
            STORM => 1800,
            NIMBUS => {
                ctx.monsters(ctx.me)
                    .iter()
                    .chain(ctx.monsters(ctx.opp).iter())
                    .map(|c| c.counters as i32 * 500)
                    .sum::<i32>()
                    + 1000
            }
            SQUALL => 2800,
            SANCTUARY | BARRIER => 2400,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !ctx.face_up_on_field(ctx.me, SANCTUARY) {
            if let Some(i) = t.activate(SANCTUARY) {
                return t.pick(i);
            }
            if !ctx.in_hand(SANCTUARY) {
                if let Some(i) = t.activate(TERRAFORM) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.activate(SQUALL) {
            return t.pick(i);
        }
        if !ctx.face_up_on_field(ctx.me, BARRIER) {
            if let Some(i) = t.activate(BARRIER) {
                return t.pick(i);
            }
        }
        for code in [CIRRO, ACID] {
            if let Some(i) = t.activate_from(code, Location::MonsterZone) {
                let pile = if code == CIRRO {
                    ctx.monsters(ctx.opp)
                } else {
                    ctx.spell_traps(ctx.opp)
                };
                if let Some(c) = pile
                    .into_iter()
                    .filter(|c| ctx.reaches(c, code, true, true))
                    .max_by_key(|c| ctx.threat(c))
                {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
        }
        if ctx.hand_size(ctx.opp) > 0 {
            if let Some(i) = t.activate_from(ALTUS, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate_from(STORM, Location::MonsterZone) {
            if let Some(c) = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| c.position.face_up && matches!(c.code, Some(ACID | CIRRO | TURB)))
                .max_by_key(|c| {
                    if ctx.is(c, CIRRO) && !ctx.monsters(ctx.opp).is_empty() {
                        4000 - c.counters as i32 * 300
                    } else if ctx.is(c, ACID) && !ctx.spell_traps(ctx.opp).is_empty() {
                        3500 - c.counters as i32 * 300
                    } else {
                        1000
                    }
                })
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if ctx.free_monster_zones(ctx.me) >= 2 || ctx.in_hand(NIMBUS) {
            if let Some(i) = t.activate_from(TURB, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, NIMBUS)
                if ctx.monsters(ctx.me).iter().any(|c| ctx.is(c, SMOKE)) =>
            {
                Some(4000.0)
            }
            (_, NIMBUS | STORM) => None,
            (ChoiceKind::NormalSummon, TURB) => Some(2800.0),
            (ChoiceKind::NormalSummon, CIRRO | ACID | ALTUS) => {
                Some(2200.0 + ctx.monsters(ctx.me).len() as f64 * 200.0)
            }
            (ChoiceKind::NormalSummon, SMOKE) if ctx.in_hand(NIMBUS) => Some(1000.0),
            (ChoiceKind::NormalSummon, SMOKE) => Some(500.0),
            (_, SMOKE) => None,
            (ChoiceKind::SetMonster, _) if ctx.data(code).in_set(0x18) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let c = t.choice(i);
        let code = t.ctx.canonical(c.code()?);
        Some(match code {
            TURB | ALTUS | ACID | CIRRO if c.description == (code as u64) << 20 => {
                Response::new(120.0)
            }
            SQUALL if c.at().map(|a| a.location) == Some(Location::SpellTrapZone) => {
                Response::new(120.0)
            }
            GRIZZLY => Response::new(120.0),
            BARRIER if !t.ctx.face_up_on_field(t.ctx.me, BARRIER) => Response::new(80.0),
            TURB | ALTUS | ACID | CIRRO | STORM | SQUALL | SANCTUARY | TERRAFORM | BARRIER => {
                Response::no()
            }
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if m.at.controller != t.ctx.me {
            return None;
        }
        let code = t.ctx.canonical(m.code?);
        if t.decision.hint == Hint::SpecialSummon {
            return Some(match code {
                TURB => 4000.0,
                CIRRO | ACID => 3000.0,
                ALTUS => 2500.0,
                _ => 1000.0,
            });
        }
        if matches!(t.decision.hint, Hint::Tribute | Hint::Release) {
            return Some(if code == SMOKE { 5000.0 } else { -3000.0 });
        }
        None
    }
    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        (t.ctx.data(code).in_set(0x18) && code != SMOKE).then_some(Position::FACE_UP_ATTACK)
    }
    fn allow_reposition(&self, t: &Turn, c: &CardView) -> bool {
        !t.ctx.view_data(c).in_set(0x18) || t.ctx.is(c, SMOKE) || !c.position.attack
    }
}
