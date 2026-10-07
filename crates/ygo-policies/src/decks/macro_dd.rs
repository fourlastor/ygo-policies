//! D.D. Border Patrol: banish instead of sending to the Graveyard, recycle
//! Survivor/Scout Plane as Monarch tributes, and protect the replacement effect.
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::knowledge::{
    BANISHER_OF_THE_RADIANCE as BANISHER, DD_SCOUT_PLANE as SCOUT, DD_SURVIVOR as SURVIVOR,
    DIMENSIONAL_FISSURE as FISSURE, MACRO_COSMOS as MACRO,
};
use crate::model::{Choice, ChoiceKind, Hint, Location, Member, Phase, Position};
use crate::staples;

pub const DECK: &str = "D.D. Border Patrol";
const CAIUS: u32 = 9748752;
const RAIZA: u32 = 73125233;
const WARRIOR_LADY: u32 = 7572887;
const ASSAILANT: u32 = 70074904;
const CYBER_DRAGON: u32 = 70095154;
const FORTRESS: u32 = 79229522;
const REINFORCEMENT: u32 = 32807846;
const RETURN: u32 = 27174286;

#[derive(Clone, Default)]
pub struct MacroDd;

impl MacroDd {
    fn monarch_target(ctx: &Ctx) -> bool {
        !ctx.monsters(ctx.opp).is_empty() || !ctx.spell_traps(ctx.opp).is_empty()
    }

    fn return_worthwhile(ctx: &Ctx) -> bool {
        if !ctx.my_turn() || ctx.phase() != Some(Phase::Main1) || !ctx.attack_locks().is_empty() {
            return false;
        }
        let mut attacks: Vec<_> = ctx
            .banished(ctx.me)
            .iter()
            .filter(|c| ctx.view_data(c).is_monster())
            .map(|c| ctx.view_data(c).attack)
            .collect();
        attacks.sort_unstable_by(|a, b| b.cmp(a));
        let damage: i32 = attacks.iter().take(ctx.free_monster_zones(ctx.me)).sum();
        let ours: i32 = ctx
            .monsters(ctx.me)
            .iter()
            .filter(|c| ctx.can_attack(c))
            .map(|c| c.attack)
            .sum();
        (damage + ours >= ctx.opp_lp() && ctx.monsters(ctx.opp).is_empty())
            || (attacks
                .iter()
                .filter(|&&a| a > ctx.opp_best_attack())
                .count()
                >= 2
                && damage >= 4000)
    }

    fn lady_worthwhile(ctx: &Ctx) -> bool {
        let (Some(a), Some(b)) = (ctx.battle_attacker(), ctx.battle_target()) else {
            return false;
        };
        let (ours, theirs) = if a.at.controller == ctx.me {
            (a, b)
        } else {
            (b, a)
        };
        ctx.is(ours, WARRIOR_LADY)
            && (ctx.battle_proof(theirs)
                || ctx.battle_stat(theirs) >= ours.attack
                || ctx.threat(theirs) >= 2400)
    }
}

impl Strategy for MacroDd {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            MACRO | FISSURE => {
                if ctx.monsters_banished() {
                    1600
                } else {
                    2600
                }
            }
            CAIUS | RAIZA => 2500,
            FORTRESS => 3000,
            CYBER_DRAGON => 2100,
            SURVIVOR => 1800,
            WARRIOR_LADY | ASSAILANT => 1700,
            BANISHER => {
                if ctx.monsters_banished() {
                    1600
                } else {
                    2300
                }
            }
            SCOUT => 700,
            REINFORCEMENT => 1900,
            RETURN => 1400,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Preserve Cyber Dragon's empty-field window before a Normal Summon.
        if let Some(i) = t.find(
            ChoiceKind::SpecialSummon,
            Some(CYBER_DRAGON),
            Some(Location::Hand),
        ) {
            return t.pick(i);
        }
        if !ctx.monsters_banished() {
            for code in [FISSURE, MACRO] {
                if let Some(i) = t.activate(code) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.activate(REINFORCEMENT) {
            return t.pick(i);
        }
        if Self::return_worthwhile(&ctx) {
            if let Some(i) = t.activate(RETURN) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, CAIUS | RAIZA) if Self::monarch_target(&ctx) => {
                let fodder = ctx.monsters(ctx.me).iter().any(|c| {
                    (ctx.monsters_banished()
                        && c.position.face_up
                        && (ctx.is(c, SURVIVOR) || ctx.is(c, SCOUT)))
                        || value(self, &ctx, c.code, Some(c)) <= 1800
                });
                fodder.then_some(3200.0)
            }
            (_, CAIUS | RAIZA | CYBER_DRAGON) => None,
            (ChoiceKind::NormalSummon, SURVIVOR) => Some(if ctx.monsters_banished() {
                2400.0
            } else {
                1800.0
            }),
            (ChoiceKind::NormalSummon, BANISHER) => Some(if ctx.monsters_banished() {
                1600.0
            } else {
                2600.0
            }),
            (ChoiceKind::NormalSummon, SCOUT) if ctx.monsters_banished() => Some(1000.0),
            (ChoiceKind::SetMonster, SCOUT) if !ctx.monsters_banished() => Some(200.0),
            (_, SCOUT) => None,
            (ChoiceKind::NormalSummon, WARRIOR_LADY | ASSAILANT) => {
                Some(if ctx.opp_best_attack() >= 2400 {
                    2300.0
                } else {
                    1700.0
                })
            }
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        if t.ctx.canonical(c.code()?) == FORTRESS {
            return Some(t.ctx.monsters(t.ctx.opp).iter().any(|m| {
                m.position.face_up && t.ctx.view_data(m).race & crate::cards::races::MACHINE != 0
            }));
        }
        None
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let code = t.ctx.canonical(t.choice(index).code()?);
        Some(match code {
            MACRO if !t.ctx.monsters_banished() => Response::new(45.0),
            MACRO => Response::no(),
            RETURN if Self::return_worthwhile(&t.ctx) => Response::new(40.0),
            RETURN => Response::no(),
            SURVIVOR | SCOUT | CAIUS | RAIZA => Response::new(120.0),
            WARRIOR_LADY if Self::lady_worthwhile(&t.ctx) => Response::new(65.0),
            WARRIOR_LADY | FISSURE | REINFORCEMENT => Response::no(),
            _ => return None,
        })
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        match t.decision.subject.map(|k| t.ctx.canonical(k)) {
            Some(WARRIOR_LADY) => Some(Self::lady_worthwhile(&t.ctx)),
            _ => None,
        }
    }

    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let worth = value(self, &ctx, Some(code), None) as f64;
        if matches!(t.decision.hint, Hint::Release | Hint::Tribute) {
            let returns = ctx.monsters_banished()
                && (code == SCOUT
                    || (code == SURVIVOR && ctx.card(m.at).map_or(false, |c| c.position.face_up)));
            return Some(if returns { 3000.0 - worth } else { -worth });
        }
        if t.memory.last_activated.map(|k| ctx.canonical(k)) == Some(staples::ALLURE_OF_DARKNESS)
            && t.decision.hint == Hint::Banish
        {
            // Scout returns from any banishment; Survivor requires leaving a face-up field.
            return Some(if code == SCOUT { 5000.0 } else { -worth });
        }
        if t.decision.hint == Hint::AddToHand && m.at.location == Location::Deck {
            return Some(if code == SURVIVOR && ctx.monsters_banished() {
                4000.0
            } else if code == WARRIOR_LADY && ctx.opp_best_attack() >= 2400 {
                3500.0
            } else {
                worth
            });
        }
        if t.decision.hint == Hint::SpecialSummon && t.memory.last_activated == Some(RETURN) {
            return Some(ctx.data(code).attack as f64);
        }
        None
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        if matches!(t.ctx.canonical(code), SURVIVOR | SCOUT) && t.ctx.phase() == Some(Phase::End) {
            Some(Position::FACE_UP_DEFENSE)
        } else {
            None
        }
    }
}
