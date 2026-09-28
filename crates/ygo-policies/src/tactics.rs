//! Deck-independent play: summons, attacks, positions, setting backrow.

use crate::agent::{value, Outcome, Strategy, Turn};
use crate::cards::types;
use crate::ctx::Ctx;
use crate::model::{CardRef, CardView, ChoiceKind, Location};

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
pub fn default_outcome(ctx: &Ctx, attacker: &CardView, target: &CardView, trick: i32) -> Outcome {
    let atk = attacker.attack;
    let stat = ctx.battle_stat(target);
    let defending = target.position.face_up && !target.position.attack;
    if !target.position.face_up && !target.known() {
        // Unknown face-down monster: only strong attackers risk it.
        if atk >= 1900 {
            return Outcome::Win { trick: false };
        }
        if trick > 0 && atk + trick >= 1900 {
            return Outcome::Win { trick: true };
        }
        return Outcome::Lose;
    }
    if atk > stat {
        return Outcome::Win { trick: false };
    }
    if atk == stat && !defending {
        return Outcome::Trade;
    }
    if trick > 0 && atk + trick > stat {
        return Outcome::Win { trick: true };
    }
    if defending {
        Outcome::Bounce
    } else {
        Outcome::Lose
    }
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
    let mut best: Option<(f64, usize, CardRef)> = None;
    for (i, attacker) in &attackers {
        let trick = s.attack_trick(&ctx, attacker);
        for target in &targets {
            let outcome = s
                .attack_outcome(&ctx, attacker, target)
                .unwrap_or_else(|| default_outcome(&ctx, attacker, target, trick));
            let gain = ctx.threat(target) as f64;
            let score = match outcome {
                // Prefer the weakest attacker that still wins; keep tricks.
                Outcome::Win { trick } => {
                    1000.0 + gain - attacker.attack as f64 / 10.0 - if trick { 600.0 } else { 0.0 }
                }
                Outcome::Trade => {
                    let mine = value(s, &ctx, None, Some(attacker)) as f64;
                    if gain < mine {
                        continue;
                    }
                    200.0 + gain - mine
                }
                _ => continue,
            };
            if best.as_ref().map_or(true, |b| score > b.0) {
                best = Some((score, *i, target.at));
            }
        }
    }
    best.map(|(_, i, at)| (i, Some(at)))
}
