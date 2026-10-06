//! "Claudi-oh's Verdict": Skill Drain beatdown behind a wall of Traps.
//!
//! Skill Drain switches every monster effect off, and the deck is built to
//! be the better deck once it has: Beast King Barbaros keeps 3000 ATK
//! without Tributes, Chainsaw Insect and Goblin Attack Force lose their
//! drawbacks, and what the opponent Summons is only as good as its printed
//! ATK.  Gene-Warped Warwolf, Thunder King Rai-Oh and Doomcaliber Knight are
//! 1900 ATK and more on any board, and the era's Traps do the rest: Solemn
//! Judgment and Warning, Bottomless Trap Hole, Dimensional Prison, Mirror
//! Force, Torrential Tribute.
//!
//! Skill Drain goes face-up at the first chance (measured: waiting for a
//! board it helps on wins less), and decides how good a Normal Summon is.

use crate::agent::{Response, Strategy, Turn};
use crate::cards::types;
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Location};

pub const DECK: &str = "Claudi-oh's Verdict";

const BARBAROS: u32 = 78651105;
const RAI_OH: u32 = 71564252;
const DOOMCALIBER: u32 = 78700060;
const WARWOLF: u32 = 69247929;
const GOBLIN_ATTACK_FORCE: u32 = 78658564;
const CHAINSAW_INSECT: u32 = 77252217;
const SKILL_DRAIN: u32 = 82732705;

#[derive(Clone, Default)]
pub struct Verdict;

impl Verdict {
    fn want_skill_drain(ctx: &Ctx) -> bool {
        !ctx.effects_drained() && ctx.my_lp() > 2000
    }

    /// Skill Drain is Set: face-up by their next turn.
    fn drain_coming(ctx: &Ctx) -> bool {
        ctx.spell_traps(ctx.me).iter().any(|c| ctx.is(c, SKILL_DRAIN))
    }
}

impl Strategy for Verdict {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            SKILL_DRAIN => 2800,
            BARBAROS => 2600,
            CHAINSAW_INSECT | GOBLIN_ATTACK_FORCE => 2200,
            WARWOLF => 2000,
            RAI_OH => 1950,
            DOOMCALIBER => 1900,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if Self::want_skill_drain(&t.ctx) {
            if let Some(i) = t.activate_from(SKILL_DRAIN, Location::SpellTrapZone) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let drained = ctx.effects_drained();
        let coming = Self::drain_coming(&ctx);
        Some(match (choice.kind, code) {
            // Without Tributes: 1900 ATK, 3000 under Skill Drain.
            (ChoiceKind::NormalSummon, BARBAROS) => Some(if drained { 3200.0 } else { 2400.0 }),
            (ChoiceKind::SetMonster, BARBAROS) => None,
            // 2400 ATK and a card for the opponent each time it battles;
            // 2300 ATK and to Defense Position, 0 DEF, after it attacks.
            (ChoiceKind::NormalSummon, CHAINSAW_INSECT) => Some(if drained { 2800.0 } else if coming { 2300.0 } else { 1900.0 }),
            (ChoiceKind::NormalSummon, GOBLIN_ATTACK_FORCE) => Some(if drained { 2700.0 } else if coming { 2250.0 } else { 1850.0 }),
            (ChoiceKind::NormalSummon, RAI_OH) => Some(if drained { 1900.0 } else { 2350.0 }),
            (ChoiceKind::NormalSummon, DOOMCALIBER) => Some(if drained { 1900.0 } else { 2100.0 }),
            (ChoiceKind::NormalSummon, WARWOLF) => Some(2000.0),
            _ => return None,
        })
    }

    /// Beast King Barbaros without Tributes, never for two of our monsters.
    fn option(&self, t: &Turn) -> Option<usize> {
        t.choices().find(|(_, c)| c.description == (BARBAROS as u64) << 20).map(|(i, _)| i)
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let from_field = choice.at().map_or(false, |a| a.location.is_field());
        Some(match code {
            SKILL_DRAIN if Self::want_skill_drain(&ctx) => Response::new(35.0),
            SKILL_DRAIN => Response::no(),
            // Rai-Oh for the Special Summon of one monster.
            RAI_OH if from_field => Response::new(60.0),
            _ => return None,
        })
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let data = t.ctx.data(t.ctx.canonical(code));
        Some(data.is_trap() || data.is(types::QUICKPLAY))
    }
}
