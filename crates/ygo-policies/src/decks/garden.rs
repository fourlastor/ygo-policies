//! Garden of Thorns: develop Plants before activating Black Garden, turn
//! its tokens into repeated Rose Tentacles attacks, and revive exact ATK fits.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::{races, types};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
use crate::tactics;
pub const DECK: &str = "Garden of Thorns";
const GARDEN: u32 = 71645242;
const TERRAFORM: u32 = 73628505;
const TENTACLES: u32 = 41160533;
const LONEFIRE: u32 = 48686504;
const LION: u32 = 20546916;
const EVIL: u32 = 85431040;
const GIRL: u32 = 84824601;
const WITCH: u32 = 62379337;
const SPORE: u32 = 11747708;
const BULB: u32 = 67441435;
const DANDY: u32 = 15341821;
const COPY: u32 = 66457407;
const THORN: u32 = 65079854;
const MARK: u32 = 45247637;
const ONE: u32 = 2295440;
const FOOLISH: u32 = 81439173;
const SPLENDID: u32 = 4290468;
#[derive(Clone, Default)]
pub struct Garden;
impl Strategy for Garden {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            GARDEN => 2600,
            LONEFIRE => 2500,
            LION => 2200,
            TENTACLES => 2600,
            MARK => 2500,
            THORN => 1700,
            SPLENDID => 2600,
            GIRL | WITCH => 1500,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = support::copy_plant(self, t) {
            return Some(i);
        }
        let ctx = t.ctx;
        for code in [LONEFIRE, EVIL, ONE, FOOLISH] {
            if let Some(i) = t.activate(code) {
                return t.pick(i);
            }
        }
        if !ctx.in_hand(GARDEN) && !ctx.face_up_on_field(ctx.me, GARDEN) {
            if let Some(i) = t.activate(TERRAFORM) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(MARK) {
            if let Some(c) = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| c.position.face_up && ctx.reaches(c, MARK, true, false))
                .max_by_key(|c| ctx.threat(c))
            {
                if c.attack >= 1600 {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
        }
        if let Some(i) = t.activate(THORN) {
            if let Some(c) = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| {
                    c.position.face_up
                        && ctx.view_data(c).race & races::PLANT != 0
                        && c.attack >= 1600
                })
                .max_by_key(|c| if ctx.is(c, TENTACLES) { 5000 } else { c.attack })
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if let Some(i) = t.find_where(|c| {
            c.kind == ChoiceKind::Activate
                && c.code() == Some(GARDEN)
                && c.description == ((GARDEN as u64) << 20) + 1
        }) {
            let ours = ctx
                .monsters(ctx.me)
                .iter()
                .filter(|c| ctx.view_data(c).race & races::PLANT != 0)
                .map(|c| value(self, &ctx, c.code, Some(c)))
                .sum::<i32>();
            let theirs = ctx
                .monsters(ctx.opp)
                .iter()
                .filter(|c| ctx.view_data(c).race & races::PLANT != 0)
                .map(|c| ctx.threat(c))
                .sum::<i32>();
            if ours < 1800 || theirs > ours {
                return t.pick(i);
            }
        }
        if let Some(i) = t.find_where(|c| {
            c.kind == ChoiceKind::Activate
                && c.code() == Some(SPLENDID)
                && c.description == (SPLENDID as u64) << 20
        }) {
            if let Some(c) = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| {
                    c.position.face_up && c.attack > 1000 && ctx.reaches(c, SPLENDID, true, false)
                })
                .max_by_key(|c| c.attack)
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if tactics::synchro_with_tuner(self, &ctx, 1).is_some() {
            if let Some(i) = t.activate_from(BULB, Location::Graveyard) {
                return t.pick(i);
            }
        }
        if ctx
            .monsters(ctx.me)
            .iter()
            .any(|c| !ctx.view_data(c).is_tuner())
        {
            if let Some(i) = t.activate_from(SPORE, Location::Graveyard) {
                return t.pick(i);
            }
        }
        None
    }
    fn main_phase_late(&mut self, t: &mut Turn) -> Option<usize> {
        if !t.ctx.face_up_on_field(t.ctx.me, GARDEN) && !t.has(ChoiceKind::SpecialSummon) {
            if let Some(i) = t.activate(GARDEN) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, LONEFIRE) => Some(3700.0),
            (ChoiceKind::NormalSummon, EVIL) => Some(3000.0),
            (ChoiceKind::NormalSummon, TENTACLES)
                if ctx.monsters(ctx.me).iter().any(|c| c.attack < 1000) =>
            {
                Some(3500.0)
            }
            (_, TENTACLES) => None,
            (ChoiceKind::NormalSummon, LION) => Some(2700.0),
            (ChoiceKind::NormalSummon, SPORE | BULB | COPY)
                if tactics::synchro_with_tuner(self, &ctx, 1).is_some() =>
            {
                Some(3500.0)
            }
            (ChoiceKind::NormalSummon, SPORE | BULB | COPY) => None,
            (ChoiceKind::NormalSummon, GIRL | WITCH)
                if ctx.monsters(ctx.opp).is_empty() && t.has(ChoiceKind::EnterBattle) =>
            {
                Some(1800.0)
            }
            (ChoiceKind::SetMonster, GIRL | WITCH | DANDY) => Some(1700.0),
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let c = t.choice(i);
        Some(match t.ctx.canonical(c.code()?) {
            GIRL | WITCH | DANDY | TENTACLES => Response::new(120.0),
            GARDEN if c.description == (GARDEN as u64) << 20 => Response::new(120.0),
            SPLENDID
                if c.description == ((SPLENDID as u64) << 20) + 1
                    && t.ctx.my_turn()
                    && t.ctx.monsters(t.ctx.opp).is_empty() =>
            {
                Response::new(75.0)
            }
            GARDEN | SPLENDID | EVIL | ONE | FOOLISH | LONEFIRE | TERRAFORM | THORN | MARK
            | SPORE | BULB | COPY => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if let Some(score) = support::spore_cost(self, t, m) {
            return Some(score);
        }
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        if t.decision.hint == Hint::SpecialSummon {
            return Some(match code {
                LION => 4500.0,
                LONEFIRE => 3500.0,
                _ => support::body_score(self, &ctx, code),
            });
        }
        if t.decision.hint == Hint::AddToHand {
            return Some(match code {
                TENTACLES if !ctx.in_hand(TENTACLES) => 4500.0,
                LONEFIRE => 4000.0,
                LION => 3500.0,
                _ => support::body_score(self, &ctx, code),
            });
        }
        if t.decision.hint == Hint::Release {
            return Some(if code == LONEFIRE || ctx.data(code).is(types::TOKEN) {
                4500.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(match code {
                DANDY => 5000.0,
                BULB => 4000.0,
                SPORE => 3500.0,
                _ => 0.0,
            });
        }
        if t.decision.hint == Hint::Discard && code == DANDY && !ctx.monsters_banished() {
            return Some(5000.0);
        }
        if t.decision.hint == Hint::Banish && m.at.location == Location::Graveyard {
            return Some(if code == SPORE || code == BULB {
                -3000.0
            } else {
                1000.0
            });
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
}
