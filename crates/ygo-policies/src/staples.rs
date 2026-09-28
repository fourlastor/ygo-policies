//! Cards many decks share, played the same way everywhere.  A deck can veto a
//! staple with [`Strategy::allow_staple`] or take it over in its own hooks.

use crate::agent::{Response, Strategy, Turn};
use crate::cards::attributes;
use crate::model::CardRef;

pub const MYSTICAL_SPACE_TYPHOON: u32 = 5318639;
pub const GIANT_TRUNADE: u32 = 42703248;
pub const HEAVY_STORM: u32 = 19613556;
pub const COLD_WAVE: u32 = 60682203;
pub const DARK_HOLE: u32 = 53129443;
pub const RAIGEKI: u32 = 12580477;
pub const BOOK_OF_MOON: u32 = 14087893;
pub const ALLURE_OF_DARKNESS: u32 = 1475311;
pub const POT_OF_DUALITY: u32 = 98645731;
pub const MONSTER_REBORN: u32 = 83764718;
pub const CALL_OF_THE_HAUNTED: u32 = 97077563;
pub const PREMATURE_BURIAL: u32 = 70828912;
pub const TRAP_HOLE: u32 = 4206964;
pub const BOTTOMLESS_TRAP_HOLE: u32 = 29401950;
pub const TORRENTIAL_TRIBUTE: u32 = 53582587;
pub const MIRROR_FORCE: u32 = 44095762;
pub const DIMENSIONAL_PRISON: u32 = 70342110;
pub const THREATENING_ROAR: u32 = 36361633;
pub const SEVEN_TOOLS: u32 = 3819470;
pub const SWORDS_OF_REVEALING_LIGHT: u32 = 72302403;
pub const GRAVITY_BIND: u32 = 85742772;
pub const WALL_OF_REVEALING_LIGHT: u32 = 17078030;
pub const MORPHTRONIC_BIND: u32 = 85101228;
/// Attackers are stuck in Defense Position for a turn: half the attacks.
pub const SPIDER_WEB: u32 = 69408987;

/// Continuous cards that keep the other player from attacking: a stall
/// deck's win condition, so they are the first thing to remove.
pub const ATTACK_LOCKS: &[u32] = &[SWORDS_OF_REVEALING_LIGHT, GRAVITY_BIND, WALL_OF_REVEALING_LIGHT, MORPHTRONIC_BIND, SPIDER_WEB];

pub fn value(code: u32) -> Option<i32> {
    Some(match code {
        DARK_HOLE | RAIGEKI | HEAVY_STORM => 2400,
        MONSTER_REBORN => 2100,
        MIRROR_FORCE | TORRENTIAL_TRIBUTE => 1800,
        BOTTOMLESS_TRAP_HOLE | DIMENSIONAL_PRISON | CALL_OF_THE_HAUNTED | PREMATURE_BURIAL => 1600,
        BOOK_OF_MOON => 1550,
        MYSTICAL_SPACE_TYPHOON => 1400,
        TRAP_HOLE | POT_OF_DUALITY => 1300,
        ALLURE_OF_DARKNESS => 1200,
        THREATENING_ROAR => 1100,
        SEVEN_TOOLS | GIANT_TRUNADE => 1000,
        COLD_WAVE => 900,
        _ => return None,
    })
}

pub fn is_revival(code: u32) -> bool {
    matches!(code, MONSTER_REBORN | CALL_OF_THE_HAUNTED | PREMATURE_BURIAL)
}

/// Staple spells/traps we would activate in our own Main Phase.
pub fn main_phase<S: Strategy + ?Sized>(s: &S, t: &mut Turn) -> Option<usize> {
    let ctx = t.ctx;
    let (me, opp) = (ctx.me, ctx.opp);
    let usable = |t: &Turn, code: u32| -> Option<usize> {
        if s.allow_staple(t, code) {
            t.activate(code)
        } else {
            None
        }
    };
    let my_monsters = ctx.monsters(me);
    let opp_monsters = ctx.monsters(opp);
    let backrow = ctx.set_backrow(opp);
    let can_battle = t.has(crate::model::ChoiceKind::EnterBattle);
    let hand_monsters = ctx.hand().iter().filter(|c| ctx.view_data(c).is_monster()).count();

    // 0. The opponent's attack lock stalls us out: break it whatever it
    // costs -- but never our own locks with it.
    let locks = ctx.attack_locks();
    if !locks.is_empty() && (!my_monsters.is_empty() || hand_monsters > 0) {
        if let Some(i) = usable(t, MYSTICAL_SPACE_TYPHOON) {
            return t.pick_targeting(i, refs(&locks));
        }
        // These two hit both sides.
        if ctx.own_attack_locks().is_empty() {
            if let Some(i) = usable(t, HEAVY_STORM) {
                return t.pick(i);
            }
            if let Some(i) = usable(t, GIANT_TRUNADE) {
                return t.pick(i);
            }
        }
    }

    // 1. Clear the opponent's backrow before we commit to attacks.
    if ctx.main1() && can_battle && (!my_monsters.is_empty() || hand_monsters > 0) {
        // Cold Wave also stops us setting our own backrow: only for an attack.
        if !backrow.is_empty() && attack_is_coming(t) {
            if let Some(i) = usable(t, COLD_WAVE) {
                return t.pick(i);
            }
        }
        let my_set = ctx.set_backrow(me).len();
        // Both of these also sweep our own backrow: never over our own locks.
        let own_locks = !ctx.own_attack_locks().is_empty();
        if backrow.len() >= 2 && my_set <= 1 && !own_locks {
            if let Some(i) = usable(t, HEAVY_STORM) {
                return t.pick(i);
            }
        }
        if ctx.spell_traps(opp).len() >= 2 && my_set <= 1 && !own_locks {
            if let Some(i) = usable(t, GIANT_TRUNADE) {
                return t.pick(i);
            }
        }
        if !backrow.is_empty() {
            if let Some(i) = usable(t, MYSTICAL_SPACE_TYPHOON) {
                return t.pick_targeting(i, refs(&backrow));
            }
        }
    }

    // 2. Wipe a board we cannot beat.
    if !opp_monsters.is_empty() {
        let opp_strength = ctx.field_strength(opp);
        let my_strength = ctx.field_strength(me);
        if let Some(i) = usable(t, RAIGEKI) {
            if opp_strength >= 1500 {
                return t.pick(i);
            }
        }
        if let Some(i) = usable(t, DARK_HOLE) {
            let outclassed = ctx.opp_best_attack() > ctx.my_best_attack();
            if my_strength <= opp_strength
                && (opp_monsters.len() >= 2 || outclassed || my_monsters.is_empty())
            {
                return t.pick(i);
            }
        }
        if ctx.main1() && !my_monsters.is_empty() {
            if let Some(i) = usable(t, BOOK_OF_MOON) {
                // Flip a face-up monster that walls our whole team.
                let best = ctx.my_best_attack();
                let wall = opp_monsters
                    .iter()
                    .filter(|c| c.position.face_up && c.position.attack && c.attack >= best && c.defense < best)
                    .max_by_key(|c| c.attack);
                if let Some(w) = wall {
                    return t.pick_targeting(i, vec![w.at]);
                }
            }
        }
    }

    // 3. Card advantage.
    let dark_in_hand = ctx
        .hand()
        .iter()
        .any(|c| {
            let d = ctx.view_data(c);
            d.is_monster() && d.attribute & attributes::DARK != 0
        });
    if dark_in_hand {
        if let Some(i) = usable(t, ALLURE_OF_DARKNESS) {
            return t.pick(i);
        }
    }
    let wants_special = t.has(crate::model::ChoiceKind::SpecialSummon);
    if !ctx.main1() || !wants_special {
        if let Some(i) = usable(t, POT_OF_DUALITY) {
            return t.pick(i);
        }
    }
    if let Some(i) = usable(t, MONSTER_REBORN) {
        let best = ctx
            .graveyard(me)
            .into_iter()
            .chain(ctx.graveyard(opp))
            .filter(|c| ctx.view_data(c).is_monster())
            .max_by_key(|c| ctx.view_data(c).attack);
        if let Some(b) = best {
            if ctx.view_data(b).attack >= 1600 || my_monsters.is_empty() {
                return t.pick_targeting(i, vec![b.at]);
            }
        }
    }
    None
}

/// Will one of our monsters (on the field, or Normal Summoned from the hand
/// this turn) get damage through this turn on public numbers?
fn attack_is_coming(t: &Turn) -> bool {
    let ctx = t.ctx;
    let mut attackers: Vec<i32> = ctx
        .monsters(ctx.me)
        .iter()
        .filter(|c| c.position.face_up && c.position.attack)
        .map(|c| c.attack)
        .collect();
    if t.has(crate::model::ChoiceKind::NormalSummon) {
        let best_in_hand = ctx
            .hand()
            .iter()
            .map(|c| ctx.view_data(c))
            .filter(|d| d.is_monster() && d.tributes() == 0)
            .map(|d| d.attack)
            .max();
        attackers.extend(best_in_hand);
    }
    let Some(best) = attackers.iter().copied().max() else { return false };
    let blockers = ctx.monsters(ctx.opp);
    blockers.is_empty() || blockers.iter().any(|c| best > ctx.battle_stat(c))
}

fn refs(cards: &[&crate::model::CardView]) -> Vec<CardRef> {
    cards.iter().map(|c| c.at).collect()
}

/// How much we want to chain a staple.  `None` for non-staples.
pub fn chain<S: Strategy + ?Sized>(s: &S, t: &Turn, index: usize) -> Option<Response> {
    let ctx = t.ctx;
    let choice = t.choice(index);
    let code = ctx.canonical(choice.code()?);
    if !s.allow_staple(t, code) {
        return Some(Response::no());
    }
    let (me, opp) = (ctx.me, ctx.opp);
    let incoming = ctx.incoming_attack();
    Some(match code {
        TRAP_HOLE | BOTTOMLESS_TRAP_HOLE => Response::new(100.0),
        TORRENTIAL_TRIBUTE => {
            let gain = ctx.field_strength(opp) - ctx.field_strength(me);
            if gain >= 1000 {
                // Capped below Trap Hole so it never stacks on our own answer.
                Response::new((gain as f64 / 10.0).min(95.0))
            } else {
                Response::no()
            }
        }
        MIRROR_FORCE => match incoming {
            Some(_) => {
                let hit = ctx.monsters(opp).iter().filter(|c| c.position.attack).count();
                Response::new(50.0 + 20.0 * hit as f64)
            }
            None => Response::no(),
        },
        DIMENSIONAL_PRISON => match incoming {
            Some((attacker, target)) if ctx.attack_hurts(attacker, target) => {
                Response::targeting(85.0, vec![attacker.at])
            }
            _ => Response::no(),
        },
        SEVEN_TOOLS => {
            let hostile_trap = t.hostile_top().matches(|link| ctx.data(link.code).is_trap());
            if hostile_trap && ctx.my_lp() > 1500 {
                Response::new(90.0)
            } else {
                Response::no()
            }
        }
        BOOK_OF_MOON => match incoming {
            Some((attacker, target)) if attacker.position.face_up && ctx.attack_hurts(attacker, target) => {
                Response::targeting(80.0, vec![attacker.at])
            }
            _ => Response::no(),
        },
        THREATENING_ROAR => {
            let battle_opening = !ctx.my_turn()
                && matches!(ctx.phase(), Some(crate::model::Phase::BattleStart | crate::model::Phase::BattleStep));
            if !battle_opening {
                return Some(Response::no());
            }
            let declared = incoming.map(|(a, _)| a.at);
            let attackers: Vec<_> = ctx
                .monsters(opp)
                .into_iter()
                .filter(|c| c.position.face_up && c.position.attack && Some(c.at) != declared)
                .collect();
            let mine = ctx.monsters(me);
            let weakest = mine.iter().map(|c| ctx.battle_stat(c)).min().unwrap_or(0);
            let dangerous: Vec<_> = attackers.iter().filter(|c| mine.is_empty() || c.attack > weakest).collect();
            let damage: i32 = if mine.is_empty() { dangerous.iter().map(|c| c.attack).sum() } else { 0 };
            if !dangerous.is_empty() && (damage >= 1500 || !mine.is_empty()) {
                Response::new(30.0)
            } else {
                Response::no()
            }
        }
        MYSTICAL_SPACE_TYPHOON => {
            let backrow = ctx.set_backrow(opp);
            let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(crate::model::Phase::End);
            let before_our_attack = ctx.my_turn() && ctx.main1();
            if !backrow.is_empty() && (end_of_their_turn || before_our_attack) {
                Response::targeting(20.0, refs(&backrow))
            } else {
                Response::no()
            }
        }
        CALL_OF_THE_HAUNTED => {
            let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(crate::model::Phase::End);
            let best = ctx
                .graveyard(me)
                .into_iter()
                .filter(|c| ctx.view_data(c).is_monster())
                .max_by_key(|c| ctx.view_data(c).attack);
            match best {
                Some(b) if end_of_their_turn || incoming.is_some() => Response::targeting(25.0, vec![b.at]),
                _ => Response::no(),
            }
        }
        DARK_HOLE | RAIGEKI | HEAVY_STORM | COLD_WAVE | GIANT_TRUNADE | MONSTER_REBORN
        | POT_OF_DUALITY | ALLURE_OF_DARKNESS | PREMATURE_BURIAL => Response::no(),
        _ => return None,
    })
}
