//! Cards many decks share, played the same way everywhere.  A deck can veto a
//! staple with [`Strategy::allow_staple`] or take it over in its own hooks.

use crate::agent::{self, Hostile, Response, Strategy, Turn};
use crate::cards::attributes;
use crate::ctx::Ctx;
use crate::model::{CardRef, CardView, ChainLink, ChoiceKind, Location, Phase};

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
pub const MESSENGER_OF_PEACE: u32 = 44656491;
pub const LEVEL_LIMIT_AREA_B: u32 = 3136426;
pub const NIGHTMARES_STEELCAGE: u32 = 58775978;
/// Synchros several Extra Decks share: discard N, bounce N of theirs.
pub const BRIONAC: u32 = 50321796;
/// Destroy 1 card we control and 1 card they control.
pub const SCRAP_DRAGON: u32 = 76774528;
pub const WABOKU: u32 = 12607053;
pub const NEGATE_ATTACK: u32 = 14315573;
pub const DRAINING_SHIELD: u32 = 43250041;
pub const SAKURETSU_ARMOR: u32 = 56120475;
pub const MAGIC_CYLINDER: u32 = 62279055;
pub const SOLEMN_WARNING: u32 = 84749824;
/// What Solemn Warning costs, in Life Points.
pub const SOLEMN_WARNING_PRICE: i32 = 2000;
pub const SOLEMN_JUDGMENT: u32 = 41420027;
pub const COMPULSORY_EVACUATION_DEVICE: u32 = 94192409;
pub const DUST_TORNADO: u32 = 60082869;
pub const TRAP_DUSTSHOOT: u32 = 64697231;
pub const EFFECT_VEILER: u32 = 97268402;
pub const SHRINK: u32 = 55713623;
pub const ENEMY_CONTROLLER: u32 = 98045062;
pub const SMASHING_GROUND: u32 = 97169186;
pub const SHIELD_CRUSH: u32 = 30683373;
pub const BRAIN_CONTROL: u32 = 87910978;
pub const POT_OF_AVARICE: u32 = 67169062;
pub const DOUBLE_SUMMON: u32 = 43422537;
pub const LIGHTNING_VORTEX: u32 = 69162969;
/// From the hand, after an attack leaves us with no cards.
pub const GORZ: u32 = 44330098;
/// Removal the opponent may play: what Solemn Judgment weighs (see `taken_by`).
const FISSURE: u32 = 66788016;
const HAMMER_SHOT: u32 = 26412047;
const ICARUS_ATTACK: u32 = 53567095;
const RAIGEKI_BREAK: u32 = 4178474;
const SOUL_TAKER: u32 = 81510157;
const TRIBUTE_TO_THE_DOOMED: u32 = 79759861;
const OFFERINGS_TO_THE_DOOMED: u32 = 19230407;
const MYSTIC_BOX: u32 = 25774450;
const TWISTER: u32 = 45939841;
const KARMA_CUT: u32 = 71587526;
const NOBLEMAN_OF_CROSSOUT: u32 = 71044499;
const PHOENIX_WING_WIND_BLAST: u32 = 63356631;

/// Spells and Traps whose Special Summon is not worth a Solemn Warning:
/// Scapegoat, Fires of Doomsday, Ojama Trio, One for One, Emergency Teleport.
pub const SMALL_SUMMONS: &[u32] = &[73915051, 46173679, 29843091, 2295440, 67723438];

/// Continuous cards that keep the other player from attacking: a stall
/// deck's win condition, so they are the first thing to remove.
pub const ATTACK_LOCKS: &[u32] = &[
    SWORDS_OF_REVEALING_LIGHT,
    GRAVITY_BIND,
    WALL_OF_REVEALING_LIGHT,
    MORPHTRONIC_BIND,
    SPIDER_WEB,
    MESSENGER_OF_PEACE,
    LEVEL_LIMIT_AREA_B,
    NIGHTMARES_STEELCAGE,
];

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
        SOLEMN_JUDGMENT => 1900,
        SOLEMN_WARNING => 1800,
        MAGIC_CYLINDER => 1700,
        BRAIN_CONTROL => 1600,
        SAKURETSU_ARMOR | COMPULSORY_EVACUATION_DEVICE | SMASHING_GROUND | ENEMY_CONTROLLER => 1500,
        DUST_TORNADO => 1300,
        DRAINING_SHIELD | NEGATE_ATTACK | EFFECT_VEILER | SHRINK | SHIELD_CRUSH | POT_OF_AVARICE => 1200,
        WABOKU => 1100,
        TRAP_DUSTSHOOT => 1000,
        DOUBLE_SUMMON => 900,
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
    // Not into a monster whose controller negates it for free every time.
    let usable = |t: &Turn, code: u32| -> Option<usize> {
        if s.allow_staple(t, code) && !ctx.wasted(code) {
            t.activate(code)
        } else {
            None
        }
    };
    let my_monsters = ctx.monsters(me);
    let opp_monsters = ctx.monsters(opp);
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
    if let Some(i) = clear_backrow(s, t) {
        return Some(i);
    }
    // 1b. A face-up Spell/Trap their deck runs on (Toon World, Necrovalley...).
    if let Some(key) = key_spell_traps(&ctx).first() {
        if let Some(i) = usable(t, MYSTICAL_SPACE_TYPHOON) {
            return t.pick_targeting(i, vec![key.at]);
        }
    }

    // 2. Wipe a board we cannot beat.
    if !opp_monsters.is_empty() {
        let my_strength = ctx.field_strength(me);
        if let Some(i) = usable(t, RAIGEKI) {
            if ctx.swept_strength(RAIGEKI) >= 1500 {
                return t.pick(i);
            }
        }
        if let Some(i) = usable(t, DARK_HOLE) {
            let outclassed = ctx.opp_best_attack() > ctx.my_best_attack();
            if my_strength <= ctx.swept_strength(DARK_HOLE)
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
                    .filter(|c| ctx.reaches(c, BOOK_OF_MOON, true, false))
                    .max_by_key(|c| c.attack);
                if let Some(w) = wall {
                    return t.pick_targeting(i, vec![w.at]);
                }
            }
        }
    }

    // 2b. Synchro removal: always one of their cards, paid with one of ours.
    let their_best = |by: u32, destroys: bool| {
        ctx.monsters(opp)
            .into_iter()
            .chain(ctx.spell_traps(opp))
            .filter(|c| ctx.reaches(c, by, true, destroys))
            .max_by_key(|c| ctx.threat(c))
    };
    // Brionac: discard our cheapest card, bounce their best.
    if let Some(target) = their_best(BRIONAC, false) {
        if let Some(i) = usable(t, BRIONAC) {
            let cheapest = ctx.hand().iter().map(|c| agent::value(s, &ctx, c.code, None)).min();
            if cheapest.map_or(false, |v| v < ctx.threat(target)) {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
    }
    // Scrap Dragon: our cheapest card on the field for their best.
    if let Some(target) = their_best(SCRAP_DRAGON, true) {
        let threat = ctx.threat(target);
        if let Some(i) = usable(t, SCRAP_DRAGON) {
            let cheapest: Option<&CardView> =
                ctx.monsters(me).into_iter().chain(ctx.spell_traps(me)).min_by_key(|c| agent::value(s, &ctx, None, Some(c)));
            if let Some(ours) = cheapest {
                if threat >= 1200 && agent::value(s, &ctx, None, Some(ours)) + 300 < threat {
                    return t.pick_targeting(i, vec![ours.at, target.at]);
                }
            }
        }
    }

    // 2c. One-for-one removal and stalling.
    if let Some(i) = usable(t, SMASHING_GROUND) {
        // It destroys their face-up monster with the highest DEF.
        let highest = ctx.monsters(opp).into_iter().filter(|c| c.position.face_up).max_by_key(|c| c.defense);
        if highest.map_or(false, |c| ctx.reaches(c, SMASHING_GROUND, false, true) && ctx.sweep_worth(c) >= 1500) {
            return t.pick(i);
        }
    }
    if let Some(i) = usable(t, SHIELD_CRUSH) {
        let wall = ctx
            .monsters(opp)
            .into_iter()
            .filter(|c| !c.position.attack && ctx.reaches(c, SHIELD_CRUSH, true, true))
            .max_by_key(|c| ctx.threat(c));
        if let Some(wall) = wall.filter(|c| ctx.threat(c) >= 1200) {
            return t.pick_targeting(i, vec![wall.at]);
        }
    }
    // Brain Control: borrow their best monster for this turn's attacks.
    if ctx.main1() && t.has(ChoiceKind::EnterBattle) && ctx.my_lp() > 2000 && ctx.free_monster_zones(me) > 0 {
        if let Some(i) = usable(t, BRAIN_CONTROL) {
            let best = ctx
                .monsters(opp)
                .into_iter()
                .filter(|c| c.position.face_up && !ctx.view_data(c).is_extra() && ctx.reaches(c, BRAIN_CONTROL, true, false))
                .max_by_key(|c| ctx.threat(c));
            if let Some(best) = best.filter(|c| ctx.threat(c) >= 1800) {
                return t.pick_targeting(i, vec![best.at]);
            }
        }
    }
    if !opp_monsters.is_empty() && ctx.opp_best_attack() > ctx.my_best_attack() {
        if let Some(i) = usable(t, SWORDS_OF_REVEALING_LIGHT) {
            return t.pick(i);
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
    if let Some(i) = usable(t, POT_OF_AVARICE) {
        let monsters = ctx.graveyard(me).iter().filter(|c| ctx.view_data(c).is_monster()).count();
        if monsters >= 5 && ctx.deck_size(me) >= 3 {
            return t.pick(i);
        }
    }
    // Double Summon once the Normal Summon is spent and another monster is ready.
    if let Some(i) = usable(t, DOUBLE_SUMMON) {
        let spent = !t.has(ChoiceKind::NormalSummon) && !t.has(ChoiceKind::SetMonster);
        let ready = ctx.hand().iter().any(|c| {
            let d = ctx.view_data(c);
            d.is_monster() && !d.is_extra() && d.tributes() == 0
        });
        if spent && ready && ctx.free_monster_zones(me) > 0 {
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

/// Staples that clear the opponent's backrow before we commit to attacks.
pub fn clear_backrow<S: Strategy + ?Sized>(s: &S, t: &mut Turn) -> Option<usize> {
    let ctx = t.ctx;
    let (me, opp) = (ctx.me, ctx.opp);
    let usable = |t: &Turn, code: u32| -> Option<usize> {
        if s.allow_staple(t, code) && !ctx.wasted(code) { t.activate(code) } else { None }
    };
    let backrow = ctx.set_backrow(opp);
    let can_battle = t.has(crate::model::ChoiceKind::EnterBattle);
    let hand_monsters = ctx.hand().iter().filter(|c| ctx.view_data(c).is_monster()).count();
    if !ctx.main1() || !can_battle || (ctx.monsters(me).is_empty() && hand_monsters == 0) {
        return None;
    }
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

/// The opponent's face-up Spells/Traps worth a card to remove: the ones
/// their deck runs on ([`crate::knowledge::owner_worth`]) and attack locks,
/// most valuable first.
pub fn key_spell_traps<'a>(ctx: &Ctx<'a>) -> Vec<&'a crate::model::CardView> {
    let mut key: Vec<_> = ctx
        .spell_traps(ctx.opp)
        .into_iter()
        .filter(|c| c.position.face_up && ctx.threat(c) >= 1500)
        .collect();
    key.sort_by_key(|c| -ctx.threat(c));
    key
}

/// The monster the opponent is Summoning, if we can see it.
pub fn opponent_summoning<'a>(ctx: &Ctx<'a>) -> Option<(&'a CardView, u32)> {
    ctx.obs
        .event_cards
        .iter()
        .filter(|(at, _)| at.controller == ctx.opp)
        .find_map(|(at, code)| Some((ctx.card(*at)?, (*code)?)))
}

/// Shrink the monster ours is battling, ATK against ATK, when halving it
/// turns our loss into a win.
fn shrink(ctx: &Ctx) -> Response {
    if ctx.phase().map_or(false, |p| !p.is_battle()) {
        return Response::no();
    }
    let (Some(attacker), Some(target)) = (ctx.battle_attacker(), ctx.battle_target()) else { return Response::no() };
    let (ours, theirs) = if attacker.at.controller == ctx.me { (attacker, target) } else { (target, attacker) };
    if !ours.position.attack || !theirs.position.attack || !theirs.position.face_up {
        return Response::no();
    }
    let halved = theirs.attack - ctx.view_data(theirs).attack / 2;
    if ours.attack <= theirs.attack && ours.attack > halved {
        Response::targeting(65.0, vec![theirs.at])
    } else {
        Response::no()
    }
}

/// Staples whose effect targets the card it answers.
const TARGETING: &[u32] = &[
    BOOK_OF_MOON,
    DIMENSIONAL_PRISON,
    SAKURETSU_ARMOR,
    MAGIC_CYLINDER,
    COMPULSORY_EVACUATION_DEVICE,
    ENEMY_CONTROLLER,
    SHRINK,
    DRAINING_SHIELD,
    EFFECT_VEILER,
];

/// How much we want to chain a staple.  `None` for non-staples.  An answer
/// whose effect would not do its work is no answer: its target cannot be
/// reached (The Fool's coin, White Night Dragon against Spells and Traps),
/// or a monster of theirs negates every card of its kind.
pub fn chain<S: Strategy + ?Sized>(s: &S, t: &Turn, index: usize) -> Option<Response> {
    let response = chain_response(s, t, index)?;
    let ctx = t.ctx;
    let code = ctx.canonical(t.choice(index).code()?);
    let destroys = matches!(code, SAKURETSU_ARMOR | TRAP_HOLE | BOTTOMLESS_TRAP_HOLE);
    let unreachable = |card: &crate::model::CardView, aimed: bool| {
        card.at.controller == ctx.opp && !ctx.reaches(card, code, aimed, destroys)
    };
    let blocked = if TARGETING.contains(&code) {
        response.intent.iter().filter_map(|at| ctx.card(*at)).any(|c| unreachable(c, true))
    } else if matches!(code, TRAP_HOLE | BOTTOMLESS_TRAP_HOLE) {
        opponent_summoning(&ctx).map_or(false, |(card, _)| unreachable(card, false))
    } else {
        false
    };
    Some(if blocked || (response.score > 0.0 && ctx.wasted(code)) { Response::no() } else { response })
}

/// What Torrential Tribute would take from them, less what it takes from us.
fn torrential_gain(ctx: &Ctx) -> i32 {
    ctx.swept_strength(TORRENTIAL_TRIBUTE) - ctx.field_strength(ctx.me)
}

/// On their turn, with their Normal Summon still to be made and cards in
/// hand to make it with, a Torrential Tribute kept one Summon longer takes
/// two monsters for one.  It is kept when what stands now cannot end the
/// duel and has no effect that could take the Trap first.
fn more_to_come(ctx: &Ctx) -> bool {
    !ctx.my_turn()
        && !ctx.obs.summon_used
        && ctx.hand_size(ctx.opp) > 0
        && ctx.opp_attack_potential() < ctx.my_lp()
        && ctx.monsters(ctx.opp).iter().all(|c| !c.position.face_up || ctx.effectless(c))
}

/// On their turn, a Set Trap of ours that answers this Summon once it is
/// made, and for nothing: the Solemn cards are then not worth their price.
/// Only for a monster that brings no effect with it: negating the Summon of
/// any other also stops what it does when it arrives, and when it goes.
fn answered_free<S: Strategy + ?Sized>(s: &S, t: &Turn, card: &CardView) -> bool {
    let ctx = t.ctx;
    let facts = ctx.facts(card);
    if ctx.my_turn() || !ctx.effectless(card) || facts.effect_payoff > 0 || facts.effect_collateral != 0 {
        return false;
    }
    let ready = |code: u32| {
        s.allow_staple(t, code)
            && !ctx.wasted(code)
            && ctx.reaches(card, code, false, true)
            && ctx.spell_traps(ctx.me).iter().any(|c| !c.position.face_up && ctx.is(c, code))
    };
    let bottomless = card.attack >= 1500 && ready(BOTTOMLESS_TRAP_HOLE);
    let torrential = ready(TORRENTIAL_TRIBUTE) && torrential_gain(&ctx) >= 1000 && !more_to_come(&ctx);
    bottomless || torrential
}

/// Spells and Traps that destroy the cards they target.
const DESTROYS_ITS_TARGETS: &[u32] = &[
    SAKURETSU_ARMOR,
    TRAP_HOLE,
    SHIELD_CRUSH,
    MYSTICAL_SPACE_TYPHOON,
    DUST_TORNADO,
    ICARUS_ATTACK,
    RAIGEKI_BREAK,
    SOUL_TAKER,
    TRIBUTE_TO_THE_DOOMED,
    OFFERINGS_TO_THE_DOOMED,
    MYSTIC_BOX,
    TWISTER,
];
/// Spells and Traps that banish the monster they target, or take control of it.
const TAKES_ITS_TARGET: &[u32] = &[DIMENSIONAL_PRISON, BRAIN_CONTROL, KARMA_CUT, NOBLEMAN_OF_CROSSOUT];
/// Spells and Traps that return the monster they target to the hand or the
/// Deck: a card lost only when it goes back to the Extra Deck.
const RETURNS_ITS_TARGET: &[u32] = &[COMPULSORY_EVACUATION_DEVICE, PHOENIX_WING_WIND_BLAST];

/// What one of their Spells or Traps would take from us, in the worth of our
/// cards ([`agent::value`]): what a wipe destroys, less their own cards that
/// go with ours; what a removal targets, or picks by its text.  Nothing for a
/// card not known here.  `spent`: the card we would answer with, gone either
/// way.
fn taken_by<S: Strategy + ?Sized>(s: &S, t: &Turn, link: &ChainLink, spent: Option<CardRef>) -> i32 {
    let ctx = t.ctx;
    let code = ctx.canonical(link.code);
    let kinds = ctx.kind_of(code);
    let worth = |c: &CardView| agent::value(s, &ctx, c.code, Some(c));
    let total = |cards: Vec<&CardView>| -> i32 { cards.into_iter().map(worth).sum() };
    // Our monsters it destroys without targeting them.
    let hit = |pred: &dyn Fn(&CardView) -> bool| -> Vec<&CardView> {
        ctx.monsters(ctx.me).into_iter().filter(|c| pred(c) && ctx.reached_by(c, ctx.opp, kinds, false, true)).collect()
    };
    let face_up = |c: &CardView| c.position.face_up;
    // Our cards it targets, other than the answer itself.
    let aimed = |destroys: bool| -> Vec<&CardView> {
        link.targets
            .iter()
            .filter(|at| at.controller == ctx.me && at.location.is_field() && Some(**at) != spent)
            .filter_map(|at| ctx.card(*at))
            .filter(|c| ctx.reached_by(c, ctx.opp, kinds, true, destroys))
            .collect()
    };
    match code {
        DARK_HOLE | TORRENTIAL_TRIBUTE => total(hit(&|_| true)) - ctx.field_strength(ctx.opp),
        RAIGEKI => total(hit(&|_| true)),
        LIGHTNING_VORTEX => total(hit(&face_up)),
        MIRROR_FORCE => total(hit(&|c| c.position.attack)),
        HEAVY_STORM => {
            let ours = total(ctx.spell_traps(ctx.me).into_iter().filter(|c| Some(c.at) != spent).collect());
            let theirs: i32 = ctx.spell_traps(ctx.opp).into_iter().filter(|c| c.at != link.source).map(|c| ctx.threat(c)).sum();
            ours - theirs
        }
        // Their pick is the text's: the highest DEF, the lowest ATK, the
        // highest ATK in Attack Position.
        SMASHING_GROUND => hit(&face_up).into_iter().max_by_key(|c| c.defense).map_or(0, worth),
        FISSURE => hit(&face_up).into_iter().min_by_key(|c| c.attack).map_or(0, worth),
        HAMMER_SHOT => hit(&|c| c.position.face_up && c.position.attack).into_iter().max_by_key(|c| c.attack).map_or(0, worth),
        // The monsters we are Summoning.
        BOTTOMLESS_TRAP_HOLE => {
            let summoned = ctx.obs.event_cards.iter().filter(|(at, _)| at.controller == ctx.me).filter_map(|(at, _)| ctx.card(*at));
            total(summoned.filter(|c| c.attack >= 1500 && ctx.reached_by(c, ctx.opp, kinds, false, true)).collect())
        }
        _ if DESTROYS_ITS_TARGETS.contains(&code) => total(aimed(true)),
        _ if TAKES_ITS_TARGET.contains(&code) => total(aimed(false)),
        _ if RETURNS_ITS_TARGET.contains(&code) => total(aimed(false).into_iter().filter(|c| ctx.view_data(c).is_extra()).collect()),
        _ => 0,
    }
}

/// Would this Spell or Trap of theirs destroy our card at `at`?  Of the
/// removal [`taken_by`] knows.
pub fn destroys(ctx: &Ctx, link: &ChainLink, at: CardRef) -> bool {
    let code = ctx.canonical(link.code);
    match code {
        HEAVY_STORM => at.location == Location::SpellTrapZone,
        _ => DESTROYS_ITS_TARGETS.contains(&code) && link.targets.contains(&at),
    }
}

/// Solemn Judgment costs half our Life Points, whatever they are: it can
/// always be paid, and the fewer we have the less it costs.  That price is
/// weighed against what the answer saves.
fn judgment<S: Strategy + ?Sized>(s: &S, t: &Turn, spent: Option<CardRef>) -> Response {
    let ctx = t.ctx;
    let price = ctx.my_lp() / 2;
    let worth = match t.hostile_top() {
        // A Spell or Trap: what it would take from us is worth the price,
        // and more than the Trap we give for it.
        Hostile::Link(link) => {
            let stake = taken_by(s, t, link, spent);
            stake >= price && stake > agent::value(s, &ctx, Some(SOLEMN_JUDGMENT), None)
        }
        // A Summon: the big monsters at any price, and any monster whose one
        // attack would cost as much as the Trap does.
        Hostile::No => opponent_summoning(&ctx).map_or(false, |(card, code)| {
            let data = ctx.data(code);
            let attack = card.attack.max(data.attack);
            (attack >= 2400 || data.is_extra() || attack >= price) && !answered_free(s, t, card)
        }),
        Hostile::Unseen => false,
    };
    // Next to a Solemn Warning that answers too (80), the cheaper goes first.
    let score = if price > SOLEMN_WARNING_PRICE { 79.0 } else { 85.0 };
    if worth { Response::new(score) } else { Response::no() }
}

fn chain_response<S: Strategy + ?Sized>(s: &S, t: &Turn, index: usize) -> Option<Response> {
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
            let gain = torrential_gain(&ctx);
            if gain >= 1000 && !more_to_come(&ctx) {
                // Capped below Trap Hole so it never stacks on our own answer.
                Response::new((gain as f64 / 10.0).min(95.0))
            } else {
                Response::no()
            }
        }
        MIRROR_FORCE => match incoming {
            Some(_) => {
                let hit = ctx.monsters(opp).iter().filter(|c| c.position.attack && ctx.reaches(c, MIRROR_FORCE, false, true)).count();
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
            let key = key_spell_traps(&ctx);
            let backrow = ctx.set_backrow(opp);
            let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(crate::model::Phase::End);
            let before_our_attack = ctx.my_turn() && ctx.main1();
            if !key.is_empty() && (end_of_their_turn || before_our_attack) {
                Response::targeting(25.0, refs(&key))
            } else if !backrow.is_empty() && (end_of_their_turn || before_our_attack) {
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
        // Answers to a declared attack, weakest first: the stronger ones
        // are kept for when they are the only way out.
        WABOKU | NEGATE_ATTACK | DRAINING_SHIELD => match incoming {
            Some((attacker, target)) if ctx.attack_hurts(attacker, target) => {
                let others = ctx
                    .monsters(opp)
                    .iter()
                    .filter(|c| c.position.face_up && c.position.attack && c.at != attacker.at)
                    .count() as f64;
                let score = match code {
                    WABOKU => 45.0 + 10.0 * others,
                    NEGATE_ATTACK => 50.0 + 10.0 * others,
                    _ => 55.0,
                };
                Response::targeting(score, vec![attacker.at])
            }
            _ => Response::no(),
        },
        SAKURETSU_ARMOR => match incoming {
            Some((attacker, target)) if ctx.attack_hurts(attacker, target) || ctx.threat(attacker) >= 1500 => {
                Response::targeting(75.0, vec![attacker.at])
            }
            _ => Response::no(),
        },
        MAGIC_CYLINDER => match incoming {
            Some((attacker, _)) if attacker.attack >= ctx.opp_lp() => Response::targeting(99.0, vec![attacker.at]),
            Some((attacker, target)) if attacker.attack >= 1500 || ctx.attack_hurts(attacker, target) => {
                Response::targeting(88.0, vec![attacker.at])
            }
            _ => Response::no(),
        },
        COMPULSORY_EVACUATION_DEVICE => match incoming {
            Some((attacker, target)) if ctx.attack_hurts(attacker, target) => {
                // Back to the Extra Deck is as good as destroyed.
                let score = if ctx.view_data(attacker).is_extra() { 80.0 } else { 70.0 };
                Response::targeting(score, vec![attacker.at])
            }
            _ => Response::no(),
        },
        // Summons are negated with an empty chain; a stale Summon must not
        // make us negate an unrelated activation.  2000 Life Points, paid
        // whenever they leave us any.
        SOLEMN_WARNING => match (t.hostile_top(), opponent_summoning(&ctx)) {
            (Hostile::No, Some((card, code))) if ctx.my_lp() > SOLEMN_WARNING_PRICE => {
                let data = ctx.data(code);
                let worth = card.attack.max(data.attack) >= 1900 || data.is_extra();
                if worth && !answered_free(s, t, card) { Response::new(80.0) } else { Response::no() }
            }
            // Against an activation it is only offered when that activation
            // Special Summons.  A Spell or Trap that does brings a Fusion or
            // the best monster of a Graveyard; tokens and Tuners are not
            // worth the Life Points.
            (Hostile::Link(link), _) if ctx.my_lp() > SOLEMN_WARNING_PRICE => {
                let code = ctx.canonical(link.code);
                if ctx.data(code).is_monster() || SMALL_SUMMONS.contains(&code) { Response::no() } else { Response::new(78.0) }
            }
            _ => Response::no(),
        },
        SOLEMN_JUDGMENT => judgment(s, t, choice.at()),
        DUST_TORNADO => {
            let locks = ctx.attack_locks();
            if !locks.is_empty() {
                return Some(Response::targeting(60.0, refs(&locks)));
            }
            let key = key_spell_traps(&ctx);
            let backrow = ctx.set_backrow(opp);
            let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
            let before_our_attack = ctx.my_turn() && ctx.main1();
            if !key.is_empty() && (end_of_their_turn || before_our_attack) {
                Response::targeting(25.0, refs(&key))
            } else if !backrow.is_empty() && (end_of_their_turn || before_our_attack) {
                Response::targeting(20.0, refs(&backrow))
            } else {
                Response::no()
            }
        }
        // Only offered while their hand holds 4+ cards.
        TRAP_DUSTSHOOT if ctx.phase().map_or(true, |p| !p.is_damage_step()) => Response::new(30.0),
        TRAP_DUSTSHOOT => Response::no(),
        EFFECT_VEILER => match t.hostile_top() {
            Hostile::Link(link)
                if link.source.controller == opp
                    && link.source.location == Location::MonsterZone
                    && ctx.card(link.source).map_or(false, |c| c.position.face_up && c.code == Some(link.code)) =>
            {
                Response::targeting(70.0, vec![link.source])
            }
            _ => Response::no(),
        },
        SHRINK => shrink(&ctx),
        // Only offered when it is free: our field is empty and we took damage.
        GORZ => Response::new(70.0),
        ENEMY_CONTROLLER => match incoming {
            Some((attacker, target)) if attacker.position.face_up && ctx.attack_hurts(attacker, target) => {
                Response::targeting(60.0, vec![attacker.at])
            }
            _ => Response::no(),
        },
        DARK_HOLE | RAIGEKI | HEAVY_STORM | COLD_WAVE | GIANT_TRUNADE | MONSTER_REBORN
        | POT_OF_DUALITY | ALLURE_OF_DARKNESS | PREMATURE_BURIAL | BRIONAC | SCRAP_DRAGON
        | SMASHING_GROUND | SHIELD_CRUSH | BRAIN_CONTROL | POT_OF_AVARICE | DOUBLE_SUMMON => Response::no(),
        _ => return None,
    })
}
