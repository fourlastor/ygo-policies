//! Visitors Beneath: Xex/Yagan and repeated Flips support Worm tributes;
//! Future Fusion builds a diverse Worm Zero for revival and removal.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Visitors Beneath";
const XEX: u32 = 11722335;
const YAGAN: u32 = 47111934;
const CARTAROS: u32 = 51043243;
const KING: u32 = 10026986;
const QUEEN: u32 = 81254059;
const VICTORY: u32 = 2088870;
const APOC: u32 = 88650530;
const ZERO: u32 = 74506079;
const FUTURE: u32 = 77565204;
const POLY: u32 = 24094653;
const TAIYOU: u32 = 38699854;
const ECLIPSE: u32 = 35480699;
const OFFERING: u32 = 93217231;
const RYKO: u32 = 21502796;
#[derive(Clone, Default)]
pub struct Worm;
impl Strategy for Worm {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            XEX => 2400,
            YAGAN => 1800,
            CARTAROS => 2000,
            KING => 3000,
            QUEEN => 2500,
            VICTORY => 2600,
            ZERO => 3400,
            FUTURE => 2600,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = support::flip(t, &[21502796]) {
            return Some(i);
        }
        let ctx = t.ctx;
        if let Some(i) = t.find_where(|c| {
            c.kind == ChoiceKind::ChangePosition
                && t.view(c).map_or(false, |c| !c.position.face_up)
                && match c.code() {
                    Some(CARTAROS) => true,
                    Some(YAGAN | VICTORY) => !ctx.monsters(ctx.opp).is_empty(),
                    Some(APOC) => !ctx.spell_traps(ctx.opp).is_empty(),
                    _ => false,
                }
        }) {
            return t.pick(i);
        }
        if let Some(i) = t.activate_from(YAGAN, Location::Graveyard) {
            return t.pick(i);
        }
        if let Some(i) = t.activate(FUTURE) {
            return t.pick(i);
        }
        for (offset, needs_space) in [(2, false), (0, true)] {
            if !needs_space || ctx.free_monster_zones(ctx.me) > 0 {
                if let Some(i) = t.find_where(|c| {
                    c.kind == ChoiceKind::Activate
                        && c.code() == Some(ZERO)
                        && c.description == ((ZERO as u64) << 20) + offset
                }) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.find_where(|c| {
            c.kind == ChoiceKind::Activate
                && c.code() == Some(ZERO)
                && c.description == ((ZERO as u64) << 20) + 1
        }) {
            if let Some(c) = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| ctx.reaches(c, ZERO, true, false))
                .max_by_key(|c| ctx.threat(c))
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if let Some(i) = t.activate(KING) {
            if let Some(c) = support::target(&ctx, KING, true) {
                let cost = ctx
                    .monsters(ctx.me)
                    .into_iter()
                    .filter(|c| {
                        ctx.view_data(c).in_set(0x3e)
                            && !matches!(c.code, Some(KING | QUEEN | ZERO))
                    })
                    .min_by_key(|c| value(self, &ctx, c.code, Some(c)));
                if let Some(cost) = cost {
                    if ctx.threat(c) > value(self, &ctx, cost.code, Some(cost)) + 300 {
                        return t.pick_targeting(i, vec![cost.at, c.at]);
                    }
                }
            }
        }
        if let Some(i) = t.activate(QUEEN) {
            return t.pick(i);
        }
        if let Some(i) = t.activate(TAIYOU) {
            if let Some(c) = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| {
                    !c.position.face_up
                        && ctx.view_data(c).is(crate::cards::types::FLIP)
                        && (!ctx.is(c, APOC) || !ctx.spell_traps(ctx.opp).is_empty())
                })
                .max_by_key(|c| value(self, &ctx, c.code, Some(c)))
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        let names: std::collections::HashSet<_> = ctx
            .hand()
            .into_iter()
            .chain(ctx.monsters(ctx.me))
            .filter(|c| ctx.view_data(c).in_set(0x3e))
            .filter_map(|c| c.code)
            .collect();
        if names.len() >= 4 && !ctx.face_up_on_field(ctx.me, ZERO) {
            if let Some(i) = t.activate(POLY) {
                return t.pick(i);
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
            (ChoiceKind::NormalSummon, XEX) => Some(3500.0),
            (ChoiceKind::NormalSummon, KING | QUEEN)
                if ctx
                    .monsters(ctx.me)
                    .iter()
                    .any(|c| ctx.view_data(c).in_set(0x3e) && c.attack < 1800) =>
            {
                Some(3600.0)
            }
            (ChoiceKind::SetMonster, CARTAROS | YAGAN | APOC) => Some(2400.0),
            (_, KING | QUEEN | VICTORY | support::HONEST) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let c = t.choice(i);
        Some(match t.ctx.canonical(c.code()?) {
            XEX | YAGAN | CARTAROS | VICTORY => Response::new(130.0),
            APOC if !t.ctx.spell_traps(t.ctx.opp).is_empty() => Response::new(120.0),
            FUTURE if c.at().map(|a| a.location) == Some(Location::SpellTrapZone) => {
                Response::new(120.0)
            }
            ECLIPSE if t.ctx.incoming_attack().is_some() => Response::new(80.0),
            OFFERING
                if t.ctx.monsters(t.ctx.opp).len() + t.ctx.spell_traps(t.ctx.opp).len() >= 2 =>
            {
                Response::new(70.0)
            }
            ZERO | KING | QUEEN | FUTURE | POLY | TAIYOU | ECLIPSE | APOC | OFFERING => {
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
        let src = t.memory.last_activated.map(|k| ctx.canonical(k));
        if matches!(t.decision.hint, Hint::FusionMaterial | Hint::ToGraveyard)
            && matches!(src, Some(FUTURE | POLY))
        {
            let duplicate = t.decision.selected.iter().any(|a| {
                t.decision
                    .choices
                    .iter()
                    .flat_map(|c| c.members.iter())
                    .any(|m| m.at == *a && m.code == Some(code))
            });
            return Some(if duplicate || t.decision.selected.len() >= 6 {
                -5000.0
            } else {
                5000.0 - ctx.data(code).attack as f64 / 10.0
            });
        }
        if t.decision.hint == Hint::ToGraveyard && src == Some(XEX) {
            return Some(
                if code == YAGAN && ctx.count_in(ctx.me, Location::Graveyard, YAGAN) == 0 {
                    6000.0
                } else if code == VICTORY {
                    4000.0
                } else {
                    1000.0
                },
            );
        }
        if t.decision.hint == Hint::AddToHand {
            return Some(if code == XEX && !ctx.in_hand(XEX) {
                6000.0
            } else {
                value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(
                if code == VICTORY && src == Some(ZERO) && !ctx.monsters(ctx.opp).is_empty() {
                    6000.0
                } else if code == KING {
                    5000.0
                } else if code == CARTAROS {
                    4500.0
                } else {
                    value(self, &ctx, Some(code), None) as f64
                },
            );
        }
        None
    }
    fn attack_trick(&self, ctx: &Ctx, c: &CardView) -> i32 {
        support::honest_trick(ctx, c)
    }
    fn yes_no(&self, t: &Turn) -> Option<bool> {
        (t.decision.subject == Some(RYKO)).then(|| {
            !t.ctx.monsters(t.ctx.opp).is_empty() || !t.ctx.spell_traps(t.ctx.opp).is_empty()
        })
    }
}
