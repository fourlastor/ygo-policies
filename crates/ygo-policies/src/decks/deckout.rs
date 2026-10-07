//! Last Page: mill with Needle Worm and Jar #2, reset Flips with Books,
//! and defend the setup while avoiding drawing our own empty deck.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::cards::types;
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Member, Position};
pub const DECK: &str = "Last Page";
const JAR: u32 = 33508719;
const JAR2: u32 = 79106360;
const WORM: u32 = 81843628;
const TAIYOU: u32 = 38699854;
const ECLIPSE: u32 = 35480699;
const MOON: u32 = 14087893;
const SHALLOW: u32 = 43434803;
const UPSTART: u32 = 70368879;
const DESTRUCTION: u32 = 72892473;
const HAND: u32 = 74519184;
const VALLEY: u32 = 3657444;
const CROW: u32 = 18964575;
#[derive(Clone, Default)]
pub struct Deckout;
impl Strategy for Deckout {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            WORM => 3200,
            JAR2 => 2800,
            JAR => 2600,
            TAIYOU | ECLIPSE => 2300,
            SHALLOW => 2200,
            VALLEY | CROW => 1800,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Store spare spells/traps before Jar discards the hand. Keep Taiyou
        // available to turn the freshly set Jar over this turn.
        let jar = ctx
            .monsters(ctx.me)
            .iter()
            .any(|c| ctx.is(c, JAR) && !c.position.face_up);
        if jar && ctx.hand_size(ctx.me) > 2 {
            if let Some(i) = t.find_where(|c| {
                c.kind == ChoiceKind::SetSpellTrap
                    && c.code().map_or(false, |k| ctx.canonical(k) != TAIYOU)
            }) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(TAIYOU) {
            if let Some(c) = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| {
                    !c.position.face_up
                        && matches!(c.code.map(|k| ctx.canonical(k)), Some(WORM | JAR2 | JAR))
                })
                .max_by_key(|c| {
                    if ctx.is(c, WORM) {
                        3
                    } else if ctx.is(c, JAR2) {
                        2
                    } else {
                        1
                    }
                })
            {
                if !ctx.is(c, JAR) || ctx.deck_size(ctx.me) >= 5 {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
        }
        if let Some(i) = t.activate(SHALLOW) {
            return t.pick(i);
        }
        let reset = ctx
            .monsters(ctx.me)
            .into_iter()
            .filter(|c| {
                c.position.face_up && (ctx.is(c, WORM) || ctx.is(c, JAR2) || ctx.is(c, JAR))
            })
            .max_by_key(|c| if ctx.is(c, WORM) { 3 } else { 2 });
        if let Some(c) = reset {
            if ctx.in_hand(TAIYOU) || ctx.obs.summon_used {
                for code in [MOON, ECLIPSE] {
                    if let Some(i) = t.activate(code) {
                        return t.pick_targeting(i, vec![c.at]);
                    }
                }
            }
        }
        if ctx.deck_size(ctx.me) > 6 {
            for code in [UPSTART, HAND] {
                if let Some(i) = t.activate(code) {
                    return t.pick(i);
                }
            }
        }
        if ctx.deck_size(ctx.opp) <= ctx.hand_size(ctx.opp)
            || ctx.deck_size(ctx.me) > ctx.hand_size(ctx.me) + 5
        {
            if let Some(i) = t.activate(DESTRUCTION) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, _: &Turn, c: &Choice) -> Option<Option<f64>> {
        Some(match (c.kind, c.code()?) {
            (ChoiceKind::SetMonster, WORM) => Some(3200.0),
            (ChoiceKind::SetMonster, JAR2 | JAR) => Some(2800.0),
            (ChoiceKind::NormalSummon, VALLEY) => Some(1600.0),
            _ => None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        Some(match ctx.canonical(t.choice(i).code()?) {
            WORM | JAR2 | JAR => Response::new(120.0),
            ECLIPSE if ctx.incoming_attack().is_some() => Response::new(80.0),
            TAIYOU | ECLIPSE | SHALLOW | UPSTART | DESTRUCTION | HAND => Response::no(),
            _ => return support::stall_chain(t, i).or_else(|| support::chain(t, i)),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if m.at.controller != t.ctx.me {
            return None;
        }
        let code = t.ctx.canonical(m.code?);
        if t.decision.hint == Hint::SpecialSummon {
            return Some(match code {
                WORM => 5000.0,
                JAR2 => 4000.0,
                JAR if t.ctx.deck_size(t.ctx.me) >= 5 => 3500.0,
                _ => 500.0,
            });
        }
        None
    }
    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        Some(t.ctx.data(code).is_trap() || t.ctx.data(code).is(types::QUICKPLAY))
    }
    fn position(&self, _: &Turn, _: u32) -> Option<Position> {
        Some(Position::FACE_UP_DEFENSE)
    }
    fn wants_battle(&self, _: &Turn) -> Option<bool> {
        Some(false)
    }
}
