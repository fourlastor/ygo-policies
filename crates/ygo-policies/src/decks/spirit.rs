//! Passing Spirits: reusable Normal Summons, Kinka's Level-1 revival,
//! returning Creature Swap gifts, and tribute Spirits behind Frog/Fader.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
use crate::tactics;
pub const DECK: &str = "Passing Spirits";
const KINKA: u32 = 45452224;
const ASURA: u32 = 2134346;
const DUST: u32 = 89111398;
const HINO: u32 = 75745607;
const YAMATA: u32 = 76862289;
const IZANAGI: u32 = 6544078;
const FROG: u32 = 12538374;
const SPORE: u32 = 11747708;
const BULB: u32 = 67441435;
const DANDY: u32 = 15341821;
const FADER: u32 = 19665973;
const SWAP: u32 = 31036355;
const ONE: u32 = 2295440;
const FOOLISH: u32 = 81439173;
const DOUBLE: u32 = 43422537;
#[derive(Clone, Default)]
pub struct Spirit;
impl Spirit {
    fn swap(&self, t: &Turn) -> Option<Response> {
        let ctx = t.ctx;
        let ours = ctx.monsters(ctx.me).into_iter().min_by_key(|c| {
            if ctx.view_data(c).is(crate::cards::types::SPIRIT) {
                100
            } else {
                value(self, &ctx, c.code, Some(c))
            }
        })?;
        let theirs = ctx
            .monsters(ctx.opp)
            .into_iter()
            .map(|c| ctx.threat(c))
            .min()?;
        let price = if ctx.view_data(ours).is(crate::cards::types::SPIRIT) {
            100
        } else {
            value(self, &ctx, ours.code, Some(ours))
        };
        (theirs > price + 800).then(|| Response::targeting(70.0, vec![ours.at]))
    }
}
impl Strategy for Spirit {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            KINKA => 2000,
            ASURA => 2000,
            HINO => 3300,
            YAMATA => 2900,
            IZANAGI => 2700,
            DUST => 2600,
            FROG => 1700,
            SWAP => 2400,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let (Some(i), Some(r)) = (t.activate(SWAP), self.swap(t)) {
            return t.pick_targeting(i, r.intent);
        }
        if ctx.count_in(ctx.me, Location::Graveyard, FROG) == 0 {
            if let Some(i) = t.activate(FOOLISH) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(ONE) {
            return t.pick(i);
        }
        if ctx.obs.summon_used
            && ctx.monsters(ctx.me).len() >= 2
            && (ctx.in_hand(HINO) || ctx.in_hand(YAMATA))
        {
            if let Some(i) = t.activate(DOUBLE) {
                return t.pick(i);
            }
        }
        if tactics::synchro_with_tuner(self, &ctx, 1).is_some() {
            for code in [BULB, SPORE] {
                if let Some(i) = t.activate_from(code, Location::Graveyard) {
                    return t.pick(i);
                }
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, DUST)
                if ctx.field_strength(ctx.opp) > ctx.field_strength(ctx.me) + 1000 =>
            {
                Some(4500.0)
            }
            (_, DUST) => None,
            (ChoiceKind::NormalSummon, HINO | YAMATA)
                if ctx
                    .monsters(ctx.me)
                    .iter()
                    .filter(|c| c.attack < 1000)
                    .count()
                    >= 2 =>
            {
                Some(if code == HINO { 4000.0 } else { 3500.0 })
            }
            (_, HINO | YAMATA | IZANAGI) => None,
            (ChoiceKind::NormalSummon, KINKA)
                if ctx.graveyard(ctx.me).iter().any(|c| {
                    ctx.view_data(c).level == 1 && !ctx.view_data(c).is(crate::cards::types::SPIRIT)
                }) =>
            {
                Some(3300.0)
            }
            (ChoiceKind::NormalSummon, KINKA) => None,
            (ChoiceKind::NormalSummon, ASURA) => Some(2300.0),
            (_, FADER) => None,
            (ChoiceKind::NormalSummon, SPORE | BULB)
                if tactics::synchro_with_tuner(self, &ctx, 1).is_some() =>
            {
                Some(3700.0)
            }
            (ChoiceKind::SetMonster, FROG | DANDY) => Some(1600.0),
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        Some(match ctx.canonical(t.choice(i).code()?) {
            KINKA | ASURA | HINO | YAMATA | FROG | DANDY => Response::new(120.0),
            DUST => Response::new(120.0),
            SWAP | ONE | FOOLISH | BULB | SPORE => Response::no(),
            _ => return support::stall_chain(t, i).or_else(|| support::chain(t, i)),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let src = t.memory.last_activated.map(|k| ctx.canonical(k));
        if t.decision.hint == Hint::SpecialSummon {
            return Some(match code {
                BULB | SPORE => support::body_score(self, &ctx, code),
                FROG => {
                    if ctx.count_in(ctx.me, Location::Graveyard, FROG) == 0 {
                        3000.0
                    } else {
                        1000.0
                    }
                }
                _ => value(self, &ctx, Some(code), None) as f64,
            });
        }
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(if code == FROG {
                5000.0
            } else if code == DANDY {
                4000.0
            } else {
                0.0
            });
        }
        if t.decision.hint == Hint::Discard && code == DANDY && !ctx.monsters_banished() {
            return Some(4000.0);
        }
        if t.decision.hint == Hint::Banish && src == Some(IZANAGI) {
            return Some(-value(self, &ctx, Some(code), None) as f64);
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
}
