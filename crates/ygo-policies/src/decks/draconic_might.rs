//! "Draconic Might": Dragon beatdown around Red-Eyes Darkness Metal Dragon
//! and the LV monsters.
//!
//! Red-Eyes Darkness Metal Dragon comes out whenever it can: a small Dragon is
//! Normal Summoned face-up (or flipped) and banished for it, even a dear one,
//! and every turn its effect Special Summons another Dragon from the hand or
//! the Graveyard, so a discarded Dragon is not lost.  The rest of the deck
//! protects it: Prime Material Dragon (its first choice) negates destruction,
//! Jinzo their Traps, Horus LV8 their Spells (Horus LV4 opens when Level Up!
//! can follow), and their Set cards go before it arrives (Stamping
//! Destruction, Mystical Space Typhoon, Giant Trunade even for one card).
//!
//! The LV lines climb on their own (Armed Dragon LV3 in the Standby Phase, LV5
//! and the Horus after destroying a monster in battle, so they get the kills)
//! or through Level Up!, never down, taking a copy stuck in the hand before
//! the Deck's.  Armed Dragon LV5 and LV7 send a monster from the hand to
//! destroy one or all of the opponent's monsters with less ATK when that does
//! more for the turn's attacks than the card is worth.  Masked Dragon and
//! Twin-Headed Behemoth float into the next body, Call of the Haunted revives
//! an LV3 at the end of their turn for a free level-up, and Foolish Burial puts
//! Prime Material or White Night Dragon where REDMD, Monster Reborn and Call
//! reach it.

use crate::agent::{value, Hostile, Outcome, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{CardRef, CardView, Choice, ChoiceKind, Hint, Location, Member, Phase};
use crate::staples::{
    self, CALL_OF_THE_HAUNTED, DARK_HOLE, GIANT_TRUNADE, MONSTER_REBORN, MYSTICAL_SPACE_TYPHOON,
};
use crate::tactics::default_outcome;

pub const DECK: &str = "Draconic Might";

const ARMED_DRAGON_LV3: u32 = 980973;
const ARMED_DRAGON_LV5: u32 = 46384672;
const ARMED_DRAGON_LV7: u32 = 73879377;
const HORUS_LV4: u32 = 75830094;
const HORUS_LV6: u32 = 11224103;
const HORUS_LV8: u32 = 48229808;
const RED_EYES_DARKNESS_METAL: u32 = 88264978;
const WHITE_NIGHT_DRAGON: u32 = 79473793;
const PRIME_MATERIAL_DRAGON: u32 = 12298909;
const CHTHONIAN_EMPEROR: u32 = 95888876;
const JINZO: u32 = 77585513;
const MASKED_DRAGON: u32 = 39191307;
const TWIN_HEADED_BEHEMOTH: u32 = 43586926;
const VANGUARD: u32 = 77135531;
const STARDUST_DRAGON: u32 = 44508094;
const LEVEL_UP: u32 = 25290459;
const FOOLISH_BURIAL: u32 = 81439173;
const STAMPING_DESTRUCTION: u32 = 81385346;
const FOURTH_DIMENSION: u32 = 88089103;
/// Their face-up cards that shut our engine (Necrovalley: every Graveyard
/// effect; Royal Oppression: every Special Summon; Skill Drain: every
/// monster effect) or run theirs.
const NECROVALLEY: u32 = 47355498;
const ROYAL_OPPRESSION: u32 = 93016201;
const SKILL_DRAIN: u32 = 82732705;
const GATEWAY_OF_THE_SIX: u32 = 27970830;
const FUSION_GATE: u32 = 33550694;

/// White Night Dragon's second effect: an attack on another monster of ours
/// goes to it instead (the first negates Spells/Traps and is forced).
const REDIRECT: u64 = 1;

/// Each LV monster and the one it levels up into.
const LEVEL_UPS: [(u32, u32); 4] =
    [(ARMED_DRAGON_LV3, ARMED_DRAGON_LV5), (ARMED_DRAGON_LV5, ARMED_DRAGON_LV7), (HORUS_LV4, HORUS_LV6), (HORUS_LV6, HORUS_LV8)];

#[derive(Default)]
pub struct DraconicMight {
    /// Turn of our last Red-Eyes Darkness Metal Dragon Special Summon from
    /// the hand, and of its last effect: both once per turn.
    redmd_summoned: u32,
    redmd_effect: u32,
}

impl DraconicMight {
    /// Copies of each LV monster in the deck list.
    fn copies(code: u32) -> usize {
        match code {
            ARMED_DRAGON_LV3 => 3,
            ARMED_DRAGON_LV5 | ARMED_DRAGON_LV7 | HORUS_LV4 => 2,
            HORUS_LV6 | HORUS_LV8 => 1,
            _ => 0,
        }
    }

    /// Is a copy of `code` still in the hand or the Deck (for a level-up)?
    fn available(ctx: &Ctx, code: u32) -> bool {
        let seen: usize = [Location::MonsterZone, Location::Graveyard, Location::Banished]
            .iter()
            .map(|l| ctx.count_in(ctx.me, *l, code))
            .sum();
        seen < Self::copies(code)
    }

    /// The monster `code` levels up into, if one is left to summon.
    fn upgrade(ctx: &Ctx, code: u32) -> Option<u32> {
        LEVEL_UPS.iter().find(|(from, _)| *from == code).map(|(_, to)| *to).filter(|to| Self::available(ctx, *to))
    }

    fn code(ctx: &Ctx, card: &CardView) -> u32 {
        card.code.map(|c| ctx.canonical(c)).unwrap_or(0)
    }

    fn is_dragon(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.race & races::DRAGON != 0
    }

    fn turn(ctx: &Ctx) -> u32 {
        ctx.obs.turn
    }

    /// Necrovalley negates every effect that would move a card out of a
    /// Graveyard: REDMD's revival, Monster Reborn, Call of the Haunted.
    fn valley(ctx: &Ctx) -> bool {
        ctx.face_up_on_field(ctx.opp, NECROVALLEY) || ctx.face_up_on_field(ctx.me, NECROVALLEY)
    }

    /// How much we want one of their Spells/Traps gone.
    fn backrow_threat(ctx: &Ctx, card: &CardView) -> i32 {
        if !card.position.face_up {
            return ctx.threat(card);
        }
        match Self::code(ctx, card) {
            ROYAL_OPPRESSION => 2800,
            NECROVALLEY => 2600,
            SKILL_DRAIN => 2500,
            FUSION_GATE => 2400,
            GATEWAY_OF_THE_SIX => 2200,
            _ => ctx.threat(card),
        }
    }

    fn face_up(ctx: &Ctx, code: u32) -> bool {
        ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.is(c, code))
    }

    /// Our face-up Dragons: REDMD's cost, Stamping Destruction's condition.
    fn dragons_up<'a>(ctx: &Ctx<'a>) -> Vec<&'a CardView> {
        ctx.monsters(ctx.me).into_iter().filter(|c| c.position.face_up && Self::is_dragon(ctx, c)).collect()
    }

    /// What banishing this face-up Dragon for REDMD gives up.
    fn fodder_cost(&self, ctx: &Ctx, card: &CardView) -> i32 {
        match Self::code(ctx, card) {
            // Revived (1000 ATK), it has nothing left to give.
            TWIN_HEADED_BEHEMOTH if card.attack <= 1000 => 100,
            TWIN_HEADED_BEHEMOTH => 300,
            MASKED_DRAGON => 400,
            VANGUARD => 600,
            HORUS_LV4 if Self::upgrade(ctx, HORUS_LV4).is_some() => 900,
            ARMED_DRAGON_LV3 if Self::upgrade(ctx, ARMED_DRAGON_LV3).is_some() => 1000,
            HORUS_LV4 | ARMED_DRAGON_LV3 => 500,
            // One REDMD for another gains nothing: its effect is once per turn.
            RED_EYES_DARKNESS_METAL => 100_000,
            _ => value(self, ctx, None, Some(card)),
        }
    }

    /// The Dragon REDMD would banish: the cheapest face-up one.
    fn cheapest_fodder(&self, ctx: &Ctx) -> Option<i32> {
        Self::dragons_up(ctx).iter().map(|c| self.fodder_cost(ctx, c)).min()
    }

    /// Worth of Special Summoning `code` from `from` with REDMD's effect.
    fn summon_worth(&self, t: &Turn, code: u32, from: Location) -> i32 {
        let ctx = t.ctx;
        let fresh_ns = t.has(ChoiceKind::NormalSummon);
        let base = match code {
            // Its negation keeps REDMD and the rest of the board.
            PRIME_MATERIAL_DRAGON => 3300,
            WHITE_NIGHT_DRAGON => 3100,
            // A second Normal Summon, and it attacks twice.
            CHTHONIAN_EMPEROR if fresh_ns && ctx.main1() => 3000,
            CHTHONIAN_EMPEROR => 2400,
            // Horus LV8 negates their Spells.
            HORUS_LV6 if Self::upgrade(&ctx, code).is_some() => 2800,
            ARMED_DRAGON_LV5 | HORUS_LV6 => ctx.data(code).attack + if Self::upgrade(&ctx, code).is_some() { 300 } else { 0 },
            ARMED_DRAGON_LV3 if Self::upgrade(&ctx, code).is_some() => 1900,
            _ => ctx.data(code).attack,
        };
        // A card back from the Graveyard is a card more; a Level 4 or lower
        // one in the hand could have been Normal Summoned anyway.
        match from {
            Location::Graveyard => base + 200,
            _ if ctx.data(code).level <= 4 => base - 300,
            _ => base,
        }
    }

    /// Dragons REDMD's effect can Special Summon (the LV7 and LV8 only come
    /// out through their LV5 / LV6).
    fn revivable(ctx: &Ctx, card: &CardView) -> bool {
        let code = Self::code(ctx, card);
        Self::is_dragon(ctx, card)
            && !ctx.view_data(card).is_extra()
            && !matches!(code, RED_EYES_DARKNESS_METAL | ARMED_DRAGON_LV7 | HORUS_LV8)
    }

    /// The best target of REDMD's effect in the hand or Graveyard.
    fn best_redmd_target(&self, t: &Turn) -> Option<i32> {
        let ctx = t.ctx;
        let grave = if Self::valley(&ctx) { Vec::new() } else { ctx.graveyard(ctx.me) };
        ctx.hand()
            .into_iter()
            .chain(grave)
            .filter(|c| Self::revivable(&ctx, c))
            .map(|c| self.summon_worth(t, Self::code(&ctx, c), c.at.location))
            .max()
    }

    /// Is a REDMD waiting in the hand for a face-up Dragon this turn?
    fn redmd_waiting(&self, ctx: &Ctx) -> bool {
        ctx.in_hand(RED_EYES_DARKNESS_METAL) && self.redmd_summoned != Self::turn(ctx)
    }

    /// Would REDMD's effect still be usable this turn once it is on the field?
    fn redmd_effect_left(&self, ctx: &Ctx) -> bool {
        self.redmd_effect != Self::turn(ctx) && ctx.free_monster_zones(ctx.me) > 0
    }

    /// What sending this card from the hand to the Graveyard costs (discards,
    /// Armed Dragon's and Vanguard's costs): Dragons REDMD brings back cost
    /// little.
    fn discard_cost(&self, ctx: &Ctx, card: &CardView) -> i32 {
        let code = Self::code(ctx, card);
        let redmd = ctx.in_hand(RED_EYES_DARKNESS_METAL)
            || Self::face_up(ctx, RED_EYES_DARKNESS_METAL);
        match code {
            // Out of reach except through their LV5 / LV6.
            ARMED_DRAGON_LV7 => 500,
            HORUS_LV8 if !Self::face_up(ctx, HORUS_LV6) => 700,
            // REDMD brings it back sooner or later.
            _ if redmd && Self::revivable(ctx, card) && code != ARMED_DRAGON_LV3 => {
                value(self, ctx, card.code, None) / 4
            }
            _ => value(self, ctx, card.code, None),
        }
    }

    /// Opponent monsters an Armed Dragon effect paid with `attack` ATK
    /// would destroy (face-up ones only).
    fn in_reach<'a>(ctx: &Ctx<'a>, attack: i32) -> Vec<&'a CardView> {
        ctx.monsters(ctx.opp).into_iter().filter(|c| c.position.face_up && c.attack <= attack).collect()
    }

    /// Rough worth of this turn's attacks against `targets`: each one the
    /// weakest attacker that wins can destroy counts its threat (one attack
    /// each); once none is left, the free attackers hit directly.
    fn attack_value(ctx: &Ctx, targets: &[&CardView]) -> i32 {
        let mut attackers: Vec<&CardView> =
            ctx.monsters(ctx.me).into_iter().filter(|c| ctx.can_attack(c) && c.attack > 0).collect();
        attackers.sort_by_key(|c| c.attack);
        let mut targets = targets.to_vec();
        targets.sort_by_key(|c| -ctx.threat(c));
        let mut worth = 0;
        let mut standing = 0;
        for target in targets {
            let winner = attackers
                .iter()
                .position(|a| matches!(default_outcome(ctx, a, target, 0), Outcome::Win { trick: false }));
            match winner {
                Some(k) => {
                    attackers.remove(k);
                    worth += ctx.threat(target);
                }
                None => standing += 1,
            }
        }
        if standing == 0 {
            worth += attackers.iter().map(|a| a.attack).sum::<i32>();
        }
        worth
    }

    /// What destroying `hit` adds to this turn: their threat, and what our
    /// attackers then do to the rest.
    fn removal_gain(ctx: &Ctx, hit: &[&CardView]) -> i32 {
        let theirs = ctx.monsters(ctx.opp);
        let rest: Vec<&CardView> = theirs.iter().copied().filter(|c| !hit.iter().any(|h| h.at == c.at)).collect();
        hit.iter().map(|c| ctx.threat(c)).sum::<i32>() + Self::attack_value(ctx, &rest) - Self::attack_value(ctx, &theirs)
    }

    /// The hand monster to send for Armed Dragon LV5 (`sweep` false) or LV7,
    /// and the opponent monster(s) it destroys: the best gain for the cost.
    fn dragon_cost<'a>(&self, ctx: &Ctx<'a>, sweep: bool) -> Option<(&'a CardView, Vec<&'a CardView>)> {
        let mut best: Option<(i32, &CardView, Vec<&CardView>)> = None;
        for card in ctx.hand() {
            let data = ctx.view_data(card);
            if !data.is_monster() {
                continue;
            }
            let reach = Self::in_reach(ctx, data.attack);
            let (gain, hit) = if sweep {
                (Self::removal_gain(ctx, &reach), reach)
            } else {
                match reach.iter().map(|c| (Self::removal_gain(ctx, &[*c]), *c)).max_by_key(|(g, _)| *g) {
                    Some((g, c)) => (g, vec![c]),
                    None => continue,
                }
            };
            let net = gain - self.discard_cost(ctx, card);
            if gain >= 1200 && net > 0 && best.as_ref().map_or(true, |b| net > b.0) {
                best = Some((net, card, hit));
            }
        }
        best.map(|(_, card, hit)| (card, hit))
    }

    /// Level Up! pays with the face-up LV monster whose upgrade gains most.
    fn level_up_pick<'a>(ctx: &Ctx<'a>) -> Option<&'a CardView> {
        ctx.monsters(ctx.me)
            .into_iter()
            .filter(|c| c.position.face_up)
            .filter_map(|c| {
                let code = Self::code(ctx, c);
                Self::upgrade(ctx, code)?;
                let gain = match code {
                    HORUS_LV6 => 1500,
                    HORUS_LV4 => 1300,
                    ARMED_DRAGON_LV5 => 1200,
                    // It levels up by itself next Standby Phase, if it
                    // survives: 1200 ATK rarely does.
                    ARMED_DRAGON_LV3 => 600,
                    _ => return None,
                };
                Some((gain, c))
            })
            .max_by_key(|(gain, _)| *gain)
            .map(|(_, c)| c)
    }

    /// Is this Spell/Trap of ours worth keeping (White Night Dragon's cost)?
    fn backrow_cost(&self, ctx: &Ctx, card: &CardView) -> i32 {
        let worth = value(self, ctx, card.code, None);
        if ctx.own_attack_locks().iter().any(|c| c.at == card.at) {
            return worth + 2000;
        }
        worth
    }

    /// Backrow removal before the attack: Stamping Destruction (a face-up
    /// Dragon of ours), then the staples.
    fn clear_backrow(&self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Their face-up locks and engines go whenever we can.
        let lock = ctx
            .spell_traps(ctx.opp)
            .into_iter()
            .filter(|c| c.position.face_up)
            .max_by_key(|c| Self::backrow_threat(&ctx, c))
            .filter(|c| Self::backrow_threat(&ctx, c) >= 2200);
        if let Some(lock) = lock {
            if !Self::dragons_up(&ctx).is_empty() {
                if let Some(i) = t.activate(STAMPING_DESTRUCTION) {
                    return t.pick_targeting(i, vec![lock.at]);
                }
            }
            if let Some(i) = t.activate(MYSTICAL_SPACE_TYPHOON) {
                return t.pick_targeting(i, vec![lock.at]);
            }
        }
        if !ctx.main1() || !t.has(ChoiceKind::EnterBattle) {
            return None;
        }
        let theirs = ctx.spell_traps(ctx.opp);
        if theirs.is_empty() {
            return None;
        }
        if !Self::dragons_up(&ctx).is_empty() {
            let target = theirs.iter().max_by_key(|c| Self::backrow_threat(&ctx, c)).map(|c| c.at);
            if let Some(i) = t.activate(STAMPING_DESTRUCTION) {
                return t.pick_targeting(i, target.into_iter().collect());
            }
        }
        staples::clear_backrow(self, t)
    }

    /// Before REDMD comes out, their Set cards go (Solemn Warning, Bottomless
    /// Trap Hole, Torrential Tribute wait for it): Stamping Destruction with
    /// a face-up Dragon, Mystical Space Typhoon, or Giant Trunade even for a
    /// single card.
    fn clear_for_redmd(&self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let set = ctx.set_backrow(ctx.opp);
        if set.is_empty() || !self.redmd_waiting(&ctx) {
            return None;
        }
        let fodder_in_hand = ctx.hand().iter().any(|c| {
            matches!(Self::code(&ctx, c), TWIN_HEADED_BEHEMOTH | MASKED_DRAGON | VANGUARD | HORUS_LV4 | ARMED_DRAGON_LV3)
        });
        let redmd_ready = t.find(ChoiceKind::SpecialSummon, Some(RED_EYES_DARKNESS_METAL), Some(Location::Hand)).is_some()
            || (t.has(ChoiceKind::NormalSummon) && fodder_in_hand);
        if !redmd_ready {
            return None;
        }
        let refs: Vec<CardRef> = set.iter().map(|c| c.at).collect();
        // Giant Trunade also returns our own Spells/Traps.
        let trunade = ctx.own_attack_locks().is_empty() && ctx.set_backrow(ctx.me).len() <= 1;
        if set.len() >= 2 && trunade {
            if let Some(i) = t.activate(GIANT_TRUNADE) {
                return t.pick(i);
            }
        }
        if !Self::dragons_up(&ctx).is_empty() {
            if let Some(i) = t.activate(STAMPING_DESTRUCTION) {
                return t.pick_targeting(i, refs);
            }
        }
        if let Some(i) = t.activate(MYSTICAL_SPACE_TYPHOON) {
            return t.pick_targeting(i, refs);
        }
        if trunade {
            if let Some(i) = t.activate(GIANT_TRUNADE) {
                return t.pick(i);
            }
        }
        None
    }

    /// Dark Hole before we summon: when their board outclasses ours.
    /// Horus LV6 is unaffected by it.
    fn dark_hole(&self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let theirs = ctx.monsters(ctx.opp);
        if theirs.is_empty() {
            return None;
        }
        let i = t.activate(DARK_HOLE)?;
        let ours: Vec<&CardView> = ctx.monsters(ctx.me).into_iter().filter(|c| !ctx.is(c, HORUS_LV6)).collect();
        let their_strength = ctx.field_strength(ctx.opp);
        let our_strength: i32 = ours.iter().map(|c| value(self, &ctx, None, Some(c))).sum();
        let our_best = ctx.monsters(ctx.me).iter().filter(|c| c.position.face_up).map(|c| c.attack).max().unwrap_or(0);
        let outclassed = ctx.opp_best_attack() > our_best;
        if our_strength + 500 <= their_strength && (theirs.len() >= 2 || outclassed || ours.is_empty()) {
            return t.pick(i);
        }
        None
    }

    /// Monster Reborn: the strongest monster it can bring back (Armed Dragon
    /// LV7 and Horus LV8 cannot be revived), or the body a play needs: a
    /// face-up Dragon for the REDMD in our hand, a Tribute for a big monster.
    fn monster_reborn(&self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let i = t.activate(MONSTER_REBORN)?;
        if ctx.free_monster_zones(ctx.me) == 0 || Self::valley(&ctx) {
            return None;
        }
        let candidates: Vec<&CardView> = ctx
            .graveyard(ctx.me)
            .into_iter()
            .chain(ctx.graveyard(ctx.opp))
            .filter(|c| {
                ctx.view_data(c).is_monster() && !matches!(Self::code(&ctx, c), ARMED_DRAGON_LV7 | HORUS_LV8 | STARDUST_DRAGON)
            })
            .collect();
        let best = candidates.iter().copied().max_by_key(|c| ctx.view_data(c).attack)?;
        let attack = ctx.view_data(best).attack;
        if attack >= 2000 {
            return t.pick_targeting(i, vec![best.at]);
        }
        if self.redmd_waiting(&ctx) && Self::dragons_up(&ctx).is_empty() {
            let fodder = candidates.iter().filter(|c| Self::is_dragon(&ctx, c)).max_by_key(|c| ctx.view_data(c).attack);
            if let Some(fodder) = fodder {
                return t.pick_targeting(i, vec![fodder.at]);
            }
        }
        let tribute_for = ctx.hand().iter().any(|c| {
            let d = ctx.view_data(c);
            d.is_monster() && d.tributes() == 1 && value(self, &ctx, c.code, None) >= 2300
        });
        let empty = ctx.monsters(ctx.me).is_empty();
        if empty && (attack >= 1200 || (tribute_for && t.has(ChoiceKind::NormalSummon))) {
            return t.pick_targeting(i, vec![best.at]);
        }
        None
    }

    /// A face-down Dragon of ours flipped up for the REDMD in our hand.
    fn flip_for_redmd(&self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !self.redmd_waiting(&ctx) || !Self::dragons_up(&ctx).is_empty() {
            return None;
        }
        let i = t.find_where(|c| {
            c.kind == ChoiceKind::ChangePosition
                && c.at().and_then(|at| ctx.card(at)).map_or(false, |v| !v.position.face_up && Self::is_dragon(&ctx, v))
        })?;
        t.pick(i)
    }

    /// Vanguard of the Dragon: send spare Dragons for 300 ATK each when that
    /// wins a battle it would otherwise lose.
    fn vanguard_pump(&self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !ctx.main1() || !t.has(ChoiceKind::EnterBattle) {
            return None;
        }
        let i = t.activate_from(VANGUARD, Location::MonsterZone)?;
        let vanguard = t.view(t.choice(i))?;
        if !ctx.can_attack(vanguard) {
            return None;
        }
        let spare = ctx
            .hand()
            .into_iter()
            .filter(|c| Self::is_dragon(&ctx, c))
            .map(|c| self.discard_cost(&ctx, c))
            .min()?;
        let target = ctx
            .monsters(ctx.opp)
            .into_iter()
            .filter(|c| c.position.face_up)
            .filter(|c| {
                !ctx.monsters(ctx.me).iter().any(|a| {
                    a.at != vanguard.at && ctx.can_attack(a) && matches!(default_outcome(&ctx, a, c, 0), Outcome::Win { trick: false })
                })
            })
            .filter(|c| {
                let stat = ctx.battle_stat(c);
                stat >= vanguard.attack && stat < vanguard.attack + 300
            })
            .max_by_key(|c| ctx.threat(c))?;
        (ctx.threat(target) > spare).then(|| t.pick(i)).flatten()
    }

    /// Worth of a monster in our Graveyard, for Foolish Burial: what REDMD's
    /// effect, Monster Reborn and Call of the Haunted bring back.  Armed
    /// Dragon LV7 and Horus LV8 stay in the Deck for their LV5 / LV6.
    fn burial_worth(&self, ctx: &Ctx, code: u32) -> i32 {
        let reborn = ctx.in_hand(MONSTER_REBORN) || Self::call_ready(ctx);
        match code {
            PRIME_MATERIAL_DRAGON => 3100,
            WHITE_NIGHT_DRAGON => 3000,
            // REDMD's effect cannot bring back another REDMD.
            RED_EYES_DARKNESS_METAL | JINZO if reborn => 2800,
            CHTHONIAN_EMPEROR => 2400,
            ARMED_DRAGON_LV5 | HORUS_LV6 => 2200,
            ARMED_DRAGON_LV7 | HORUS_LV8 => -1000,
            _ => ctx.data(code).attack / 2,
        }
    }

    /// Worth of reviving this monster with Call of the Haunted.  At the end
    /// of their turn an Armed Dragon LV3 is an LV5 in our Standby Phase.
    fn call_worth(ctx: &Ctx, card: &CardView, end_of_their_turn: bool) -> i32 {
        let code = Self::code(ctx, card);
        match code {
            ARMED_DRAGON_LV7 | HORUS_LV8 => -1,
            ARMED_DRAGON_LV3 if end_of_their_turn && Self::upgrade(ctx, code).is_some() => 2450,
            _ => ctx.view_data(card).attack,
        }
    }

    fn call_ready(ctx: &Ctx) -> bool {
        ctx.in_hand(CALL_OF_THE_HAUNTED)
            || ctx.spell_traps(ctx.me).iter().any(|c| !c.position.face_up && ctx.is(c, CALL_OF_THE_HAUNTED))
    }

    /// Foolish Burial once something can bring the monster back: REDMD (on
    /// the field with its effect unused, or in the hand), Monster Reborn,
    /// Call of the Haunted.
    fn foolish_burial(&self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if Self::valley(&ctx) {
            return None;
        }
        let i = t.activate(FOOLISH_BURIAL)?;
        let redmd = (Self::face_up(&ctx, RED_EYES_DARKNESS_METAL) && self.redmd_effect_left(&ctx))
            || ctx.in_hand(RED_EYES_DARKNESS_METAL);
        (redmd || ctx.in_hand(MONSTER_REBORN) || Self::call_ready(&ctx)).then(|| t.pick(i)).flatten()
    }

    /// The Graveyard in the Fourth Dimension: back to the Deck the LV
    /// monster a face-up one of ours needs to level up.
    fn fourth_dimension(&self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let i = t.activate(FOURTH_DIMENSION)?;
        let lv_in_grave: Vec<&CardView> =
            ctx.graveyard(ctx.me).into_iter().filter(|c| Self::copies(Self::code(&ctx, c)) > 0).collect();
        if lv_in_grave.len() < 2 {
            return None;
        }
        let needed: Vec<u32> = ctx
            .monsters(ctx.me)
            .iter()
            .filter(|c| c.position.face_up)
            .filter_map(|c| LEVEL_UPS.iter().find(|(from, _)| ctx.is(c, *from)).map(|(_, to)| *to))
            .filter(|to| !Self::available(&ctx, *to))
            .collect();
        let first = lv_in_grave.iter().find(|c| needed.iter().any(|k| ctx.is(c, *k)))?;
        let second = lv_in_grave.iter().filter(|c| c.at != first.at).max_by_key(|c| ctx.view_data(c).attack)?;
        t.pick_targeting(i, vec![first.at, second.at])
    }
}

impl Strategy for DraconicMight {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        let up = |from: u32| Self::upgrade(ctx, from).is_some();
        Some(match code {
            // Level 8: without a REDMD to summon it, it waits in the hand.
            WHITE_NIGHT_DRAGON if ctx.in_hand(RED_EYES_DARKNESS_METAL) || Self::face_up(ctx, RED_EYES_DARKNESS_METAL) => 3000,
            WHITE_NIGHT_DRAGON => 2000,
            RED_EYES_DARKNESS_METAL => 2900,
            // In the hand they wait for their LV5 / LV6 (on the field their
            // ATK counts): costs and discards first.
            ARMED_DRAGON_LV7 if Self::face_up(ctx, ARMED_DRAGON_LV5) => 2000,
            HORUS_LV8 if Self::face_up(ctx, HORUS_LV6) => 2000,
            ARMED_DRAGON_LV7 | HORUS_LV8 => 900,
            PRIME_MATERIAL_DRAGON => 2500,
            ARMED_DRAGON_LV5 => 2400 + if up(ARMED_DRAGON_LV5) { 200 } else { 0 },
            HORUS_LV6 => 2300 + if up(HORUS_LV6) { 300 } else { 0 },
            CHTHONIAN_EMPEROR | JINZO => 2400,
            LEVEL_UP => 1700,
            VANGUARD => 1700,
            HORUS_LV4 => 1600 + if up(HORUS_LV4) { 200 } else { 0 },
            ARMED_DRAGON_LV3 => if up(ARMED_DRAGON_LV3) { 1600 } else { 1000 },
            FOOLISH_BURIAL => 1500,
            TWIN_HEADED_BEHEMOTH => 1300,
            MASKED_DRAGON => 1400,
            STAMPING_DESTRUCTION => 1300,
            FOURTH_DIMENSION => 600,
            STARDUST_DRAGON => 0,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let turn = Self::turn(&ctx);
        // Their board goes before ours comes.
        if let Some(i) = self.dark_hole(t) {
            return Some(i);
        }
        if let Some(i) = self.clear_backrow(t) {
            return Some(i);
        }
        if let Some(i) = self.clear_for_redmd(t) {
            return Some(i);
        }
        if let Some(i) = self.foolish_burial(t) {
            return Some(i);
        }
        if let Some(i) = self.flip_for_redmd(t) {
            return Some(i);
        }
        // Red-Eyes Darkness Metal Dragon whenever it can come out, even for
        // a dear Dragon: it holds the field and revives one every turn.  A
        // second one only for cheap fodder or a better Dragon back.
        if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(RED_EYES_DARKNESS_METAL), Some(Location::Hand)) {
            let fodder = self.cheapest_fodder(&ctx).unwrap_or(i32::MAX);
            let follow_up = if self.redmd_effect != turn { self.best_redmd_target(t).unwrap_or(0) } else { 0 };
            let first = !Self::face_up(&ctx, RED_EYES_DARKNESS_METAL);
            if (first && fodder < 100_000) || fodder <= 1100 || follow_up + 300 >= fodder {
                self.redmd_summoned = turn;
                return t.pick(i);
            }
        }
        // ... then a Dragon from the hand or the Graveyard.
        if let Some(i) = t.activate_from(RED_EYES_DARKNESS_METAL, Location::MonsterZone) {
            if self.best_redmd_target(t).map_or(false, |w| w >= 1000) {
                self.redmd_effect = turn;
                return t.pick(i);
            }
        }
        // Level Up!: the upgrade that gains most.
        if let Some(i) = t.activate(LEVEL_UP) {
            if Self::level_up_pick(&ctx).is_some() {
                return t.pick(i);
            }
        }
        // Armed Dragon LV7 wipes every monster the sent one reaches, LV5 one.
        for (code, sweep) in [(ARMED_DRAGON_LV7, true), (ARMED_DRAGON_LV5, false)] {
            if let Some(i) = t.activate_from(code, Location::MonsterZone) {
                if let Some((_, reach)) = self.dragon_cost(&ctx, sweep) {
                    let target = reach.iter().max_by_key(|c| ctx.threat(c)).map(|c| c.at);
                    return t.pick_targeting(i, target.into_iter().collect());
                }
            }
        }
        if let Some(i) = self.monster_reborn(t) {
            return Some(i);
        }
        if let Some(i) = self.vanguard_pump(t) {
            return Some(i);
        }
        if let Some(i) = self.fourth_dimension(t) {
            return Some(i);
        }
        None
    }

    fn allow_staple(&self, _t: &Turn, code: u32) -> bool {
        // Played by the deck's own rules above.
        !matches!(code, DARK_HOLE | MONSTER_REBORN)
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let on_field = choice.at().map_or(false, |a| a.location == Location::MonsterZone);
        // Chthonian Emperor Dragon, Normal Summoned again: it attacks twice.
        if code == CHTHONIAN_EMPEROR && on_field {
            let swing = ctx.main1() && t.has(ChoiceKind::EnterBattle);
            return Some(swing.then_some(2600.0));
        }
        // Face-up fodder for the REDMD in our hand.
        let fodder_needed = self.redmd_waiting(&ctx) && self.cheapest_fodder(&ctx).map_or(true, |c| c > 1100);
        let small = matches!(code, TWIN_HEADED_BEHEMOTH | MASKED_DRAGON | VANGUARD | HORUS_LV4 | ARMED_DRAGON_LV3);
        if fodder_needed && small {
            if choice.kind != ChoiceKind::NormalSummon {
                return Some(None);
            }
            let cost = match code {
                TWIN_HEADED_BEHEMOTH => 300,
                MASKED_DRAGON => 400,
                VANGUARD => 600,
                HORUS_LV4 => 900,
                _ => 1000,
            };
            return Some(Some(3000.0 - cost as f64));
        }
        Some(match (choice.kind, code) {
            // Its own Special Summon is better than two Tributes.
            (_, RED_EYES_DARKNESS_METAL) => None,
            // A Tribute for a face-down wall that dies to the attack anyway
            // (and Prime Material, White Night, Horus LV6 work face-up only).
            (ChoiceKind::SetMonster, _) if ctx.data(code).level >= 5 => None,
            // It levels up in the next Standby Phase, if 1200 ATK survives
            // their turn (or Level Up! does it now).
            (ChoiceKind::NormalSummon, ARMED_DRAGON_LV3) if Self::upgrade(&ctx, ARMED_DRAGON_LV3).is_some() => {
                let exposed = ctx.monsters(ctx.opp).iter().any(|c| !c.position.face_up || c.attack > 1200);
                if !exposed || ctx.in_hand(LEVEL_UP) { Some(2000.0) } else { return None }
            }
            // The Horus line first: LV6 is unaffected by Spells, LV8 negates
            // them.  Level Up! makes LV6 at once; otherwise it levels up after
            // destroying a monster in battle.
            (ChoiceKind::NormalSummon, HORUS_LV4) if Self::upgrade(&ctx, HORUS_LV4).is_some() => {
                let kill = ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up && ctx.battle_stat(c) < 1600);
                if ctx.in_hand(LEVEL_UP) { Some(2100.0) } else if kill { Some(1950.0) } else { return None }
            }
            // Jinzo shuts their Traps (and ours): it keeps REDMD alive.
            // Never for REDMD or a protector as the Tribute.
            (ChoiceKind::NormalSummon, JINZO) => {
                let spare = ctx.monsters(ctx.me).iter().map(|c| value(self, &ctx, None, Some(c))).min();
                if spare.map_or(false, |v| v <= 2000) { Some(2750.0) } else { None }
            }
            _ => return None,
        })
    }

    fn special_summon(&self, _t: &Turn, choice: &Choice) -> Option<bool> {
        // REDMD comes out through the plan (it picks the Dragon to banish).
        (choice.code() == Some(RED_EYES_DARKNESS_METAL)).then_some(false)
    }

    fn battle(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // A monster destroyed in battle by Armed Dragon LV5 or a Horus levels
        // it up in the End Phase: give it the kill.
        let mut best: Option<(i32, usize, CardRef)> = None;
        for (i, choice) in t.choices() {
            if choice.kind != ChoiceKind::Attack || !t.fresh(i) {
                continue;
            }
            let Some(attacker) = t.view(choice) else { continue };
            let code = Self::code(&ctx, attacker);
            if !matches!(code, ARMED_DRAGON_LV5 | HORUS_LV4 | HORUS_LV6) || Self::upgrade(&ctx, code).is_none() {
                continue;
            }
            for target in ctx.monsters(ctx.opp) {
                if matches!(default_outcome(&ctx, attacker, target, 0), Outcome::Win { trick: false }) {
                    let threat = ctx.threat(target);
                    if best.map_or(true, |b| threat > b.0) {
                        best = Some((threat, i, target.at));
                    }
                }
            }
        }
        let (_, i, target) = best?;
        t.pick_targeting(i, vec![target])
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let hostile = t.hostile_top();
        Some(match code {
            // Negates Spells: the opponent's only.
            HORUS_LV8 => match hostile {
                Hostile::Link(link) if ctx.data(link.code).is_spell() => Response::new(90.0),
                Hostile::Unseen => Response::new(90.0),
                _ => Response::no(),
            },
            // Negates destruction of monsters for a card from the hand.
            PRIME_MATERIAL_DRAGON => {
                let hits_ours = match hostile {
                    Hostile::Link(link) => {
                        link.targets.is_empty()
                            || link.targets.iter().any(|at| at.controller == ctx.me && at.location == Location::MonsterZone)
                    }
                    Hostile::Unseen => true,
                    Hostile::No => false,
                };
                let at_stake: i32 = ctx.monsters(ctx.me).iter().map(|c| value(self, &ctx, None, Some(c))).sum();
                let cheapest = ctx.hand().iter().map(|c| self.discard_cost(&ctx, c)).min().unwrap_or(i32::MAX);
                // A spare card beats Solemn Judgment's half our Life Points.
                if hits_ours && at_stake > cheapest + 500 { Response::new(95.0) } else { Response::no() }
            }
            // At the end of their turn (a free level-up for LV3), or as a
            // blocker when they attack.
            CALL_OF_THE_HAUNTED => {
                let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
                if Self::valley(&ctx) || !(end_of_their_turn || ctx.incoming_attack().is_some()) {
                    return Some(Response::no());
                }
                let best = ctx
                    .graveyard(ctx.me)
                    .into_iter()
                    .filter(|c| ctx.view_data(c).is_monster() && !ctx.view_data(c).is_extra())
                    .max_by_key(|c| Self::call_worth(&ctx, c, end_of_their_turn))
                    .filter(|c| Self::call_worth(&ctx, c, end_of_their_turn) > 0);
                match best {
                    Some(b) => Response::targeting(25.0, vec![b.at]),
                    None => Response::no(),
                }
            }
            STARDUST_DRAGON => match hostile {
                Hostile::No => Response::no(),
                _ => Response::new(70.0),
            },
            // White Night Dragon takes an attack aimed at a weaker monster,
            // for a Spell/Trap of ours.
            WHITE_NIGHT_DRAGON if choice.description & 0xf == REDIRECT => {
                let Some((attacker, Some(target))) = ctx.incoming_attack() else { return Some(Response::no()) };
                let white_night = choice.at().and_then(|at| ctx.card(at));
                let beats = white_night.map_or(false, |w| w.attack > attacker.attack);
                let loses = ctx.attack_hurts(attacker, Some(target));
                let cheapest = ctx.spell_traps(ctx.me).iter().map(|c| self.backrow_cost(&ctx, c)).min().unwrap_or(i32::MAX);
                let saved = value(self, &ctx, None, Some(target));
                if beats && loses && cheapest < saved { Response::new(60.0) } else { Response::no() }
            }
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let hint = t.decision.hint;
        let view = ctx.card(member.at);
        let mine = member.at.controller == ctx.me;
        let code = member.code.map(|c| ctx.canonical(c));
        let last = t.memory.last_activated.map(|c| ctx.canonical(c));
        // REDMD's Special Summon banishes one of our face-up Dragons.
        if hint == Hint::Banish && mine && member.at.location == Location::MonsterZone {
            return view.map(|v| -(self.fodder_cost(&ctx, v) as f64));
        }
        match last {
            Some(RED_EYES_DARKNESS_METAL) if mine && hint == Hint::SpecialSummon => {
                return code.map(|c| self.summon_worth(t, c, member.at.location) as f64);
            }
            // Masked Dragon: a Dragon with 1500 or less ATK from the Deck.
            Some(MASKED_DRAGON) if mine && hint == Hint::SpecialSummon => {
                let more_attacks = !ctx.my_turn()
                    && ctx.monsters(ctx.opp).iter().filter(|c| c.position.face_up && c.position.attack && c.attack >= 1400).count() >= 2;
                return code.map(|c| match c {
                    MASKED_DRAGON if more_attacks => 3000.0,
                    ARMED_DRAGON_LV3 if Self::upgrade(&ctx, ARMED_DRAGON_LV3).is_some() => 2500.0,
                    TWIN_HEADED_BEHEMOTH => 2000.0,
                    _ => 1500.0,
                });
            }
            Some(LEVEL_UP) if mine && member.at.location == Location::MonsterZone && hint.is_cost() => {
                let pick = Self::level_up_pick(&ctx).map(|c| c.at);
                return Some(if pick == Some(member.at) { 5000.0 } else { -(value(self, &ctx, code, view) as f64) });
            }
            Some(ARMED_DRAGON_LV5 | ARMED_DRAGON_LV7) if mine && member.at.location == Location::Hand && hint.is_cost() => {
                let chosen = self.dragon_cost(&ctx, last == Some(ARMED_DRAGON_LV7)).map(|(card, _)| card.at);
                let cost = view.map_or(0, |v| self.discard_cost(&ctx, v));
                return Some(if chosen == Some(member.at) { 5000.0 } else { -(cost as f64) });
            }
            _ if mine && member.at.location == Location::Hand && matches!(hint, Hint::Discard | Hint::ToGraveyard) => {
                return view.map(|v| -(self.discard_cost(&ctx, v) as f64));
            }
            Some(FOOLISH_BURIAL) if mine && member.at.location == Location::Deck => {
                return code.map(|c| self.burial_worth(&ctx, c) as f64);
            }
            Some(WHITE_NIGHT_DRAGON) if mine && member.at.location == Location::SpellTrapZone && hint.is_cost() => {
                return view.map(|v| -(self.backrow_cost(&ctx, v) as f64));
            }
            _ => {}
        }
        // A level-up (the LV triggers, Level Up!) summons from the hand or
        // the Deck: the copy stuck in the hand first.
        if mine && hint == Hint::SpecialSummon && code.map_or(false, |c| Self::copies(c) > 0) {
            let bonus = if member.at.location == Location::Hand { 1000.0 } else { 0.0 };
            return code.map(|c| value(self, &ctx, Some(c), None) as f64 + bonus);
        }
        None
    }
}
