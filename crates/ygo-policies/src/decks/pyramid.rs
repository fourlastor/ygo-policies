//! "Pyramid of Light": the Sphinxes of the desert tomb.
//!
//! Pyramid of Light (a Continuous Trap) lets Andro Sphinx and Sphinx Teleia
//! be Special Summoned from the hand for 500 LP each; Temple of the Kings
//! activates it the turn it is Set.  If both Sphinxes are destroyed at once,
//! Theinen the Great Sphinx takes their place.
//!
//! The rest of the deck lives face-down: Guardian Sphinx returns every
//! monster the opponent controls to the hand whenever it is Flip Summoned
//! (and turns itself face-down again), Des Lacooda draws on each Flip
//! Summon, Sand Moth comes back when destroyed face-down, and Hieracosphinx
//! keeps the opponent from attacking face-down monsters.  Criosphinx makes a
//! bounced monster cost a discard.  The Zombies (Pyramid Turtle, Regenerating
//! Mummy, Spirit Reaper) float and grind; Call of the Mummy refills an
//! empty field; Curse of Anubis turns attackers into 0 DEF walls.

use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Location, Member, Phase, Position};

pub const DECK: &str = "Pyramid of Light";

const ANDRO_SPHINX: u32 = 15013468;
const SPHINX_TELEIA: u32 = 51402177;
const THEINEN: u32 = 87997872;
const GUARDIAN_SPHINX: u32 = 40659562;
const CRIOSPHINX: u32 = 18654201;
const HIERACOSPHINX: u32 = 82260502;
const END_OF_ANUBIS: u32 = 65403020;
const PYRAMID_TURTLE: u32 = 77044671;
const REGENERATING_MUMMY: u32 = 70821187;
const DES_LACOODA: u32 = 2326738;
const SAND_MOTH: u32 = 73648243;
const SPIRIT_REAPER: u32 = 23205979;
const PYRAMID_ENERGY: u32 = 76754619;
const TEMPLE_OF_THE_KINGS: u32 = 29762407;
const CALL_OF_THE_MUMMY: u32 = 4861205;
const PYRAMID_OF_LIGHT: u32 = 53569894;
const PHARAOHS_TREASURE: u32 = 63571750;
const CURSE_OF_ANUBIS: u32 = 66742250;

#[derive(Default)]
pub struct Pyramid;

impl Pyramid {
    fn sphinx_in_hand(ctx: &Ctx) -> bool {
        ctx.in_hand(ANDRO_SPHINX) || ctx.in_hand(SPHINX_TELEIA)
    }

    fn pyramid_up(ctx: &Ctx) -> bool {
        ctx.face_up_on_field(ctx.me, PYRAMID_OF_LIGHT)
    }

    /// A Flip Summon of ours worth doing now: Guardian Sphinx with monsters
    /// to bounce, Des Lacooda for its draw.
    fn flip_summon(t: &Turn) -> Option<usize> {
        let ctx = t.ctx;
        let bounce = ctx.monsters(ctx.opp).len() >= 1 && ctx.field_strength(ctx.opp) >= 1000;
        t.find_where(|c| {
            if c.kind != ChoiceKind::ChangePosition {
                return false;
            }
            let Some(card) = c.at().and_then(|at| ctx.card(at)) else { return false };
            if card.position.face_up {
                return false;
            }
            (ctx.is(card, GUARDIAN_SPHINX) && bounce) || ctx.is(card, DES_LACOODA)
        })
    }

    /// Turn a face-up Guardian Sphinx / Des Lacooda face-down again, ready
    /// for next turn's Flip Summon.
    fn flip_down(t: &Turn) -> Option<usize> {
        let ctx = t.ctx;
        t.find_where(|c| {
            c.kind == ChoiceKind::Activate
                && c.description & 0xf == 0
                && c.at().map_or(false, |a| a.location == Location::MonsterZone)
                && c.code().map_or(false, |k| matches!(ctx.canonical(k), GUARDIAN_SPHINX | DES_LACOODA))
        })
    }

    fn is_zombie(ctx: &Ctx, card: &CardView) -> bool {
        ctx.view_data(card).race & crate::cards::races::ZOMBIE != 0
    }
}

impl Strategy for Pyramid {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            THEINEN => 3500,
            ANDRO_SPHINX | SPHINX_TELEIA => 2800,
            PYRAMID_OF_LIGHT => 2600,
            GUARDIAN_SPHINX => 2400,
            HIERACOSPHINX => 2200,
            END_OF_ANUBIS => 2000,
            TEMPLE_OF_THE_KINGS | CURSE_OF_ANUBIS => 1600,
            REGENERATING_MUMMY | CRIOSPHINX => 1500,
            DES_LACOODA | SPIRIT_REAPER => 1400,
            SAND_MOTH | PYRAMID_TURTLE | CALL_OF_THE_MUMMY => 1300,
            PYRAMID_ENERGY => 1000,
            PHARAOHS_TREASURE => 800,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // The Pyramid first: Temple of the Kings lets it go up this turn.
        if !Self::pyramid_up(&ctx) {
            if let Some(i) = t.activate_from(PYRAMID_OF_LIGHT, Location::SpellTrapZone) {
                return t.pick(i);
            }
            if ctx.in_hand(PYRAMID_OF_LIGHT) {
                if !ctx.face_up_on_field(ctx.me, TEMPLE_OF_THE_KINGS) {
                    if let Some(i) = t.activate_from(TEMPLE_OF_THE_KINGS, Location::Hand) {
                        return t.pick(i);
                    }
                } else if let Some(i) = t.find(ChoiceKind::SetSpellTrap, Some(PYRAMID_OF_LIGHT), None) {
                    return t.pick(i);
                }
            }
        }
        // Guardian Sphinx's bounce and Des Lacooda's draw.
        if let Some(i) = Self::flip_summon(t) {
            return t.pick(i);
        }
        // Call of the Mummy: a Zombie from the hand onto an empty field.
        if !ctx.face_up_on_field(ctx.me, CALL_OF_THE_MUMMY) {
            if let Some(i) = t.activate_from(CALL_OF_THE_MUMMY, Location::Hand) {
                return t.pick(i);
            }
        }
        if ctx.monsters(ctx.me).is_empty() && ctx.hand().iter().any(|c| Self::is_zombie(&ctx, c) && ctx.view_data(c).is_monster()) {
            if let Some(i) = t.activate_from(CALL_OF_THE_MUMMY, Location::SpellTrapZone) {
                return t.pick(i);
            }
        }
        // After the attacks, turn the flip monsters face-down again.
        if !ctx.main1() || !t.has(ChoiceKind::EnterBattle) {
            if let Some(i) = Self::flip_down(t) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let fodder = ctx.monsters(ctx.me).len();
        Some(match (choice.kind, code) {
            // The Level 10 Sphinxes come out through the Pyramid only.
            (_, ANDRO_SPHINX | SPHINX_TELEIA | THEINEN) => None,
            // Guardian Sphinx goes down face-down: its Flip Summon is the bounce.
            (ChoiceKind::SetMonster, GUARDIAN_SPHINX) if fodder >= 1 => Some(2600.0),
            (ChoiceKind::NormalSummon, GUARDIAN_SPHINX) => None,
            (ChoiceKind::NormalSummon, HIERACOSPHINX) if fodder >= 1 => Some(2300.0),
            (ChoiceKind::SetMonster, DES_LACOODA) => Some(1500.0),
            (ChoiceKind::SetMonster, SAND_MOTH) => Some(1300.0),
            (ChoiceKind::NormalSummon, DES_LACOODA | SAND_MOTH) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // 500 LP each: not when it would leave us in burn range.
            ANDRO_SPHINX | SPHINX_TELEIA => ctx.my_lp() > 1500,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let incoming = ctx.incoming_attack();
        let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
        Some(match code {
            // The Pyramid goes up at the end of their turn, or when we can use it.
            PYRAMID_OF_LIGHT if !Self::pyramid_up(&ctx) => {
                if end_of_their_turn || (ctx.my_turn() && Self::sphinx_in_hand(&ctx)) { Response::new(30.0) } else { Response::no() }
            }
            PYRAMID_OF_LIGHT => Response::no(),
            // Their attacker drops to a 0 DEF wall and stays there.
            CURSE_OF_ANUBIS => match incoming {
                Some((attacker, target)) if ctx.attack_hurts(attacker, target) && ctx.view_data(attacker).is(crate::cards::types::EFFECT) => {
                    Response::new(52.0)
                }
                _ => Response::no(),
            },
            // +500 DEF saves a defender that would lose.
            PYRAMID_ENERGY => match incoming {
                Some((attacker, Some(target))) if !target.position.attack && target.defense <= attacker.attack && target.defense + 500 > attacker.attack => {
                    Response::new(40.0)
                }
                _ => Response::no(),
            },
            // Back into the Deck face-up: a free card when drawn.
            PHARAOHS_TREASURE if end_of_their_turn => Response::new(10.0),
            PHARAOHS_TREASURE => Response::no(),
            _ => return None,
        })
    }

    fn option(&self, t: &Turn) -> Option<usize> {
        // Pyramid Energy: +500 DEF (the second option) is the defensive one.
        (t.memory.last_activated.map(|c| t.ctx.canonical(c)) == Some(PYRAMID_ENERGY)).then_some(1)
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        // Pharaoh's Treasure: take back the best card in the Graveyard.
        if member.at.controller == ctx.me && member.at.location == Location::Graveyard && t.decision.hint.is_gain() {
            return member.code.map(|c| crate::agent::value(self, &ctx, Some(ctx.canonical(c)), None) as f64);
        }
        None
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        let ctx = t.ctx;
        match ctx.canonical(code) {
            ANDRO_SPHINX | SPHINX_TELEIA | THEINEN | HIERACOSPHINX | END_OF_ANUBIS => Some(Position::FACE_UP_ATTACK),
            _ => None,
        }
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(code);
        Some(ctx.data(code).is_trap() || code == PYRAMID_ENERGY)
    }
}
