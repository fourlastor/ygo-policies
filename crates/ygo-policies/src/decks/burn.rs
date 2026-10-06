//! "Burn Princess": burn + lifegain, no beatdown.
//!
//! Fire Princess turns every life point gain into 500 damage and Spell
//! Absorption turns every Spell into a gain, so the engine is set up first
//! (Absorption, then Princess) and the burn/lifegain Spells follow.  Swords of
//! Revealing Light, Scapegoat, Fires of Doomsday and Magic Cylinder stall;
//! Wave-Motion Cannon, Secret Barrel and Magical Explosion finish.

use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Location, Member, Phase, Position};

pub const DECK: &str = "Burn Princess";

const FIRE_PRINCESS: u32 = 64752646;
const UFO_TURTLE: u32 = 60806437;
const MISFORTUNE: u32 = 1036974;
const DIAN_KETO: u32 = 84257639;
const GOBLIN_THIEF: u32 = 45311864;
const TREMENDOUS_FIRE: u32 = 46918794;
const SPELL_ABSORPTION: u32 = 51481927;
const FIRES_OF_DOOMSDAY: u32 = 46173679;
const FINAL_FLAME: u32 = 73134081;
const WAVE_MOTION_CANNON: u32 = 38992735;
const OOKAZI: u32 = 19523799;
const SPIDER_WEB: u32 = 69408987;
const SCAPEGOAT: u32 = 73915051;
const SWORDS: u32 = 72302403;
const COMPULSORY_EVACUATION: u32 = 94192409;
const SECRET_BARREL: u32 = 27053506;
const MAGIC_CYLINDER: u32 = 62279055;
const MAGICAL_EXPLOSION: u32 = 32723153;

/// Plain damage of a burn Spell, before Princess/Absorption bonuses.
fn burn(code: u32) -> i32 {
    match code {
        GOBLIN_THIEF => 500,
        TREMENDOUS_FIRE => 1000,
        FINAL_FLAME => 600,
        OOKAZI => 800,
        _ => 0,
    }
}

#[derive(Clone, Default)]
pub struct Burn {
    /// Turn in which each face-up Wave-Motion Cannon (by zone) was activated.
    cannons: Vec<(u32, u32)>,
}

impl Burn {
    fn princess_up(ctx: &Ctx) -> bool {
        ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && c.code == Some(FIRE_PRINCESS))
    }

    fn absorptions(ctx: &Ctx) -> i32 {
        ctx.spell_traps(ctx.me)
            .iter()
            .filter(|c| c.position.face_up && c.code == Some(SPELL_ABSORPTION))
            .count() as i32
    }

    /// Damage a Spell activation deals through the engine, besides its own text.
    fn engine_bonus(ctx: &Ctx, gains_life: bool) -> i32 {
        let princesses = ctx
            .monsters(ctx.me)
            .iter()
            .filter(|c| c.position.face_up && c.code == Some(FIRE_PRINCESS))
            .count() as i32;
        let gains = Self::absorptions(ctx) + gains_life as i32;
        500 * princesses * gains
    }

    fn secret_barrel_damage(ctx: &Ctx) -> i32 {
        let opp = ctx.opp;
        let cards = ctx.hand_size(opp) as i32 + (ctx.monsters(opp).len() + ctx.spell_traps(opp).len()) as i32;
        200 * cards
    }

    fn explosion_damage(ctx: &Ctx) -> i32 {
        200 * ctx.graveyard(ctx.me).iter().filter(|c| ctx.view_data(c).is_spell()).count() as i32
    }

    fn cannon_damage(&self, ctx: &Ctx, card: &CardView) -> i32 {
        let Some((_, since)) = self.cannons.iter().find(|(seq, _)| *seq == card.at.sequence) else { return 0 };
        // One of our Standby Phases passes in each of our later turns.
        let turns = ctx.obs.turn.saturating_sub(*since);
        let standbys = if ctx.my_turn() { (turns + 1) / 2 } else { turns / 2 };
        1000 * standbys as i32
    }

    fn update_cannons(&mut self, ctx: &Ctx) {
        let up: Vec<u32> = ctx
            .spell_traps(ctx.me)
            .iter()
            .filter(|c| c.position.face_up && c.code == Some(WAVE_MOTION_CANNON))
            .map(|c| c.at.sequence)
            .collect();
        self.cannons.retain(|(seq, _)| up.contains(seq));
        for seq in up {
            if !self.cannons.iter().any(|(s, _)| *s == seq) {
                self.cannons.push((seq, ctx.obs.turn));
            }
        }
    }

    /// Is it safe / worth it to put Fire Princess face-up now?
    fn princess_welcome(ctx: &Ctx) -> bool {
        let protected = ctx.face_up_on_field(ctx.me, SWORDS)
            || ctx.set_backrow(ctx.me).len() >= 2
            || ctx.monsters(ctx.me).len() >= 2;
        let engines_in_hand = ctx
            .hand_codes()
            .iter()
            .filter(|c| burn(**c) > 0 || matches!(**c, DIAN_KETO | SPELL_ABSORPTION))
            .count();
        ctx.opp_best_attack() <= 1300 || protected || engines_in_hand >= 2
    }
}

impl Strategy for Burn {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            FIRE_PRINCESS => 2600,
            SPELL_ABSORPTION => 2200,
            SWORDS => 2100,
            MAGIC_CYLINDER => 2000,
            WAVE_MOTION_CANNON => 1900,
            SCAPEGOAT | FIRES_OF_DOOMSDAY => 1800,
            TREMENDOUS_FIRE => 1700,
            COMPULSORY_EVACUATION => 1600,
            SECRET_BARREL => 1500,
            UFO_TURTLE | OOKAZI => 1400,
            GOBLIN_THIEF | MISFORTUNE => 1300,
            FINAL_FLAME | MAGICAL_EXPLOSION => 1200,
            DIAN_KETO => 1100,
            SPIDER_WEB => 700,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        self.update_cannons(&ctx);
        let opp_lp = ctx.opp_lp();
        let opp_monsters = ctx.monsters(ctx.opp);

        // Lethal / finishers first.
        for (i, choice) in t.choices() {
            if choice.kind != ChoiceKind::Activate || !t.fresh(i) {
                continue;
            }
            let Some(code) = choice.code() else { continue };
            let from_field = choice.at().map(|a| a.location) == Some(Location::SpellTrapZone);
            let damage = match code {
                WAVE_MOTION_CANNON if from_field => {
                    t.view(choice).map_or(0, |v| self.cannon_damage(&ctx, v))
                }
                MAGICAL_EXPLOSION => Self::explosion_damage(&ctx),
                SECRET_BARREL => Self::secret_barrel_damage(&ctx),
                code if burn(code) > 0 => burn(code) + Self::engine_bonus(&ctx, code == GOBLIN_THIEF),
                _ => continue,
            };
            // Cannons are easy to destroy: cash them in once they are worth two burns.
            let big_finisher = matches!(code, WAVE_MOTION_CANNON if from_field) && damage >= 2000;
            if damage >= opp_lp || big_finisher {
                return t.pick(i);
            }
        }

        // Stall a board that would run us over.
        if !opp_monsters.is_empty() {
            if let Some(i) = t.activate(SWORDS) {
                return t.pick(i);
            }
        }
        // Build the engine: Absorption, Cannon, then Fire Princess.
        if let Some(i) = t.activate_from(SPELL_ABSORPTION, Location::Hand) {
            return t.pick(i);
        }
        if let Some(i) = t.activate_from(WAVE_MOTION_CANNON, Location::Hand) {
            return t.pick(i);
        }
        if Self::princess_welcome(&ctx) {
            if let Some(i) = t.find(ChoiceKind::NormalSummon, Some(FIRE_PRINCESS), None) {
                return t.pick(i);
            }
        }
        let princess_coming = ctx.in_hand(FIRE_PRINCESS) && t.has(ChoiceKind::NormalSummon) && Self::princess_welcome(&ctx);
        if princess_coming {
            return None; // summon first, burn afterwards
        }

        // Burn and lifegain Spells.
        for code in [TREMENDOUS_FIRE, OOKAZI, FINAL_FLAME, GOBLIN_THIEF] {
            if code == TREMENDOUS_FIRE && ctx.my_lp() <= 1000 {
                continue;
            }
            if let Some(i) = t.activate_from(code, Location::Hand) {
                return t.pick(i);
            }
        }
        if Self::princess_up(&ctx) || Self::absorptions(&ctx) > 0 || ctx.my_lp() <= 3000 {
            if let Some(i) = t.activate_from(DIAN_KETO, Location::Hand) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(MISFORTUNE) {
            let best = opp_monsters
                .iter()
                .filter(|c| c.position.face_up)
                .max_by_key(|c| ctx.view_data(c).attack);
            if let Some(best) = best {
                if ctx.view_data(best).attack / 2 >= 700 || ctx.view_data(best).attack / 2 >= opp_lp {
                    return t.pick_targeting(i, vec![best.at]);
                }
            }
        }
        if let Some(i) = t.activate(SPIDER_WEB) {
            return t.pick(i);
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = choice.code()?;
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, FIRE_PRINCESS) if Self::princess_welcome(&ctx) => Some(3000.0),
            (ChoiceKind::SetMonster, UFO_TURTLE) => Some(2000.0),
            (ChoiceKind::SetMonster, FIRE_PRINCESS) => Some(1000.0),
            (ChoiceKind::NormalSummon, UFO_TURTLE) if ctx.opp_best_attack() < 1400 => Some(1500.0),
            _ => None,
        })
    }

    fn wants_battle(&self, t: &Turn) -> Option<bool> {
        // Only swing when it is free: the default attack planner still only
        // takes winning attacks.
        Some(t.ctx.monsters(t.ctx.me).iter().any(|c| t.ctx.can_attack(c)))
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let data = t.ctx.data(code);
        Some(data.is_trap() || matches!(code, SCAPEGOAT | FIRES_OF_DOOMSDAY))
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = choice.code()?;
        let incoming = ctx.incoming_attack();
        let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
        Some(match code {
            MAGIC_CYLINDER => match incoming {
                Some((attacker, _)) if attacker.attack >= 800 => Response::targeting(90.0, vec![attacker.at]),
                _ => Response::no(),
            },
            COMPULSORY_EVACUATION => match incoming {
                Some((attacker, target)) if ctx.attack_hurts(attacker, target) => {
                    Response::targeting(70.0, vec![attacker.at])
                }
                _ => Response::no(),
            },
            SCAPEGOAT | FIRES_OF_DOOMSDAY => {
                let exposed = ctx.monsters(ctx.me).is_empty();
                let battle_start = !ctx.my_turn() && ctx.phase() == Some(Phase::BattleStart);
                match incoming {
                    Some((attacker, target)) if exposed && ctx.attack_hurts(attacker, target) => Response::new(60.0),
                    None if exposed && battle_start && ctx.opp_attack_potential() >= 1500 => Response::new(60.0),
                    // Unused walls still ping at the end of their turn.
                    _ if end_of_their_turn && ctx.free_monster_zones(ctx.me) >= 2 => Response::new(15.0),
                    _ => Response::no(),
                }
            }
            SECRET_BARREL => {
                let damage = Self::secret_barrel_damage(&ctx);
                if damage >= ctx.opp_lp() || damage >= 1400 || (end_of_their_turn && damage >= 1000) {
                    Response::new(40.0)
                } else {
                    Response::no()
                }
            }
            MAGICAL_EXPLOSION => {
                let damage = Self::explosion_damage(&ctx);
                if damage >= ctx.opp_lp() || damage >= 1600 { Response::new(40.0) } else { Response::no() }
            }
            GOBLIN_THIEF | TREMENDOUS_FIRE | FINAL_FLAME | OOKAZI | DIAN_KETO | MISFORTUNE => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        // UFO Turtle: bring out Fire Princess before anything else.
        let ctx = t.ctx;
        if member.at.controller == ctx.me && t.decision.hint.is_gain() {
            let code = member.code?;
            return Some(value(self, &ctx, Some(code), None) as f64);
        }
        None
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        // Walls and tokens defend; UFO Turtle's summon is always Attack.
        let ctx = t.ctx;
        (code == FIRE_PRINCESS && ctx.opp_best_attack() > 1300).then_some(Position::FACE_UP_DEFENSE)
    }
}
