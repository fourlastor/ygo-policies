//! Crown of Venom: Snake Rain powers the kings, Damage = Reptile deploys
//! Vennominon, and Offering can turn it into Vennominaga with Rise set.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Crown of Venom";
const KING: u32 = 72677437;
const QUEEN: u32 = 8062132;
const SNAKE: u32 = 73899015;
const SERPENT: u32 = 36278828;
const BOA: u32 = 9284723;
const GARDNA: u32 = 43002864;
const VASKII: u32 = 16886617;
const RAIN: u32 = 17189677;
const SWAMP: u32 = 54306223;
const TERRAFORM: u32 = 73628505;
const FOOLISH: u32 = 81439173;
const RISE: u32 = 16067089;
const DAMAGE: u32 = 44584775;
const OFFERING: u32 = 93217231;
#[derive(Clone, Default)]
pub struct Venom;
impl Venom {
    fn reptiles(ctx: &Ctx) -> usize {
        ctx.graveyard(ctx.me)
            .iter()
            .filter(|c| ctx.view_data(c).race & races::REPTILE != 0)
            .count()
    }
    fn offering(&self, t: &Turn) -> Response {
        let ctx = t.ctx;
        let rise_ready = !ctx.my_turn()
            && ctx
                .spell_traps(ctx.me)
                .iter()
                .any(|c| ctx.is(c, RISE) && !c.position.face_up);
        let ours = ctx
            .monsters(ctx.me)
            .into_iter()
            .filter(|c| {
                c.position.face_up
                    && ctx.view_data(c).race & races::REPTILE != 0
                    && !ctx.is(c, QUEEN)
            })
            .min_by_key(|c| {
                if rise_ready && ctx.is(c, KING) {
                    0
                } else {
                    value(self, &ctx, c.code, Some(c))
                }
            });
        let mut theirs: Vec<_> = ctx
            .monsters(ctx.opp)
            .into_iter()
            .chain(ctx.spell_traps(ctx.opp))
            .filter(|c| ctx.reaches(c, OFFERING, true, true))
            .collect();
        theirs.sort_by_key(|c| -ctx.threat(c));
        if let Some(ours) = ours {
            if theirs.len() >= 2
                && (rise_ready && ctx.is(ours, KING)
                    || ctx.threat(theirs[0]) + ctx.threat(theirs[1])
                        > value(self, &ctx, ours.code, Some(ours)) + 600)
            {
                return Response::targeting(85.0, vec![ours.at, theirs[0].at, theirs[1].at]);
            }
        }
        Response::no()
    }
}
impl Strategy for Venom {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            KING => (Self::reptiles(ctx) as i32 * 500 + 800).max(2000),
            QUEEN => 5000,
            RAIN => 3000,
            SWAMP => 2000,
            RISE => 2400,
            DAMAGE => 2500,
            OFFERING => 2300,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.activate(RAIN) {
            return t.pick(i);
        }
        if !ctx.face_up_on_field(ctx.me, SWAMP) {
            if let Some(i) = t.activate(SWAMP) {
                return t.pick(i);
            }
            if !ctx.in_hand(SWAMP) {
                if let Some(i) = t.activate(TERRAFORM) {
                    return t.pick(i);
                }
            }
        }
        if ctx.count_in(ctx.me, Location::Graveyard, KING) == 0 {
            if let Some(i) = t.activate(FOOLISH) {
                return t.pick(i);
            }
        }
        if !ctx.face_up_on_field(ctx.me, DAMAGE) {
            if let Some(i) = t.activate(DAMAGE) {
                return t.pick(i);
            }
        }
        for code in [VASKII, SERPENT, SNAKE, BOA] {
            if let Some(i) = t.activate(code) {
                if let Some(c) = ctx
                    .monsters(ctx.opp)
                    .into_iter()
                    .filter(|c| c.position.face_up && ctx.reaches(c, code, true, code == VASKII))
                    .max_by_key(|c| ctx.threat(c))
                {
                    if code == SERPENT
                        || code == VASKII
                        || !ctx.main1()
                        || c.attack
                            >= ctx
                                .monsters(ctx.me)
                                .iter()
                                .map(|c| c.attack)
                                .max()
                                .unwrap_or(0)
                    {
                        return t.pick_targeting(i, vec![c.at]);
                    }
                }
            }
        }
        if let Some(i) = t.activate(OFFERING) {
            let r = self.offering(t);
            if r.score > 0.0 {
                return t.pick_targeting(i, r.intent);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, KING) if Self::reptiles(&ctx) >= 5 => Some(4000.0),
            (_, KING | QUEEN | VASKII | BOA) => None,
            (ChoiceKind::SetMonster, GARDNA | SERPENT) => Some(1800.0),
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        let c = t.choice(i);
        Some(match ctx.canonical(c.code()?) {
            KING | QUEEN | RISE | GARDNA => Response::new(130.0),
            SWAMP if c.at().map(|a| a.location) == Some(Location::SpellTrapZone) => {
                Response::new(120.0)
            }
            DAMAGE => Response::new(115.0),
            OFFERING => self.offering(t),
            RAIN | SWAMP | TERRAFORM | FOOLISH | SNAKE | SERPENT | BOA | VASKII => Response::no(),
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
        if t.decision.hint == Hint::SpecialSummon {
            return Some(match code {
                QUEEN => 8000.0,
                KING => 6000.0,
                VASKII => 3500.0,
                _ => ctx.data(code).attack as f64,
            });
        }
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(
                if code == KING && ctx.count_in(ctx.me, Location::Graveyard, KING) == 0 {
                    6000.0
                } else if code == QUEEN {
                    -10000.0
                } else {
                    if code == BOA {
                        4000.0
                    } else {
                        3000.0
                    }
                },
            );
        }
        if t.decision.hint == Hint::Discard {
            return Some(
                if code == KING && ctx.count_in(ctx.me, Location::Graveyard, KING) == 0 {
                    4000.0
                } else if code == QUEEN {
                    1000.0
                } else {
                    -value(self, &ctx, Some(code), None) as f64
                },
            );
        }
        if t.decision.hint == Hint::Banish && matches!(src, Some(KING | QUEEN)) {
            return Some(if code == KING { -3000.0 } else { 1000.0 });
        }
        None
    }
}
