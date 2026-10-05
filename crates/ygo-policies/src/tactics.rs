//! Deck-independent play: summons, attacks, positions, setting backrow.

use crate::agent::{value, Outcome, Strategy, Turn};
use crate::cards::types;
use crate::ctx::Ctx;
use crate::knowledge::{Burn, Fate, ALWAYS};
use crate::model::{CardRef, CardView, ChoiceKind, Location};

/// Our monsters that can still attack take the opponent's last Life Points
/// on an open field (no monsters, no attack lock): attack before anything
/// that would spend them (Synchro or Fusion materials, Tributes).
pub fn lethal_on_board(t: &Turn) -> bool {
    let ctx = t.ctx;
    if !ctx.main1() || !t.has(ChoiceKind::EnterBattle) || !ctx.monsters(ctx.opp).is_empty() || !ctx.attack_locks().is_empty() {
        return false;
    }
    let damage: i32 = ctx.monsters(ctx.me).iter().filter(|c| ctx.can_attack(c)).map(|c| c.attack.max(0)).sum();
    damage >= ctx.opp_lp()
}

/// Best Normal Summon / Set, or `None` to keep the summon.
pub fn normal_summon<S: Strategy + ?Sized>(s: &S, t: &Turn) -> Option<usize> {
    let ctx = t.ctx;
    let threat = ctx.opp_best_attack();
    let opp_empty = ctx.monsters(ctx.opp).is_empty();
    let mut own: Vec<i32> = ctx
        .monsters(ctx.me)
        .iter()
        .map(|c| value(s, &ctx, None, Some(c)))
        .collect();
    own.sort_unstable();
    let mut best: Option<(f64, usize)> = None;
    for (i, choice) in t.choices() {
        if !matches!(choice.kind, ChoiceKind::NormalSummon | ChoiceKind::SetMonster) || !t.fresh(i) {
            continue;
        }
        let Some(code) = choice.code() else { continue };
        let score = match s.summon_score(t, choice) {
            Some(None) => continue,
            Some(Some(score)) => score,
            None => {
                let data = ctx.data(code);
                let worth = value(s, &ctx, Some(code), None);
                let tributes = data.tributes() as usize;
                if tributes > 0 {
                    // Only tribute when the new monster is clearly better.
                    if own.len() < tributes {
                        continue;
                    }
                    let cost: i32 = own.iter().take(tributes).sum();
                    if worth <= cost + 300 {
                        continue;
                    }
                }
                let face_up_ok = data.attack >= threat || opp_empty;
                let flip = data.is(types::FLIP);
                if choice.kind == ChoiceKind::SetMonster {
                    if face_up_ok && !flip {
                        continue;
                    }
                    data.defense as f64 - 500.0 + if flip { 900.0 } else { 0.0 }
                } else {
                    if !face_up_ok || flip {
                        continue;
                    }
                    worth as f64
                }
            }
        };
        if best.map_or(true, |b| score > b.0) {
            best = Some((score, i));
        }
    }
    best.map(|b| b.1)
}

/// Their one monster is smaller than the monster this turn's Normal Summon
/// brings: it is left to that monster, a board wipe is kept for more.
pub fn outgrown<S: Strategy + ?Sized>(s: &S, t: &Turn) -> bool {
    let ctx = t.ctx;
    let theirs = ctx.monsters(ctx.opp);
    theirs.len() == 1
        && normal_summon(s, t)
            .map(|i| t.choice(i))
            .filter(|c| c.kind == ChoiceKind::NormalSummon)
            .and_then(|c| c.code())
            .map_or(false, |code| ctx.data(code).attack > ctx.battle_stat(theirs[0]))
}

/// Generic Extra Deck summon: take the strongest one when it helps.
pub fn extra_deck_summon<S: Strategy + ?Sized>(s: &S, t: &Turn) -> Option<usize> {
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
    let mut allowed = Vec::new();
    for i in options {
        let choice = t.choice(i);
        match s.special_summon(t, choice) {
            Some(false) => continue,
            Some(true) => allowed.push(i),
            None => {
                if ctx.main1() && ctx.monsters(ctx.opp).is_empty() {
                    // Open field: several direct attackers beat one big one.
                    let attack: i32 = ctx.monsters(ctx.me).iter().filter(|c| ctx.can_attack(c)).map(|c| c.attack).sum();
                    if attack >= ctx.data(choice.code().unwrap_or(0)).attack {
                        continue;
                    }
                }
                allowed.push(i);
            }
        }
    }
    allowed
        .into_iter()
        .max_by_key(|i| value(s, &ctx, t.choice(*i).code(), None))
}

pub fn reposition<S: Strategy + ?Sized>(s: &S, t: &Turn) -> Option<usize> {
    let ctx = t.ctx;
    let threat = ctx.opp_best_attack();
    let opp_empty = ctx.monsters(ctx.opp).is_empty();
    let can_battle = t.has(ChoiceKind::EnterBattle);
    t.find_where(|c| {
        if c.kind != ChoiceKind::ChangePosition {
            return false;
        }
        let Some(card) = c.at().and_then(|at| ctx.card(at)) else { return false };
        if !s.allow_reposition(t, card) { return false; }
        let data = ctx.view_data(card);
        if ctx.main1() && can_battle && !card.position.attack {
            // Face-down Flip monsters flip up on their own terms.
            // Swing when it clears the board's biggest threat, or at least
            // wins some battle on public numbers.
            let wins_something = ctx.monsters(ctx.opp).iter().any(|target| {
                target.known()
                    && matches!(
                        s.attack_outcome(&ctx, card, target)
                            .unwrap_or_else(|| default_outcome(&ctx, card, target, 0)),
                        Outcome::Win { trick: false }
                    )
            });
            let worth_swinging = card.attack >= 500 && (card.attack > threat || opp_empty || wins_something);
            (worth_swinging && card.position.face_up)
                || (!card.position.face_up && data.attack > threat.max(499) && !data.is(types::FLIP) && data.attack > data.defense)
        } else if !ctx.main1() && card.position.attack && card.position.face_up {
            card.attack < threat && card.defense >= card.attack
        } else {
            false
        }
    })
}

pub fn set_spell_trap<S: Strategy + ?Sized>(s: &S, t: &Turn) -> Option<usize> {
    let ctx = t.ctx;
    t.find_where(|c| {
        if c.kind != ChoiceKind::SetSpellTrap {
            return false;
        }
        let Some(code) = c.code() else { return false };
        let data = ctx.data(code);
        s.set_spell_trap(t, code)
            .unwrap_or(data.is_trap() || data.is(types::QUICKPLAY))
    })
}

/// Default battle resolution on public numbers, with an optional hand trick.
/// Card facts adjust it ([`crate::knowledge`]): monsters battle does not
/// destroy, monsters that take their attacker with them, Damage Step ATK
/// changes, monsters that switch position when attacked.
pub fn default_outcome(ctx: &Ctx, attacker: &CardView, target: &CardView, trick: i32) -> Outcome {
    let striking = ctx.facts(attacker).striking;
    if !target.position.face_up && !target.known() {
        // Unknown face-down monster: only strong attackers risk it.
        let atk = attacker.attack + striking.bonus;
        if atk >= 1900 {
            return Outcome::Win { trick: false };
        }
        if trick > 0 && atk + trick >= 1900 {
            return Outcome::Win { trick: true };
        }
        return Outcome::Lose;
    }
    let met = ctx.attack_meets(Some(attacker), target);
    let removes = ctx.strike_removes(attacker, target);
    // No battle: one of the two is gone before damage calculation.
    if met.facts.attacker != Fate::Unharmed && met.facts.before_damage {
        return if met.facts.leaves { Outcome::Trade } else { Outcome::Lose };
    }
    if removes == Some(true) {
        return if striking.leaves { Outcome::Trade } else { Outcome::Win { trick: false } };
    }
    let atk = met.facts.attacker_atk.apply(attacker.attack + striking.bonus);
    let (stat, defending) = (met.stat, met.defending);
    let outcome = if met.survives {
        // Nothing of theirs is destroyed; an Attack Position wall at least as
        // strong still destroys our attacker.
        if !defending && atk <= stat { Outcome::Lose } else { Outcome::Bounce }
    } else if atk > stat {
        Outcome::Win { trick: false }
    } else if atk == stat && !defending {
        // Two monsters with 0 ATK destroy nothing.
        if atk > 0 { Outcome::Trade } else { Outcome::Bounce }
    } else if trick > 0 && atk + trick > stat {
        Outcome::Win { trick: true }
    } else if defending {
        Outcome::Bounce
    } else {
        Outcome::Lose
    };
    // After the battle, their monster takes ours with it (and may go too):
    // whenever it battles, or only when the battle destroys it...
    let takes = met.facts.attacker != Fate::Unharmed;
    let outcome = match outcome {
        Outcome::Win { .. } if takes => Outcome::Trade,
        Outcome::Bounce if takes && !met.facts.when_destroyed => {
            if met.facts.leaves { Outcome::Trade } else { Outcome::Lose }
        }
        other => other,
    };
    // ...or ours takes theirs (D.D. Warrior Lady), when the battle did not.
    match outcome {
        Outcome::Lose | Outcome::Bounce if removes.is_some() => {
            if striking.leaves || outcome == Outcome::Lose { Outcome::Trade } else { Outcome::Win { trick: false } }
        }
        other => other,
    }
}

/// Battle damage an attack on a monster battle does not destroy still deals:
/// worth swinging at an Attack Position wall with less ATK.
fn damage_through_wall(ctx: &Ctx, attacker: &CardView, target: &CardView) -> i32 {
    let met = ctx.attack_meets(Some(attacker), target);
    if !met.survives || met.defending || met.facts.no_damage {
        return 0;
    }
    (met.facts.attacker_atk.apply(attacker.attack + ctx.facts(attacker).striking.bonus) - met.stat).max(0)
}

/// Life Points an attack on this monster costs us on top of the battle's
/// own damage (Reflect Bounder, Amazoness Swords Woman).
fn attack_burn(ctx: &Ctx, attacker: &CardView, target: &CardView) -> i32 {
    let met = ctx.attack_meets(Some(attacker), target);
    let atk = attacker.attack + ctx.facts(attacker).striking.bonus;
    match met.facts.burn {
        Burn::None => 0,
        Burn::Fixed(n) => n,
        Burn::AttackerAtk => atk,
        Burn::Reflected => {
            if met.defending { 0 } else { (atk - met.stat).max(0) }
        }
    }
}

/// What destroying this monster by battle costs us and gives them back: the
/// cards its controller gets, and the other cards of ours it takes along.
fn battle_price<S: Strategy + ?Sized>(s: &S, ctx: &Ctx, attacker: &CardView, target: &CardView) -> i32 {
    let met = ctx.attack_meets(Some(attacker), target).facts;
    let others = || ctx.monsters(ctx.me).into_iter().filter(|c| c.at != attacker.at).map(|c| value(s, ctx, None, Some(c)));
    let collateral = match met.collateral {
        0 => 0,
        ALWAYS => others().sum(),
        n => {
            let mut values: Vec<i32> = others().collect();
            values.sort_unstable_by(|a, b| b.cmp(a));
            values.into_iter().take(n as usize).sum::<i32>().max(n as i32 * 500)
        }
    };
    met.payoff + collateral
}

/// Worth of the best Synchro Monster of this Level in our Extra Deck.
/// Material restrictions are left to the engine: it only offers legal ones.
pub fn synchro_worth<S: Strategy + ?Sized>(s: &S, ctx: &Ctx, level: u32) -> Option<i32> {
    ctx.pile(ctx.me, Location::Extra)
        .into_iter()
        .filter_map(|c| c.code)
        .filter(|code| ctx.data(*code).is(types::SYNCHRO) && ctx.data(*code).level == level)
        .map(|code| value(s, ctx, Some(code), None))
        .max()
}

/// The best Synchro a Tuner of `tuner_level` would make with one of our
/// face-up non-Tuner monsters.
pub fn synchro_with_tuner<S: Strategy + ?Sized>(s: &S, ctx: &Ctx, tuner_level: u32) -> Option<i32> {
    ctx.monsters(ctx.me)
        .into_iter()
        .filter(|c| c.position.face_up && !ctx.view_data(c).is_tuner() && c.level > 0)
        .filter_map(|c| synchro_worth(s, ctx, c.level + tuner_level))
        .max()
}

/// OCGCore's "Attack directly?" prompt, for an attacker that may attack
/// directly while the opponent has monsters.
pub const ATTACK_DIRECTLY: u64 = 31;

/// Our answer to "Attack directly?": yes unless the attack was planned
/// against a monster (its target is then the intent).  `None` for other prompts.
pub fn attack_directly(t: &Turn) -> Option<bool> {
    t.decision
        .choices
        .iter()
        .any(|c| c.description == ATTACK_DIRECTLY && c.card.is_none())
        .then(|| t.memory.intent.is_empty())
}

/// A direct attack past the opponent's monsters: the strongest attacker the
/// engine lets attack directly.  (On an empty field `plan_attack` already
/// attacks directly.)
pub fn direct_attack(t: &Turn) -> Option<usize> {
    let ctx = t.ctx;
    if ctx.monsters(ctx.opp).is_empty() {
        return None;
    }
    t.choices()
        .filter(|(i, c)| c.kind == ChoiceKind::Attack && t.fresh(*i) && c.card.map_or(false, |m| m.value == 1))
        .filter_map(|(i, c)| c.at().and_then(|at| ctx.card(at)).map(|v| (i, v.attack)))
        .filter(|(_, attack)| *attack > 0)
        .max_by_key(|(_, attack)| *attack)
        .map(|(i, _)| i)
}

/// An attack we cannot decline (a monster that must attack if able): the
/// attacker and target that lose least.  Attacks already declined three
/// times are still allowed, the engine offers nothing else.
pub fn forced_attack<S: Strategy + ?Sized>(s: &S, t: &Turn) -> Option<(usize, Option<CardRef>)> {
    let ctx = t.ctx;
    let targets = ctx.monsters(ctx.opp);
    let mut best: Option<(f64, (usize, Option<CardRef>))> = None;
    for (i, choice) in t.choices().filter(|(_, c)| c.kind == ChoiceKind::Attack) {
        let Some(attacker) = choice.at().and_then(|at| ctx.card(at)) else { continue };
        let direct = targets.is_empty() || choice.card.map_or(false, |m| m.value == 1);
        let mut options: Vec<(f64, Option<CardRef>)> = Vec::new();
        if direct {
            options.push((attacker.attack as f64, None));
        }
        for target in &targets {
            let outcome = s
                .attack_outcome(&ctx, attacker, target)
                .unwrap_or_else(|| default_outcome(&ctx, attacker, target, s.attack_trick(&ctx, attacker)));
            let mine = value(s, &ctx, None, Some(attacker)) as f64;
            let score = match outcome {
                Outcome::Win { .. } => 1000.0 + ctx.threat(target) as f64,
                Outcome::Trade => ctx.threat(target) as f64 - mine,
                Outcome::Bounce => -(ctx.battle_stat(target) - attacker.attack).max(0) as f64 / 10.0,
                Outcome::Lose => -mine - (ctx.battle_stat(target) - attacker.attack) as f64 / 10.0,
            };
            options.push((score, Some(target.at)));
        }
        for (score, target) in options {
            if best.as_ref().map_or(true, |b| score > b.0) {
                best = Some((score, (i, target)));
            }
        }
    }
    best.map(|(_, choice)| choice)
}

/// Pick the next attack: (attack choice, target or `None` for direct).
pub fn plan_attack<S: Strategy + ?Sized>(s: &S, t: &Turn) -> Option<(usize, Option<CardRef>)> {
    let ctx = t.ctx;
    let attackers: Vec<(usize, &CardView)> = t
        .choices()
        .filter(|(i, c)| c.kind == ChoiceKind::Attack && t.fresh(*i))
        .filter_map(|(i, c)| c.at().and_then(|at| ctx.card(at)).map(|v| (i, v)))
        .collect();
    if attackers.is_empty() {
        return None;
    }
    let targets = ctx.monsters(ctx.opp);
    if targets.is_empty() {
        let (i, _) = attackers.iter().filter(|(_, v)| v.attack > 0).max_by_key(|(_, v)| v.attack)?;
        return Some((*i, None));
    }
    let mut scored: Vec<(f64, (usize, CardRef))> = Vec::new();
    for (i, attacker) in &attackers {
        let trick = s.attack_trick(&ctx, attacker);
        for target in &targets {
            let outcome = s
                .attack_outcome(&ctx, attacker, target)
                .unwrap_or_else(|| default_outcome(&ctx, attacker, target, trick));
            // A monster that pays its controller back when battle destroys it
            // is worth less to kill; one whose controller can negate the
            // attack is still worth a try, after the others.
            let negatable = if ctx.attack_negatable(target) { 600 } else { 0 };
            let burn = attack_burn(&ctx, attacker, target);
            if burn > 0 && burn >= ctx.my_lp() {
                continue;
            }
            let gain = (ctx.threat(target) - battle_price(s, &ctx, attacker, target) - negatable - burn / 2) as f64;
            let score = match outcome {
                // Prefer the weakest attacker that still wins; keep tricks.
                Outcome::Win { trick } => {
                    1000.0 + gain - attacker.attack as f64 / 10.0 - if trick { 600.0 } else { 0.0 }
                }
                Outcome::Trade => {
                    let (mine, gain) = trade(s, &ctx, attacker, target, gain);
                    // Equal ATK: only for a target worth our attacker.  A
                    // monster that takes its attacker with it: one for one
                    // beats waiting behind it (measured), unless it takes
                    // more than the attacker.
                    let met = ctx.attack_meets(Some(attacker), target).facts;
                    let by_effect = met.attacker != Fate::Unharmed || ctx.strike_removes(attacker, target).is_some();
                    if gain < mine && (!by_effect || met.collateral > 0) {
                        continue;
                    }
                    200.0 + gain - mine
                }
                // A wall battle cannot destroy still takes damage in Attack Position.
                Outcome::Bounce if damage_through_wall(&ctx, attacker, target) > 0 => {
                    100.0 + damage_through_wall(&ctx, attacker, target) as f64 / 10.0
                }
                // It survives a battle or two a turn: spend our weakest
                // attacker that beats it, when another can finish the job.
                Outcome::Bounce if breaks_shield(&ctx, attacker, target, &attackers) => 150.0 - attacker.attack as f64 / 10.0,
                _ => continue,
            };
            scored.push((score, (*i, target.at)));
        }
    }
    // Equal targets (two face-down monsters) are chosen at random, not by zone.
    t.memory.ties.best(scored).map(|(_, (i, at))| (i, Some(at)))
}

/// What a battle that takes both monsters off the field costs us and them:
/// (our loss, our gain), `gain` being what removing the target is worth.  A
/// monster that only returns to the hand is a Summon to make again, not a
/// card lost; one that returns to the Extra Deck is lost.
fn trade<S: Strategy + ?Sized>(s: &S, ctx: &Ctx, attacker: &CardView, target: &CardView, gain: f64) -> (f64, f64) {
    const SUMMON_AGAIN: f64 = 400.0;
    let met = ctx.attack_meets(Some(attacker), target).facts;
    let striking = ctx.facts(attacker).striking;
    let ours_bounces = ctx.strike_removes(attacker, target).is_some() && striking.target == Fate::Returned;
    if met.attacker != Fate::Returned && !ours_bounces {
        return (value(s, ctx, None, Some(attacker)) as f64, gain);
    }
    let mine = if ctx.view_data(attacker).is_extra() { value(s, ctx, None, Some(attacker)) as f64 } else { SUMMON_AGAIN };
    // Theirs goes back too (Grand Mole), unless the battle destroyed it.
    let theirs_returns = ours_bounces || met.leaves;
    let gain = if theirs_returns && !ctx.view_data(target).is_extra() { SUMMON_AGAIN } else { gain };
    (mine, gain)
}

/// The target survives only so many battles a turn, this attacker beats it
/// on the numbers, and enough of our other attackers do too to get through.
fn breaks_shield(ctx: &Ctx, attacker: &CardView, target: &CardView, attackers: &[(usize, &CardView)]) -> bool {
    let met = ctx.attack_meets(Some(attacker), target);
    if !met.survives || met.facts.survives == ALWAYS || met.facts.survives == 0 {
        return false;
    }
    let beats = |a: &CardView| {
        let m = ctx.attack_meets(Some(a), target);
        m.facts.attacker == Fate::Unharmed && m.facts.attacker_atk.apply(a.attack + ctx.facts(a).striking.bonus) > m.stat
    };
    let left = (met.facts.survives as u32).saturating_sub(target.battles) as usize;
    beats(attacker) && attackers.iter().filter(|(_, a)| a.at != attacker.at && beats(a)).count() >= left
}
