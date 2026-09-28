//! "Blackwing Assassin": beatdown.
//!
//! Normal Summon a Blackwing, swarm with the free Special Summons (Bora the
//! Spear, Gale the Whirlwind), Blizzard's revive and Black Whirlwind's search,
//! and Synchro into 2300-2800 ATK bosses when the opponent has blockers.
//! Kalut the Moon Shadow is the Damage Step trick; Icarus Attack, Book of Moon
//! and Threatening Roar protect the board on the opponent's turn.

use crate::agent::{value, Outcome, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
use crate::staples::{MONSTER_REBORN, POT_OF_DUALITY};

pub const DECK: &str = "Blackwing Assassin";

const SIROCCO: u32 = 75498415;
const SHURA: u32 = 58820853;
const BORA: u32 = 49003716;
const KALUT: u32 = 85215458;
const BLIZZARD: u32 = 22835145;
const JIN: u32 = 38562933;
const GALE: u32 = 2009101;
const ZEPHYROS: u32 = 14785765;
const BLACK_WHIRLWIND: u32 = 91351370;
const ICARUS_ATTACK: u32 = 53567095;
const ARMOR_MASTER: u32 = 69031175;
const ARMED_WING: u32 = 76913983;
const SILVERWIND: u32 = 33236860;
const BLACK_WINGED_DRAGON: u32 = 9012916;
const SET_BLACKWING: u16 = 0x33;
const KALUT_BONUS: i32 = 1400;

#[derive(Default)]
pub struct Blackwing;

impl Blackwing {
    fn is_blackwing(ctx: &Ctx, card: &CardView) -> bool {
        ctx.view_data(card).in_set(SET_BLACKWING)
    }

    fn has_blackwing(ctx: &Ctx) -> bool {
        ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && Self::is_blackwing(ctx, c))
    }

    /// Pump one Blackwing only when nobody can get through the wall alone.
    fn sirocco_target(ctx: &Ctx) -> Option<crate::model::CardRef> {
        let opp = ctx.monsters(ctx.opp);
        if opp.is_empty() || !ctx.main1() || !ctx.my_turn() {
            return None;
        }
        let ours: Vec<_> = ctx
            .monsters(ctx.me)
            .into_iter()
            .filter(|c| c.position.face_up && ctx.can_attack(c) && Self::is_blackwing(ctx, c))
            .collect();
        let wall = opp.iter().map(|c| ctx.battle_stat(c)).max()?;
        let total: i32 = ours.iter().map(|c| c.attack).sum();
        if !ours.is_empty() && ours.iter().all(|c| c.attack <= wall) && total > wall {
            ours.iter().max_by_key(|c| c.attack).map(|c| c.at)
        } else {
            None
        }
    }

    /// Kalut is only ever offered in the Damage Step (engines that report no
    /// Damage Step phase are covered by that gate).
    fn kalut(ctx: &Ctx) -> f64 {
        if ctx.phase().map_or(false, |p| !p.is_battle()) {
            return 0.0;
        }
        let Some(attacker) = ctx.battle_attacker() else { return 0.0 };
        let target = ctx.battle_target();
        let (ours, theirs) = if attacker.at.controller == ctx.me {
            (Some(attacker), target)
        } else {
            (target, Some(attacker))
        };
        let Some(ours) = ours else { return 0.0 };
        if !Self::is_blackwing(ctx, ours) || !ours.position.attack {
            return 0.0;
        }
        match theirs {
            None => {
                let lethal = ours.attack < ctx.opp_lp() && ctx.opp_lp() <= ours.attack + KALUT_BONUS;
                if lethal { 60.0 } else { 0.0 }
            }
            Some(theirs) => {
                let stat = ctx.battle_stat(theirs);
                if ours.attack <= stat && stat < ours.attack + KALUT_BONUS { 60.0 } else { 0.0 }
            }
        }
    }

    fn cheap_bounce(&self, ctx: &Ctx) -> Option<crate::model::CardRef> {
        ctx.monsters(ctx.me)
            .into_iter()
            .chain(ctx.spell_traps(ctx.me))
            .filter(|c| c.position.face_up)
            .map(|c| (value(self, ctx, None, Some(c)), c.at))
            .filter(|(v, _)| *v < 1500)
            .min_by_key(|(v, _)| *v)
            .map(|(_, at)| at)
    }

    fn icarus_targets<'a>(ctx: &Ctx<'a>) -> Vec<&'a CardView> {
        let mut targets: Vec<_> = ctx.monsters(ctx.opp).into_iter().chain(ctx.spell_traps(ctx.opp)).collect();
        targets.sort_by_key(|c| -ctx.threat(c));
        targets
    }

    /// Icarus Attack (tribute our cheapest Winged Beast, destroy two of their
    /// cards) when clearing their monsters lets the rest of our attackers
    /// deal lethal damage this turn.
    fn icarus_for_lethal(&self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !ctx.main1() || !t.has(ChoiceKind::EnterBattle) {
            return None;
        }
        let i = t.activate(ICARUS_ATTACK)?;
        let blockers = ctx.monsters(ctx.opp);
        if blockers.is_empty() || blockers.len() > 2 {
            return None;
        }
        let ours = ctx.monsters(ctx.me);
        let tribute = ours
            .iter()
            .filter(|c| ctx.view_data(c).race & races::WINGED_BEAST != 0)
            .min_by_key(|c| (ctx.can_attack(c) as i32, c.attack))?;
        let damage: i32 = ours
            .iter()
            .filter(|c| c.at != tribute.at && ctx.can_attack(c))
            .map(|c| c.attack)
            .sum();
        if damage < ctx.opp_lp() {
            return None;
        }
        let mut targets: Vec<_> = blockers.iter().map(|c| c.at).collect();
        if targets.len() < 2 {
            targets.extend(ctx.set_backrow(ctx.opp).first().map(|c| c.at));
        }
        t.pick_targeting(i, targets)
    }

    fn choose_synchro(&self, t: &Turn) -> Option<usize> {
        let ctx = t.ctx;
        let options: Vec<usize> = t
            .choices()
            .filter(|(i, c)| {
                c.kind == ChoiceKind::SpecialSummon
                    && c.at().map(|a| a.location) == Some(Location::Extra)
                    && t.fresh(*i)
            })
            .map(|(i, _)| i)
            .collect();
        if options.is_empty() {
            return None;
        }
        let opp = ctx.monsters(ctx.opp);
        if ctx.main1() && opp.is_empty() {
            let attack: i32 = ctx.monsters(ctx.me).iter().filter(|c| ctx.can_attack(c)).map(|c| c.attack).sum();
            if attack >= 2300 {
                return None;
            }
        }
        let rank = |i: &usize| -> i32 {
            let code = t.choice(*i).code().unwrap_or(0);
            let mut score = ctx.data(code).attack;
            if code == SILVERWIND {
                let breakable = opp.iter().filter(|c| c.position.face_up && c.defense < 2800).count();
                score += 400 * breakable.min(2) as i32;
            }
            if code == ARMOR_MASTER && ctx.opp_best_attack() >= 2500 {
                score += 600;
            }
            score
        };
        options.into_iter().max_by_key(rank)
    }

    fn search_bonus(ctx: &Ctx, code: u32) -> i32 {
        let has_tuner = ctx.monsters(ctx.me).iter().any(|c| ctx.view_data(c).is_tuner());
        let mut bonus = 0;
        if code == GALE && !has_tuner {
            bonus += 600;
        }
        if (code == BORA || code == GALE) && Self::has_blackwing(ctx) {
            bonus += 300;
        }
        let hand_has_monster = ctx.hand().iter().any(|c| ctx.view_data(c).is_monster());
        if ctx.data(code).is_monster() && !hand_has_monster {
            bonus += 300;
        }
        bonus
    }
}

impl Strategy for Blackwing {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            SILVERWIND => 3200,
            BLACK_WINGED_DRAGON => 3000,
            ARMOR_MASTER => 2900,
            ARMED_WING => 2500,
            SIROCCO => 2000,
            SHURA => 1850,
            BORA => 1750,
            GALE => 1700,
            ICARUS_ATTACK => 1650,
            ZEPHYROS => 1600,
            BLACK_WHIRLWIND | KALUT => 1500,
            BLIZZARD => 1400,
            JIN => 800,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = self.icarus_for_lethal(t) {
            return Some(i);
        }
        if ctx.main1() {
            if let Some(i) = t.activate(ICARUS_ATTACK) {
                let targets = Self::icarus_targets(&ctx);
                let cheap = ctx.monsters(ctx.me).iter().any(|c| {
                    ctx.view_data(c).race & races::WINGED_BEAST != 0 && value(self, &ctx, None, Some(c)) <= 1500
                });
                let pressure: i32 = targets.iter().take(2).map(|c| ctx.threat(c)).sum();
                if targets.len() >= 2 && cheap && pressure >= 2500 {
                    let intent = targets.iter().take(2).map(|c| c.at).collect();
                    return t.pick_targeting(i, intent);
                }
            }
        }
        let whirlwind_up = ctx.face_up_on_field(ctx.me, BLACK_WHIRLWIND);
        if !whirlwind_up && t.has(ChoiceKind::NormalSummon) {
            if let Some(i) = t.activate_from(BLACK_WHIRLWIND, Location::Hand) {
                return t.pick(i);
            }
        }
        None
    }

    fn allow_staple(&self, t: &Turn, code: u32) -> bool {
        if code != POT_OF_DUALITY || !t.ctx.main1() {
            return true;
        }
        let ctx = t.ctx;
        let wants_special = [BORA, GALE, MONSTER_REBORN].iter().any(|c| ctx.in_hand(*c))
            || ctx.graveyard(ctx.me).iter().any(|c| c.code == Some(ZEPHYROS))
            || t.has(ChoiceKind::SpecialSummon);
        !wants_special
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = choice.code()?;
        let data = ctx.data(code);
        if data.level >= 5 && !ctx.monsters(ctx.me).is_empty() {
            return Some(None); // never tribute (only Sirocco; its free summon is better)
        }
        let revive = code == BLIZZARD
            && ctx.graveyard(ctx.me).iter().any(|c| {
                let d = ctx.view_data(c);
                d.is_monster() && d.in_set(SET_BLACKWING) && d.level <= 4
            });
        let follow_up = data.in_set(SET_BLACKWING) && (ctx.in_hand(BORA) || ctx.in_hand(GALE));
        let mut score = data.attack as f64;
        if code == KALUT {
            score -= 700.0; // worth more as the Damage Step trick
        }
        if revive {
            score += 1500.0;
        }
        if code == SIROCCO {
            score += 300.0;
        }
        if follow_up {
            score += 300.0;
        }
        let face_up_ok = data.attack >= ctx.opp_best_attack() || follow_up || revive || ctx.monsters(ctx.opp).is_empty();
        Some(match choice.kind {
            // Kalut is a Damage Step trick from the hand, not a wall.
            ChoiceKind::SetMonster if code == KALUT => None,
            ChoiceKind::SetMonster if face_up_ok => None,
            ChoiceKind::SetMonster => Some(data.defense as f64 - 500.0),
            _ if !face_up_ok => None,
            _ => Some(score),
        })
    }

    fn special_summon(&self, _t: &Turn, choice: &Choice) -> Option<bool> {
        // Extra Deck summons are chosen in `main_phase_late`.
        Some(choice.at().map(|a| a.location) != Some(Location::Extra))
    }

    fn main_phase_late(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let opp = ctx.monsters(ctx.opp);
        if let Some(i) = t.activate_from(ZEPHYROS, Location::Graveyard) {
            if let Some(at) = self.cheap_bounce(&ctx) {
                return t.pick_targeting(i, vec![at]);
            }
        }
        if !opp.is_empty() {
            if let Some(i) = t.activate_from(GALE, Location::MonsterZone) {
                let big = opp.iter().filter(|c| c.position.face_up && c.attack >= 1000).max_by_key(|c| c.attack);
                if let Some(big) = big {
                    return t.pick_targeting(i, vec![big.at]);
                }
            }
        }
        if let Some(i) = t.activate_from(BLACK_WINGED_DRAGON, Location::MonsterZone) {
            if let Some(big) = opp.iter().filter(|c| c.position.face_up).max_by_key(|c| c.attack) {
                return t.pick_targeting(i, vec![big.at]);
            }
        }
        if !opp.is_empty() {
            if let Some(i) = t.activate_from(ARMOR_MASTER, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        if let Some(i) = self.choose_synchro(t) {
            return t.pick(i);
        }
        if t.has(ChoiceKind::EnterBattle) {
            if let Some(i) = t.activate_from(SIROCCO, Location::MonsterZone) {
                if let Some(at) = Self::sirocco_target(&ctx) {
                    return t.pick_targeting(i, vec![at]);
                }
            }
        }
        None
    }

    fn battle(&mut self, t: &mut Turn) -> Option<usize> {
        if t.ctx.monsters(t.ctx.opp).is_empty() {
            return None;
        }
        let i = t.activate(ARMOR_MASTER)?;
        t.pick(i)
    }

    fn attack_trick(&self, ctx: &Ctx, attacker: &CardView) -> i32 {
        if ctx.in_hand(KALUT) && Self::is_blackwing(ctx, attacker) {
            KALUT_BONUS
        } else {
            0
        }
    }

    fn attack_outcome(&self, ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
        let stat = ctx.battle_stat(target);
        match attacker.code {
            Some(JIN) if target.position.face_up && target.defense <= attacker.attack => {
                Some(Outcome::Win { trick: false })
            }
            Some(ARMOR_MASTER) => Some(if attacker.attack > stat { Outcome::Win { trick: false } } else { Outcome::Bounce }),
            Some(ARMED_WING) if target.position.face_up && !target.position.attack => {
                Some(if attacker.attack + 500 > stat { Outcome::Win { trick: false } } else { Outcome::Bounce })
            }
            _ => None,
        }
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let code = t.choice(index).code()?;
        Some(match code {
            ICARUS_ATTACK => {
                let targets = Self::icarus_targets(&ctx);
                if targets.len() < 2 {
                    return Some(Response::no());
                }
                if let Some((attacker, target)) = ctx.incoming_attack() {
                    if ctx.attack_hurts(attacker, target) {
                        let other = targets.iter().find(|c| c.at != attacker.at).map(|c| c.at);
                        return Some(Response::targeting(70.0, std::iter::once(attacker.at).chain(other).collect()));
                    }
                }
                let pressure: i32 = targets.iter().take(2).map(|c| ctx.threat(c)).sum();
                if ctx.my_turn() && ctx.main1() && pressure >= 3000 {
                    Response::targeting(40.0, targets.iter().take(2).map(|c| c.at).collect())
                } else {
                    Response::no()
                }
            }
            SIROCCO => match Self::sirocco_target(&ctx) {
                Some(at) => Response::targeting(15.0, vec![at]),
                None => Response::no(),
            },
            GALE | BLACK_WINGED_DRAGON | ZEPHYROS | ARMOR_MASTER => Response::no(),
            KALUT => Response::new(Self::kalut(&ctx)),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let mine = member.at.controller == ctx.me;
        let code = member.code?;
        let view = ctx.card(member.at).filter(|v| v.known());
        let worth = value(self, &ctx, Some(code), view) as f64;
        if mine && t.decision.hint.is_gain() {
            return Some(worth + Self::search_bonus(&ctx, code) as f64);
        }
        match t.memory.last_activated {
            Some(SIROCCO) | Some(BLIZZARD) if mine && t.decision.hint != Hint::Release => Some(worth),
            _ => None,
        }
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        (t.decision.subject == Some(KALUT)).then(|| Self::kalut(&t.ctx) > 0.0)
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let data = t.ctx.data(code);
        Some(data.is_trap() || code == crate::staples::BOOK_OF_MOON || code == crate::staples::MYSTICAL_SPACE_TYPHOON)
    }
}
