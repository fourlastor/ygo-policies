//! The Still Gaze: zero ATK enables Viper theft, Vaskii tributes and Hydra draws.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Position};
pub const DECK: &str = "The Still Gaze";
const NAGA: u32 = 79491903;
const GARDNA: u32 = 43002864;
const VIPER: u32 = 42303365;
const VASKII: u32 = 16886617;
const MEDUSA: u32 = 89810518;
const HYDRA: u32 = 60634565;
const SPAWN: u32 = 21179143;
const POISON: u32 = 90576781;
#[derive(Clone, Default)]
pub struct Reptilianne;
impl Strategy for Reptilianne {
    fn value(&self, ctx: &Ctx, k: u32) -> Option<i32> {
        Some(match k {
            NAGA => 1800,
            GARDNA => 1700,
            VASKII => 3300,
            HYDRA => {
                2100 + 900
                    * ctx
                        .monsters(ctx.opp)
                        .iter()
                        .filter(|c| c.position.face_up && c.attack == 0)
                        .count() as i32
            }
            MEDUSA => 2800,
            _ => return support::extra_value(k),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !ctx.effects_drained()
            && ctx
                .monsters(ctx.opp)
                .iter()
                .any(|c| c.position.face_up && c.attack == 0)
        {
            if let Some(i) = t.find(
                ChoiceKind::SpecialSummon,
                Some(HYDRA),
                Some(Location::Extra),
            ) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.find(
            ChoiceKind::SpecialSummon,
            Some(VASKII),
            Some(Location::Hand),
        ) {
            return t.pick(i);
        }
        for code in [VASKII, MEDUSA, POISON] {
            if let Some(i) = t.activate(code) {
                if let Some(c) = ctx
                    .monsters(ctx.opp)
                    .into_iter()
                    .filter(|c| ctx.reaches(c, code, true, code == VASKII))
                    .filter(|c| match code {
                        MEDUSA => c.position.face_up && c.attack > 0,
                        POISON => !c.position.attack,
                        _ => c.position.face_up,
                    })
                    .max_by_key(|c| ctx.threat(c))
                {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
        }
        if let Some(i) = t.activate(SPAWN) {
            if ctx.in_hand(VASKII)
                || ctx.monsters(ctx.me).is_empty()
                || ctx
                    .monsters(ctx.me)
                    .iter()
                    .any(|c| ctx.view_data(c).is_tuner())
            {
                return t.pick(i);
            }
        }
        if ctx.in_hand(SPAWN) {
            if let Some(i) = t.activate(81439173) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let k = c.code()?;
        Some(match (c.kind, k) {
            (ChoiceKind::NormalSummon, VIPER)
                if t.ctx
                    .monsters(t.ctx.opp)
                    .iter()
                    .any(|c| c.position.face_up && c.attack == 0) =>
            {
                Some(6000.0)
            }
            (ChoiceKind::SetMonster, NAGA | GARDNA) => Some(2000.0),
            (ChoiceKind::NormalSummon, NAGA | GARDNA | VIPER) => {
                Some(support::body_score(self, &t.ctx, k))
            }
            (ChoiceKind::NormalSummon, MEDUSA) => Some(3300.0),
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.choice(i).code()? {
            GARDNA | VIPER | HYDRA | 16909657 | 14943837 => Response::new(130.0),
            93217231 if support::target(&t.ctx, 93217231, true).is_some() => Response::new(80.0),
            VASKII | MEDUSA | SPAWN | POISON => Response::no(),
            _ => return support::chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let k = m.code?;
        // Vaskii may use either side's zero-ATK monsters; spend theirs first.
        if matches!(t.decision.hint, Hint::Release | Hint::Tribute)
            && m.at.location == Location::MonsterZone
        {
            return Some(if m.at.controller == ctx.opp {
                10000.0 + ctx.threat(ctx.card(m.at)?) as f64
            } else {
                -(value(self, &ctx, Some(k), None) as f64)
            });
        }
        if m.at.controller != ctx.me {
            return None;
        }
        if t.decision.hint == Hint::AddToHand {
            return Some(match k {
                VASKII if !ctx.in_hand(VASKII) && ctx.in_hand(SPAWN) => 6000.0,
                VIPER
                    if ctx
                        .monsters(ctx.opp)
                        .iter()
                        .any(|c| c.position.face_up && c.attack == 0) =>
                {
                    5500.0
                }
                NAGA if ctx.monsters(ctx.me).is_empty() => 4500.0,
                _ => value(self, &ctx, Some(k), None) as f64,
            });
        }
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(if k == GARDNA { 6000.0 } else { 0.0 });
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn position(&self, _: &Turn, k: u32) -> Option<Position> {
        matches!(k, NAGA | GARDNA).then_some(Position::FACE_UP_DEFENSE)
    }
    fn allow_reposition(&self, _: &Turn, c: &CardView) -> bool {
        c.code != Some(NAGA) || c.position.attack
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
}
