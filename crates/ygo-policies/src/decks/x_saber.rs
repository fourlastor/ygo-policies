//! "X-Sabers": EARTH Warriors and Beasts that swarm into Synchros.
//!
//! Two face-up X-Sabers let XX-Saber Faultroll (2400) come out of the hand,
//! and each turn it revives a Level 4 or lower X-Saber from the Graveyard.
//! The small ones keep the field full: Emmersblade floats into another from
//! the Deck, Ragigura returns one from the Graveyard to the hand, Garsem
//! searches when destroyed by an effect, Gardestrike comes out on an empty
//! field, Gottoms' Emergency Call brings two back.  The Tuners (Airbellum,
//! Fulhelmknight, Pashuul, Palomuro) make Hyunlei (destroys up to 3
//! Spells/Traps), Souza, or XX-Saber Gottoms.  Saber Slash destroys a
//! face-up card per Attack Position X-Saber; Saber Hole negates a Summon.

use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Phase, Position};
use crate::tactics;

pub const DECK: &str = "X-Sabers";

const FAULTROLL: u32 = 51808422;
const EMMERSBLADE: u32 = 42737833;
const AIRBELLUM: u32 = 90508760;
const PASHUUL: u32 = 23093604;
const GARSEM: u32 = 77330185;
const RAGIGURA: u32 = 87292536;
const GARDESTRIKE: u32 = 42024143;
const FULHELMKNIGHT: u32 = 78422252;
const GALAHAD: u32 = 50604950;
const PALOMURO: u32 = 96099959;
const COMMANDER_GOTTOMS: u32 = 53388413;
const SABER_SLASH: u32 = 11052544;
const EMERGENCY_CALL: u32 = 13504844;
const SABER_HOLE: u32 = 44901281;
const XX_SABER_GOTTOMS: u32 = 52352005;
const HYUNLEI: u32 = 2203790;
const SOUZA: u32 = 63612442;
const BLACK_ROSE_DRAGON: u32 = 73580471;
const SET_X_SABER: u16 = 0x100d;
const SET_SABER: u16 = 0xd;

#[derive(Default)]
pub struct XSaber;

impl XSaber {
    fn is_x_saber(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.in_set(SET_X_SABER)
    }

    fn x_sabers_up(ctx: &Ctx) -> usize {
        ctx.monsters(ctx.me).iter().filter(|c| c.position.face_up && Self::is_x_saber(ctx, c)).count()
    }

    /// A spare X-Saber to Tribute (Souza, XX-Saber Gottoms), by worth.
    fn spare(&self, ctx: &Ctx, except: u32) -> Option<i32> {
        ctx.monsters(ctx.me)
            .into_iter()
            .filter(|c| c.position.face_up && Self::is_x_saber(ctx, c) && !ctx.is(c, except))
            .map(|c| value(self, ctx, None, Some(c)))
            .min()
    }

    fn their_face_up_cards(ctx: &Ctx) -> usize {
        ctx.monsters(ctx.opp).iter().chain(ctx.spell_traps(ctx.opp).iter()).filter(|c| c.position.face_up).count()
    }
}

impl Strategy for XSaber {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        let their_backrow = ctx.spell_traps(ctx.opp).len() as i32;
        Some(match code {
            XX_SABER_GOTTOMS => 3100,
            SOUZA => 2500,
            FAULTROLL => 2400,
            HYUNLEI => 2300 + 200 * their_backrow.min(3),
            COMMANDER_GOTTOMS | GARDESTRIKE => 2100,
            EMERGENCY_CALL => 1900,
            GALAHAD | SABER_HOLE => 1800,
            AIRBELLUM | GARSEM | SABER_SLASH => 1600,
            EMMERSBLADE | FULHELMKNIGHT => 1500,
            RAGIGURA => 1200,
            PALOMURO => 1100,
            PASHUUL => 900,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Faultroll as soon as two X-Sabers stand, and its revival every turn.
        for code in [FAULTROLL, GARDESTRIKE] {
            if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(code), Some(Location::Hand)) {
                return t.pick(i);
            }
        }
        if ctx.free_monster_zones(ctx.me) > 0 {
            if let Some(i) = t.activate_from(FAULTROLL, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        // Saber Slash: one face-up card per Attack Position X-Saber (all theirs).
        let slashers = ctx.monsters(ctx.me).iter().filter(|c| c.position.face_up && c.position.attack && Self::is_x_saber(&ctx, c)).count();
        if slashers >= 1 && Self::their_face_up_cards(&ctx) >= slashers {
            if let Some(i) = t.activate(SABER_SLASH) {
                return t.pick(i);
            }
        }
        // Souza: a spare X-Saber for "destroy what it battles", before battle.
        if ctx.main1() && t.has(ChoiceKind::EnterBattle) {
            let souza = ctx.monsters(ctx.me).into_iter().find(|c| ctx.is(c, SOUZA) && ctx.can_attack(c));
            if let Some(souza) = souza {
                let walled = ctx.monsters(ctx.opp).iter().any(|c| ctx.battle_stat(c) >= souza.attack);
                if walled && self.spare(&ctx, SOUZA).map_or(false, |v| v <= 1800) {
                    if let Some(i) = t.activate_from(SOUZA, Location::MonsterZone) {
                        return t.pick(i);
                    }
                }
            }
        }
        // XX-Saber Gottoms: a small X-Saber for a random card from their hand.
        if !ctx.main1() && ctx.hand_size(ctx.opp) >= 2 && self.spare(&ctx, XX_SABER_GOTTOMS).map_or(false, |v| v <= 1300) {
            if let Some(i) = t.activate_from(XX_SABER_GOTTOMS, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let up = Self::x_sabers_up(&ctx);
        let faultroll = ctx.in_hand(FAULTROLL);
        let recoverable = ctx.graveyard(ctx.me).iter().any(|c| Self::is_x_saber(&ctx, c));
        Some(match (choice.kind, code) {
            // The second X-Saber is Faultroll's key.
            (ChoiceKind::NormalSummon, _) if faultroll && up == 1 && ctx.data(code).in_set(SET_X_SABER) && ctx.data(code).level <= 4 => {
                Some(2400.0 + ctx.data(code).attack as f64 / 10.0)
            }
            (ChoiceKind::NormalSummon, RAGIGURA) if recoverable => Some(1900.0),
            // Tuners only when they complete a Synchro.
            (ChoiceKind::NormalSummon, PASHUUL | PALOMURO) => {
                tactics::synchro_with_tuner(self, &ctx, ctx.data(code).level).map(|_| 2000.0)
            }
            (ChoiceKind::SetMonster, PASHUUL) => None,
            (ChoiceKind::SetMonster, EMMERSBLADE) if ctx.opp_best_attack() > 1300 => Some(1500.0),
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // Hyunlei destroys Spells/Traps: worth it only with some to hit.
            HYUNLEI => !ctx.spell_traps(ctx.opp).is_empty() || ctx.monsters(ctx.me).len() >= 3,
            BLACK_ROSE_DRAGON => ctx.field_strength(ctx.opp) > ctx.field_strength(ctx.me) + 2500,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let incoming = ctx.incoming_attack();
        Some(match code {
            // Two X-Sabers back from any Graveyard.
            EMERGENCY_CALL => {
                let graveyard = ctx.graveyard(ctx.me).iter().chain(ctx.graveyard(ctx.opp).iter()).filter(|c| Self::is_x_saber(&ctx, c)).count();
                let attacking = ctx.my_turn() && ctx.phase() == Some(Phase::Main1);
                let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
                let blockers = incoming.is_some() && ctx.monsters(ctx.me).len() <= 1;
                if graveyard < 2 || ctx.free_monster_zones(ctx.me) < 2 {
                    Response::no()
                } else if blockers {
                    Response::new(45.0)
                } else if attacking || end_of_their_turn {
                    Response::new(30.0)
                } else {
                    Response::no()
                }
            }
            // Offered only on a Summon while we control a face-up X-Saber.
            SABER_HOLE => match (t.hostile_top(), crate::staples::opponent_summoning(&ctx)) {
                (crate::agent::Hostile::No, Some((card, code))) => {
                    let data = ctx.data(code);
                    if card.attack.max(data.attack) >= 1900 || data.is_extra() { Response::new(82.0) } else { Response::no() }
                }
                _ => Response::no(),
            },
            // Once while face-up: stop an attack.
            FULHELMKNIGHT if choice.description & 0xf == 0 => match incoming {
                Some((attacker, target)) if ctx.attack_hurts(attacker, target) => Response::new(55.0),
                _ => Response::no(),
            },
            // Galahad: a spare Saber for the attack.
            GALAHAD => match incoming {
                Some((attacker, target)) if ctx.attack_hurts(attacker, target) && self.spare(&ctx, GALAHAD).map_or(false, |v| v <= 1300) => {
                    Response::new(45.0)
                }
                _ => Response::no(),
            },
            _ => return None,
        })
    }

    fn option(&self, t: &Turn) -> Option<usize> {
        // Souza: "destroy the monster it battles" is the first effect.
        let souza = t.memory.last_activated.map(|c| t.ctx.canonical(c)) == Some(SOUZA);
        souza.then(|| t.choices().find(|(_, c)| c.description & 0xf == 1).map(|(i, _)| i).unwrap_or(0))
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if member.at.controller != ctx.me {
            return None;
        }
        let code = member.code.map(|c| ctx.canonical(c))?;
        let worth = value(self, &ctx, Some(code), None) as f64;
        match t.decision.hint {
            // Tributes (Souza, XX-Saber Gottoms, Galahad): the smallest Saber.
            Hint::Release | Hint::Tribute if member.at.location == Location::MonsterZone => {
                let saber = ctx.data(code).in_set(SET_SABER);
                Some(if saber { -worth } else { -worth - 2000.0 })
            }
            _ => None,
        }
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        // Face-up in Defense Position, Pashuul burns us every turn.
        (t.ctx.canonical(code) == PASHUUL).then_some(Position::FACE_UP_ATTACK)
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let code = t.ctx.canonical(code);
        Some(t.ctx.data(code).is_trap())
    }
}
