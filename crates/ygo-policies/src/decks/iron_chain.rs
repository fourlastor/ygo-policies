//! Rust Never Sleeps: Repairman/Coil recursion, Synchros and opportunistic milling.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Rust Never Sleeps";
const REPAIR: u32 = 53274132;
const COIL: u32 = 53152590;
const SNAKE: u32 = 80769747;
const BLAST: u32 = 26157485;
const DRAGON: u32 = 19974580;
const POISON: u32 = 33302407;
const RYKO: u32 = 21502796;
const TROOPER: u32 = 85087012;
#[derive(Clone, Default)]
pub struct IronChain;
impl Strategy for IronChain {
    fn value(&self, _: &Ctx, k: u32) -> Option<i32> {
        Some(match k {
            REPAIR => 2600,
            DRAGON => 2700,
            COIL => 2000,
            _ => return support::extra_value(k),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !ctx.monsters(ctx.opp).is_empty() || !ctx.spell_traps(ctx.opp).is_empty() {
            if let Some(i) = t.find_where(|c| {
                c.kind == ChoiceKind::ChangePosition
                    && c.code() == Some(RYKO)
                    && t.view(c).map_or(false, |c| !c.position.face_up)
            }) {
                return t.pick(i);
            }
        }
        for k in [REPAIR, TROOPER] {
            if let Some(i) = t.activate(k) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(COIL) {
            if let Some(c) = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| c.position.face_up && ctx.view_data(c).in_set(0x25))
                .max_by_key(|c| c.attack)
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if let Some(i) = t.activate(SNAKE) {
            let best = ctx
                .monsters(ctx.me)
                .iter()
                .filter(|c| c.code != Some(SNAKE))
                .map(|c| c.attack)
                .max()
                .unwrap_or(0);
            if let Some(c) = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| {
                    c.position.face_up
                        && ctx.battle_stat(c) > best
                        && ctx.battle_stat(c) - 800 < best
                        && ctx.reaches(c, SNAKE, true, false)
                })
                .max_by_key(|c| ctx.threat(c))
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if let Some(i) = t.activate(DRAGON) {
            let n = ctx
                .graveyard(ctx.me)
                .iter()
                .filter(|c| ctx.view_data(c).in_set(0x25))
                .count() as i32;
            if let Some(c) = t.view(t.choice(i)) {
                if ctx.main1()
                    && n > 0
                    && ((ctx.opp_best_attack() >= c.attack
                        && ctx.opp_best_attack() < c.attack + n * 200)
                        || (ctx.monsters(ctx.opp).is_empty() && ctx.opp_lp() <= c.attack + n * 200))
                {
                    return t.pick(i);
                }
            }
        }
        if ctx.opp_lp() <= 800 {
            if let Some(i) = t.activate(BLAST) {
                return t.pick(i);
            }
        }
        if !ctx.face_up_on_field(ctx.me, POISON) {
            if let Some(i) = t.activate(POISON) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(81439173) {
            return t.pick(i);
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let k = c.code()?;
        if c.kind == ChoiceKind::NormalSummon
            && matches!(k, REPAIR | COIL | SNAKE | BLAST | 63977008 | 14943837)
        {
            return Some(Some(
                support::body_score(self, &t.ctx, k)
                    + if k == REPAIR
                        && t.ctx
                            .graveyard(t.ctx.me)
                            .iter()
                            .any(|c| c.code == Some(COIL))
                    {
                        3000.0
                    } else {
                        0.0
                    },
            ));
        }
        None
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.choice(i).code()? {
            63977008 | 14943837 | TROOPER | 15341821 | POISON => Response::new(130.0),
            DRAGON if t.choice(i).description == ((DRAGON as u64) << 20) + 1 => {
                Response::new(120.0)
            }
            REPAIR | COIL | SNAKE | BLAST | DRAGON => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if m.at.controller != t.ctx.me {
            return None;
        }
        let k = m.code?;
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(match k {
                15341821 => 8000.0,
                COIL => 6000.0,
                _ => 0.0,
            });
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(support::body_score(self, &t.ctx, k));
        }
        support::material_score(&t.ctx, m, t.decision.hint)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
    fn yes_no(&self, t: &Turn) -> Option<bool> {
        (t.decision.subject == Some(RYKO)).then(|| {
            !t.ctx.monsters(t.ctx.opp).is_empty() || !t.ctx.spell_traps(t.ctx.opp).is_empty()
        })
    }
}
