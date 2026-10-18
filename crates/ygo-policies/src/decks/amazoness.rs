//! Queens of the Wild: Village replacements, Queen protection, and Swords Woman reflection.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Queens of the Wild";
const QUEEN: u32 = 15951532;
const SWORD: u32 = 94004268;
const SAGE: u32 = 53162898;
const VILLAGE: u32 = 712559;
const SPIRIT: u32 = 36100154;
const WILL: u32 = 22082163;
#[derive(Clone, Default)]
pub struct Amazoness;
impl Strategy for Amazoness {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            QUEEN => 3200,
            VILLAGE => 2800,
            SWORD => 2300,
            SAGE => 2100,
            WILL => 2200,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        for code in [32807846, 95281259] {
            if let Some(i) = t.activate(code) {
                return t.pick(i);
            }
        }
        if !t.ctx.in_hand(VILLAGE) && !t.ctx.face_up_on_field(t.ctx.me, VILLAGE) {
            if let Some(i) = t.activate(73628505) {
                return t.pick(i);
            }
        }
        for code in [VILLAGE, SPIRIT] {
            if !t.ctx.face_up_on_field(t.ctx.me, code) {
                if let Some(i) = t.activate(code) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.activate(WILL) {
            return t.pick(i);
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let code = c.code()?;
        if c.kind != ChoiceKind::NormalSummon {
            return None;
        }
        if code == QUEEN {
            return Some((!t.ctx.face_up_on_field(t.ctx.me, QUEEN)).then_some(3700.0));
        }
        if code == SWORD && t.ctx.face_up_on_field(t.ctx.me, QUEEN) {
            return Some(Some(3500.0));
        }
        None
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.choice(i).code()? {
            VILLAGE if t.choice(i).at().map(|a| a.location) != Some(Location::Hand) => {
                Response::new(130.0)
            }
            SAGE | 29654737 | 2460565 => Response::new(120.0),
            WILL if t.ctx.incoming_attack().is_some() => Response::new(80.0),
            VILLAGE | SPIRIT | WILL => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if m.at.controller != t.ctx.me {
            return None;
        }
        if matches!(
            t.decision.hint,
            Hint::SpecialSummon | Hint::AddToHand | Hint::ReturnToHand
        ) {
            return Some(match m.code? {
                QUEEN if !t.ctx.face_up_on_field(t.ctx.me, QUEEN) => 6000.0,
                SWORD if t.ctx.face_up_on_field(t.ctx.me, QUEEN) => 5500.0,
                SAGE if !t.ctx.spell_traps(t.ctx.opp).is_empty() => 3500.0,
                47480070 => 2800.0,
                _ => t.ctx.data(m.code?).attack as f64,
            });
        }
        None
    }
    fn battle(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if ctx.effects_drained() {
            return None;
        }
        for (i, c) in t.choices() {
            if c.kind != ChoiceKind::Attack || c.code() != Some(SWORD) {
                continue;
            }
            let ours = t.view(c)?;
            if let Some(enemy) = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|e| e.position.face_up)
                .filter(|e| {
                    let met = ctx.attack_meets(Some(ours), e);
                    let damage = met.stat - ours.attack;
                    damage > 0
                        && !met.facts.no_damage
                        && !met.facts.before_damage
                        && !ctx.attack_negatable(e)
                        && (ctx.battle_proof(ours)
                            || !e.position.attack
                            || damage >= ctx.opp_lp()
                            || damage >= 1800)
                })
                .max_by_key(|e| ctx.battle_stat(e))
            {
                return t.pick_targeting(i, vec![enemy.at]);
            }
        }
        None
    }
    fn attack_trick(&self, ctx: &Ctx, c: &CardView) -> i32 {
        if ctx.view_data(c).in_set(0x4)
            && ctx.face_up_on_field(ctx.me, SPIRIT)
            && ctx.opp_best_attack() > c.attack
        {
            1000
        } else {
            0
        }
    }
}
