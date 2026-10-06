//! "Machina Gadgets": Machine card advantage with a trap wall.
//!
//! Each Gadget's summon searches the next (Red finds Yellow, Yellow Green,
//! Green Red), so every Normal Summon is also a card; Machina Gearframe finds
//! Machina Fortress, which Special Summons itself from the hand or the
//! Graveyard by discarding Machines with 8 or more Levels, and takes a card
//! with it when destroyed in battle.  The unions (Gearframe, Peacekeeper)
//! equip to a Machine and die in its place (measured: leaving them on the
//! field as attackers wins more).  Cyber Dragon comes out against
//! any board and turns into Chimeratech Fortress Dragon with Machines from
//! either field.  The rest is the era's best removal and traps.

use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};

pub const DECK: &str = "Machina Gadgets";

const MACHINA_FORTRESS: u32 = 5556499;
const MACHINA_GEARFRAME: u32 = 42940404;
const RED_GADGET: u32 = 86445415;
const GREEN_GADGET: u32 = 41172955;
const YELLOW_GADGET: u32 = 13839120;
const CYBER_DRAGON: u32 = 70095154;
const MACHINA_PEACEKEEPER: u32 = 78349103;
const CARD_TROOPER: u32 = 85087012;
const LIMITER_REMOVAL: u32 = 23171610;
const CHIMERATECH_FORTRESS_DRAGON: u32 = 79229522;

#[derive(Clone, Default)]
pub struct Machina;

impl Machina {
    fn is_machine(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.race & races::MACHINE != 0
    }

    /// The Gadget each Gadget searches.
    fn searches(code: u32) -> Option<u32> {
        match code {
            RED_GADGET => Some(YELLOW_GADGET),
            YELLOW_GADGET => Some(GREEN_GADGET),
            GREEN_GADGET => Some(RED_GADGET),
            _ => None,
        }
    }
}

impl Strategy for Machina {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            CHIMERATECH_FORTRESS_DRAGON => 3000,
            MACHINA_FORTRESS => 2500,
            CYBER_DRAGON => 2100,
            MACHINA_GEARFRAME => 1900,
            RED_GADGET | GREEN_GADGET | YELLOW_GADGET => 1700,
            CARD_TROOPER => 1300,
            MACHINA_PEACEKEEPER | LIMITER_REMOVAL => 1200,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Machina Fortress whenever its discard can be paid: a 2500 beater now
        // is worth the two cards.
        if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(MACHINA_FORTRESS), None) {
            return t.pick(i);
        }
        // Limiter Removal: doubled Machines on an open field.
        if ctx.main1() && t.has(ChoiceKind::EnterBattle) && ctx.monsters(ctx.opp).is_empty() {
            let doubled: i32 = ctx
                .monsters(ctx.me)
                .iter()
                .filter(|c| ctx.can_attack(c))
                .map(|c| if Self::is_machine(&ctx, c) { 2 * c.attack } else { c.attack })
                .sum();
            if doubled >= ctx.opp_lp() {
                if let Some(i) = t.activate(LIMITER_REMOVAL) {
                    return t.pick(i);
                }
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let fortress_found = ctx.in_hand(MACHINA_FORTRESS);
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, MACHINA_GEARFRAME) if !fortress_found => Some(2300.0),
            // A Gadget whose search is still in the Deck is two cards.
            (ChoiceKind::NormalSummon, RED_GADGET | GREEN_GADGET | YELLOW_GADGET) => {
                let next = Self::searches(code).unwrap_or(0);
                let in_hand = ctx.in_hand(next);
                Some(2100.0 + ctx.data(code).attack as f64 / 10.0 - if in_hand { 200.0 } else { 0.0 })
            }
            (ChoiceKind::NormalSummon, MACHINA_GEARFRAME) => Some(1900.0),
            (ChoiceKind::NormalSummon, CARD_TROOPER) if ctx.graveyard(ctx.me).iter().all(|c| !ctx.is(c, MACHINA_FORTRESS)) => Some(1500.0),
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // Cyber Dragon and Machines from either field: theirs first.
            CHIMERATECH_FORTRESS_DRAGON => {
                let theirs = ctx.monsters(ctx.opp).iter().filter(|c| c.position.face_up && Self::is_machine(&ctx, c)).count();
                let ours = ctx.monsters(ctx.me).iter().filter(|c| Self::is_machine(&ctx, c) && !ctx.is(c, CYBER_DRAGON)).count();
                theirs >= 1 || ours >= 2
            }
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        Some(match code {
            // Equipped, the unions' other effect would unequip them.
            MACHINA_GEARFRAME | MACHINA_PEACEKEEPER if choice.at().map_or(false, |a| a.location == Location::SpellTrapZone) => Response::no(),
            LIMITER_REMOVAL => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let mine = member.at.controller == ctx.me;
        let code = member.code.map(|c| ctx.canonical(c));
        match t.decision.hint {
            // Searches: Fortress for Gearframe.
            Hint::AddToHand if mine && member.at.location == Location::Deck => {
                code.map(|c| if c == MACHINA_FORTRESS { 5000.0 } else { value(self, &ctx, Some(c), None) as f64 })
            }
            // Chimeratech materials: their Machines first.
            Hint::FusionMaterial | Hint::ToGraveyard if member.at.location == Location::MonsterZone => {
                let worth = value(self, &ctx, code, ctx.card(member.at).filter(|c| c.known())) as f64;
                Some(if mine { -worth } else { worth + 1000.0 })
            }
            _ => None,
        }
    }
}
