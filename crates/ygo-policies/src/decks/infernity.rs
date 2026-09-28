//! "Infernity Infinity": empty-hand combo.
//!
//! Almost every Infernity effect needs an empty hand, so the turn is spent
//! getting rid of cards first (set every Trap, Infernity Launcher / Foolish
//! Burial / Dark Grepher dump monsters, Plaguespreader puts the last card back
//! on the deck) and only then comboing: Beetle into two Beetles, Mirage and
//! Launcher reviving two Infernities, Necromancer reviving one, and Synchro
//! Summons into Infernity Doom Dragon, Trishula or Mist Wurm.

use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};

pub const DECK: &str = "Infernity Infinity";

const GUARDIAN: u32 = 51566770;
const ARCHFIEND: u32 = 99177923;
const NECROMANCER: u32 = 56209279;
const BEETLE: u32 = 49080532;
const MIRAGE: u32 = 86197239;
const AVENGER: u32 = 85475641;
const GLOW_UP_BULB: u32 = 67441435;
const PLAGUESPREADER: u32 = 33420078;
const DARK_GREPHER: u32 = 14536035;
const STYGIAN_PATROL: u32 = 13521194;
const LAUNCHER: u32 = 66957584;
const FOOLISH_BURIAL: u32 = 81439174;
const FORBIDDEN_CHALICE: u32 = 25789292;
const LIGHTNING_VORTEX: u32 = 69162969;
const INFERNITY_FORCE: u32 = 18712704;
const DUST_TORNADO: u32 = 60082869;
const DIVINE_WRATH: u32 = 49010598;
const DOOM_DRAGON: u32 = 72896720;
const CATASTOR: u32 = 26593852;
const GOYO_GUARDIAN: u32 = 7391448;
const SCRAP_DRAGON: u32 = 76774528;
const STARDUST: u32 = 44508096;
const TRISHULA: u32 = 52687916;
const BRIONAC: u32 = 50321796;
const BLACK_ROSE: u32 = 73580472;
const MIST_WURM: u32 = 27315304;
const SET_INFERNITY: u16 = 0xb;

#[derive(Default)]
pub struct Infernity;

impl Infernity {
    fn infernity_monster(ctx: &Ctx, code: u32) -> bool {
        let d = ctx.data(code);
        d.is_monster() && d.in_set(SET_INFERNITY)
    }

    fn empty_hand(ctx: &Ctx) -> bool {
        ctx.hand_size(ctx.me) == 0
    }

    fn graveyard_infernities(ctx: &Ctx) -> usize {
        ctx.graveyard(ctx.me)
            .iter()
            .filter(|c| c.code.map_or(false, |k| Self::infernity_monster(ctx, k)) && !ctx.is(c, MIRAGE))
            .count()
    }

    fn opp_face_up_threat(ctx: &Ctx) -> i32 {
        ctx.monsters(ctx.opp).iter().filter(|c| c.position.face_up).map(|c| ctx.threat(c)).sum()
    }
}

impl Strategy for Infernity {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            DOOM_DRAGON => 3300,
            TRISHULA => 3200,
            MIST_WURM | SCRAP_DRAGON => 2900,
            STARDUST | GOYO_GUARDIAN => 2700,
            BRIONAC | BLACK_ROSE => 2400,
            CATASTOR => 2300,
            LAUNCHER => 2100,
            MIRAGE => 2000,
            ARCHFIEND => 1900,
            BEETLE => 1800,
            NECROMANCER => 1700,
            DARK_GREPHER | STYGIAN_PATROL => 1600,
            LIGHTNING_VORTEX | DIVINE_WRATH => 1500,
            GUARDIAN => 1300,
            INFERNITY_FORCE | DUST_TORNADO => 1300,
            AVENGER => 1000,
            FOOLISH_BURIAL => 1000,
            FORBIDDEN_CHALICE => 800,
            GLOW_UP_BULB | PLAGUESPREADER => 700,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let empty = Self::empty_hand(&ctx);

        // Board control that is worth a card.
        if let Some(i) = t.activate_from(DOOM_DRAGON, Location::MonsterZone) {
            if let Some(best) = ctx.monsters(ctx.opp).into_iter().max_by_key(|c| ctx.threat(c)) {
                return t.pick_targeting(i, vec![best.at]);
            }
        }
        if Self::opp_face_up_threat(&ctx) >= 2500 {
            if let Some(i) = t.activate(LIGHTNING_VORTEX) {
                return t.pick(i);
            }
        }

        // 1. Empty the hand.
        if !empty {
            if let Some(i) = t.find_where(|c| {
                c.kind == ChoiceKind::SetSpellTrap
                    && c.code().map_or(false, |k| ctx.data(k).is_trap() || ctx.canonical(k) == FORBIDDEN_CHALICE)
            }) {
                return t.pick(i);
            }
            if let Some(i) = t.activate_from(LAUNCHER, Location::Hand) {
                return t.pick(i);
            }
            if let Some(i) = t.activate(FOOLISH_BURIAL) {
                return t.pick(i);
            }
            let spare_infernity = ctx
                .hand()
                .iter()
                .filter(|c| c.code.map_or(false, |k| Self::infernity_monster(&ctx, k)))
                .count()
                > usize::from(t.has(ChoiceKind::NormalSummon));
            if spare_infernity {
                if let Some(i) = t.activate_from(LAUNCHER, Location::SpellTrapZone) {
                    return t.pick(i);
                }
            }
            if let Some(i) = t.activate_from(DARK_GREPHER, Location::MonsterZone) {
                return t.pick(i);
            }
            if ctx.hand_size(ctx.me) == 1 {
                if let Some(i) = t.activate_from(PLAGUESPREADER, Location::Graveyard) {
                    return t.pick(i);
                }
            }
            return None; // Normal Summon next, then come back here
        }

        // 2. Hand is empty: combo.
        for (code, location) in [
            (MIRAGE, Location::MonsterZone),
            (LAUNCHER, Location::SpellTrapZone),
            (NECROMANCER, Location::MonsterZone),
            (BEETLE, Location::MonsterZone),
            (GLOW_UP_BULB, Location::Graveyard),
        ] {
            if ctx.free_monster_zones(ctx.me) == 0 {
                break;
            }
            let needs_graveyard = matches!(code, MIRAGE | LAUNCHER | NECROMANCER);
            if needs_graveyard && Self::graveyard_infernities(&ctx) == 0 {
                continue;
            }
            if let Some(i) = t.activate_from(code, location) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let graveyard = Self::graveyard_infernities(&ctx);
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, MIRAGE) if graveyard >= 2 => Some(2600.0),
            (ChoiceKind::NormalSummon, BEETLE) => Some(2300.0),
            (ChoiceKind::NormalSummon, NECROMANCER) if graveyard >= 1 => Some(2100.0),
            (ChoiceKind::NormalSummon, ARCHFIEND | DARK_GREPHER | STYGIAN_PATROL) => Some(1800.0),
            (ChoiceKind::NormalSummon, MIRAGE) => Some(1000.0),
            (ChoiceKind::NormalSummon, GUARDIAN) => Some(1200.0),
            (ChoiceKind::NormalSummon, _) => Some(500.0),
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code().unwrap_or(0));
        Some(match code {
            BLACK_ROSE => {
                let theirs: i32 = ctx.monsters(ctx.opp).iter().chain(ctx.spell_traps(ctx.opp).iter()).map(|c| ctx.threat(c)).sum();
                theirs >= 4000 && ctx.monsters(ctx.me).len() <= 2
            }
            _ => true,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let hostile = t.hostile_top();
        Some(match code {
            INFERNITY_FORCE => Response::new(90.0),
            DIVINE_WRATH if ctx.hand_size(ctx.me) >= 1 && hostile.matches(|link| ctx.data(link.code).is_monster()) => {
                Response::new(75.0)
            }
            DIVINE_WRATH => Response::no(),
            DUST_TORNADO => {
                let backrow = ctx.spell_traps(ctx.opp);
                let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(crate::model::Phase::End);
                let target = ctx.set_backrow(ctx.opp).first().copied().or_else(|| backrow.first().copied());
                match target {
                    Some(target) if end_of_their_turn || (ctx.my_turn() && ctx.main1()) => {
                        Response::targeting(25.0, vec![target.at])
                    }
                    _ => Response::no(),
                }
            }
            ARCHFIEND => Response::new(60.0),
            AVENGER => Response::new(40.0),
            // Ignition effects run at our own pace in the Main Phase.
            MIRAGE | LAUNCHER | NECROMANCER | BEETLE | DARK_GREPHER | PLAGUESPREADER | GLOW_UP_BULB | DOOM_DRAGON
            | FORBIDDEN_CHALICE => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if member.at.controller != ctx.me {
            return None;
        }
        let code = member.code.map(|c| ctx.canonical(c));
        let worth = value(self, &ctx, code, None) as f64;
        match t.memory.last_activated.map(|c| ctx.canonical(c)) {
            // Dumps from the deck: the best revival targets.
            Some(FOOLISH_BURIAL) | Some(DARK_GREPHER) if member.at.location == Location::Deck => {
                let infernity = code.map_or(false, |k| Self::infernity_monster(&ctx, k)) && code != Some(MIRAGE);
                return Some(worth + if infernity { 1000.0 } else { 0.0 });
            }
            // Dumps from the hand: every card we can lose is progress.
            Some(LAUNCHER) | Some(DARK_GREPHER) if member.at.location == Location::Hand => return Some(-worth),
            _ => {}
        }
        if t.decision.hint == Hint::SynchroMaterial {
            return Some(-worth);
        }
        None
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let data = t.ctx.data(t.ctx.canonical(code));
        // Everything that can leave the hand should: an empty hand is the engine.
        Some(data.is_trap() || data.is(crate::cards::types::QUICKPLAY) || data.is_spell())
    }
}
