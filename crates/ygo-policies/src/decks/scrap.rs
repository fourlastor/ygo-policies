//! Scrap Renaissance: Scrapstorm turns a Tuner into a draw and Chimera,
//! Chimera/Golem rebuild the board, and the dragons spend disposable cards.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member, Position};
use crate::staples;
pub const DECK: &str = "Scrap Renaissance";
const CHIMERA: u32 = 56746202;
const BEAST: u32 = 19139516;
const GOBLIN: u32 = 83135907;
const GOLEM: u32 = 82012319;
const YARD: u32 = 1050684;
const STORM: u32 = 48445393;
const TWIN: u32 = 50278554;
const TROOPER: u32 = 85087012;
const RYKO: u32 = 21502796;
const CYBER: u32 = 70095154;
const FOOLISH: u32 = 81439173;
#[derive(Clone, Default)]
pub struct Scrap;
impl Scrap {
    fn tuner(ctx: &Ctx) -> bool {
        ctx.graveyard(ctx.me)
            .iter()
            .any(|c| ctx.is(c, BEAST) || ctx.is(c, GOBLIN))
    }
    fn storm(t: &Turn) -> Option<Response> {
        let ctx = t.ctx;
        if ctx.monsters_banished() {
            return None;
        }
        ctx.monsters(ctx.me)
            .into_iter()
            .filter(|c| c.position.face_up && (ctx.is(c, BEAST) || ctx.is(c, GOBLIN)))
            .min_by_key(|c| c.attack)
            .map(|c| Response::targeting(65.0, vec![c.at]))
    }
    fn fodder<'a>(&self, ctx: &Ctx<'a>) -> Option<&'a crate::model::CardView> {
        ctx.monsters(ctx.me)
            .into_iter()
            .chain(ctx.spell_traps(ctx.me))
            .filter(|c| !ctx.is(c, staples::SCRAP_DRAGON) && !ctx.is(c, TWIN))
            .min_by_key(|c| {
                if ctx.is(c, GOBLIN) || ctx.is(c, BEAST) {
                    200
                } else {
                    value(self, ctx, c.code, Some(c))
                }
            })
    }
}
impl Strategy for Scrap {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            staples::SCRAP_DRAGON => 3200,
            TWIN => 3300,
            GOLEM => 2600,
            CHIMERA => 2400,
            BEAST => 1600,
            GOBLIN => 1300,
            STORM => 2100,
            YARD => 1800,
            FOOLISH => 1600,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(CYBER), Some(Location::Hand)) {
            return t.pick(i);
        }
        for code in [staples::SCRAP_DRAGON, TWIN] {
            if let Some(i) = t.activate(code) {
                let mut theirs: Vec<_> = ctx
                    .monsters(ctx.opp)
                    .into_iter()
                    .chain(ctx.spell_traps(ctx.opp))
                    .filter(|c| ctx.reaches(c, code, true, code == staples::SCRAP_DRAGON))
                    .collect();
                theirs.sort_by_key(|c| -ctx.threat(c));
                let n = if code == TWIN { 2 } else { 1 };
                if theirs.len() >= n {
                    if let Some(ours) = self.fodder(&ctx) {
                        let price = if ctx.is(ours, BEAST) || ctx.is(ours, GOBLIN) {
                            300
                        } else {
                            value(self, &ctx, ours.code, Some(ours))
                        };
                        if theirs.iter().take(n).map(|c| ctx.threat(c)).sum::<i32>() > price + 400 {
                            let mut intent = vec![ours.at];
                            intent.extend(theirs.iter().take(n).map(|c| c.at));
                            return t.pick_targeting(i, intent);
                        }
                    }
                }
            }
        }
        if ctx.free_monster_zones(ctx.me) > 0 {
            if let Some(i) = t.activate_from(GOLEM, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        if let (Some(i), Some(r)) = (t.activate(STORM), Self::storm(t)) {
            return t.pick_targeting(i, r.intent);
        }
        if let Some(i) = t.activate(YARD) {
            return t.pick(i);
        }
        if !Self::tuner(&ctx) || ctx.count_in(ctx.me, Location::Graveyard, CHIMERA) == 0 {
            if let Some(i) = t.activate(FOOLISH) {
                return t.pick(i);
            }
        }
        if ctx.deck_size(ctx.me) > 6 {
            if let Some(i) = t.activate_from(TROOPER, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, CHIMERA) => {
                Some(if Self::tuner(&ctx) { 4500.0 } else { 1700.0 })
            }
            (ChoiceKind::NormalSummon, BEAST) => Some(
                support::body_score(self, &ctx, code)
                    + if ctx.in_hand(STORM) { 1800.0 } else { 0.0 },
            ),
            (ChoiceKind::NormalSummon, GOBLIN) if ctx.in_hand(STORM) => Some(2900.0),
            (ChoiceKind::NormalSummon, GOBLIN)
                if support::body_score(self, &ctx, code) > 2000.0 =>
            {
                Some(3000.0)
            }
            (ChoiceKind::SetMonster, GOBLIN) => Some(1300.0),
            (ChoiceKind::NormalSummon, GOBLIN) => None,
            (ChoiceKind::NormalSummon, GOLEM)
                if Self::tuner(&ctx)
                    && ctx
                        .monsters(ctx.me)
                        .iter()
                        .any(|c| ctx.is(c, GOBLIN) || ctx.is(c, RYKO)) =>
            {
                Some(3500.0)
            }
            (_, GOLEM) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let c = t.choice(i);
        Some(match t.ctx.canonical(c.code()?) {
            BEAST | GOBLIN | CHIMERA => Response::new(120.0),
            staples::SCRAP_DRAGON | TWIN
                if c.at().map(|a| a.location) == Some(Location::Graveyard) =>
            {
                Response::new(120.0)
            }
            STORM => Self::storm(t).unwrap_or_else(Response::no),
            GOLEM | YARD | FOOLISH => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let source = t.memory.last_activated.map(|k| ctx.canonical(k));
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(if source == Some(STORM) && code == CHIMERA {
                6000.0
            } else if !Self::tuner(&ctx) && code == BEAST {
                5000.0
            } else if code == GOLEM {
                4000.0
            } else {
                0.0
            });
        }
        if matches!(t.decision.hint, Hint::AddToHand | Hint::ReturnToHand)
            && m.at.location != Location::Hand
        {
            return Some(if code == CHIMERA {
                5000.0
            } else if code == BEAST {
                3500.0
            } else {
                value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(if code == GOLEM {
                6500.0
            } else {
                support::body_score(self, &ctx, code)
            });
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn option(&self, t: &Turn) -> Option<usize> {
        t.choices()
            .find(|(_, c)| c.description == ((GOLEM as u64) << 20) + 1)
            .map(|(i, _)| i)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        (t.ctx.canonical(code) == GOBLIN).then_some(Position::FACE_UP_DEFENSE)
    }
    fn yes_no(&self, t: &Turn) -> Option<bool> {
        (t.decision.subject == Some(RYKO)).then(|| {
            !t.ctx.monsters(t.ctx.opp).is_empty() || !t.ctx.spell_traps(t.ctx.opp).is_empty()
        })
    }
}
