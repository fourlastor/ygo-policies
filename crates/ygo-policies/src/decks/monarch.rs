//! "Emperor, Arise!": Monarch tribute control.
//!
//! Every turn, make one piece of Tribute fodder (Jester Confit, Swap Frog,
//! Treeborn Frog's self-revival, Dandylion's tokens, One for One, or the
//! opponent's own monster through Soul Exchange) and Tribute Summon a 2400 ATK
//! Monarch whose summon effect removes their best card.  Battle Fader, Gorz
//! and Tragoedia punish attacks into an empty board.  Spells/Traps stay in the
//! hand while Treeborn Frog is in the Graveyard: it only revives with an empty
//! backrow.

use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member, Position};

pub const DECK: &str = "Emperor, Arise!";

const THESTALOS: u32 = 26205777;
const CAIUS: u32 = 9748752;
const TRAGOEDIA: u32 = 98777036;
const MOBIUS: u32 = 4929256;
const RAIZA: u32 = 73125233;
const GORZ: u32 = 44330098;
const SWAP_FROG: u32 = 9126351;
const JESTER_CONFIT: u32 = 8487449;
const DANDYLION: u32 = 15341821;
const GRAVEKEEPERS_SPY: u32 = 24317029;
const BATTLE_FADER: u32 = 19665973;
const DUPE_FROG: u32 = 46239604;
const TREEBORN_FROG: u32 = 12538374;
const SOUL_EXCHANGE: u32 = 68005187;
const POT_OF_AVARICE: u32 = 67169062;
const ONE_FOR_ONE: u32 = 2295440;

const MONARCHS: [u32; 4] = [CAIUS, RAIZA, MOBIUS, THESTALOS];

#[derive(Default)]
pub struct Monarch;

impl Monarch {
    fn is_monarch(ctx: &Ctx, code: u32) -> bool {
        MONARCHS.contains(&ctx.canonical(code))
    }

    fn monarch_in_hand(ctx: &Ctx) -> bool {
        ctx.hand().iter().any(|c| c.code.map_or(false, |k| Self::is_monarch(ctx, k)))
    }

    /// Our monsters available as Tribute material.
    fn fodder(ctx: &Ctx) -> usize {
        ctx.monsters(ctx.me).len()
    }

    fn treeborn_engine(ctx: &Ctx) -> bool {
        ctx.graveyard(ctx.me).iter().any(|c| ctx.is(c, TREEBORN_FROG))
    }

    /// Mobius' optional trigger destroys Spells/Traps on either field: only
    /// worth it when some are theirs.
    fn mobius_has_targets(ctx: &Ctx) -> bool {
        !ctx.spell_traps(ctx.opp).is_empty()
    }

    /// Treeborn Frog's revival is a Special Summon: Pot of Duality cannot
    /// be activated in the same turn.  The card is worth more than a Frog on
    /// the field unless a Monarch is waiting for its Tribute.
    fn keeps_treeborn_down(ctx: &Ctx) -> bool {
        ctx.in_hand(crate::staples::POT_OF_DUALITY) && (!Self::monarch_in_hand(ctx) || Self::fodder(ctx) > 0)
    }

    /// How much a Monarch's summon effect is worth on this board.
    fn monarch_score(ctx: &Ctx, code: u32) -> f64 {
        let opp_backrow = ctx.spell_traps(ctx.opp).len();
        let opp_best = ctx
            .monsters(ctx.opp)
            .iter()
            .chain(ctx.spell_traps(ctx.opp).iter())
            .map(|c| ctx.threat(c))
            .max()
            .unwrap_or(0);
        2400.0
            + match ctx.canonical(code) {
                MOBIUS => 700.0 * opp_backrow.min(2) as f64,
                CAIUS => opp_best as f64 * 0.8 + 100.0,
                RAIZA => opp_best as f64 * 0.7 + 100.0,
                THESTALOS => 250.0 * (ctx.hand_size(ctx.opp) > 0) as i32 as f64,
                _ => 0.0,
            }
    }
}

impl Strategy for Monarch {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            CAIUS | RAIZA | MOBIUS | THESTALOS => 2400,
            GORZ => 2300,
            TRAGOEDIA => 1500,
            SOUL_EXCHANGE => 1400,
            BATTLE_FADER => 1300,
            GRAVEKEEPERS_SPY => 1100,
            DUPE_FROG => 900,
            SWAP_FROG => 700,
            ONE_FOR_ONE | POT_OF_AVARICE => 1200,
            DANDYLION => 300,
            TREEBORN_FROG => 150,
            JESTER_CONFIT => 100,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.activate(POT_OF_AVARICE) {
            let monsters = ctx.graveyard(ctx.me).iter().filter(|c| ctx.view_data(c).is_monster()).count();
            if monsters >= 5 {
                return t.pick(i);
            }
        }
        // With no Monarch in hand, Swap Frog returns the one on the field:
        // it is Tribute Summoned again, and its effect is used again.  Only
        // while that effect has something to take.
        if ctx.main1() && !ctx.obs.summon_used && !Self::monarch_in_hand(&ctx) && Self::fodder(&ctx) >= 2 {
            if let Some(i) = t.activate_from(SWAP_FROG, Location::MonsterZone) {
                let theirs = ctx.monsters(ctx.opp).len() + ctx.spell_traps(ctx.opp).len();
                let again = ctx
                    .monsters(ctx.me)
                    .into_iter()
                    .filter(|c| c.position.face_up)
                    .filter_map(|c| c.code.map(|k| (c, ctx.canonical(k))))
                    .filter(|(_, k)| Self::is_monarch(&ctx, *k) && !(matches!(*k, CAIUS | RAIZA) && theirs == 0))
                    .map(|(c, k)| (Self::monarch_score(&ctx, k), c.at))
                    .filter(|(score, _)| *score > 2400.0)
                    .max_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, at)) = again {
                    return t.pick_targeting(i, vec![at]);
                }
            }
        }
        // With only Monarchs in hand and no monster of ours, no Summon is
        // offered until the fodder is there: what counts is that the Normal
        // Summon is still to be made.
        if !Self::monarch_in_hand(&ctx) || ctx.obs.summon_used {
            return None;
        }
        let monarch_ready = t.find_where(|c| {
            c.kind == ChoiceKind::NormalSummon && c.code().map_or(false, |k| Self::is_monarch(&ctx, k))
        });
        // A Set Gravekeeper's Spy is Flip Summoned before a Monarch takes its
        // Tribute: its effect brings a second one from the Deck.
        if monarch_ready.is_some() {
            let flip = t.find_where(|c| {
                c.kind == ChoiceKind::ChangePosition
                    && c.code().map(|k| ctx.canonical(k)) == Some(GRAVEKEEPERS_SPY)
                    && t.view(c).map_or(false, |v| !v.position.face_up)
            });
            if let Some(i) = flip {
                return t.pick(i);
            }
        }
        // Their monster as the Tribute, when it is one worth removing: the
        // Monarch's own effect then takes a second card, and our monster stays.
        if let Some(i) = t.activate(SOUL_EXCHANGE) {
            let best = ctx.monsters(ctx.opp).into_iter().max_by_key(|c| ctx.threat(c));
            if let Some(best) = best.filter(|c| ctx.threat(c) >= 1500) {
                return t.pick_targeting(i, vec![best.at]);
            }
        }
        if monarch_ready.is_some() && Self::fodder(&ctx) > 0 {
            return None; // tactics::normal_summon picks the best Monarch
        }
        // Make fodder without using the Normal Summon.
        for code in [JESTER_CONFIT, SWAP_FROG] {
            if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(code), Some(Location::Hand)) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(ONE_FOR_ONE) {
            return t.pick(i);
        }
        // Tribute their monster instead of ours.
        if let Some(i) = t.activate(SOUL_EXCHANGE) {
            if let Some(best) = ctx.monsters(ctx.opp).into_iter().max_by_key(|c| ctx.threat(c)) {
                if ctx.threat(best) >= 1500 || Self::fodder(&ctx) == 0 {
                    return t.pick_targeting(i, vec![best.at]);
                }
            }
        }
        None
    }

    fn allow_staple(&self, t: &Turn, code: u32) -> bool {
        let ctx = t.ctx;
        match code {
            // Pot of Duality locks Special Summons: not while we still need fodder.
            crate::staples::POT_OF_DUALITY => {
                !(Self::monarch_in_hand(&ctx)
                    && Self::fodder(&ctx) == 0
                    && ctx.hand().iter().any(|c| [JESTER_CONFIT, SWAP_FROG].iter().any(|k| ctx.is(c, *k))))
            }
            // Dark Hole is not for one monster the Monarch of this turn beats.
            crate::staples::DARK_HOLE => !crate::tactics::outgrown(self, t),
            _ => true,
        }
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        if Self::is_monarch(&ctx, code) {
            // Caius and Raiza must target a card when they arrive: with
            // nothing of theirs on the field it is one of ours, often the
            // Monarch itself.
            let theirs = ctx.monsters(ctx.opp).len() + ctx.spell_traps(ctx.opp).len();
            let hits_our_own = matches!(code, CAIUS | RAIZA) && theirs == 0;
            return Some(match choice.kind {
                ChoiceKind::NormalSummon if !hits_our_own => Some(Self::monarch_score(&ctx, code)),
                _ => None,
            });
        }
        let data = ctx.data(code);
        if data.level >= 5 {
            return Some(None); // Gorz / Tragoedia are summoned by their effects
        }
        // No Monarch to summon: leave a body for next turn's Tribute.
        Some(match (choice.kind, code) {
            (ChoiceKind::SetMonster, GRAVEKEEPERS_SPY | DUPE_FROG) => Some(900.0),
            (ChoiceKind::SetMonster, DANDYLION) => Some(800.0),
            (ChoiceKind::NormalSummon, SWAP_FROG) => Some(850.0),
            (ChoiceKind::SetMonster, TREEBORN_FROG) => Some(300.0),
            (ChoiceKind::NormalSummon, JESTER_CONFIT | BATTLE_FADER) => None,
            // Battle Fader is a hand trap: Set, it is a monster with 0 DEF.
            (ChoiceKind::SetMonster, BATTLE_FADER) => None,
            (ChoiceKind::SetMonster, _) => Some(data.defense as f64 / 10.0),
            _ => None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code().unwrap_or(0));
        // Fodder only when a Monarch can use it.  Jester Confit also on its
        // own while they control a face-up monster: at their End Phase it
        // goes back to the hand and takes one with it.
        let bounce = code == JESTER_CONFIT && ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up);
        Some(match code {
            JESTER_CONFIT | SWAP_FROG => Self::monarch_in_hand(&ctx) || bounce,
            _ => true,
        })
    }

    fn wants_battle(&self, t: &Turn) -> Option<bool> {
        let ctx = t.ctx;
        Some(ctx.monsters(ctx.me).iter().any(|c| ctx.can_attack(c) && c.attack > 0))
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        Some(match code {
            // Hand traps: always.
            BATTLE_FADER | GORZ => Response::new(70.0),
            // Tragoedia: the hand trap and, on the field, its steal; its
            // Level change does nothing on its own.
            TRAGOEDIA if choice.description & 0xf == 2 => Response::no(),
            TRAGOEDIA => Response::new(70.0),
            TREEBORN_FROG if choice.at().map(|a| a.location) == Some(Location::Graveyard) && Self::keeps_treeborn_down(&ctx) => {
                Response::no()
            }
            TREEBORN_FROG => return None,
            MOBIUS if !Self::mobius_has_targets(&ctx) => Response::no(),
            CAIUS | RAIZA | MOBIUS | THESTALOS => Response::new(50.0),
            _ => return None,
        })
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        let ctx = t.ctx;
        match t.decision.subject.map(|c| ctx.canonical(c)) {
            Some(MOBIUS) => Some(Self::mobius_has_targets(&ctx)),
            Some(TREEBORN_FROG) if Self::keeps_treeborn_down(&ctx) => Some(false),
            _ => None,
        }
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let mine = member.at.controller == ctx.me;
        let code = member.code.map(|c| ctx.canonical(c));
        match t.memory.last_activated.map(|c| ctx.canonical(c)) {
            // Swap Frog / One for One: dump Treeborn Frog, fetch cheap fodder.
            Some(SWAP_FROG) if mine && code == Some(TREEBORN_FROG) && member.at.location == Location::Deck => {
                return Some(5000.0)
            }
            Some(ONE_FOR_ONE) if mine && t.decision.hint.is_gain() => {
                return Some(match code {
                    Some(TREEBORN_FROG) if !Self::treeborn_engine(&ctx) => 3000.0,
                    Some(BATTLE_FADER) => 2000.0,
                    Some(JESTER_CONFIT) => 1500.0,
                    _ => 0.0,
                })
            }
            _ => {}
        }
        if !mine {
            return None;
        }
        // Tribute fodder: tokens and frogs first, Dandylion (it leaves
        // tokens) and Treeborn (it comes back) are the best to lose.
        if matches!(t.decision.hint, Hint::Release | Hint::Tribute) {
            let v = value(self, &ctx, code, ctx.card(member.at).filter(|c| c.known()));
            return Some(-(v as f64));
        }
        None
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        let ctx = t.ctx;
        let code = ctx.canonical(code);
        (ctx.data(code).race & races::AQUA != 0 && ctx.data(code).level <= 2 || code == DANDYLION)
            .then_some(Position::FACE_UP_DEFENSE)
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let ctx = t.ctx;
        if Self::treeborn_engine(&ctx) {
            return Some(false);
        }
        let data = ctx.data(ctx.canonical(code));
        Some(data.is_trap())
    }
}
