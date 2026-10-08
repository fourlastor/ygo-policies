//! A Thousand Blades: concentrate equips on Ben Kei, use Maha Vailo as
//! backup, and protect the attacker. Armory waits until a body is established.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "A Thousand Blades";
const BEN: u32 = 84430950;
const MAHA: u32 = 93013676;
const KOTETSU: u32 = 73431236;
const CYBER: u32 = 70095154;
const ANGEL: u32 = 95956346;
const AXE: u32 = 40619825;
const MAGE: u32 = 83746708;
const UNITED: u32 = 56747793;
const BIGBANG: u32 = 61127349;
const CEAL: u32 = 95638658;
const ARMORY: u32 = 52105192;
const RETURN: u32 = 95281259;
#[derive(Clone, Default)]
pub struct Benkei;
impl Benkei {
    fn body<'a>(ctx: &Ctx<'a>) -> Option<&'a CardView> {
        ctx.monsters(ctx.me)
            .into_iter()
            .filter(|c| {
                c.position.face_up && (ctx.is(c, BEN) || ctx.is(c, MAHA) || ctx.is(c, CYBER))
            })
            .max_by_key(|c| {
                if ctx.is(c, BEN) {
                    10000 + c.attack
                } else {
                    c.attack
                }
            })
    }
}
impl Strategy for Benkei {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            BEN => 3200,
            MAHA => 2200,
            KOTETSU => 2000,
            AXE => 1800,
            MAGE => 1900,
            UNITED => 1900,
            BIGBANG => 1400,
            CEAL => 1000,
            ARMORY => 2000,
            RETURN => {
                if ctx.in_hand(BEN) {
                    500
                } else {
                    2300
                }
            }
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = support::flip(t, &[73431236]) {
            return Some(i);
        }
        let ctx = t.ctx;
        if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(CYBER), Some(Location::Hand)) {
            return t.pick(i);
        }
        if !ctx.in_hand(BEN) {
            if let Some(i) = t.activate(RETURN) {
                return t.pick(i);
            }
        }
        if !ctx.face_up_on_field(ctx.me, BEN)
            && ctx
                .hand()
                .iter()
                .any(|c| ctx.view_data(c).is(crate::cards::types::EQUIP))
        {
            if let Some(i) = t.find(ChoiceKind::NormalSummon, Some(BEN), Some(Location::Hand)) {
                return t.pick(i);
            }
        }
        if let Some(body) = Self::body(&ctx) {
            for code in [UNITED, MAGE, AXE, BIGBANG, CEAL] {
                if code == CEAL && (body.attack <= 1500 || ctx.monsters(ctx.opp).is_empty()) {
                    continue;
                }
                if let Some(i) = t.activate(code) {
                    return t.pick_targeting(i, vec![body.at]);
                }
            }
            if ctx.is(body, BEN) || ctx.is(body, MAHA) {
                if let Some(i) = t.activate(ARMORY) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.activate_from(support::HONEST, Location::MonsterZone) {
            return t.pick(i);
        }
        None
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        Some(match (c.kind, ctx.canonical(c.code()?)) {
            (ChoiceKind::NormalSummon, BEN)
                if ctx
                    .hand()
                    .iter()
                    .any(|c| ctx.view_data(c).is(crate::cards::types::EQUIP)) =>
            {
                Some(4500.0)
            }
            (ChoiceKind::NormalSummon, BEN) => Some(800.0),
            (ChoiceKind::SetMonster, BEN) => None,
            (ChoiceKind::NormalSummon, MAHA) => Some(2300.0),
            (ChoiceKind::SetMonster, KOTETSU) => Some(2200.0),
            (_, support::HONEST | CYBER) => None,
            _ => return None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        Some(match t.ctx.canonical(t.choice(i).code()?) {
            KOTETSU | ANGEL => Response::new(120.0),
            AXE | MAGE | UNITED | BIGBANG | CEAL | ARMORY | RETURN => Response::no(),
            _ => return support::stall_chain(t, i).or_else(|| support::chain(t, i)),
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
                BEN => 6000.0,
                MAGE => 3500.0,
                UNITED => 3000.0,
                AXE => 2800.0,
                BIGBANG => 2000.0,
                CEAL => 1000.0,
                _ => 0.0,
            });
        }
        if t.decision.hint == Hint::SpecialSummon && code == MAHA {
            return Some(4000.0);
        }
        None
    }
    fn attack_trick(&self, ctx: &Ctx, c: &CardView) -> i32 {
        support::honest_trick(ctx, c)
    }
}
