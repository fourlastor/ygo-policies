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
const FOOLISH_BURIAL: u32 = 81439173;
const FORBIDDEN_CHALICE: u32 = 25789292;
const LIGHTNING_VORTEX: u32 = 69162969;
const INFERNITY_FORCE: u32 = 18712704;
const DUST_TORNADO: u32 = 60082869;
const DIVINE_WRATH: u32 = 49010598;
const DOOM_DRAGON: u32 = 72896720;
const CATASTOR: u32 = 26593852;
const SCRAP_DRAGON: u32 = 76774528;
const STARDUST: u32 = 44508094;
const TRISHULA: u32 = 52687916;
const BRIONAC: u32 = 50321796;
const BLACK_ROSE: u32 = 73580471;
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

    /// Another monster stays in hand after this turn's Normal Summon: the
    /// hand is not empty this turn.  Plaguespreader Zombie in the Graveyard
    /// returns one card to the Deck.
    fn monster_stays(ctx: &Ctx) -> bool {
        let monsters = ctx.hand().iter().filter(|c| ctx.view_data(c).is_monster()).count();
        let returned = usize::from(ctx.graveyard(ctx.me).iter().any(|c| ctx.is(c, PLAGUESPREADER)));
        monsters > 1 + returned
    }

    /// A copy of one of our cards is still in the Deck (`copies`: how many the list plays).
    fn in_deck(ctx: &Ctx, code: u32, copies: usize) -> bool {
        let seen = [Location::Hand, Location::MonsterZone, Location::SpellTrapZone, Location::Graveyard, Location::Banished]
            .into_iter()
            .flat_map(|location| ctx.pile(ctx.me, location))
            .filter(|c| ctx.is(c, code))
            .count();
        seen < copies
    }

    /// What Archfiend's effect should add to the hand: a card that leaves it
    /// again this turn, or nothing.
    fn fetch(ctx: &Ctx) -> Option<u32> {
        let graveyard = Self::graveyard_infernities(ctx);
        let summon = !ctx.obs.summon_used;
        [
            (LAUNCHER, 1, true),
            (MIRAGE, 3, summon && graveyard >= 2),
            (BEETLE, 3, summon),
            (NECROMANCER, 3, summon && graveyard >= 1),
            (ARCHFIEND, 3, summon),
            (INFERNITY_FORCE, 2, true),
            (GUARDIAN, 3, summon),
        ]
        .into_iter()
        .find(|(code, copies, playable)| *playable && Self::in_deck(ctx, *code, *copies))
        .map(|(code, ..)| code)
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
            STARDUST => 2700,
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
            // Stygian Street Patrol in the Graveyard puts a Fiend from the hand
            // on the field: one card less in hand, and no Normal Summon spent.
            if let Some(i) = t.activate_from(STYGIAN_PATROL, Location::Graveyard) {
                return t.pick(i);
            }
            if let Some(i) = t.activate(FOOLISH_BURIAL) {
                return t.pick(i);
            }
            // Launcher sends an Infernity monster to the Graveyard whenever the
            // Normal Summon has another monster to take.
            let infernities = ctx.hand().iter().filter(|c| c.code.map_or(false, |k| Self::infernity_monster(&ctx, k))).count();
            let monsters = ctx.hand().iter().filter(|c| ctx.view_data(c).is_monster()).count();
            let spare_infernity = infernities >= 1 && monsters > usize::from(t.has(ChoiceKind::NormalSummon));
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
            // Launcher is one card for two monsters: not for one.
            if code == LAUNCHER && Self::graveyard_infernities(&ctx) < 2 {
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
        // No effect of ours works with a card in hand, and the small monsters
        // have nothing else: face-up only when this Summon empties the hand
        // and something follows.
        let stays = Self::monster_stays(&ctx);
        // Mirage brings two Infernity monsters back.
        let mirage_up = !stays && graveyard >= 2;
        // A small Tuner makes a Synchro Monster with a non-Tuner of ours
        // (whatever stays in hand), or with what Launcher, Necromancer or
        // Mirage bring back once the hand is empty.
        let reviver = [LAUNCHER, NECROMANCER, MIRAGE].iter().any(|code| ctx.face_up_on_field(ctx.me, *code));
        let partner = crate::tactics::synchro_with_tuner(self, &ctx, ctx.data(code).level).is_some();
        let small_tuner = matches!(code, AVENGER | PLAGUESPREADER | GLOW_UP_BULB);
        let tuner_up = partner || (!stays && reviver && graveyard >= 1);
        // Guardian in Attack Position takes the damage of every attack: under
        // a stronger monster it goes face-down, unless a Tuner of ours waits.
        let tuner_waits = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).is_tuner());
        let guardian_down = ctx.opp_best_attack() > 1200 && !tuner_waits;
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, MIRAGE) if mirage_up => Some(2600.0),
            (ChoiceKind::NormalSummon, BEETLE) => Some(2300.0),
            (ChoiceKind::NormalSummon, NECROMANCER) if graveyard >= 1 => Some(2100.0),
            // The Synchro Summon is there to make: before a plain attacker.
            (ChoiceKind::NormalSummon, _) if small_tuner && partner => Some(2000.0),
            (ChoiceKind::NormalSummon, ARCHFIEND | DARK_GREPHER | STYGIAN_PATROL) => Some(1800.0),
            (ChoiceKind::SetMonster, GUARDIAN) if guardian_down => Some(1200.0),
            (ChoiceKind::NormalSummon, GUARDIAN) if guardian_down => None,
            (ChoiceKind::NormalSummon, GUARDIAN) => Some(1200.0),
            (ChoiceKind::SetMonster, MIRAGE) if !mirage_up => Some(1000.0),
            (ChoiceKind::NormalSummon, MIRAGE) => None,
            (ChoiceKind::SetMonster, _) if small_tuner && !tuner_up => Some(500.0),
            (ChoiceKind::NormalSummon, _) if small_tuner && !tuner_up => None,
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
            // Its search, with nothing to fetch that leaves the hand again: the hand stays empty.
            ARCHFIEND if choice.at().map_or(false, |at| at.location == Location::MonsterZone) && Self::fetch(&ctx).is_none() => {
                Response::no()
            }
            ARCHFIEND => Response::new(60.0),
            AVENGER => Response::new(40.0),
            // Ignition effects run at our own pace in the Main Phase.
            MIRAGE | LAUNCHER | NECROMANCER | BEETLE | DARK_GREPHER | PLAGUESPREADER | GLOW_UP_BULB | DOOM_DRAGON
            | FORBIDDEN_CHALICE => Response::no(),
            _ => return None,
        })
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        let ctx = t.ctx;
        // Archfiend's search on the field (not its Special Summon from the hand).
        let on_field = t.decision.choices.iter().any(|c| c.kind == ChoiceKind::Yes && c.at().map_or(false, |at| at.location == Location::MonsterZone));
        if on_field && t.decision.subject.map(|c| ctx.canonical(c)) == Some(ARCHFIEND) {
            return Some(Self::fetch(&ctx).is_some());
        }
        None
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
            // Patrol's Fiend from the hand: the one whose effect follows.
            Some(STYGIAN_PATROL) if member.at.location == Location::Hand => {
                let graveyard = Self::graveyard_infernities(&ctx);
                return Some(match code {
                    Some(MIRAGE) if graveyard >= 2 => 2600.0,
                    Some(ARCHFIEND) => 2400.0,
                    Some(NECROMANCER) if graveyard >= 1 => 2100.0,
                    Some(STYGIAN_PATROL) => 1600.0,
                    Some(GUARDIAN) => 1300.0,
                    _ => worth / 10.0,
                });
            }
            // Archfiend's search: a card that leaves the hand again this turn.
            Some(ARCHFIEND) if member.at.location == Location::Deck => {
                return Some(if code.is_some() && code == Self::fetch(&ctx) { 5000.0 } else { worth / 10.0 });
            }
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
