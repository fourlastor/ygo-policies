//! "Undersea Ceremony": Gishki Ritual Summons.
//!
//! The plan is a Ritual Monster every turn.  Searchers find the pieces
//! (Manju: Ritual Monster or Spell; Sonic Bird: Ritual Spell; Gishki Chain:
//! top 3; Preparation of Rites: monster + spell back), Gishki Aquamirror
//! Tributes Levels from hand/field that add up exactly to the Ritual
//! Monster's (Gishki Shadow alone covers a WATER Ritual Monster) and returns
//! itself for the Ritual Monster in the Graveyard.  Forbidden Arts of the
//! Gishki Tributes face-up monsters from *either* field: removal that brings
//! a half-strength body.
//!
//! Ritual Monsters: Soul Ogre (2800, shuffles a card away), Tetrogre, Mind
//! Augus (shuffles the opponent's Graveyard into their Deck).

use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member, Phase};

pub const DECK: &str = "Undersea Ceremony";

const SHADOW: u32 = 29888389;
const NOELLIA: u32 = 63942761;
const SONIC_BIRD: u32 = 57617178;
const MANJU: u32 = 95492061;
const ABYSS: u32 = 67111213;
const ARIEL: u32 = 92784374;
const CHAIN: u32 = 66399675;
const TETROGRE: u32 = 21496848;
const MIND_AUGUS: u32 = 11877465;
const RELIEVER: u32 = 37557626;
const VANITY: u32 = 93506862;
const SOUL_OGRE: u32 = 57272170;
const AQUAMIRROR: u32 = 46159582;
const SALVAGE: u32 = 96947648;
const FORBIDDEN_ARTS: u32 = 28429121;
const PREPARATION: u32 = 96729612;
const MEDITATION: u32 = 46337945;
const TRAP_STUN: u32 = 59616123;
const SET_GISHKI: u16 = 0x3a;

#[derive(Default)]
pub struct Gishki;

impl Gishki {
    fn ritual_monster(ctx: &Ctx, code: u32) -> bool {
        ctx.data(code).is(crate::cards::types::RITUAL) && ctx.data(code).is_monster()
    }

    fn ritual_spell(ctx: &Ctx, code: u32) -> bool {
        ctx.data(code).is(crate::cards::types::RITUAL) && ctx.data(code).is_spell()
    }

    fn hand_has(ctx: &Ctx, pred: impl Fn(u32) -> bool) -> bool {
        ctx.hand().iter().filter_map(|c| c.code).any(|c| pred(ctx.canonical(c)))
    }
}

impl Strategy for Gishki {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            SOUL_OGRE => 2800,
            TETROGRE => 2600,
            MIND_AUGUS => 2500,
            AQUAMIRROR => 2200,
            CHAIN => 1800,
            MANJU | SONIC_BIRD | PREPARATION => 1700,
            // Alone, it is a whole Ritual Tribute.
            SHADOW => 1650,
            NOELLIA => 1500,
            ABYSS | SALVAGE | MEDITATION => 1300,
            ARIEL => 1200,
            VANITY => 1000,
            FORBIDDEN_ARTS => 1100,
            RELIEVER => 900,
            TRAP_STUN => 700,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if let Some(i) = t.activate(PREPARATION) {
            return t.pick(i);
        }
        // The Ritual Summon itself (only offered when the Levels can be paid).
        if let Some(i) = t.activate_from(AQUAMIRROR, Location::Hand).or_else(|| t.activate_from(AQUAMIRROR, Location::SpellTrapZone)) {
            return t.pick(i);
        }
        let ritual_monster_in_hand = Self::hand_has(&ctx, |c| Self::ritual_monster(&ctx, c));
        let ritual_spell_in_hand = Self::hand_has(&ctx, |c| Self::ritual_spell(&ctx, c));
        // Aquamirror in the Graveyard: shuffle it back for a Ritual Monster.
        if !ritual_monster_in_hand {
            if let Some(i) = t.activate_from(AQUAMIRROR, Location::Graveyard) {
                return t.pick(i);
            }
        }
        // Shadow: trade itself for the Ritual Spell when we lack one.
        if ritual_monster_in_hand && !ritual_spell_in_hand {
            if let Some(i) = t.activate_from(SHADOW, Location::Hand) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(SALVAGE) {
            return t.pick(i);
        }
        // Forbidden Arts: Tribute the opponent's biggest face-up monster.
        let threat = ctx
            .monsters(ctx.opp)
            .into_iter()
            .filter(|c| c.position.face_up)
            .max_by_key(|c| ctx.threat(c))
            .filter(|c| ctx.threat(c) >= 2000 && c.attack >= ctx.my_best_attack());
        if let Some(threat) = threat {
            if let Some(i) = t.activate(FORBIDDEN_ARTS) {
                return t.pick_targeting(i, vec![threat.at]);
            }
        }
        // Soul Ogre: discard a Gishki to shuffle their best face-up card away
        // (it can only target theirs): a card for a card, and even a small
        // monster is a blocker gone.  The discard is the cheapest Gishki; a
        // Ritual Monster is a fair price too, Aquamirror brings it back.
        if Self::hand_has(&ctx, |c| ctx.data(c).in_set(SET_GISHKI) && ctx.data(c).is_monster()) {
            let target = ctx
                .monsters(ctx.opp)
                .into_iter()
                .chain(ctx.spell_traps(ctx.opp))
                .filter(|c| c.position.face_up)
                .max_by_key(|c| ctx.threat(c).max(if ctx.view_data(c).is_monster() { 0 } else { 1500 }));
            if let Some(target) = target {
                if let Some(i) = t.activate_from(SOUL_OGRE, Location::MonsterZone) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let has_monster = Self::hand_has(&ctx, |c| Self::ritual_monster(&ctx, c));
        let has_spell = Self::hand_has(&ctx, |c| Self::ritual_spell(&ctx, c));
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, MANJU) if !(has_monster && has_spell) => Some(2100.0),
            (ChoiceKind::NormalSummon, SONIC_BIRD) if !has_spell => Some(2050.0),
            (ChoiceKind::NormalSummon, CHAIN) => Some(2000.0),
            (ChoiceKind::NormalSummon, MANJU | SONIC_BIRD) => Some(1500.0),
            (ChoiceKind::NormalSummon, ABYSS) => Some(1600.0),
            (ChoiceKind::NormalSummon, NOELLIA) => Some(1550.0),
            (ChoiceKind::SetMonster, ARIEL) => Some(1400.0),
            // Hand pieces: Shadow is a whole Ritual Tribute, Vanity a Level 2 one.
            (ChoiceKind::NormalSummon | ChoiceKind::SetMonster, SHADOW | VANITY | RELIEVER) => None,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        Some(match code {
            // Recover two Gishki: best at the end of the opponent's turn.
            MEDITATION if !ctx.my_turn() && ctx.phase() == Some(Phase::End) => Response::new(30.0),
            MEDITATION => Response::no(),
            // Trap Stun clears the way for an attack into a Set backrow.
            TRAP_STUN => {
                let attacking = ctx.my_turn() && matches!(ctx.phase(), Some(Phase::Main1 | Phase::BattleStart));
                if attacking && ctx.set_backrow(ctx.opp).len() >= 2 && ctx.monsters(ctx.me).iter().any(|c| ctx.can_attack(c)) {
                    Response::new(30.0)
                } else {
                    Response::no()
                }
            }
            // Tetrogre mills both players: not in this plan.
            TETROGRE => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let code = member.code.map(|c| ctx.canonical(c));
        let mine = member.at.controller == ctx.me;
        match t.decision.hint {
            // Ritual Tributes: prefer the opponent's monsters (Forbidden Arts),
            // then our spent or redundant pieces.
            Hint::Release | Hint::Tribute if mine => {
                let worth = value(self, &ctx, code, ctx.card(member.at).filter(|v| v.known())) as f64;
                // Tributing a face-up monster that already used its search costs less than a hand card.
                Some(if member.at.location == Location::MonsterZone { -worth * 0.6 } else { -worth })
            }
            // Mind Augus: shuffle the opponent's Graveyard, never ours.
            Hint::ToDeck if member.at.location == Location::Graveyard => Some(if mine { -1.0 } else { 1.0 }),
            _ => None,
        }
    }
}
