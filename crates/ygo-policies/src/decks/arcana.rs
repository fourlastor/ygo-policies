//! "Arcana Force Fortune": Fairies that toss a coin on every summon.
//!
//! Each Arcana Force monster gains a good or a bad effect from its coin:
//! The Chariot steals what it destroys or defects, The Emperor raises or
//! lowers every Arcana's ATK, The Lovers counts twice or blocks Tribute
//! Summons...  The deck's steady parts are around the coins: The Fool cannot
//! be destroyed by battle (a permanent wall when Set), the two EX Rulers
//! (4000/4000) come out of the hand by sending three of our monsters to the
//! Graveyard, and since every monster in the deck is a Fairy, Solidarity
//! gives them all 800 ATK.  Temperance discards itself to stop a battle's
//! damage; Nightmare's Steelcage stalls a stronger board.

use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Location};

pub const DECK: &str = "Arcana Force Fortune";

const THE_FOOL: u32 = 62892347;
const THE_MAGICIAN: u32 = 8396952;
const THE_EMPRESS: u32 = 35781051;
const THE_EMPEROR: u32 = 61175706;
const THE_LOVERS: u32 = 97574404;
const THE_CHARIOT: u32 = 34568403;
const TEMPERANCE: u32 = 60953118;
const THE_MOON: u32 = 97452817;
const LIGHT_RULER: u32 = 5861892;
const DARK_RULER: u32 = 69831560;
const SHINING_ANGEL: u32 = 95956346;
const DUNAMES_DARK_WITCH: u32 = 12493482;
const SECOND_COIN_TOSS: u32 = 36562627;
const GRACEFUL_DICE: u32 = 74137509;
const SOLIDARITY: u32 = 86780027;
const NIGHTMARES_STEELCAGE: u32 = 58775978;
const FISSURE: u32 = 66788016;
const REVERSAL_OF_FATE: u32 = 36690018;
const ARCANA_CALL: u32 = 99189322;
const SKULL_DICE: u32 = 126218;

#[derive(Default)]
pub struct Arcana;

impl Arcana {
    /// Our battle, and by how much our monster falls short of winning it.
    fn shortfall(ctx: &Ctx) -> Option<i32> {
        if ctx.phase().map_or(false, |p| !p.is_battle()) {
            return None;
        }
        let (attacker, target) = (ctx.battle_attacker()?, ctx.battle_target()?);
        let (ours, theirs) = if attacker.at.controller == ctx.me { (attacker, target) } else { (target, attacker) };
        let our_stat = if ours.position.attack { ours.attack } else { ours.defense };
        let their_stat = ctx.battle_stat(theirs);
        (ours.at.controller == ctx.me && theirs.position.face_up).then(|| their_stat - our_stat)
    }
}

impl Strategy for Arcana {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            LIGHT_RULER | DARK_RULER => 4000,
            THE_MOON => 2800,
            TEMPERANCE => 2400,
            SOLIDARITY => 2000,
            DUNAMES_DARK_WITCH => 1800,
            THE_CHARIOT | NIGHTMARES_STEELCAGE | FISSURE => 1700,
            THE_LOVERS => 1600,
            THE_EMPEROR | SHINING_ANGEL => 1400,
            THE_EMPRESS | SKULL_DICE | GRACEFUL_DICE => 1300,
            THE_MAGICIAN => 1100,
            SECOND_COIN_TOSS | REVERSAL_OF_FATE | ARCANA_CALL => 600,
            THE_FOOL => 500,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Every monster in the deck is a Fairy: Solidarity is +800 for all.
        if !ctx.face_up_on_field(ctx.me, SOLIDARITY) {
            let fairies = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).race & races::FAIRY != 0);
            if fairies {
                if let Some(i) = t.activate_from(SOLIDARITY, Location::Hand) {
                    return t.pick(i);
                }
            }
        }
        if !ctx.face_up_on_field(ctx.me, SECOND_COIN_TOSS) {
            if let Some(i) = t.activate_from(SECOND_COIN_TOSS, Location::Hand) {
                return t.pick(i);
            }
        }
        if ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up) {
            if let Some(i) = t.activate(FISSURE) {
                return t.pick(i);
            }
        }
        // A Set Arcana flips (and tosses its coin) once it can win a battle:
        // with ATK equal to DEF, the generic rule never flips it.
        if ctx.main1() && t.has(ChoiceKind::EnterBattle) {
            let threat = ctx.opp_best_attack();
            let opp_empty = ctx.monsters(ctx.opp).is_empty();
            let flip = t.find_where(|c| {
                c.kind == ChoiceKind::ChangePosition
                    && c.at().and_then(|at| ctx.card(at)).map_or(false, |card| {
                        let data = ctx.view_data(card);
                        !card.position.face_up && !ctx.is(card, THE_FOOL) && data.attack > 0 && (opp_empty || data.attack > threat)
                    })
            });
            if let Some(i) = flip {
                return t.pick(i);
            }
        }
        // A stronger board than ours: nobody attacks for two of their turns.
        let outclassed = !ctx.monsters(ctx.opp).is_empty() && ctx.opp_best_attack() > ctx.my_best_attack() + 300;
        if outclassed && ctx.own_attack_locks().is_empty() {
            if let Some(i) = t.activate(NIGHTMARES_STEELCAGE) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        Some(match (choice.kind, code) {
            // Set, The Fool is a wall nothing destroys in battle; face-up it
            // is stuck in Attack Position with 0 ATK.
            (ChoiceKind::SetMonster, THE_FOOL) => Some(1400.0),
            (ChoiceKind::NormalSummon, THE_FOOL) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // A Ruler costs three of our monsters: worth it for small ones.
            LIGHT_RULER | DARK_RULER => {
                let mut ours: Vec<i32> = ctx.monsters(ctx.me).iter().map(|c| value(self, &ctx, None, Some(c))).collect();
                ours.sort_unstable();
                ours.len() >= 3 && ours.iter().take(3).sum::<i32>() <= 4500
            }
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        Some(match code {
            // Temperance from the hand: no damage from a big hit.
            TEMPERANCE if choice.at().map_or(false, |a| a.location == Location::Hand) => match ctx.incoming_attack() {
                Some((attacker, target)) => {
                    let damage = match target {
                        None => attacker.attack,
                        Some(t) if t.position.attack => attacker.attack - t.attack,
                        Some(_) => 0,
                    };
                    if damage >= 2000 || damage >= ctx.my_lp() { Response::new(50.0) } else { Response::no() }
                }
                None => Response::no(),
            },
            // Dice: +/-100..600 swings a battle we lose by a little.
            GRACEFUL_DICE | SKULL_DICE => match Self::shortfall(&ctx) {
                Some(gap) if (0..300).contains(&gap) => Response::new(40.0),
                _ => Response::no(),
            },
            // Coin knowledge is not tracked: the redo and the reversals stay unused.
            REVERSAL_OF_FATE | ARCANA_CALL => Response::no(),
            _ => return None,
        })
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let code = t.ctx.canonical(code);
        Some(t.ctx.data(code).is_trap() && !matches!(code, REVERSAL_OF_FATE | ARCANA_CALL))
    }
}
