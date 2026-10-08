//! Tidal Assembly: deploy Coelacanth, recruit complementary Fish materials,
//! and use Oyster tokens/Fishborg to continue after the first Synchro.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::attributes;
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
use crate::tactics;
pub const DECK: &str = "Tidal Assembly";
const KING: u32 = 88307361;
const OYSTER: u32 = 83239739;
const EEL: u32 = 37953640;
const BORG: u32 = 93369354;
const DIVA: u32 = 78868119;
const GRIZZLY: u32 = 57839750;
const GILLMAN: u32 = 42463414;
const SNOWMAN: u32 = 91133740;
const ANGLER: u32 = 92084010;
const WAVE: u32 = 51562916;
const MORAY: u32 = 22123627;
const HAZARD: u32 = 49669730;
const SALVAGE: u32 = 96947648;
const FOOLISH: u32 = 81439173;
const GUNGNIR: u32 = 65749035;
#[derive(Clone, Default)]
pub struct Fish;
impl Strategy for Fish {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            KING => 3500,
            OYSTER => 1900,
            EEL => 1500,
            BORG => 1300,
            DIVA => 2300,
            ANGLER => 1500,
            WAVE => 2200,
            MORAY | SALVAGE => 1800,
            GUNGNIR => 3000,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = support::flip(t, &[SNOWMAN]) {
            return Some(i);
        }
        let ctx = t.ctx;
        if ctx.free_monster_zones(ctx.me) >= 2 {
            if let Some(i) = t.activate_from(KING, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        if ctx.in_hand(KING)
            && ctx
                .monsters(ctx.me)
                .iter()
                .any(|c| c.position.face_up && ctx.view_data(c).attribute & attributes::WATER != 0)
            && !ctx
                .monsters(ctx.me)
                .iter()
                .any(|c| ctx.is(c, KING) || ctx.view_data(c).is_extra())
        {
            if let Some(i) = t.activate(WAVE) {
                return t.pick(i);
            }
        }
        if ctx.monsters(ctx.me).is_empty() {
            if let Some(i) = t.activate(HAZARD) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(SALVAGE) {
            return t.pick(i);
        }
        if !ctx.in_hand(KING) || !ctx.in_hand(WAVE) {
            if let Some(i) = t.activate(MORAY) {
                return t.pick(i);
            }
        }
        if ctx.graveyard(ctx.me).iter().all(|c| !ctx.is(c, BORG)) {
            if let Some(i) = t.activate(FOOLISH) {
                return t.pick(i);
            }
        }
        if tactics::synchro_with_tuner(self, &ctx, 1).is_some() {
            if let Some(i) = t.activate_from(BORG, Location::Graveyard) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(GUNGNIR) {
            if let Some(c) = support::target(&ctx, GUNGNIR, true) {
                return t.pick_targeting(i, vec![c.at]);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, KING) => Some(5500.0),
            (ChoiceKind::NormalSummon, DIVA) => Some(3300.0),
            (ChoiceKind::NormalSummon, ANGLER) if ctx.in_hand(KING) => Some(2400.0),
            (ChoiceKind::NormalSummon, OYSTER | EEL | BORG | GILLMAN) => {
                Some(support::body_score(self, &ctx, code))
            }
            (ChoiceKind::SetMonster, SNOWMAN | GRIZZLY) => Some(1700.0),
            (ChoiceKind::NormalSummon, SNOWMAN) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let code = t.ctx.canonical(t.choice(i).code()?);
        Some(match code {
            KING if t.hostile_top().matches(|_| true) => Response::new(90.0),
            OYSTER | DIVA | GRIZZLY => Response::new(120.0),
            KING | WAVE | MORAY | HAZARD | SALVAGE | BORG | FOOLISH | GUNGNIR => Response::no(),
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
        if t.decision.hint == Hint::ReturnToHand && src == Some(SALVAGE) {
            return Some(
                support::body_score(self, &ctx, code)
                    + if code == DIVA && !ctx.in_hand(DIVA) {
                        3000.0
                    } else {
                        0.0
                    },
            );
        }
        if t.decision.hint == Hint::SpecialSummon {
            if src == Some(KING) {
                let selected_tuners = t
                    .decision
                    .selected
                    .iter()
                    .filter(|a| {
                        t.decision
                            .choices
                            .iter()
                            .flat_map(|c| c.members.iter())
                            .any(|m| m.at == **a && ctx.data(m.code.unwrap_or(0)).is_tuner())
                    })
                    .count();
                let tuners = selected_tuners
                    + ctx
                        .monsters(ctx.me)
                        .iter()
                        .filter(|c| ctx.view_data(c).is_tuner())
                        .count();
                return Some(match code {
                    EEL if tuners == 0 => 6000.0,
                    OYSTER => 5000.0,
                    ANGLER => 3500.0,
                    EEL | BORG => {
                        if tuners < 2 {
                            4000.0
                        } else {
                            500.0
                        }
                    }
                    _ => 2000.0,
                });
            }
            return Some(match code {
                KING => 7000.0,
                GILLMAN if src == Some(DIVA) => 4500.0,
                DIVA if src == Some(DIVA) => 3500.0,
                _ => support::body_score(self, &ctx, code),
            });
        }
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(if code == BORG { 5000.0 } else { 0.0 });
        }
        if t.decision.hint == Hint::Discard && code == BORG && !ctx.monsters_banished() {
            return Some(4000.0);
        }
        if t.decision.hint == Hint::ToDeck && src == Some(MORAY) {
            return Some(
                if code == KING && ctx.count_in(ctx.me, Location::Hand, KING) == 1 {
                    -6000.0
                } else {
                    -value(self, &ctx, Some(code), None) as f64
                },
            );
        }
        if t.decision.hint == Hint::SynchroMaterial && code == OYSTER && !ctx.monsters_banished() {
            return Some(1000.0);
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
    fn yes_no(&self, t: &Turn) -> Option<bool> {
        (t.decision.subject == Some(WAVE)).then_some(true)
    }
}
