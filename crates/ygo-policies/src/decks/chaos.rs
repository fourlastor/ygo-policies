//! Between Light and Dark: fair LIGHT/DARK trades fuel Chaos Sorcerer,
//! backed by Rai-Oh, Veiler, Alchemist recovery and generic Synchros.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::attributes;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Between Light and Dark";
const SORCERER: u32 = 9596126;
const RAI: u32 = 71564252;
const ALCHEMIST: u32 = 36733451;
const LADY: u32 = 7572887;
const BREAKER: u32 = 71413901;
const KYCOO: u32 = 88240808;
const VEILER: u32 = 97268402;
const RYKO: u32 = 21502796;
const GORZ: u32 = 44330098;
const TRAG: u32 = 98777036;
const CYBER: u32 = 70095154;
const FOOLISH: u32 = 81439173;
#[derive(Clone, Default)]
pub struct Chaos;
impl Strategy for Chaos {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            SORCERER => 3200,
            RAI => 2300,
            ALCHEMIST => 2000,
            LADY => 1800,
            BREAKER | KYCOO => 2100,
            VEILER => 1900,
            GORZ => 2900,
            TRAG => 2500,
            _ => return support::extra_value(code),
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
        for code in [CYBER, SORCERER] {
            if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(code), Some(Location::Hand)) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(SORCERER) {
            if let Some(c) = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| c.position.face_up && ctx.reaches(c, SORCERER, true, false))
                .max_by_key(|c| ctx.threat(c))
            {
                if c.attack >= 2200 || ctx.battle_proof(c) {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
        }
        if let Some(i) = t.activate(BREAKER) {
            if let Some(c) = ctx
                .spell_traps(ctx.opp)
                .into_iter()
                .filter(|c| ctx.reaches(c, BREAKER, true, true))
                .max_by_key(|c| ctx.threat(c))
            {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        if ctx.deck_size(ctx.me) > 4 {
            if let Some(i) = t.activate_from(ALCHEMIST, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        if ctx.in_hand(SORCERER) {
            if let Some(i) = t.activate(FOOLISH) {
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
            (ChoiceKind::NormalSummon, VEILER)
                if support::body_score(self, &ctx, code) > 2000.0 =>
            {
                Some(3500.0)
            }
            (_, SORCERER | VEILER | GORZ | TRAG | CYBER | support::HONEST) => None,
            (ChoiceKind::NormalSummon, RAI) => Some(2500.0),
            (ChoiceKind::SetMonster, RYKO) => Some(2200.0),
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        let c = t.choice(i);
        Some(match ctx.canonical(c.code()?) {
            GORZ | TRAG => Response::new(130.0),
            RAI => Response::new(95.0),
            ALCHEMIST if c.at().map(|a| a.location) == Some(Location::Graveyard) => {
                Response::new(120.0)
            }
            BREAKER if c.description == (BREAKER as u64) << 20 => Response::new(120.0),
            KYCOO => Response::new(120.0),
            VEILER => {
                if let Some(l) = ctx.obs.chain.last().filter(|l| {
                    l.controller == ctx.opp && l.source.location == Location::MonsterZone
                }) {
                    if let Some(target) = ctx
                        .monsters(ctx.opp)
                        .into_iter()
                        .find(|c| c.at == l.source && ctx.reaches(c, VEILER, true, false))
                    {
                        return Some(Response::targeting(90.0, vec![target.at]));
                    }
                }
                Response::no()
            }
            LADY => {
                let enemy = ctx
                    .battle_attacker()
                    .into_iter()
                    .chain(ctx.battle_target())
                    .find(|c| c.at.controller == ctx.opp);
                if enemy.map_or(false, |c| ctx.threat(c) > 1800 || ctx.battle_proof(c)) {
                    Response::new(90.0)
                } else {
                    Response::no()
                }
            }
            SORCERER | ALCHEMIST | BREAKER | FOOLISH => Response::no(),
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
        if t.decision.hint == Hint::ToGraveyard
            && m.at.location == Location::Deck
            && src == Some(FOOLISH)
        {
            let light = ctx
                .graveyard(ctx.me)
                .iter()
                .any(|c| ctx.view_data(c).attribute & attributes::LIGHT != 0);
            let needed = if light {
                attributes::DARK
            } else {
                attributes::LIGHT
            };
            return Some(if ctx.data(code).attribute & needed != 0 {
                5000.0 - ctx.data(code).attack as f64 / 10.0
            } else {
                0.0
            });
        }
        if t.decision.hint == Hint::Banish {
            return Some(-value(self, &ctx, Some(code), None) as f64);
        }
        if matches!(t.decision.hint, Hint::AddToHand | Hint::ReturnToHand) && src == Some(ALCHEMIST)
        {
            return Some(if code == SORCERER {
                6000.0
            } else {
                value(self, &ctx, Some(code), None) as f64
            });
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn attack_trick(&self, ctx: &Ctx, c: &CardView) -> i32 {
        support::honest_trick(ctx, c)
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
