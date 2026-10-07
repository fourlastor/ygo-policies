//! Chain Reaction: extend chains with different draw/burn cards, then
//! collect Accumulated Fortune and a late Chain Strike. Defense buys reloads.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Location};
pub const DECK: &str = "Chain Reaction";
const STRIKE: u32 = 91623717;
const FORTUNE: u32 = 98444741;
const DESSERTS: u32 = 24068492;
const BARREL: u32 = 27053506;
const RECKLESS: u32 = 37576645;
const POISON: u32 = 8842266;
const JAR: u32 = 83968380;
const LEGACY: u32 = 30461781;
const LAVA: u32 = 102380;
const TRIO: u32 = 29843091;
const WALL: u32 = 67095270;
#[derive(Clone, Default)]
pub struct ChainBurn;
impl ChainBurn {
    fn burn(&self, t: &Turn, code: u32) -> i32 {
        let ctx = t.ctx;
        match code {
            DESSERTS => 500 * ctx.monsters(ctx.opp).len() as i32,
            BARREL => {
                200 * (ctx.hand_size(ctx.opp) as i32
                    + ctx.monsters(ctx.opp).len() as i32
                    + ctx.spell_traps(ctx.opp).len() as i32)
            }
            STRIKE => 400 * (ctx.obs.chain.len() + 1) as i32,
            POISON => 800,
            _ => 0,
        }
    }
}
impl Strategy for ChainBurn {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            STRIKE => 2500,
            FORTUNE | RECKLESS => 2300,
            DESSERTS | BARREL => 2100,
            LAVA => 2000,
            POISON => 1400,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(LAVA), Some(Location::Hand)) {
            return t.pick(i);
        }
        for code in [TRIO, POISON] {
            if let Some(i) = t.activate(code) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, _: &Turn, _: &Choice) -> Option<Option<f64>> {
        Some(None)
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let code = t.ctx.canonical(t.choice(i).code()?);
        let damage = self.burn(t, code);
        Some(match code {
            JAR | LEGACY if t.ctx.deck_size(t.ctx.me) > 1 => Response::new(145.0),
            RECKLESS if t.ctx.deck_size(t.ctx.me) > 2 => Response::new(140.0),
            TRIO => Response::new(150.0),
            DESSERTS | BARREL
                if damage >= 1000
                    || damage >= t.ctx.opp_lp()
                    || (!t.ctx.obs.chain.is_empty() && damage > 0) =>
            {
                Response::new(130.0)
            }
            POISON => Response::new(125.0),
            STRIKE => Response::new(115.0),
            FORTUNE => Response::new(110.0),
            WALL if t
                .ctx
                .incoming_attack()
                .map_or(false, |(a, b)| b.is_none() && a.attack >= 1000) =>
            {
                Response::new(95.0)
            }
            JAR | LEGACY | RECKLESS | DESSERTS | BARREL | WALL => Response::no(),
            _ => return support::stall_chain(t, i),
        })
    }
    fn allow_repeated_chain(&self, t: &Turn, i: usize) -> bool {
        matches!(
            t.choice(i).code(),
            Some(STRIKE | FORTUNE | JAR | LEGACY | RECKLESS | DESSERTS | BARREL | POISON | TRIO)
        )
    }
    fn option(&self, t: &Turn) -> Option<usize> {
        let code = t.memory.last_activated?;
        let wanted = if code == POISON {
            ((POISON as u64) << 20) + 1
        } else if code == LEGACY {
            (LEGACY as u64) << 20
        } else {
            return None;
        };
        t.choices()
            .find(|(_, c)| c.description == wanted)
            .map(|(i, _)| i)
    }
    fn wants_battle(&self, _: &Turn) -> Option<bool> {
        Some(false)
    }
}
