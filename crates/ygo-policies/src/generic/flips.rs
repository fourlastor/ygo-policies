//! Flip Summons that spend a Flip effect now, instead of waiting for an
//! attack that may never come.
//!
//! The specialists of flip decks (Needle Worm, Morphing Jar, Gravekeeper's
//! Spy, Guardian Sphinx) turn their monsters face-up at the start of the Main
//! Phase. Which effects are worth it is read from the card's own Flip effect
//! and Flip Summon triggers: what they do (categories) and whether the board
//! gives them something to do now. Effects stay opaque otherwise.
use crate::agent::Turn;
use crate::cards::effect_categories as cat;
use crate::cards::effect_events as event;
use crate::model::{CardView, ChoiceKind, DecisionKind, Location, Phase};

/// EFFECT_TYPE_FLIP, and EVENT_FLIP for triggers on any flip.
const FLIP_EFFECT: u16 = 0x20;
const FLIPPED: u32 = 1001;
/// CATEGORY_DECKDES and CATEGORY_HANDES, absent from the shared list.
const MILL: u64 = 0x40;
const DISCARD: u64 = 0x80;
const COUNTER: u64 = 0x800000;

/// What the card's Flip effects and Flip Summon triggers do.
fn flip_categories(t: &Turn, code: u32) -> u64 {
    t.ctx.db.effects(code).iter()
        .filter(|e| e.effect_type & FLIP_EFFECT != 0 || matches!(e.event, FLIPPED | event::FLIP_SUMMON_SUCCESS))
        .fold(0, |all, e| all | e.category)
}

/// Whether flipping this face-down monster of ours does something now.
fn worth(t: &Turn, card: &CardView, code: u32) -> bool {
    let ctx = t.ctx;
    let what = flip_categories(t, code);
    if what == 0 || ctx.effects_drained() || ctx.wasted(code) {
        return false;
    }
    let their_cards = !ctx.monsters(ctx.opp).is_empty() || !ctx.spell_traps(ctx.opp).is_empty();
    let their_face_up = ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up);
    // A swing that ends the duel once ours stands face-up in Attack Position.
    let fatal = ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up && c.attack.saturating_sub(card.attack.max(0)) >= ctx.my_lp());
    if fatal {
        return false;
    }
    // Both hands for five new cards: only from a small hand, with a Deck to draw.
    if what & DISCARD != 0 && what & cat::DRAW != 0 {
        return ctx.hand_size(ctx.me) <= 3 && ctx.deck_size(ctx.me) >= 5;
    }
    what & (cat::DESTROY | cat::REMOVE | cat::TO_HAND | cat::TO_DECK | cat::CONTROL) != 0 && their_cards
        || what & (cat::SPECIAL_SUMMON | cat::TOKEN) != 0 && ctx.free_monster_zones(ctx.me) > 0
        || what & (cat::DRAW | cat::SEARCH) != 0 && ctx.deck_size(ctx.me) > 1
        || what & MILL != 0 && ctx.deck_size(ctx.opp) > 0
        || what & COUNTER != 0 && their_face_up
}

/// In our Main Phase: the Flip Summon of a Set monster whose flip is worth it.
pub(super) fn flip_summon(t: &mut Turn) -> Option<usize> {
    let ctx = t.ctx;
    if t.decision.kind != DecisionKind::Idle || !ctx.my_turn() || !matches!(ctx.phase(), Some(Phase::Main1 | Phase::Main2)) {
        return None;
    }
    let index = t.choices().find(|(index, choice)| {
        choice.kind == ChoiceKind::ChangePosition && t.fresh(*index)
            && choice.at().is_some_and(|at| at.controller == ctx.me && at.location == Location::MonsterZone)
            && t.view(choice).is_some_and(|card| !card.position.face_up
                && card.code.is_some_and(|code| worth(t, card, ctx.canonical(code))))
    }).map(|(index, _)| index)?;
    t.pick(index)
}
