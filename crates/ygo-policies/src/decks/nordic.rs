//! Road to Ragnarok: Hamster/Tanngnjostr recruit Nordic bodies, Guldfaxe
//! turns two Level-3 non-Tuners into Thor, and Valkyrie supplies Odin tokens.
use super::support;
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Position};
pub const DECK: &str = "Road to Ragnarok";
const GULD: u32 = 41788781;
const TAN: u32 = 14677495;
const RIS: u32 = 15394083;
const VALK: u32 = 40844552;
const LJOS: u32 = 40666140;
const HAMSTER: u32 = 5220687;
const RYKO: u32 = 21502796;
const CYBER: u32 = 70095154;
const GLEIPNIR: u32 = 14464864;
const LAEV: u32 = 89792713;
const THOR: u32 = 30604579;
const ODIN: u32 = 93483212;
#[derive(Clone, Default)]
pub struct Nordic;
impl Strategy for Nordic {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            THOR => 3900,
            ODIN => 4100,
            TAN => 2400,
            GULD => 2100,
            RIS => 1800,
            VALK => 2000,
            GLEIPNIR => 2300,
            _ => return support::extra_value(code),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = support::flip(t, &[21502796, 5220687]) {
            return Some(i);
        }
        for code in [CYBER, GULD] {
            if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(code), Some(Location::Hand)) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.find_where(|c| {
            c.kind == ChoiceKind::ChangePosition
                && c.code() == Some(TAN)
                && t.view(c).map_or(false, |c| !c.position.attack)
        }) {
            return t.pick(i);
        }
        for code in [THOR, ODIN] {
            if let Some(i) = t.activate_from(code, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(GLEIPNIR) {
            return t.pick(i);
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, VALK)
                if ctx.monsters(ctx.me).is_empty()
                    && ctx.spell_traps(ctx.me).is_empty()
                    && !ctx.monsters(ctx.opp).is_empty()
                    && ctx
                        .hand()
                        .iter()
                        .filter(|c| ctx.view_data(c).in_set(0x42))
                        .count()
                        >= 3 =>
            {
                Some(5000.0)
            }
            (ChoiceKind::NormalSummon, GULD | VALK | TAN | RIS | LJOS) => {
                Some(support::body_score(self, &ctx, code))
            }
            (ChoiceKind::SetMonster, TAN | HAMSTER | RIS) => Some(2400.0),
            (_, CYBER) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let c = t.choice(i);
        let code = t.ctx.canonical(c.code()?);
        Some(match code {
            TAN | RIS | VALK | LJOS | HAMSTER | GLEIPNIR => Response::new(130.0),
            THOR | ODIN
                if c.at().map(|a| a.location) == Some(Location::Graveyard)
                    || c.description == ((code as u64) << 20) + 2 =>
            {
                Response::new(130.0)
            }
            LAEV if support::target(&t.ctx, LAEV, true).is_some() => Response::new(90.0),
            THOR | ODIN | LAEV => Response::no(),
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
                TAN if src == Some(HAMSTER) => 6500.0,
                GULD if !ctx
                    .monsters(ctx.me)
                    .iter()
                    .any(|c| ctx.view_data(c).is_tuner()) =>
                {
                    5500.0
                }
                RIS => 4000.0,
                _ => support::body_score(self, &ctx, code),
            });
        }
        if t.decision.hint == Hint::AddToHand {
            return Some(match code {
                TAN if !ctx.in_hand(TAN) => 5500.0,
                GULD if !ctx.in_hand(GULD) => 4500.0,
                RIS => 3500.0,
                _ => value(self, &ctx, Some(code), None) as f64,
            });
        }
        if t.decision.hint == Hint::SynchroMaterial && matches!(code, THOR | ODIN) {
            return Some(-15000.0);
        }
        support::material_score(&ctx, m, t.decision.hint)
    }
    fn position(&self, _: &Turn, code: u32) -> Option<Position> {
        (code == TAN).then_some(Position::FACE_UP_DEFENSE)
    }
    fn allow_reposition(&self, _: &Turn, c: &CardView) -> bool {
        c.code != Some(TAN) || !c.position.attack
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
