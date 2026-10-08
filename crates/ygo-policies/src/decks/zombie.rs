//! Graveyard Shift: recruiters stock the Graveyard; Zombie Master, Mezuki
//! and Book of Life turn it back into bodies and Plaguespreader Synchros.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
use crate::tactics;
pub const DECK: &str = "Graveyard Shift";
const MASTER: u32 = 17259470;
const GOBLIN: u32 = 63665875;
const TURTLE: u32 = 77044671;
const MEZUKI: u32 = 92826944;
const PLAGUE: u32 = 33420078;
const IL_BLUD: u32 = 70595331;
const REAPER: u32 = 23205979;
const LIFE: u32 = 2204140;
const FOOLISH: u32 = 81439173;
const TROOPER: u32 = 85087012;
const RYKO: u32 = 21502796;
#[derive(Clone, Default)]
pub struct Zombie;
impl Zombie {
    fn revive_score(&self, ctx: &Ctx, code: u32) -> f64 {
        if code == PLAGUE && tactics::synchro_with_tuner(self, ctx, 2).is_some() {
            return 5500.0;
        }
        if code == MASTER && ctx.hand().iter().any(|c| ctx.view_data(c).is_monster()) {
            return 4000.0;
        }
        support::body_score(self, ctx, code)
    }
}
impl Strategy for Zombie {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            MASTER => 2200,
            GOBLIN => 1900,
            TURTLE => 1600,
            MEZUKI => 1800,
            PLAGUE => 1700,
            IL_BLUD => 2400,
            REAPER => 1500,
            LIFE => 2200,
            FOOLISH => 1900,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = support::flip(t, &[21502796]) {
            return Some(i);
        }
        let ctx = t.ctx;
        if let Some(i) = t.activate(FOOLISH) {
            return t.pick(i);
        }
        if ctx.deck_size(ctx.me) > 6 {
            if let Some(i) = t.activate_from(TROOPER, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        if ctx.free_monster_zones(ctx.me) > 0 {
            for code in [LIFE, MEZUKI, IL_BLUD, MASTER] {
                if let Some(i) = t.activate(code) {
                    // Engine legality supplies valid targets; keep Master from paying
                    // a useful hand monster for a weak blocker without a Synchro partner.
                    let target = ctx
                        .graveyard(ctx.me)
                        .into_iter()
                        .filter(|c| ctx.view_data(c).race & races::ZOMBIE != 0)
                        .max_by(|a, b| {
                            self.revive_score(&ctx, a.code.unwrap_or(0))
                                .total_cmp(&self.revive_score(&ctx, b.code.unwrap_or(0)))
                        });
                    if let Some(c) = target {
                        if code != MASTER
                            || self.revive_score(&ctx, c.code.unwrap_or(0)) >= 2000.0
                            || ctx.in_hand(MEZUKI)
                            || ctx.in_hand(PLAGUE)
                        {
                            return t.pick(i);
                        }
                    }
                }
            }
        }
        if tactics::synchro_with_tuner(self, &ctx, 2).is_some() {
            if let Some(i) = t.activate_from(PLAGUE, Location::Graveyard) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, IL_BLUD)
                if c.at().map(|a| a.location) == Some(Location::MonsterZone) =>
            {
                Some(4500.0)
            }
            (_, IL_BLUD) => None,
            (ChoiceKind::NormalSummon, MASTER) => Some(
                if ctx
                    .graveyard(ctx.me)
                    .iter()
                    .any(|c| ctx.view_data(c).race & races::ZOMBIE != 0)
                {
                    3300.0
                } else {
                    1800.0
                },
            ),
            (ChoiceKind::NormalSummon, PLAGUE)
                if tactics::synchro_with_tuner(self, &ctx, 2).is_some() =>
            {
                Some(4000.0)
            }
            (ChoiceKind::NormalSummon, PLAGUE) => None,
            (ChoiceKind::NormalSummon, GOBLIN)
                if ctx
                    .monsters(ctx.me)
                    .iter()
                    .any(|c| ctx.view_data(c).is_tuner()) =>
            {
                Some(3500.0)
            }
            (ChoiceKind::SetMonster, GOBLIN | TURTLE | REAPER) => Some(1700.0),
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.ctx.canonical(t.choice(i).code()?) {
            TURTLE | GOBLIN => Response::new(120.0),
            MASTER | MEZUKI | PLAGUE | IL_BLUD | LIFE | FOOLISH => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let code = ctx.canonical(m.code?);
        // Zombie Master may legally revive from either public Graveyard.
        if t.decision.hint == Hint::SpecialSummon {
            return Some(self.revive_score(&ctx, code));
        }
        if m.at.controller != ctx.me {
            return None;
        }
        let source = t.memory.last_activated.map(|k| ctx.canonical(k));
        if t.decision.hint == Hint::ToGraveyard {
            return Some(if code == MEZUKI {
                5000.0
            } else if code == PLAGUE {
                4500.0
            } else if m.at.location == Location::Deck && code == GOBLIN {
                3000.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::AddToHand && source == Some(GOBLIN) {
            return Some(if code == MASTER && !ctx.in_hand(MASTER) {
                5000.0
            } else if code == PLAGUE && !ctx.in_hand(PLAGUE) {
                4000.0
            } else {
                2000.0
            });
        }
        if t.decision.hint == Hint::SynchroMaterial && code == GOBLIN && !ctx.monsters_banished() {
            return Some(1000.0);
        }
        support::material_score(&ctx, m, t.decision.hint)
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
