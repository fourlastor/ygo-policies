//! Prismatic Forge: Armadillo/Alexandrite find materials, Thunder Dragon
//! and Shell supply spare cards, and Gem-Knight Fusion recycles for follow-ups.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Prismatic Forge";
const FUSION: u32 = 1264319;
const POLY: u32 = 24094653;
const ARMA: u32 = 27004302;
const ALEX: u32 = 90019393;
const GARNET: u32 = 91731841;
const TOURM: u32 = 54620698;
const SAPPHIRE: u32 = 27126980;
const EMERALD: u32 = 69243722;
const THUNDER: u32 = 31786629;
const SHELL: u32 = 33365932;
const FACTORY: u32 = 90928333;
const ENHANCE: u32 = 41777;
const RUBY: u32 = 76614340;
const CITRINE: u32 = 67985943;
const TOPAZ: u32 = 49597193;
const AQUA: u32 = 13108445;
const PRISMA: u32 = 93379652;
#[derive(Clone, Default)]
pub struct GemKnight;
impl GemKnight {
    fn enhance(&self, t: &Turn) -> Response {
        let ctx = t.ctx;
        let theirs = ctx
            .graveyard(ctx.me)
            .into_iter()
            .filter(|c| ctx.view_data(c).in_set(0x1047))
            .max_by_key(|c| value(self, &ctx, c.code, Some(c)));
        let ours = ctx
            .monsters(ctx.me)
            .into_iter()
            .filter(|c| ctx.view_data(c).in_set(0x1047))
            .min_by_key(|c| value(self, &ctx, c.code, Some(c)));
        if let (Some(a), Some(b)) = (ours, theirs) {
            if value(self, &ctx, b.code, Some(b)) > value(self, &ctx, a.code, Some(a)) + 500 {
                return Response::targeting(75.0, vec![a.at, b.at]);
            }
        }
        Response::no()
    }
}
impl Strategy for GemKnight {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            FUSION => 2900,
            ARMA => 2400,
            ALEX => 2100,
            GARNET => 1900,
            TOURM => 1600,
            SAPPHIRE => 1000,
            EMERALD => 1700,
            PRISMA => 3400,
            CITRINE => 3000,
            RUBY => 2900,
            TOPAZ => 2900,
            AQUA => 2700,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.activate_from(THUNDER, Location::Hand) {
            return t.pick(i);
        }
        if ctx.my_lp() > 1200 {
            if let Some(i) = t.activate_from(SHELL, Location::Graveyard) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(FACTORY) {
            return t.pick(i);
        }
        if let Some(i) = t.activate(PRISMA) {
            if let Some(c) = ctx
                .monsters(ctx.opp)
                .into_iter()
                .chain(ctx.spell_traps(ctx.opp))
                .filter(|c| c.position.face_up && ctx.reaches(c, PRISMA, true, true))
                .max_by_key(|c| ctx.threat(c))
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        // Armadillo adds material before Fusion can consume it from the hand.
        if let Some(i) = t.find(ChoiceKind::NormalSummon, Some(ARMA), Some(Location::Hand)) {
            return t.pick(i);
        }
        // Attack with an established Fusion before combining it away on an open field.
        let attack_first = ctx.main1()
            && t.has(ChoiceKind::EnterBattle)
            && ctx.monsters(ctx.opp).is_empty()
            && ctx
                .monsters(ctx.me)
                .iter()
                .any(|c| c.position.face_up && ctx.view_data(c).is_extra() && c.attack >= 2000);
        if !attack_first {
            if let Some(i) = t.activate_from(FUSION, Location::Hand) {
                return t.pick(i);
            }
            if let Some(i) = t.activate(POLY) {
                return t.pick(i);
            }
        }
        if !ctx.in_hand(FUSION) {
            if let Some(i) = t.activate_from(FUSION, Location::Graveyard) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(ALEX) {
            return t.pick(i);
        }
        if let Some(i) = t.activate(EMERALD) {
            if ctx
                .graveyard(ctx.me)
                .iter()
                .any(|c| ctx.view_data(c).is_extra())
                && ctx.monsters(ctx.me).iter().any(|c| ctx.is(c, SAPPHIRE))
            {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(ENHANCE) {
            let r = self.enhance(t);
            if r.score > 0.0 {
                return t.pick_targeting(i, r.intent);
            }
        }
        None
    }
    fn summon_score(&self, _: &Turn, c: &Choice) -> Option<Option<f64>> {
        Some(match (c.kind, c.code()?) {
            (ChoiceKind::NormalSummon, ARMA) => Some(3800.0),
            (ChoiceKind::NormalSummon, ALEX) => Some(2600.0),
            (_, THUNDER | SHELL) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.ctx.canonical(t.choice(i).code()?) {
            ARMA | AQUA | TOPAZ => Response::new(120.0),
            ENHANCE => self.enhance(t),
            FUSION | POLY | THUNDER | SHELL | FACTORY | ALEX | EMERALD | PRISMA | RUBY => {
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
        if t.decision.hint == Hint::AddToHand {
            return Some(match code {
                GARNET if !ctx.in_hand(GARNET) => 4500.0,
                TOURM if !ctx.in_hand(TOURM) => 4000.0,
                ALEX => 3000.0,
                _ => value(self, &ctx, Some(code), None) as f64,
            });
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(if src == Some(ALEX) {
                match code {
                    GARNET => 4500.0,
                    TOURM => 3500.0,
                    _ => 1000.0,
                }
            } else {
                value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::ToGraveyard
            && src == Some(PRISMA)
            && m.at.location == Location::Hand
        {
            return Some(if code == FUSION {
                6000.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::FusionMaterial {
            return Some(if code == AQUA && m.at.location == Location::MonsterZone {
                2000.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
                    - if ctx.data(code).is_extra() {
                        3000.0
                    } else {
                        0.0
                    }
            });
        }
        if t.decision.hint == Hint::Banish && src == Some(FUSION) {
            return Some(if ctx.data(code).is_extra() {
                -5000.0
            } else {
                -ctx.data(code).attack as f64
            });
        }
        None
    }
}
