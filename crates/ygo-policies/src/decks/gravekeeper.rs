//! "Gravekeeper's Tomb": the Gravekeepers and their valley.
//!
//! Necrovalley gives every Gravekeeper's monster 500 ATK/DEF and seals both
//! Graveyards: nothing leaves them, which shuts off the opponent's revival
//! and recursion (the Chief exempts our own).  Commandant and Terraforming
//! find it; Priestess stands in for it.  The monsters then grind: Spy's Flip
//! Summon brings a second Gravekeeper, Guard's bounces a monster, Descendant
//! Tributes a spent Gravekeeper to destroy any card, Assailant flips its
//! battle target's position as it attacks, Spear Soldier pierces, the Chief
//! revives one, Visionary grows with the Graveyard.  Stele and Rite of
//! Spirit recover what died, Royal Tribute empties both hands of monsters,
//! Gravekeeper's Servant makes every attack cost the opponent a card.

use crate::agent::{value, Outcome, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Location, Phase, Position};

pub const DECK: &str = "Gravekeeper's Tomb";

const SPY: u32 = 24317029;
const COMMANDANT: u32 = 17393207;
const ASSAILANT: u32 = 25262697;
const SPEAR_SOLDIER: u32 = 63695531;
const DESCENDANT: u32 = 30213599;
const CHIEF: u32 = 62473983;
const VISIONARY: u32 = 3825890;
const GUARD: u32 = 37101832;
const CURSE: u32 = 50712728;
const CANNONHOLDER: u32 = 99877698;
const PRIESTESS: u32 = 3381441;
const NECROVALLEY: u32 = 47355498;
const TERRAFORMING: u32 = 73628505;
const ROYAL_TRIBUTE: u32 = 72405967;
const STELE: u32 = 99523325;
const SERVANT: u32 = 16762927;
const RITE_OF_SPIRIT: u32 = 30450531;
const RAIGEKI_BREAK: u32 = 4178474;
const PHARAOHS_TREASURE: u32 = 63571750;
const SET_GRAVEKEEPER: u16 = 0x2e;

#[derive(Clone, Default)]
pub struct Gravekeeper;

impl Gravekeeper {
    fn is_gravekeeper(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.in_set(SET_GRAVEKEEPER)
    }

    fn valley_up(ctx: &Ctx) -> bool {
        ctx.face_up_on_field(ctx.me, NECROVALLEY)
    }

    /// Necrovalley in play, or on the way.
    fn valley_found(ctx: &Ctx) -> bool {
        Self::valley_up(ctx) || ctx.in_hand(NECROVALLEY)
    }

    /// ATK a Gravekeeper gains on this field.
    fn bonus(ctx: &Ctx) -> i32 {
        let valley = if Self::valley_up(ctx) { 500 } else { 0 };
        let priestess = if Self::face_up(ctx, PRIESTESS) { 200 } else { 0 };
        valley + priestess
    }

    fn face_up(ctx: &Ctx, code: u32) -> bool {
        ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.is(c, code))
    }

    /// Assailant flips its battle target: with the valley it beats any
    /// face-up monster whose weaker stat is below its ATK.
    fn valley_or_priestess(ctx: &Ctx) -> bool {
        Self::valley_up(ctx) || Self::face_up(ctx, PRIESTESS)
    }

    /// A face-down Spy / Guard to Flip Summon now.
    fn flip_summon(t: &Turn) -> Option<usize> {
        let ctx = t.ctx;
        let bounce = ctx.monsters(ctx.opp).iter().any(|c| ctx.threat(c) >= 1500);
        t.find_where(|c| {
            c.kind == ChoiceKind::ChangePosition
                && c.at().and_then(|at| ctx.card(at)).map_or(false, |card| {
                    !card.position.face_up && (ctx.is(card, SPY) || (ctx.is(card, GUARD) && bounce))
                })
        })
    }

    fn gravekeepers_in_graveyard(ctx: &Ctx) -> usize {
        ctx.graveyard(ctx.me).iter().filter(|c| Self::is_gravekeeper(ctx, c)).count()
    }
}

impl Strategy for Gravekeeper {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            VISIONARY => 2600,
            NECROVALLEY => 2500,
            CHIEF => 2300,
            DESCENDANT => 2000,
            SPY => 1900,
            ASSAILANT => 1800,
            COMMANDANT | SPEAR_SOLDIER => 1700,
            GUARD | STELE | RAIGEKI_BREAK => 1600,
            PRIESTESS | TERRAFORMING | RITE_OF_SPIRIT => 1500,
            CANNONHOLDER => 1400,
            ROYAL_TRIBUTE => 1300,
            SERVANT => 1200,
            CURSE => 1100,
            PHARAOHS_TREASURE => 800,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Necrovalley first, found by Terraforming or Commandant.
        if !Self::valley_up(&ctx) {
            if let Some(i) = t.activate_from(NECROVALLEY, Location::Hand) {
                return t.pick(i);
            }
            if !ctx.in_hand(NECROVALLEY) {
                if let Some(i) = t.activate(TERRAFORMING) {
                    return t.pick(i);
                }
                if !ctx.in_hand(TERRAFORMING) {
                    if let Some(i) = t.activate_from(COMMANDANT, Location::Hand) {
                        return t.pick(i);
                    }
                }
            }
        }
        if !ctx.face_up_on_field(ctx.me, SERVANT) {
            if let Some(i) = t.activate_from(SERVANT, Location::Hand) {
                return t.pick(i);
            }
        }
        if Self::gravekeepers_in_graveyard(&ctx) >= 2 {
            if let Some(i) = t.activate(STELE) {
                return t.pick(i);
            }
        }
        // Use our Normal Summon before Royal Tribute discards the last
        // monster in our hand. Only their public hand size is consulted.
        let our_monsters = ctx.hand().iter().filter(|c| ctx.view_data(c).is_monster()).count();
        if Self::valley_up(&ctx) && ctx.hand_size(ctx.opp) >= 3 && our_monsters <= 1 && (our_monsters == 0 || ctx.obs.summon_used) {
            if let Some(i) = t.activate(ROYAL_TRIBUTE) {
                return t.pick(i);
            }
        }
        // Descendant: a spent Gravekeeper for their best card.
        if let Some(i) = t.activate_from(DESCENDANT, Location::MonsterZone) {
            let target = ctx
                .monsters(ctx.opp)
                .into_iter()
                .chain(ctx.spell_traps(ctx.opp))
                .filter(|c| ctx.reaches(c, DESCENDANT, true, true))
                .max_by_key(|c| ctx.threat(c))
                .filter(|c| ctx.threat(c) >= 1500);
            let fodder = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| c.position.face_up && Self::is_gravekeeper(&ctx, c) && !ctx.is(c, DESCENDANT))
                .map(|c| value(self, &ctx, None, Some(c)))
                .min();
            if let (Some(target), Some(fodder)) = (target, fodder) {
                if fodder < ctx.threat(target) + 500 {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
        }
        if ctx.main1() {
            if let Some(i) = Self::flip_summon(t) {
                return t.pick(i);
            }
        }
        // Cannonholder: 700 for a spare Gravekeeper, to finish or after battle.
        let spare = ctx.monsters(ctx.me).len() >= 3;
        if ctx.opp_lp() <= 700 || (!ctx.main1() && spare) {
            if let Some(i) = t.activate_from(CANNONHOLDER, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let bonus = Self::bonus(&ctx) as f64;
        let revive = ctx.graveyard(ctx.me).iter().any(|c| Self::is_gravekeeper(&ctx, c) && ctx.view_data(c).attack >= 1500);
        Some(match (choice.kind, code) {
            // Visionary can use one Gravekeeper instead of two Tributes.
            (ChoiceKind::NormalSummon, VISIONARY) if ctx.monsters(ctx.me).iter().any(|c| Self::is_gravekeeper(&ctx, c)) => Some(2800.0),
            // Spy's Flip Summon is a second Gravekeeper.
            (ChoiceKind::SetMonster, SPY) => Some(2100.0),
            (ChoiceKind::SetMonster, GUARD) if !ctx.monsters(ctx.opp).is_empty() => Some(1700.0),
            (ChoiceKind::NormalSummon, CHIEF) if revive && !ctx.monsters(ctx.me).is_empty() => Some(2500.0),
            (ChoiceKind::NormalSummon, DESCENDANT) => Some(1900.0 + bonus),
            (ChoiceKind::NormalSummon, ASSAILANT) if Self::valley_or_priestess(&ctx) => Some(1850.0 + bonus),
            (ChoiceKind::NormalSummon, CURSE) => Some(1300.0),
            (ChoiceKind::NormalSummon, SPY | GUARD) => None,
            // Commandant fetches Necrovalley from the hand instead.
            (ChoiceKind::NormalSummon | ChoiceKind::SetMonster, COMMANDANT) if !Self::valley_found(&ctx) => None,
            _ => return None,
        })
    }

    fn attack_outcome(&self, ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
        // Assailant turns its target around: its weaker stat is the one that counts.
        if ctx.is(attacker, ASSAILANT) && Self::valley_or_priestess(ctx) && target.position.face_up {
            let weaker = target.attack.min(target.defense);
            return Some(if attacker.attack > weaker { Outcome::Win { trick: false } } else { Outcome::Bounce });
        }
        None
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let incoming = ctx.incoming_attack();
        let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
        Some(match code {
            RITE_OF_SPIRIT => {
                let target = ctx
                    .graveyard(ctx.me)
                    .into_iter()
                    .filter(|c| Self::is_gravekeeper(&ctx, c))
                    .max_by_key(|c| value(self, &ctx, None, Some(c)));
                let Some(target) = target else { return Some(Response::no()) };
                let blocker = incoming.is_some() && ctx.monsters(ctx.me).is_empty();
                let attacker = ctx.my_turn() && ctx.main1();
                if blocker || attacker || end_of_their_turn {
                    Response::targeting(if blocker { 50.0 } else { 20.0 }, vec![target.at])
                } else {
                    Response::no()
                }
            }
            RAIGEKI_BREAK => {
                if ctx.hand_size(ctx.me) == 0 {
                    return Some(Response::no());
                }
                match incoming {
                    Some((attacker, target)) if ctx.attack_hurts(attacker, target) => Response::targeting(72.0, vec![attacker.at]),
                    _ if end_of_their_turn => {
                        let target = ctx.monsters(ctx.opp).into_iter().chain(ctx.spell_traps(ctx.opp)).max_by_key(|c| ctx.threat(c));
                        match target {
                            Some(target) if ctx.threat(target) >= 1800 => Response::targeting(20.0, vec![target.at]),
                            _ => Response::no(),
                        }
                    }
                    _ => Response::no(),
                }
            }
            // Necrovalley would stop the later Graveyard recovery and can
            // leave Treasure cycling through the Deck without a payoff.
            PHARAOHS_TREASURE if end_of_their_turn && (!Self::valley_up(&ctx) || Self::face_up(&ctx, CHIEF)) => Response::new(10.0),
            PHARAOHS_TREASURE => Response::no(),
            _ => return None,
        })
    }

    fn option(&self, t: &Turn) -> Option<usize> {
        t.choices().find(|(_, c)| c.description == ((VISIONARY as u64) << 4)).map(|(i, _)| i)
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        let ctx = t.ctx;
        let data = ctx.data(ctx.canonical(code));
        if !data.in_set(SET_GRAVEKEEPER) {
            return None;
        }
        let attack = data.attack + Self::bonus(&ctx);
        Some(if attack >= ctx.opp_best_attack() || ctx.monsters(ctx.opp).is_empty() {
            Position::FACE_UP_ATTACK
        } else {
            Position::FACE_UP_DEFENSE
        })
    }
}
