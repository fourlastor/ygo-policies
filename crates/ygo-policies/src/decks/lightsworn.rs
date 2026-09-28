//! "Lightsworn Judgment": self-mill into Judgment Dragon.
//!
//! Every Lightsworn mills the deck in the End Phase; Charge of the Light
//! Brigade and Solar Recharge speed that up, Wulf revives itself when milled
//! and Lumina / Glorious Illusion recycle the Graveyard.  Four different
//! Lightsworn names in the Graveyard turn on Judgment Dragon, whose 1000 LP
//! board wipe is used only when the opponent's side is worth more than ours.
//! Honest is the Damage Step trick for our LIGHT attackers.

use crate::agent::{value, Outcome, Response, Strategy, Turn};
use crate::cards::attributes;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Location, Member, Phase};

pub const DECK: &str = "Lightsworn Judgment";

const JUDGMENT_DRAGON: u32 = 57774843;
const CELESTIA: u32 = 94381039;
const GRAGONITH: u32 = 21785144;
const HONEST: u32 = 37742479;
const GLOW_UP_BULB: u32 = 67441435;
const HERALD_OF_CREATION: u32 = 66337215;
const PLAGUESPREADER: u32 = 33420078;
const NECRO_GARDNA: u32 = 4906301;
const GAROTH: u32 = 59019082;
const LUMINA: u32 = 95503687;
const SHIRE: u32 = 2420921;
const AURKUS: u32 = 7183277;
const JAIN: u32 = 96235275;
const RYKO: u32 = 21502796;
const WULF: u32 = 58996430;
const LYLA: u32 = 22624373;
const EHREN: u32 = 44178886;
const MONSTER_REINCARNATION: u32 = 74848038;
const SOLAR_RECHARGE: u32 = 691925;
const CHARGE_OF_THE_LIGHT_BRIGADE: u32 = 94886282;
const SOLEMN_JUDGMENT: u32 = 41420027;
const COMPULSORY_EVACUATION: u32 = 94192409;
const GLORIOUS_ILLUSION: u32 = 61962135;
const BLACK_ROSE_DRAGON: u32 = 73580472;
const SET_LIGHTSWORN: u16 = 0x38;

#[derive(Default)]
pub struct Lightsworn;

impl Lightsworn {
    fn is_lightsworn(ctx: &Ctx, code: u32) -> bool {
        ctx.data(code).in_set(SET_LIGHTSWORN)
    }

    /// Is the opponent's side worth a Judgment Dragon wipe of everything?
    fn wipe_pays(ctx: &Ctx) -> bool {
        let theirs: i32 = ctx
            .monsters(ctx.opp)
            .iter()
            .chain(ctx.spell_traps(ctx.opp).iter())
            .map(|c| ctx.threat(c))
            .sum();
        let ours: i32 = ctx
            .monsters(ctx.me)
            .iter()
            .filter(|c| !ctx.is(c, JUDGMENT_DRAGON))
            .map(|c| c.attack.max(1000))
            .sum::<i32>()
            + 700 * ctx.set_backrow(ctx.me).len() as i32;
        ctx.my_lp() > 1500 && theirs >= 2500 && theirs > ours + 1000
    }

    /// Honest is only ever offered in the Damage Step.
    fn honest(ctx: &Ctx) -> f64 {
        if ctx.phase().map_or(false, |p| !p.is_battle()) {
            return 0.0;
        }
        let (Some(attacker), Some(target)) = (ctx.battle_attacker(), ctx.battle_target()) else { return 0.0 };
        let (ours, theirs) = if attacker.at.controller == ctx.me { (attacker, target) } else { (target, attacker) };
        let light = ctx.view_data(ours).attribute & attributes::LIGHT != 0;
        if !light || !ours.position.attack {
            return 0.0;
        }
        let stat = ctx.battle_stat(theirs);
        if ours.attack <= stat && ours.attack + theirs.attack > stat { 60.0 } else { 0.0 }
    }
}

impl Strategy for Lightsworn {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            JUDGMENT_DRAGON => 3200,
            CELESTIA => 2300,
            GRAGONITH => 2200,
            SOLEMN_JUDGMENT | GLORIOUS_ILLUSION => 1900,
            LUMINA => 1900,
            JAIN | LYLA | GAROTH => 1800,
            CHARGE_OF_THE_LIGHT_BRIGADE | SOLAR_RECHARGE => 1700,
            HERALD_OF_CREATION | EHREN => 1600,
            COMPULSORY_EVACUATION | HONEST => 1500,
            MONSTER_REINCARNATION => 1400,
            RYKO | AURKUS => 1300,
            SHIRE => 1100,
            NECRO_GARDNA => 900,
            PLAGUESPREADER | GLOW_UP_BULB => 800,
            // Wulf can only come out by being milled: discarding it is free.
            WULF => 300,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Judgment Dragon's wipe.
        if let Some(i) = t.activate_from(JUDGMENT_DRAGON, Location::MonsterZone) {
            if Self::wipe_pays(&ctx) {
                return t.pick(i);
            }
        }
        // Mill / draw engine.
        if let Some(i) = t.activate(CHARGE_OF_THE_LIGHT_BRIGADE) {
            return t.pick(i);
        }
        if let Some(i) = t.activate(SOLAR_RECHARGE) {
            let discardable = ctx.hand().iter().filter(|c| c.code.map_or(false, |k| Self::is_lightsworn(&ctx, k))).count();
            if discardable >= 1 {
                return t.pick(i);
            }
        }
        // Lyla trades her attack for a Spell/Trap.
        if let Some(i) = t.activate_from(LYLA, Location::MonsterZone) {
            let backrow = ctx.set_backrow(ctx.opp);
            let target = backrow.first().copied().or_else(|| ctx.spell_traps(ctx.opp).into_iter().next());
            if let Some(target) = target {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        // Lumina: discard the cheapest card, revive the best Lightsworn.
        if ctx.hand_size(ctx.me) >= 1 && ctx.free_monster_zones(ctx.me) > 0 {
            if let Some(i) = t.activate_from(LUMINA, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        if ctx.graveyard(ctx.me).iter().any(|c| ctx.is(c, JUDGMENT_DRAGON)) && ctx.hand_size(ctx.me) >= 2 {
            if let Some(i) = t.activate_from(HERALD_OF_CREATION, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(MONSTER_REINCARNATION) {
            let best = ctx
                .graveyard(ctx.me)
                .into_iter()
                .filter(|c| ctx.view_data(c).is_monster())
                .max_by_key(|c| value(self, &ctx, c.code, None));
            if let Some(best) = best {
                if value(self, &ctx, best.code, None) >= 1800 && ctx.hand_size(ctx.me) >= 2 {
                    return t.pick_targeting(i, vec![best.at]);
                }
            }
        }
        if let Some(i) = t.activate_from(GLOW_UP_BULB, Location::Graveyard) {
            if ctx.deck_size(ctx.me) > 10 {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let lightsworn_fodder = ctx
            .monsters(ctx.me)
            .iter()
            .any(|c| c.code.map_or(false, |k| Self::is_lightsworn(&ctx, k)) && !ctx.is(c, JUDGMENT_DRAGON));
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, CELESTIA) if lightsworn_fodder && !ctx.monsters(ctx.opp).is_empty() => Some(3000.0),
            (ChoiceKind::SetMonster, RYKO) if !ctx.monsters(ctx.opp).is_empty() || !ctx.spell_traps(ctx.opp).is_empty() => {
                Some(1900.0)
            }
            (ChoiceKind::NormalSummon, LUMINA) if ctx.graveyard(ctx.me).iter().any(|c| {
                let d = ctx.view_data(c);
                d.in_set(SET_LIGHTSWORN) && d.level <= 4 && d.attack >= 1700
            }) => Some(2500.0),
            (ChoiceKind::NormalSummon, CELESTIA | GRAGONITH) => None,
            (ChoiceKind::NormalSummon, HONEST | NECRO_GARDNA | GLOW_UP_BULB | PLAGUESPREADER) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code().unwrap_or(0));
        // Black Rose Dragon wipes our own board too.
        (code == BLACK_ROSE_DRAGON).then(|| Self::wipe_pays(&ctx))
    }

    fn attack_trick(&self, ctx: &Ctx, attacker: &CardView) -> i32 {
        let light = ctx.view_data(attacker).attribute & attributes::LIGHT != 0;
        if light && ctx.in_hand(HONEST) {
            // Honest adds the defender's ATK; count it as a flat boost.
            ctx.opp_best_attack()
        } else {
            0
        }
    }

    fn attack_outcome(&self, ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
        let stat = ctx.battle_stat(target);
        if ctx.is(attacker, JAIN) && target.known() {
            return Some(if attacker.attack + 300 > stat { Outcome::Win { trick: false } } else { Outcome::Lose });
        }
        if ctx.is(attacker, EHREN) && !target.position.attack {
            return Some(Outcome::Win { trick: false });
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
            HONEST => Response::new(Self::honest(&ctx)),
            NECRO_GARDNA => match incoming {
                Some((a, target)) if ctx.attack_hurts(a, target) => Response::new(55.0),
                _ => Response::no(),
            },
            COMPULSORY_EVACUATION => match incoming {
                Some((a, target)) if ctx.attack_hurts(a, target) => Response::targeting(70.0, vec![a.at]),
                _ => Response::no(),
            },
            SOLEMN_JUDGMENT => {
                let summoned = ctx.obs.event_cards.iter().filter(|(at, _)| at.controller == ctx.opp).find_map(|(at, code)| {
                    let card = ctx.card(*at)?;
                    Some((card, (*code)?))
                });
                let spell = ctx.obs.chain.last().filter(|l| l.controller == ctx.opp).map(|l| ctx.canonical(l.code));
                let big_summon = summoned.map_or(false, |(c, code)| c.attack.max(ctx.data(code).attack) >= 2400 || ctx.data(code).is_extra());
                let wipe = spell.map_or(false, |s| matches!(s, 53129443 | 12580477 | 19613556 | 53582587));
                if ctx.my_lp() >= 4000 && (big_summon || wipe) { Response::new(85.0) } else { Response::no() }
            }
            GLORIOUS_ILLUSION => {
                let empty = ctx.monsters(ctx.me).is_empty();
                if end_of_their_turn || (empty && incoming.is_some()) {
                    let best = ctx
                        .graveyard(ctx.me)
                        .into_iter()
                        .filter(|c| c.code.map_or(false, |k| Self::is_lightsworn(&ctx, k)) && ctx.view_data(c).is_monster())
                        .max_by_key(|c| ctx.view_data(c).attack);
                    match best {
                        Some(best) => Response::targeting(25.0, vec![best.at]),
                        None => Response::no(),
                    }
                } else {
                    Response::no()
                }
            }
            JUDGMENT_DRAGON if choice.at().map(|a| a.location) == Some(Location::MonsterZone) => {
                if Self::wipe_pays(&ctx) { Response::new(30.0) } else { Response::no() }
            }
            // Ignition effects are used at our own pace in the Main Phase.
            LYLA | LUMINA | HERALD_OF_CREATION | GLOW_UP_BULB | PLAGUESPREADER => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if member.at.controller != ctx.me {
            return None;
        }
        let code = member.code.map(|c| ctx.canonical(c));
        // Discard costs (Lumina, Solar Recharge, Herald): Wulf and extra
        // copies first; they feed Judgment Dragon's count either way.
        if t.decision.hint == crate::model::Hint::Discard {
            let worth = value(self, &ctx, code, None) as f64;
            let new_name = code.map_or(false, |k| {
                Self::is_lightsworn(&ctx, k) && !ctx.graveyard(ctx.me).iter().any(|c| c.code == Some(k))
            });
            return Some(-worth + if new_name { 600.0 } else { 0.0 });
        }
        None
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let data = t.ctx.data(t.ctx.canonical(code));
        Some(data.is_trap() || code == crate::staples::MYSTICAL_SPACE_TYPHOON)
    }
}
