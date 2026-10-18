//! Normal Summon or Set, from what the monster's own effects need.
//!
//! The shared rule Sets a monster that cannot face the opponent's best
//! attacker and Summons one that can, or any monster onto an empty field. A
//! weak monster whose effects never work face-up on the field (Reptilianne
//! Gardna searches when it is destroyed; Marshmallon cannot be destroyed by
//! battle) loses nothing Set, and face-up in Attack Position it only invites
//! an attack. One whose effects work only face-up keeps the shared rule.
use crate::agent::Turn;
use crate::cards::{effect_events as event, effect_locations as range, types};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind};

/// The ATK under which a monster is no attacker at all.
const WEAK: i32 = 500;

/// An effect that works only while the monster is face-up on the field, or
/// one its Normal Summon starts.
fn face_up_effect(ctx: &Ctx, code: u32) -> bool {
    ctx.db.effects(code).iter().any(|effect| {
        effect.event == event::SUMMON_SUCCESS || effect.range & (range::MONSTER | range::MONSTER_MAIN) != 0
    })
}

pub(super) fn summon_score(t: &Turn, choice: &Choice) -> Option<Option<f64>> {
    let ctx = t.ctx;
    let code = ctx.canonical(choice.code()?);
    let data = ctx.data(code);
    // A Tuner is Synchro material, which has to be face-up.
    if data.tributes() != 0 || data.is(types::FLIP) || !data.is(types::EFFECT) || data.is_tuner() || ctx.db.effects(code).is_empty() {
        return None;
    }
    // Summoning every monster whose effects need the field instead of Setting
    // it was measured: it exposes too many weak ones (+289 / -288 games).
    if data.attack > WEAK || data.defense <= data.attack || face_up_effect(&ctx, code) {
        return None;
    }
    match choice.kind {
        ChoiceKind::NormalSummon => Some(None),
        ChoiceKind::SetMonster => Some(Some(data.defense as f64)),
        _ => None,
    }
}
