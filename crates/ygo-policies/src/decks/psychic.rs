//! Mind Over Matter: trade expendable Psychics for threats, use Teleporter
//! and Teleport to assemble Synchros, and replenish LP with Psychic bosses.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "Mind Over Matter";
const TELEPORTER: u32 = 1834753;
const COMMANDER: u32 = 21454943;
const PROTECTOR: u32 = 85060248;
const JUMPER: u32 = 52430902;
const KREBONS: u32 = 59575539;
const DESTRUCTO: u32 = 11232355;
const DOCTOR: u32 = 22171591;
const MASTER: u32 = 96782886;
const SNAIL: u32 = 58453942;
const TELEPORT: u32 = 67723438;
const WELL: u32 = 28741524;
const KINESIS: u32 = 32180819;
const FUSION: u32 = 36484016;
const LAB: u32 = 85668449;
const OVERLOAD: u32 = 82633308;
const LIFETRANCER: u32 = 45379225;
const THOUGHT: u32 = 70780151;
const AXON: u32 = 40101111;
#[derive(Clone, Default)]
pub struct Psychic;
impl Psychic {
    fn exchange(&self, t: &Turn) -> Option<Response> {
        let ctx = t.ctx;
        if ctx.my_lp() <= 1800 {
            return None;
        }
        let ours = ctx
            .monsters(ctx.me)
            .into_iter()
            .filter(|c| {
                c.position.face_up
                    && ctx.view_data(c).race & races::PSYCHIC != 0
                    && !ctx.is(c, JUMPER)
            })
            .min_by_key(|c| value(self, &ctx, c.code, Some(c)))?;
        let theirs = ctx
            .monsters(ctx.opp)
            .into_iter()
            .filter(|c| c.position.face_up && ctx.reaches(c, JUMPER, true, false))
            .max_by_key(|c| ctx.threat(c))?;
        (ctx.threat(theirs) > value(self, &ctx, ours.code, Some(ours)) + 700)
            .then(|| Response::targeting(80.0, vec![ours.at, theirs.at]))
    }
    fn commander(t: &Turn) -> Response {
        let ctx = t.ctx;
        if !ctx.phase().map_or(false, |p| p.is_damage_step()) || ctx.my_lp() <= 700 {
            return Response::no();
        }
        let (Some(a), Some(b)) = (ctx.battle_attacker(), ctx.battle_target()) else {
            return Response::no();
        };
        let (ours, theirs) = if a.at.controller == ctx.me {
            (a, b)
        } else {
            (b, a)
        };
        let difference = ctx.battle_stat(theirs) - ctx.battle_stat(ours);
        if ctx.view_data(ours).race & races::PSYCHIC != 0 && difference >= 0 && difference < 100 {
            Response::new(80.0)
        } else {
            Response::no()
        }
    }
}
impl Strategy for Psychic {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            TELEPORTER => 3000,
            JUMPER => 1600,
            COMMANDER => 1700,
            KREBONS => 1600,
            MASTER => 1600,
            PROTECTOR => 1000,
            AXON => 3500,
            THOUGHT => 3000,
            LIFETRANCER => 2600,
            TELEPORT => 2400,
            WELL => 2100,
            LAB => 1200,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if ctx.my_lp() > 3000 && ctx.free_monster_zones(ctx.me) >= 2 {
            if let Some(i) = t.activate(TELEPORTER) {
                return t.pick(i);
            }
        }
        if let (Some(i), Some(r)) = (t.activate(JUMPER), self.exchange(t)) {
            return t.pick_targeting(i, r.intent);
        }
        if ctx.my_lp() > 2500 {
            if let Some(i) = t.activate(DESTRUCTO) {
                if let Some(c) = ctx
                    .spell_traps(ctx.opp)
                    .into_iter()
                    .filter(|c| !c.position.face_up && ctx.reaches(c, DESTRUCTO, true, true))
                    .max_by_key(|c| ctx.threat(c))
                {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
            if let Some(i) = t.activate(KINESIS) {
                if let Some(c) = support::target(&ctx, KINESIS, true) {
                    return t.pick_targeting(i, vec![c.at]);
                }
            }
        }
        if let Some(i) = t.activate(LIFETRANCER) {
            return t.pick(i);
        }
        if ctx
            .graveyard(ctx.me)
            .iter()
            .any(|c| ctx.view_data(c).is_extra())
        {
            if let Some(i) = t.activate(FUSION) {
                return t.pick(i);
            }
        }
        if !ctx.monsters(ctx.me).is_empty() {
            if let Some(i) = t.activate(TELEPORT) {
                return t.pick(i);
            }
        }
        if ctx.my_lp() > 2500
            && ctx
                .monsters(ctx.me)
                .iter()
                .any(|c| !ctx.view_data(c).is_tuner())
        {
            if let Some(i) = t.activate(WELL) {
                return t.pick(i);
            }
        }
        if !ctx.face_up_on_field(ctx.me, LAB)
            && ctx
                .hand()
                .iter()
                .filter(|c| ctx.view_data(c).race & races::PSYCHIC != 0)
                .count()
                >= 2
        {
            if let Some(i) = t.activate(LAB) {
                return t.pick(i);
            }
        }
        if ctx.my_lp() > 2000
            && ctx
                .monsters(ctx.me)
                .iter()
                .any(|c| ctx.is(c, DOCTOR) || ctx.is(c, PROTECTOR))
        {
            if let Some(i) = t.activate(MASTER) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, TELEPORTER) if ctx.my_lp() > 3000 => Some(4500.0),
            (_, TELEPORTER) => None,
            (ChoiceKind::NormalSummon, COMMANDER | KREBONS | JUMPER | MASTER) => {
                Some(support::body_score(self, &ctx, code))
            }
            (ChoiceKind::SetMonster, DOCTOR | PROTECTOR) => Some(1800.0),
            (ChoiceKind::NormalSummon, PROTECTOR | DOCTOR) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        let c = t.choice(i);
        Some(match ctx.canonical(c.code()?) {
            DOCTOR if ctx.my_lp() > 1600 => Response::new(120.0),
            KREBONS
                if ctx.my_lp() > 1600
                    && ctx
                        .incoming_attack()
                        .map_or(false, |(a, b)| ctx.attack_hurts(a, b)) =>
            {
                Response::new(80.0)
            }
            COMMANDER => Self::commander(t),
            THOUGHT if t.hostile_top().matches(|_| true) && ctx.my_lp() > 1800 => {
                Response::new(90.0)
            }
            THOUGHT if ctx.obs.chain.is_empty() => Response::new(120.0),
            OVERLOAD => Response::new(40.0),
            TELEPORT
                if !ctx.my_turn()
                    && ctx.monsters(ctx.me).is_empty()
                    && ctx.incoming_attack().is_some() =>
            {
                Response::new(70.0)
            }
            FUSION if c.at().map(|a| a.location) == Some(Location::Graveyard) => {
                Response::new(120.0)
            }
            DOCTOR | KREBONS | THOUGHT | TELEPORTER | TELEPORT | WELL | KINESIS | FUSION | LAB
            | JUMPER | MASTER | LIFETRANCER | SNAIL | DESTRUCTO => Response::no(),
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
            if src == Some(TELEPORTER) {
                let selected_tuner = t.decision.selected.iter().any(|a| {
                    t.decision
                        .choices
                        .iter()
                        .flat_map(|c| c.card.iter().chain(c.members.iter()))
                        .any(|m| m.at == *a && ctx.data(m.code.unwrap_or(0)).is_tuner())
                });
                return Some(if (code == COMMANDER) != selected_tuner {
                    5000.0
                } else {
                    2000.0
                });
            }
            if src == Some(TELEPORT) && !ctx.my_turn() {
                return Some(if code == KREBONS { 5000.0 } else { 1000.0 });
            }
            return Some(support::body_score(self, &ctx, code));
        }
        if t.decision.hint == Hint::AddToHand && src == Some(DOCTOR) {
            return Some(
                if code == TELEPORTER && !ctx.in_hand(TELEPORTER) && ctx.my_lp() > 3000 {
                    5000.0
                } else {
                    support::body_score(self, &ctx, code)
                },
            );
        }
        if t.decision.hint == Hint::Release && src == Some(MASTER) {
            return Some(if code == DOCTOR || code == PROTECTOR {
                4000.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        if t.decision.hint == Hint::FusionMaterial || t.decision.hint == Hint::Banish {
            return Some(if m.at.location == Location::Graveyard {
                2000.0
            } else {
                -value(self, &ctx, Some(code), None) as f64
            });
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn attack_trick(&self, ctx: &Ctx, c: &CardView) -> i32 {
        if ctx.view_data(c).race & races::PSYCHIC != 0
            && ctx.my_lp() > 700
            && ctx.face_up_on_field(ctx.me, COMMANDER)
        {
            100
        } else {
            0
        }
    }
    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        support::extra_allowed(t, c)
    }
}
