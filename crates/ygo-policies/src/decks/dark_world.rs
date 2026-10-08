//! Gates Unopened: effect discards summon the early Dark World bosses.
//! Costs and sending are deliberately not treated as Dark World triggers.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Member};
pub const DECK: &str = "Gates Unopened";
const BROWW: u32 = 79126789;
const GOLDD: u32 = 78004197;
const SILLVA: u32 = 32619583;
const BEIIGE: u32 = 33731070;
const BRRON: u32 = 6214884;
const RAVEN: u32 = 47217354;
const DEALINGS: u32 = 74117290;
const LIGHTNING: u32 = 93554166;
const DRAGGED: u32 = 16435215;
const DESTRUCTION: u32 = 72892473;
const JAR: u32 = 33508719;
#[derive(Clone, Default)]
pub struct DarkWorld;
impl DarkWorld {
    fn fuel(ctx: &Ctx) -> bool {
        !ctx.monsters_banished()
            && ctx
                .hand()
                .iter()
                .any(|c| matches!(c.code, Some(BROWW | GOLDD | SILLVA | BEIIGE)))
    }
}
impl Strategy for DarkWorld {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            BROWW => 2000,
            GOLDD | SILLVA => 2500,
            BEIIGE => 1500,
            BRRON => 2000,
            RAVEN => 2300,
            DEALINGS | DESTRUCTION => 2400,
            LIGHTNING => 2200,
            DRAGGED => 1800,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if ctx.deck_size(ctx.me) >= 5 {
            let can_flip_jar = t.choices().any(|(_, c)| {
                c.kind == ChoiceKind::ChangePosition
                    && c.code() == Some(JAR)
                    && t.view(c).map_or(false, |v| !v.position.face_up)
            });
            if can_flip_jar {
                if let Some(i) = t.find_where(|c| c.kind == ChoiceKind::SetSpellTrap) {
                    return t.pick(i);
                }
            }
            if let Some(i) = t.find_where(|c| {
                c.kind == ChoiceKind::ChangePosition
                    && c.code() == Some(JAR)
                    && t.view(c).map_or(false, |c| !c.position.face_up)
            }) {
                return t.pick(i);
            }
        }
        if Self::fuel(&ctx) {
            if let Some(i) = t.activate(LIGHTNING) {
                if let Some(c) = ctx
                    .monsters(ctx.opp)
                    .into_iter()
                    .chain(ctx.spell_traps(ctx.opp))
                    .filter(|c| !c.position.face_up && ctx.reaches(c, LIGHTNING, true, true))
                    .max_by_key(|c| ctx.threat(c))
                {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
            for code in [RAVEN, DESTRUCTION, DEALINGS] {
                if let Some(i) = t.activate(code) {
                    return t.pick(i);
                }
            }
            // Before revealing our hand, set useful backrow so the opponent is
            // left choosing among monsters that benefit from an effect discard.
            if ctx.hand().iter().all(|c| {
                c.code.map_or(false, |k| {
                    matches!(k, BROWW | GOLDD | SILLVA | BEIIGE | DRAGGED)
                })
            }) {
                if let Some(i) = t.activate(DRAGGED) {
                    return t.pick(i);
                }
            }
        }
        None
    }
    fn main_phase_late(&mut self, t: &mut Turn) -> Option<usize> {
        if Self::fuel(&t.ctx) && t.ctx.in_hand(DRAGGED) {
            if let Some(i) =
                t.find_where(|c| c.kind == ChoiceKind::SetSpellTrap && c.code() != Some(DRAGGED))
            {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        Some(match (c.kind, c.code()?) {
            (ChoiceKind::NormalSummon, RAVEN) if Self::fuel(&t.ctx) => Some(3800.0),
            (ChoiceKind::NormalSummon, BROWW) if t.ctx.monsters(t.ctx.opp).is_empty() => {
                Some(1400.0)
            }
            (ChoiceKind::SetMonster, BROWW) if t.ctx.monsters(t.ctx.me).is_empty() => Some(800.0),
            (_, GOLDD | SILLVA | BROWW) => None,
            (ChoiceKind::SetMonster, JAR) => Some(2400.0),
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.ctx.canonical(t.choice(i).code()?) {
            BROWW | GOLDD | SILLVA | BEIIGE | JAR => Response::new(130.0),
            BRRON if Self::fuel(&t.ctx) => Response::new(120.0),
            BRRON | RAVEN | DEALINGS | LIGHTNING | DRAGGED | DESTRUCTION => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn allow_repeated_chain(&self, t: &Turn, i: usize) -> bool {
        matches!(t.choice(i).code(), Some(BROWW | GOLDD | SILLVA | BEIIGE))
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let src = t.memory.last_activated.map(|k| ctx.canonical(k));
        if t.decision.hint == Hint::Discard
            && !ctx.monsters_banished()
            && matches!(
                src,
                Some(DEALINGS | LIGHTNING | DRAGGED | DESTRUCTION | RAVEN | BRRON | JAR)
            )
        {
            let rank = match code {
                GOLDD | SILLVA if ctx.free_monster_zones(ctx.me) > 0 => 6000.0,
                BROWW => 5000.0,
                BEIIGE if ctx.free_monster_zones(ctx.me) > 0 => 3500.0,
                _ => -value(self, &ctx, Some(code), None) as f64,
            };
            // One Raven discard makes a Level-3 Tuner beside a Level-5 boss.
            return Some(if src == Some(RAVEN) && !t.decision.selected.is_empty() {
                -1000.0
            } else {
                rank
            });
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
}
