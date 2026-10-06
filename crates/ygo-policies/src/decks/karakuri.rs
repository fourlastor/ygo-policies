//! "Karakuri Workshop": Machines that must attack and switch positions.
//!
//! Every Karakuri monster must attack if it can, and turns to Defense
//! Position when it is attacked.  Each position change pays: Karakuri
//! Anatomy counts them into draws, Karakuri Steel Shogun "Bureido" draws,
//! Karakuri Trick House destroys a card, Karakuri Klock destroys every
//! face-up monster of the opponent's when a Defense Position Karakuri is
//! attacked.  The engine: Merchant "Inashichi" searches on its Normal Summon,
//! Machine Duplication copies a 500 ATK one twice, and the Tuners
//! (Strategist "Nishipachi", Watchdog "Saizan", Genex Ally Birdman, Synchro
//! Magnet) turn two Karakuri into a Shogun, which brings another Karakuri
//! from the Deck.  Karakuri Shogun "Burei" flips a monster each turn.
//!
//! "Must attack" makes the Battle Phase the danger: it is entered only when
//! no Karakuri would have to suicide into a bigger monster.

use crate::agent::{value, Outcome, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Position};
use crate::tactics::default_outcome;

pub const DECK: &str = "Karakuri Workshop";

const MERCHANT: u32 = 30230789;
const SOLDIER: u32 = 3846170;
const KUICK: u32 = 6276588;
const SAZANK: u32 = 93724592;
const BUSHI: u32 = 39118197;
const NISHIPACHI: u32 = 66625883;
const SAIZAN: u32 = 70271583;
const SYNCHRO_MAGNET: u32 = 50702124;
const BIRDMAN: u32 = 64034255;
const CYBER_DRAGON: u32 = 70095154;
const ANATOMY: u32 = 85541675;
const SHOWDOWN_CASTLE: u32 = 22751868;
const CASH_CACHE: u32 = 80204957;
const GOLD_DUST: u32 = 16708652;
const MACHINE_DUPLICATION: u32 = 63995093;
const MACHINA_ARMORED_UNIT: u32 = 31828916;
const LIMITER_REMOVAL: u32 = 23171610;
const TRICK_HOUSE: u32 = 33184236;
const KLOCK: u32 = 79178930;
const BUREI: u32 = 23874409;
const BUREIDO: u32 = 66976526;
const BLACK_ROSE_DRAGON: u32 = 73580471;
const SET_KARAKURI: u16 = 0x11;

#[derive(Clone, Default)]
pub struct Karakuri;

impl Karakuri {
    fn is_karakuri(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.in_set(SET_KARAKURI)
    }

    fn is_machine(ctx: &Ctx, card: &CardView) -> bool {
        ctx.view_data(card).race & crate::cards::races::MACHINE != 0
    }

    /// Would entering the Battle Phase force a Karakuri worth keeping into a
    /// battle it loses?
    fn suicide(&self, ctx: &Ctx) -> bool {
        let targets = ctx.monsters(ctx.opp);
        if targets.is_empty() {
            return false;
        }
        ctx.monsters(ctx.me).into_iter().filter(|c| ctx.can_attack(c) && Self::is_karakuri(ctx, c)).any(|attacker| {
            let best = targets
                .iter()
                .map(|target| {
                    self.attack_outcome(ctx, attacker, target).unwrap_or_else(|| default_outcome(ctx, attacker, target, 0))
                })
                .max_by_key(|o| match o {
                    Outcome::Win { .. } => 3,
                    Outcome::Trade => 2,
                    Outcome::Bounce => 1,
                    Outcome::Lose => 0,
                });
            best == Some(Outcome::Lose) && value(self, ctx, None, Some(attacker)) >= 1000
        })
    }

    /// Their face-up Attack Position monster that Burei should flip into a
    /// weaker Defense Position one for our attackers.
    fn burei_target<'a>(ctx: &Ctx<'a>) -> Option<&'a CardView> {
        let best = ctx.my_best_attack();
        ctx.monsters(ctx.opp)
            .into_iter()
            .filter(|c| c.position.face_up && c.position.attack && c.attack >= best && c.defense < best)
            .max_by_key(|c| ctx.threat(c))
    }
}

impl Strategy for Karakuri {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            BUREIDO => 2800,
            BUREI => 2600,
            CYBER_DRAGON => 2100,
            KUICK | KLOCK => 1900,
            BUSHI => 1800,
            MERCHANT | TRICK_HOUSE => 1700,
            NISHIPACHI | SAZANK | SHOWDOWN_CASTLE | CASH_CACHE => 1600,
            SAIZAN | SOLDIER | ANATOMY => 1500,
            MACHINE_DUPLICATION => 1400,
            BIRDMAN | MACHINA_ARMORED_UNIT => 1300,
            SYNCHRO_MAGNET | LIMITER_REMOVAL => 1200,
            GOLD_DUST => 1100,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        for code in [SHOWDOWN_CASTLE, ANATOMY, MACHINA_ARMORED_UNIT] {
            if !ctx.face_up_on_field(ctx.me, code) {
                if let Some(i) = t.activate_from(code, Location::Hand) {
                    return t.pick(i);
                }
            }
        }
        // Karakuri Anatomy: two counters are two cards.
        let counters = ctx.spell_traps(ctx.me).iter().find(|c| ctx.is(c, ANATOMY)).map_or(0, |c| c.counters);
        if counters >= 2 {
            if let Some(i) = t.activate_from(ANATOMY, Location::SpellTrapZone) {
                return t.pick(i);
            }
        }
        // Machine Duplication: two more copies of a 500 ATK Karakuri.
        let copied = ctx
            .monsters(ctx.me)
            .into_iter()
            .filter(|c| c.position.face_up && Self::is_machine(&ctx, c) && c.attack <= 500)
            .max_by_key(|c| (ctx.is(c, NISHIPACHI), ctx.is(c, MERCHANT)));
        if let Some(copied) = copied {
            if ctx.free_monster_zones(ctx.me) >= 2 {
                if let Some(i) = t.activate(MACHINE_DUPLICATION) {
                    return t.pick_targeting(i, vec![copied.at]);
                }
            }
        }
        // Cash Cache: a Karakuri from the Deck, and a position change to cash in.
        if ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && Self::is_karakuri(&ctx, c)) {
            if let Some(i) = t.activate(CASH_CACHE) {
                return t.pick(i);
            }
        }
        // Burei: turn their wall into a Defense Position target.
        if ctx.main1() && t.has(ChoiceKind::EnterBattle) {
            if let Some(target) = Self::burei_target(&ctx) {
                if let Some(i) = t.activate_from(BUREI, Location::MonsterZone) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
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

    fn main_phase_late(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Genex Ally Birdman: bounce a spent monster for a Tuner when a
        // Machine non-Tuner can meet it in a Synchro.
        let partners = ctx
            .monsters(ctx.me)
            .iter()
            .filter(|c| c.position.face_up && Self::is_machine(&ctx, c) && !ctx.view_data(c).is_tuner() && matches!(c.level, 4 | 5))
            .count();
        if partners >= 1 && ctx.monsters(ctx.me).len() >= 2 {
            if let Some(i) = t.activate_from(BIRDMAN, Location::Hand) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let blocked = ctx.opp_best_attack();
        Some(match (choice.kind, code) {
            // The Merchant's search is worth more than any body.
            (ChoiceKind::NormalSummon, MERCHANT) => Some(2200.0),
            (ChoiceKind::NormalSummon, BUSHI) if blocked < 1800 => Some(1900.0),
            (ChoiceKind::NormalSummon, KUICK) if blocked < 1700 => Some(1850.0),
            (ChoiceKind::NormalSummon, NISHIPACHI) => Some(1700.0),
            // Sazank's flip sends a face-up monster to the Graveyard.
            (ChoiceKind::SetMonster, SAZANK) if ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up) => Some(1750.0),
            (ChoiceKind::SetMonster, SAIZAN) => Some(1500.0),
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code().unwrap_or(0));
        Some(match code {
            // Destroys every card on the field: only when losing badly.
            BLACK_ROSE_DRAGON => ctx.field_strength(ctx.opp) > ctx.field_strength(ctx.me) + 2500,
            _ => return None,
        })
    }

    fn wants_battle(&self, t: &Turn) -> Option<bool> {
        // Past this point every Karakuri that can attack must.
        let ctx = t.ctx;
        let lethal = ctx.monsters(ctx.opp).is_empty();
        Some(lethal || !self.suicide(&ctx))
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        let ctx = t.ctx;
        let data = ctx.data(ctx.canonical(code));
        // A Karakuri in Attack Position must attack: keep the small ones back.
        (data.in_set(SET_KARAKURI) && data.attack < ctx.opp_best_attack()).then_some(Position::FACE_UP_DEFENSE)
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let best_target = ctx.monsters(ctx.opp).into_iter().chain(ctx.spell_traps(ctx.opp)).max_by_key(|c| ctx.threat(c));
        Some(match code {
            // Only offered when a Defense Position Karakuri is attacked.
            KLOCK => {
                let wipe: i32 = ctx.monsters(ctx.opp).iter().filter(|c| c.position.face_up).map(|c| ctx.threat(c)).sum();
                if wipe >= 1500 { Response::new(92.0) } else { Response::no() }
            }
            // Only offered when a Karakuri changes position.
            TRICK_HOUSE => match best_target {
                Some(target) => Response::targeting(70.0, vec![target.at]),
                None => Response::no(),
            },
            // From the hand, when we Synchro Summon: a free Tuner.
            SYNCHRO_MAGNET => Response::new(30.0),
            GOLD_DUST | LIMITER_REMOVAL | BIRDMAN => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if member.at.controller != ctx.me {
            return None;
        }
        let code = member.code.map(|c| ctx.canonical(c))?;
        // Searches: a Tuner if we have none, else the best Karakuri.
        if matches!(t.decision.hint, Hint::AddToHand | Hint::SpecialSummon) && member.at.location == Location::Deck {
            let tuner = ctx.monsters(ctx.me).iter().chain(ctx.hand().iter()).any(|c| ctx.view_data(c).is_tuner());
            let bonus = if !tuner && ctx.data(code).is_tuner() { 800.0 } else { 0.0 };
            return Some(value(self, &ctx, Some(code), None) as f64 + bonus);
        }
        None
    }
}
