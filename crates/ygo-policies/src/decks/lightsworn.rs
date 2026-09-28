//! "Lightsworn Judgment": self-mill into Judgment Dragon.
//!
//! Every Lightsworn mills the deck in the End Phase; Charge of the Light
//! Brigade and Solar Recharge speed that up, Wulf revives itself when milled
//! and Lumina / Glorious Illusion recycle the Graveyard.  Four different
//! Lightsworn names in the Graveyard turn on Judgment Dragon, whose 1000 LP
//! board wipe is used only when the opponent's side is worth more than ours.
//! Honest is the Damage Step trick for our LIGHT attackers.
//!
//! Self-mill is also how the deck loses: the End Phase mills are mandatory
//! and a patient opponent just waits for the empty draw.  Every optional mill
//! (Charge, Solar Recharge, Celestia, Glow-Up Bulb, Glorious Illusion,
//! Lumina's revive) and Judgment Dragon are paid from a budget: the Deck
//! after next turn's mandatory mills and draw must keep a reserve.
//! And since a face-up Lightsworn keeps milling every turn, a low Deck sheds
//! them: Celestia Tributes one, Synchros use them as material, they take the
//! losing battles, and Compulsory Evacuation Device can bounce our own.

use crate::agent::{value, Outcome, Response, Strategy, Turn};
use crate::cards::attributes;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Location, Member, Phase};

pub const DECK: &str = "Lightsworn Judgment";

const JUDGMENT_DRAGON: u32 = 57774843;
const CELESTIA: u32 = 94381039;
const GRAGONITH: u32 = 21785144;
const HONEST: u32 = 37742478;
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
const BLACK_ROSE_DRAGON: u32 = 73580471;
const SET_LIGHTSWORN: u16 = 0x38;

#[derive(Default)]
pub struct Lightsworn;

impl Lightsworn {
    fn is_lightsworn(ctx: &Ctx, code: u32) -> bool {
        ctx.data(code).in_set(SET_LIGHTSWORN)
    }

    /// Cards a face-up copy of `code` sends from our Deck every End Phase.
    fn end_phase_mill(code: u32) -> i32 {
        match code {
            JUDGMENT_DRAGON => 4,
            LYLA | LUMINA | EHREN | GRAGONITH => 3,
            JAIN | AURKUS | SHIRE | GLORIOUS_ILLUSION => 2,
            _ => 0,
        }
    }

    /// Cards our next End Phase will mill, as the board stands.
    fn pending_mill(ctx: &Ctx) -> i32 {
        let sources: Vec<i32> = ctx
            .monsters(ctx.me)
            .into_iter()
            .chain(ctx.spell_traps(ctx.me))
            .filter(|c| c.position.face_up)
            .filter_map(|c| c.code.map(|code| Self::end_phase_mill(ctx.canonical(code))))
            .filter(|m| *m > 0)
            .collect();
        let garoth = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.is(c, GAROTH));
        // Garoth mills 2 (and draws) each time another Lightsworn mills.
        sources.iter().sum::<i32>() + if garoth { 3 * sources.len() as i32 } else { 0 }
    }

    /// Deck cards left after the pending End Phase mills and the next draw.
    fn spare(ctx: &Ctx) -> i32 {
        ctx.deck_size(ctx.me) as i32 - Self::pending_mill(ctx) - 1
    }

    /// Cards to keep for later turns; small when the board is about to win.
    fn reserve(ctx: &Ctx) -> i32 {
        let attack: i32 = ctx.monsters(ctx.me).iter().filter(|c| c.position.face_up && c.position.attack).map(|c| c.attack).sum();
        if ctx.monsters(ctx.opp).is_empty() && attack >= ctx.opp_lp() { 1 } else { 8 }
    }

    /// Different Lightsworn names in our Graveyard (Judgment Dragon, Gragonith, Shire).
    fn graveyard_names(ctx: &Ctx) -> usize {
        let mut names: Vec<u32> = ctx
            .graveyard(ctx.me)
            .iter()
            .filter_map(|c| c.code.map(|k| ctx.canonical(k)))
            .filter(|k| Self::is_lightsworn(ctx, *k) && ctx.data(*k).is_monster())
            .collect();
        names.sort_unstable();
        names.dedup();
        names.len()
    }

    /// Would `extra` more ATK make this turn's attacks lethal on an open board?
    fn lethal_with(ctx: &Ctx, extra: i32) -> bool {
        let attack: i32 = ctx.monsters(ctx.me).iter().filter(|c| ctx.can_attack(c)).map(|c| c.attack).sum();
        ctx.main1() && ctx.monsters(ctx.opp).is_empty() && ctx.attack_locks().is_empty() && attack + extra >= ctx.opp_lp()
    }

    /// Our turns the Deck lasts if `extra` more cards join each End Phase's mills.
    fn turns_left(ctx: &Ctx, extra: i32) -> i32 {
        (ctx.deck_size(ctx.me) as i32 - 1) / (Self::pending_mill(ctx) + extra + 1)
    }

    /// Fewer than about three turns of mills left.
    fn low(ctx: &Ctx) -> bool {
        Self::spare(ctx) < 3 * Self::pending_mill(ctx).max(2)
    }

    /// Our face-up monster that mills the most every End Phase.
    fn biggest_miller<'a>(ctx: &Ctx<'a>) -> Option<&'a CardView> {
        ctx.monsters(ctx.me)
            .into_iter()
            .filter(|c| c.position.face_up)
            .filter_map(|c| c.code.map(|code| (Self::end_phase_mill(ctx.canonical(code)), c)))
            .filter(|(m, _)| *m > 0)
            .max_by_key(|(m, _)| *m)
            .map(|(_, c)| c)
    }

    /// Can we afford `cards` more leaving the Deck?
    fn can_mill(ctx: &Ctx, cards: i32) -> bool {
        Self::spare(ctx) - cards >= Self::reserve(ctx)
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
        // About to deck out: bounce our own biggest miller with Compulsory Evacuation Device.
        if Self::spare(&ctx) < Self::pending_mill(&ctx) {
            if let Some(miller) = Self::biggest_miller(&ctx) {
                if let Some(i) = t.activate(COMPULSORY_EVACUATION) {
                    return t.pick_targeting(i, vec![miller.at]);
                }
            }
        }
        // Mill / draw engine, within the Deck budget.
        if Self::can_mill(&ctx, 4) {
            if let Some(i) = t.activate(CHARGE_OF_THE_LIGHT_BRIGADE) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(SOLAR_RECHARGE).filter(|_| Self::can_mill(&ctx, 4)) {
            let discardable = ctx.hand().iter().filter(|c| c.code.map_or(false, |k| Self::is_lightsworn(&ctx, k))).count();
            if discardable >= 1 {
                return t.pick(i);
            }
        }
        // Lyla trades her attack for a Spell/Trap.
        if let Some(i) = t.activate_from(LYLA, Location::MonsterZone) {
            let target = ctx.spell_traps(ctx.opp).into_iter().max_by_key(|c| ctx.threat(c));
            if let Some(target) = target {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        // Lumina: discard the cheapest card, revive the best Lightsworn.
        // (The revived Lightsworn will mill too.)
        if ctx.hand_size(ctx.me) >= 1 && ctx.free_monster_zones(ctx.me) > 0 && Self::can_mill(&ctx, 3) {
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
            if Self::can_mill(&ctx, 1) {
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
        // A new face-up miller that would leave fewer than 2 turns of Deck
        // is Set instead (face-down, it does not mill), unless it wins now.
        let mill = Self::end_phase_mill(code) + if code == GAROTH { 3 } else { 0 };
        if mill > 0 && Self::turns_left(&ctx, mill) < 2 && !Self::lethal_with(&ctx, ctx.data(code).attack) {
            return Some(match choice.kind {
                ChoiceKind::SetMonster => Some(1000.0 + ctx.data(code).defense as f64 / 10.0),
                _ => None,
            });
        }
        // Low on Deck: Celestia (no End Phase mill) Tributes one of our millers away.
        if code == CELESTIA && choice.kind == ChoiceKind::NormalSummon && Self::low(&ctx) && Self::biggest_miller(&ctx).is_some() {
            return Some(Some(3100.0));
        }
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, CELESTIA) if lightsworn_fodder && !ctx.monsters(ctx.opp).is_empty() => Some(3000.0),
            (ChoiceKind::SetMonster, RYKO) if !Self::can_mill(&ctx, 3) => None,
            (ChoiceKind::SetMonster, RYKO) if !ctx.monsters(ctx.opp).is_empty() || !ctx.spell_traps(ctx.opp).is_empty() => {
                Some(1900.0)
            }
            (ChoiceKind::NormalSummon, LUMINA) if ctx.graveyard(ctx.me).iter().any(|c| {
                let d = ctx.view_data(c);
                d.in_set(SET_LIGHTSWORN) && d.level <= 4 && d.attack >= 1700
            }) => Some(2500.0),
            // Gragonith: +300 ATK per Lightsworn name in the Graveyard, piercing.
            (ChoiceKind::NormalSummon, GRAGONITH) if Self::graveyard_names(&ctx) >= 3 => {
                Some(2000.0 + 300.0 * Self::graveyard_names(&ctx) as f64)
            }
            (ChoiceKind::NormalSummon, CELESTIA | GRAGONITH) => None,
            (ChoiceKind::NormalSummon, HONEST | NECRO_GARDNA | GLOW_UP_BULB | PLAGUESPREADER) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code().unwrap_or(0));
        match code {
            // Black Rose Dragon wipes our own board too.
            BLACK_ROSE_DRAGON => Some(Self::wipe_pays(&ctx)),
            // Judgment Dragon mills 4 every End Phase: only if the Deck can pay,
            // or when its wipe is needed now and the next draw is still safe.
            // Judgment Dragon adds 4 to every End Phase: the Deck must last.
            JUDGMENT_DRAGON => Some(
                Self::lethal_with(&ctx, 3000)
                    || Self::turns_left(&ctx, 4) >= 3
                    || (Self::wipe_pays(&ctx) && Self::turns_left(&ctx, 4) >= 1),
            ),
            _ => None,
        }
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
        // Low on Deck, a miller that dies in battle stops milling: worth a cheap loss.
        let miller = attacker.code.map_or(false, |c| Self::end_phase_mill(ctx.canonical(c)) > 0);
        if miller && Self::low(ctx) && target.known() && target.position.attack && stat - attacker.attack <= 800 {
            return Some(Outcome::Win { trick: false });
        }
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
            // Celestia's mill is optional ("you can"): only within budget.
            CELESTIA => if Self::can_mill(&ctx, 4) { Response::new(20.0) } else { Response::no() },
            GLORIOUS_ILLUSION if !Self::can_mill(&ctx, 5) => Response::no(),
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
        // Low on Deck: Tribute / Synchro Material our millers first.
        if Self::low(&ctx)
            && member.at.location == Location::MonsterZone
            && matches!(t.decision.hint, crate::model::Hint::Release | crate::model::Hint::Tribute | crate::model::Hint::SynchroMaterial)
        {
            let mill = code.map_or(0, Self::end_phase_mill) as f64;
            return Some(-(value(self, &ctx, code, None) as f64) + 1500.0 * mill);
        }
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

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        let ctx = t.ctx;
        (t.decision.subject.map(|c| ctx.canonical(c)) == Some(CELESTIA)).then(|| Self::can_mill(&ctx, 4))
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let data = t.ctx.data(t.ctx.canonical(code));
        Some(data.is_trap() || code == crate::staples::MYSTICAL_SPACE_TYPHOON)
    }
}
