//! Armageddon Hour: ritual Demise clears the field before Doom Dozer or
//! Swing deploy follow-up damage. Megamorph exploits the paid LP.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Armageddon Hour";
const DEMISE: u32 = 72426662;
const RUIN: u32 = 46427957;
const END: u32 = 8198712;
const ART: u32 = 46052429;
const CONTRACT: u32 = 69035382;
const MANJU: u32 = 95492061;
const SENJU: u32 = 23401839;
const BIRD: u32 = 57617178;
const DOZER: u32 = 76039636;
const TRADE: u32 = 38120068;
const MEGA: u32 = 22046459;
const SWING: u32 = 96765646;
const FADER: u32 = 19665973;
#[derive(Clone, Default)]
pub struct Demise;
impl Strategy for Demise {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            DEMISE => 3600,
            RUIN => 2800,
            DOZER => 3100,
            MANJU => 2500,
            SENJU | BIRD => 2100,
            END | ART | CONTRACT => 2400,
            MEGA => 1700,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.activate(DEMISE) {
            let theirs = ctx.field_strength(ctx.opp);
            let ours = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| !ctx.is(c, DEMISE))
                .map(|c| value(self, &ctx, c.code, Some(c)))
                .sum::<i32>()
                + ctx.spell_traps(ctx.me).len() as i32 * 1400;
            if ctx.my_lp() > 2000 && theirs > ours + 1000 {
                return t.pick(i);
            }
        }
        if !ctx.face_up_on_field(ctx.me, DEMISE) {
            for code in [ART, END, CONTRACT] {
                if let Some(i) = t.activate(code) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(DOZER), Some(Location::Hand)) {
            return t.pick(i);
        }
        if ctx.main1() && t.has(ChoiceKind::EnterBattle) {
            if ctx.my_lp() < ctx.opp_lp() {
                if let Some(i) = t.activate(MEGA) {
                    if let Some(c) = ctx
                        .monsters(ctx.me)
                        .into_iter()
                        .filter(|c| {
                            c.position.face_up
                                && c.attack >= 2300
                                && c.attack < 2 * ctx.view_data(c).attack
                        })
                        .max_by_key(|c| c.attack)
                    {
                        return t.pick_targeting(i, vec![c.at]);
                    }
                }
            }
            if let Some(i) = t.activate(SWING) {
                return t.pick(i);
            }
        }
        if ctx.count_in(ctx.me, Location::Hand, DEMISE) > 1
            || ctx.count_in(ctx.me, Location::Hand, DOZER) > 1
            || ctx.face_up_on_field(ctx.me, DEMISE)
            || ctx.in_hand(RUIN)
            || (ctx.in_hand(DOZER)
                && !ctx.in_hand(ART)
                && ctx
                    .graveyard(ctx.me)
                    .iter()
                    .filter(|c| ctx.view_data(c).race & crate::cards::races::INSECT != 0)
                    .count()
                    < 2)
        {
            if let Some(i) = t.activate(TRADE) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, _: &Turn, c: &Choice) -> Option<Option<f64>> {
        Some(match (c.kind, c.code()?) {
            (ChoiceKind::NormalSummon, MANJU) => Some(3300.0),
            (ChoiceKind::NormalSummon, SENJU | BIRD) => Some(2600.0),
            (_, DEMISE | RUIN | DOZER | FADER) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.ctx.canonical(t.choice(i).code()?) {
            MANJU | SENJU | BIRD | RUIN | DOZER => Response::new(120.0),
            DEMISE | END | ART | CONTRACT | TRADE | MEGA | SWING => Response::no(),
            _ => return support::stall_chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        if t.decision.hint == Hint::AddToHand {
            return Some(match code {
                DEMISE if !ctx.in_hand(DEMISE) && !ctx.face_up_on_field(ctx.me, DEMISE) => 6500.0,
                ART if !ctx.in_hand(ART) => 5500.0,
                END | CONTRACT if !ctx.in_hand(END) && !ctx.in_hand(CONTRACT) => 5000.0,
                _ => value(self, &ctx, Some(code), None) as f64,
            });
        }
        if t.decision.hint == Hint::Discard && t.memory.last_activated == Some(TRADE) {
            return Some(if ctx.count_in(ctx.me, Location::Hand, code) > 1 {
                6000.0
            } else if code == RUIN {
                4500.0
            } else if code == DEMISE && !ctx.face_up_on_field(ctx.me, DEMISE) {
                -10000.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(if code == DEMISE {
                6500.0
            } else {
                value(self, &ctx, Some(code), None) as f64
            });
        }
        if matches!(
            t.decision.hint,
            Hint::Tribute | Hint::Release | Hint::ToGraveyard
        ) {
            return Some(if m.at.location == Location::Deck {
                4000.0
            } else if code == DEMISE && m.at.location == Location::MonsterZone {
                -20000.0
            } else if m.at.location == Location::MonsterZone && ctx.data(code).level == 4 {
                2000.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        None
    }
}
