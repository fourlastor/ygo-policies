//! Heaven's Rebuttal: protect Artemis/Meltiel, exchange Counter Traps for
//! cards and removal, then finish with Harvest Angel or Van'Dalgyon.
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::{attributes, types};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
use crate::staples;

pub const DECK: &str = "Heaven's Rebuttal";
const ARTEMIS: u32 = 32296881;
const MELTIEL: u32 = 49905576;
const HARVEST: u32 = 85399281;
const ZERADIAS: u32 = 12171659;
const VANDALGYON: u32 = 24857466;
const HONEST: u32 = 37742478;
const SANCTUARY: u32 = 56433456;
const PUNISHMENT: u32 = 81066751;
const WRATH: u32 = 49010598;
const BRIBE: u32 = 77538567;
const MAGIC_DRAIN: u32 = 59344077;

#[derive(Clone, Default)]
pub struct CounterFairy;

impl CounterFairy {
    fn sanctuary(ctx: &Ctx) -> bool {
        ctx.face_up_on_field(ctx.me, SANCTUARY) || ctx.face_up_on_field(ctx.opp, SANCTUARY)
    }

    fn engine(ctx: &Ctx) -> bool {
        !ctx.effects_drained()
            && (ctx.face_up_on_field(ctx.me, ARTEMIS) || ctx.face_up_on_field(ctx.me, MELTIEL))
    }

    fn honest(ctx: &Ctx) -> bool {
        if !ctx.phase().map_or(false, |p| p.is_damage_step()) {
            return false;
        }
        let (Some(a), Some(b)) = (ctx.battle_attacker(), ctx.battle_target()) else {
            return false;
        };
        let (ours, theirs) = if a.at.controller == ctx.me {
            (a, b)
        } else {
            (b, a)
        };
        ctx.view_data(ours).attribute & attributes::LIGHT != 0
            && ours.position.attack
            && (ours.attack <= ctx.battle_stat(theirs)
                || (ctx.my_turn()
                    && ours.attack + theirs.attack - ctx.battle_stat(theirs) >= ctx.opp_lp()))
    }
}

impl Strategy for CounterFairy {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            ARTEMIS => 2600,
            MELTIEL if Self::sanctuary(ctx) => 2700,
            MELTIEL => 1900,
            VANDALGYON => 2800,
            HARVEST => 2000,
            SANCTUARY => {
                if Self::sanctuary(ctx) {
                    300
                } else {
                    2200
                }
            }
            ZERADIAS => {
                if Self::sanctuary(ctx) || ctx.in_hand(SANCTUARY) {
                    1400
                } else {
                    2300
                }
            }
            HONEST => 1900,
            PUNISHMENT if Self::sanctuary(ctx) => 2300,
            PUNISHMENT => 900,
            WRATH | BRIBE => 1800,
            MAGIC_DRAIN => 1500,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        if !Self::sanctuary(&t.ctx) {
            if let Some(i) = t.activate(SANCTUARY) {
                return t.pick(i);
            }
            if !t.ctx.in_hand(SANCTUARY) {
                if let Some(i) = t.activate_from(ZERADIAS, Location::Hand) {
                    return t.pick(i);
                }
            }
        }
        // Return a previously summoned Honest to its job as a hand trap.
        if let Some(i) = t.activate_from(HONEST, Location::MonsterZone) {
            return t.pick(i);
        }
        None
    }

    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, ARTEMIS) => Some(3000.0),
            (ChoiceKind::NormalSummon, MELTIEL) => Some(if Self::sanctuary(&ctx) {
                2900.0
            } else {
                1800.0
            }),
            (ChoiceKind::NormalSummon, HARVEST) => Some(2000.0),
            (ChoiceKind::NormalSummon, ZERADIAS) if Self::sanctuary(&ctx) => Some(1900.0),
            (_, VANDALGYON | HONEST | ZERADIAS) => None,
            (ChoiceKind::SetMonster, _) => None,
            _ => return None,
        })
    }

    fn attack_trick(&self, ctx: &Ctx, attacker: &CardView) -> i32 {
        if ctx.in_hand(HONEST) && ctx.view_data(attacker).attribute & attributes::LIGHT != 0 {
            ctx.opp_best_attack()
        } else {
            0
        }
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let c = t.choice(index);
        let code = ctx.canonical(c.code()?);
        let hostile = t.hostile_top();
        Some(match code {
            VANDALGYON | HARVEST => Response::new(120.0),
            HONEST if c.at().map(|a| a.location) == Some(Location::Hand) && Self::honest(&ctx) => {
                Response::new(95.0)
            }
            HONEST | ZERADIAS | SANCTUARY => Response::no(),
            PUNISHMENT if hostile.matches(|_| true) => Response::new(95.0),
            WRATH if hostile.matches(|link| ctx.data(link.code).is_monster()) => {
                Response::new(88.0)
            }
            MAGIC_DRAIN if hostile.matches(|link| ctx.data(link.code).is_spell()) => {
                Response::new(82.0)
            }
            BRIBE
                if hostile.matches(|link| {
                    !ctx.data(link.code).is_monster()
                        && (Self::engine(&ctx)
                            || link.targets.iter().any(|at| at.controller == ctx.me)
                            || matches!(
                                ctx.canonical(link.code),
                                staples::DARK_HOLE
                                    | staples::GIANT_TRUNADE
                                    | staples::MONSTER_REBORN
                                    | staples::MIRROR_FORCE
                                    | staples::TORRENTIAL_TRIBUTE
                            )
                            || ctx.my_lp() <= 2000)
                }) =>
            {
                Response::new(80.0)
            }
            PUNISHMENT | WRATH | MAGIC_DRAIN | BRIBE => Response::no(),
            _ => return None, // Solemns, Seven Tools and battle traps use shared tactics.
        })
    }

    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let source = t.memory.last_activated.map(|c| ctx.canonical(c));
        let worth = value(self, &ctx, Some(code), None) as f64;
        if t.decision.hint == Hint::Discard {
            return Some(
                if matches!(code, ZERADIAS | SANCTUARY) && Self::sanctuary(&ctx) {
                    2000.0
                } else if ctx.count_in(ctx.me, Location::Hand, code) > 1 {
                    1000.0 - worth
                } else {
                    -worth
                },
            );
        }
        if t.decision.hint == Hint::AddToHand && source == Some(staples::POT_OF_DUALITY) {
            return Some(
                worth
                    + if code == ARTEMIS
                        && !ctx.in_hand(ARTEMIS)
                        && !ctx.face_up_on_field(ctx.me, ARTEMIS)
                    {
                        3000.0
                    } else {
                        0.0
                    }
                    + if matches!(code, SANCTUARY | ZERADIAS)
                        && !Self::sanctuary(&ctx)
                        && !ctx.in_hand(SANCTUARY)
                    {
                        1500.0
                    } else {
                        0.0
                    },
            );
        }
        if t.decision.hint == Hint::SpecialSummon && source == Some(VANDALGYON) {
            return Some(worth + if code == ARTEMIS { 2000.0 } else { 0.0 });
        }
        None
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let d = t.ctx.data(code);
        Some(d.is_trap() || d.is(types::QUICKPLAY))
    }
}
