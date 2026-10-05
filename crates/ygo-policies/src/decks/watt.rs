//! "Watt Grid": small Thunder monsters that go around the opponent's.
//!
//! The Watts are 700-1200 ATK, so they do not fight: Wattgiraffe,
//! Wattpheasant and the Synchros (Wattchimera, Watthydra) attack directly,
//! Wattkey lets every Watt do it for a turn, Wattwoodpecker and Wattsquirrel
//! attack twice, and each direct hit carries a rider (no activations,
//! a banished monster, a discard).  On defence the Watts punish being
//! destroyed: Wattlemur skips the opponent's next Battle Phase, Wattdragonfly
//! floats into the next Watt, Wattfox shuts off activations; Wattcastle
//! shrinks whatever attacks a Watt.  Wattcannon burns 600 on each summon,
//! Wattcine turns battle damage into Life Points, Burden of the Mighty
//! shrinks the opponent's big monsters.

use crate::agent::{Outcome, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Location, Phase, Position};
use crate::tactics;

pub const DECK: &str = "Watt Grid";

const WATTKID: u32 = 27324313;
const WATTWOODPECKER: u32 = 12296376;
const WATTGIRAFFE: u32 = 402568;
const WATTPHEASANT: u32 = 81896771;
const WATTSQUIRREL: u32 = 23274061;
const WATTLEMUR: u32 = 45801022;
const WATTDRAGONFLY: u32 = 97885363;
const WATTMOLE: u32 = 32548609;
const WATTFOX: u32 = 46897277;
const WATTBERYX: u32 = 5554990;
const WATTKIWI: u32 = 24996659;
const WATTCASTLE: u32 = 58924378;
const WATTKEY: u32 = 53193261;
const WATTCUBE: u32 = 65612454;
const WATTJUSTMENT: u32 = 84428023;
const WATTCINE: u32 = 1834107;
const RECYCLING_BATTERIES: u32 = 99995595;
const BURDEN_OF_THE_MIGHTY: u32 = 44947065;
const WATTKEEPER: u32 = 32061744;
const WATTCANNON: u32 = 95084054;
const WATTCHIMERA: u32 = 2772236;
const WATTHYDRA: u32 = 29765339;
const SET_WATT: u16 = 0xe;

/// Watts that attack directly on their own.
const DIRECT: [u32; 4] = [WATTGIRAFFE, WATTPHEASANT, WATTCHIMERA, WATTHYDRA];

#[derive(Default)]
pub struct Watt;

impl Watt {
    fn is_watt(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.in_set(SET_WATT)
    }

    fn is_thunder(ctx: &Ctx, card: &CardView) -> bool {
        ctx.view_data(card).race & races::THUNDER != 0
    }

    fn direct(ctx: &Ctx, card: &CardView) -> bool {
        DIRECT.iter().any(|k| ctx.is(card, *k))
    }

    /// Our face-up Watts that could still attack.
    fn attackers<'a>(ctx: &Ctx<'a>) -> Vec<&'a CardView> {
        ctx.monsters(ctx.me).into_iter().filter(|c| c.position.face_up && ctx.can_attack(c) && Self::is_watt(ctx, c)).collect()
    }

    /// Where Wattjustment (+800, effects negated) costs least: a Level 3 or
    /// lower Thunder without an effect worth keeping.
    fn justment_target<'a>(ctx: &Ctx<'a>) -> Option<&'a CardView> {
        let rank = |c: &CardView| -> i32 {
            if ctx.is(c, WATTKID) {
                3
            } else if ctx.is(c, WATTBERYX) || ctx.is(c, WATTKIWI) {
                2
            } else if ctx.is(c, WATTMOLE) || ctx.is(c, WATTSQUIRREL) || ctx.is(c, WATTWOODPECKER) {
                0
            } else {
                1
            }
        };
        ctx.monsters(ctx.me)
            .into_iter()
            .filter(|c| c.position.face_up && c.position.attack && Self::is_thunder(ctx, c) && c.level <= 3 && ctx.can_attack(c))
            .max_by_key(|c| rank(c))
    }
}

impl Strategy for Watt {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            WATTHYDRA | WATTCHIMERA => 1900,
            WATTGIRAFFE => 1700,
            WATTPHEASANT => 1650,
            WATTKEY => 1500,
            WATTWOODPECKER | WATTCASTLE | WATTJUSTMENT | BURDEN_OF_THE_MIGHTY => 1400,
            WATTLEMUR | WATTDRAGONFLY | WATTCUBE | RECYCLING_BATTERIES | WATTCANNON => 1300,
            WATTSQUIRREL | WATTFOX | WATTKIWI | WATTKEEPER => 1200,
            WATTBERYX | WATTCINE => 1100,
            WATTMOLE => 1000,
            WATTKID => 900,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !ctx.face_up_on_field(ctx.me, WATTCASTLE) {
            if let Some(i) = t.activate_from(WATTCASTLE, Location::Hand) {
                return t.pick(i);
            }
        }
        let big = ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up && c.level >= 4);
        if big && !ctx.face_up_on_field(ctx.me, BURDEN_OF_THE_MIGHTY) {
            if let Some(i) = t.activate_from(BURDEN_OF_THE_MIGHTY, Location::Hand) {
                return t.pick(i);
            }
        }
        let thunder_up = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && Self::is_thunder(&ctx, c));
        if thunder_up && !ctx.face_up_on_field(ctx.me, WATTCINE) {
            if let Some(i) = t.activate_from(WATTCINE, Location::Hand) {
                return t.pick(i);
            }
        }
        let recyclable = ctx
            .graveyard(ctx.me)
            .iter()
            .filter(|c| Self::is_thunder(&ctx, c) && ctx.view_data(c).is_monster() && ctx.view_data(c).attack <= 1500)
            .count();
        if recyclable >= 2 {
            if let Some(i) = t.activate(RECYCLING_BATTERIES) {
                return t.pick(i);
            }
        }
        if ctx.main1() && t.has(ChoiceKind::EnterBattle) {
            if let Some(holder) = Self::justment_target(&ctx) {
                if let Some(i) = t.activate(WATTJUSTMENT) {
                    return t.pick_targeting(i, vec![holder.at]);
                }
            }
            // Wattcube on the attacker that connects: a direct one first.
            let holder = Self::attackers(&ctx).into_iter().max_by_key(|c| (Self::direct(&ctx, c), c.attack));
            if let Some(holder) = holder {
                if let Some(i) = t.activate_from(WATTCUBE, Location::Hand) {
                    return t.pick_targeting(i, vec![holder.at]);
                }
            }
            // Wattkey: every Watt goes around their monsters this turn.
            if !ctx.monsters(ctx.opp).is_empty() {
                let stuck: i32 = Self::attackers(&ctx).iter().filter(|c| !Self::direct(&ctx, c)).map(|c| c.attack).sum();
                if stuck >= 1000 {
                    if let Some(i) = t.activate(WATTKEY) {
                        return t.pick(i);
                    }
                }
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let blocked = !ctx.monsters(ctx.opp).is_empty();
        let pressure = ctx.opp_best_attack() >= 1500;
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, WATTGIRAFFE) => Some(1700.0 + if blocked { 500.0 } else { 0.0 }),
            (ChoiceKind::NormalSummon, WATTPHEASANT) => Some(1600.0 + if blocked { 500.0 } else { 0.0 }),
            (ChoiceKind::NormalSummon, WATTWOODPECKER) if !blocked => Some(2000.0),
            (ChoiceKind::NormalSummon, WATTSQUIRREL) if !blocked => Some(1400.0),
            // Destroyed by the opponent, these punish them: face-down walls.
            (ChoiceKind::SetMonster, WATTLEMUR) if pressure => Some(1500.0),
            (ChoiceKind::SetMonster, WATTDRAGONFLY) if pressure => Some(1450.0),
            (ChoiceKind::SetMonster, WATTFOX) if pressure => Some(1300.0),
            (ChoiceKind::SetMonster, WATTMOLE | WATTKID | WATTBERYX | WATTKIWI) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code().unwrap_or(0));
        // The Synchros attack directly too: worth two small Watts only when
        // those could not get past the blockers.
        Some(match code {
            WATTCHIMERA | WATTHYDRA => {
                let blocked = ctx.monsters(ctx.opp).iter().any(|c| ctx.battle_stat(c) >= 1200);
                let direct = ctx.monsters(ctx.me).iter().filter(|c| Self::direct(&ctx, c)).count();
                ctx.main1() && blocked && direct == 0
            }
            _ => return None,
        })
    }

    fn battle(&mut self, t: &mut Turn) -> Option<usize> {
        if let Some((i, target)) = tactics::plan_attack(self, t) {
            return t.pick_targeting(i, target.into_iter().collect());
        }
        let i = tactics::direct_attack(t)?;
        t.pick_targeting(i, Vec::new())
    }

    fn attack_outcome(&self, ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
        // Wattmole destroys a face-down Defense Position monster outright.
        if ctx.is(attacker, WATTMOLE) && !target.position.face_up && !target.position.attack {
            return Some(Outcome::Win { trick: false });
        }
        None
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let damage_step = ctx.phase().map_or(false, |p| p.is_damage_step());
        Some(match code {
            // Continuous: up early, it burns on every summon of ours.
            WATTCANNON if !damage_step && choice.at().map_or(false, |a| a.location == Location::SpellTrapZone) => {
                let set = t.view(choice).map_or(false, |v| !v.position.face_up);
                if set { Response::new(20.0) } else { Response::no() }
            }
            // Wattkeeper: a Watt back for this turn's attacks.
            WATTKEEPER => {
                let attacking = ctx.my_turn() && ctx.phase() == Some(Phase::Main1);
                let target = ctx
                    .graveyard(ctx.me)
                    .into_iter()
                    .filter(|c| Self::is_watt(&ctx, c) && ctx.view_data(c).level <= 4)
                    .max_by_key(|c| (Self::direct(&ctx, c), ctx.view_data(c).attack));
                match target {
                    Some(target) if attacking && ctx.free_monster_zones(ctx.me) > 0 => Response::targeting(20.0, vec![target.at]),
                    _ => Response::no(),
                }
            }
            WATTCUBE => Response::no(),
            _ => return None,
        })
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        tactics::attack_directly(t)
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        let ctx = t.ctx;
        let code = ctx.canonical(code);
        // Direct attackers attack whatever is in the way.
        (DIRECT.contains(&code) && ctx.my_turn()).then_some(Position::FACE_UP_ATTACK)
    }
}
