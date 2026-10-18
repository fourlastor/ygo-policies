//! Footprints in Fire: battle recruitment into Dinosaur Synchros.
use super::support;
use crate::agent::{Outcome, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Member};
pub const DECK: &str = "Footprints in Fire";
const GUAIBA: u32 = 11012887;
const VELO: u32 = 59312550;
const DINO: u32 = 17948378;
const AEOLO: u32 = 80727721;
const DIG: u32 = 47325505;
const REKINDLE: u32 = 74845897;
const GIGA: u32 = 80032567;
const METEOR: u32 = 17548456;
#[derive(Clone, Default)]
pub struct Jurrac;
impl Strategy for Jurrac {
    fn value(&self, _: &Ctx, k: u32) -> Option<i32> {
        Some(match k {
            GUAIBA => 2400,
            VELO => 2200,
            GIGA => 3400,
            65961683 => 2600,
            METEOR => 1900,
            _ => return support::extra_value(k),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        for code in [DIG, REKINDLE] {
            if let Some(i) = t.activate(code) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(AEOLO) {
            if let Some(c) = t
                .ctx
                .graveyard(t.ctx.me)
                .into_iter()
                .filter(|c| {
                    t.ctx.view_data(c).in_set(0x22) && c.level <= 4 && c.code != Some(AEOLO)
                })
                .max_by(|a, b| {
                    support::body_score(self, &t.ctx, a.code.unwrap_or(0))
                        .total_cmp(&support::body_score(self, &t.ctx, b.code.unwrap_or(0)))
                })
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let code = c.code()?;
        if c.kind == ChoiceKind::NormalSummon && t.ctx.data(code).in_set(0x22) {
            return Some(Some(
                support::body_score(self, &t.ctx, code) + if code == GUAIBA { 700.0 } else { 0.0 },
            ));
        }
        None
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.choice(i).code()? {
            GUAIBA | VELO | 48411996 | 16111820 | 71106375 => Response::new(130.0),
            DINO if t
                .ctx
                .monsters(t.ctx.me)
                .iter()
                .any(|c| t.ctx.view_data(c).in_set(0x22) && c.attack < 2200) =>
            {
                Response::new(120.0)
            }
            METEOR => Response::new(120.0),
            DINO | AEOLO | DIG | REKINDLE => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if m.at.controller != t.ctx.me {
            return None;
        }
        let code = m.code?;
        if matches!(t.decision.hint, Hint::SpecialSummon | Hint::AddToHand) {
            return Some(
                support::body_score(self, &t.ctx, code)
                    + match code {
                        GUAIBA => 600.0,
                        DINO => 300.0,
                        _ => 0.0,
                    },
            );
        }
        support::material_score(&t.ctx, m, t.decision.hint)
    }
    fn attack_outcome(&self, ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
        let shrink = crate::staples::SHRINK;
        if ctx.in_hand(shrink)
            && target.position.face_up
            && target.position.attack
            && !ctx.wasted(shrink)
            && ctx.reaches(target, shrink, true, false)
            && ![ctx.me, ctx.opp].iter().any(|&p| {
                ctx.face_up_on_field(p, 58921041)
                    || ctx.face_up_on_field(p, 61740673)
                    || ctx.face_up_on_field(p, 84636823)
            })
        {
            return Some(crate::tactics::default_outcome(
                ctx,
                attacker,
                target,
                (ctx.view_data(target).attack / 2)
                    .min(target.attack / 2)
                    .max(0),
            ));
        }
        None
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        if c.code() == Some(METEOR) {
            return Some(t.ctx.field_strength(t.ctx.opp) > t.ctx.field_strength(t.ctx.me) + 2500);
        }
        support::extra_allowed(t, c)
    }
}
