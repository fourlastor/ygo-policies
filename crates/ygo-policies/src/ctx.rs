//! Read-only helpers over an [`Observation`] plus printed card data.

use crate::cards::{types, CardData, CardDatabase};
use crate::knowledge::{self, kind, Against, Attacked, Facts, Fate, Needs, ALWAYS};
use crate::model::{CardRef, CardView, Coin, Location, Observation, Phase};

/// Assumed battle stat of a face-down monster we cannot see.
pub const UNKNOWN_MONSTER_STAT: i32 = 1500;

static EMPTY: CardData = CardData {
    code: 0,
    alias: 0,
    kind: 0,
    attack: 0,
    defense: 0,
    level: 0,
    race: 0,
    attribute: 0,
    setcodes: Vec::new(),
};

/// What an attack on a monster meets (see [`Ctx::attack_meets`]).
#[derive(Clone, Copy, Debug)]
pub struct Meets {
    /// The facts of the battle position the monster ends up in.
    pub facts: Attacked,
    /// The stat the attacker's ATK is compared with.
    pub stat: i32,
    pub defending: bool,
    /// Battle will not destroy it this time.
    pub survives: bool,
}

#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub obs: &'a Observation,
    pub db: &'a dyn CardDatabase,
    pub me: u8,
    pub opp: u8,
}

impl<'a> Ctx<'a> {
    pub fn new(obs: &'a Observation, db: &'a dyn CardDatabase) -> Self {
        Ctx { obs, db, me: obs.me, opp: obs.opponent() }
    }

    pub fn data(&self, code: u32) -> &'a CardData {
        self.db.card(code).unwrap_or(&EMPTY)
    }

    /// The base passcode of an alternate-art printing (e.g. Monster Reborn
    /// 83764719 -> 83764718), so card knowledge keyed by code matches both.
    pub fn canonical(&self, code: u32) -> u32 {
        let alias = self.data(code).alias;
        if alias != 0 && (code as i64 - alias as i64).abs() < 20 {
            alias
        } else {
            code
        }
    }

    pub fn view_data(&self, card: &CardView) -> &'a CardData {
        card.code.map(|c| self.data(c)).unwrap_or(&EMPTY)
    }

    pub fn card(&self, at: CardRef) -> Option<&'a CardView> {
        self.obs.card(at)
    }

    pub fn pile(&self, controller: u8, location: Location) -> Vec<&'a CardView> {
        self.obs.pile(controller, location).collect()
    }

    pub fn monsters(&self, controller: u8) -> Vec<&'a CardView> {
        self.pile(controller, Location::MonsterZone)
    }

    pub fn spell_traps(&self, controller: u8) -> Vec<&'a CardView> {
        self.pile(controller, Location::SpellTrapZone)
    }

    pub fn hand(&self) -> Vec<&'a CardView> {
        self.pile(self.me, Location::Hand)
    }

    pub fn graveyard(&self, controller: u8) -> Vec<&'a CardView> {
        self.pile(controller, Location::Graveyard)
    }

    pub fn banished(&self, controller: u8) -> Vec<&'a CardView> {
        self.pile(controller, Location::Banished)
    }

    pub fn hand_codes(&self) -> Vec<u32> {
        self.hand().iter().filter_map(|c| c.code).collect()
    }

    pub fn is(&self, card: &CardView, code: u32) -> bool {
        card.code.map(|c| self.canonical(c)) == Some(code)
    }

    pub fn in_hand(&self, code: u32) -> bool {
        self.hand().iter().any(|c| self.is(c, code))
    }

    pub fn count_in(&self, controller: u8, location: Location, code: u32) -> usize {
        self.pile(controller, location).iter().filter(|c| self.is(c, code)).count()
    }

    pub fn face_up_on_field(&self, controller: u8, code: u32) -> bool {
        self.monsters(controller)
            .iter()
            .chain(self.spell_traps(controller).iter())
            .any(|c| c.position.face_up && self.is(c, code))
    }

    /// Can this monster of ours still declare an attack?  Engines that do not
    /// expose it get the public approximation: a face-up Attack Position
    /// monster during our Main Phase 1 / Battle Phase.
    pub fn can_attack(&self, card: &CardView) -> bool {
        if self.obs.can_attack_known {
            return card.can_attack;
        }
        card.at.controller == self.me
            && card.at.location == Location::MonsterZone
            && card.position.face_up
            && card.position.attack
            && self.my_turn()
            && self.phase().map_or(false, |p| p == Phase::Main1 || p.is_battle())
    }

    pub fn my_turn(&self) -> bool {
        self.obs.turn_player == Some(self.me)
    }

    pub fn phase(&self) -> Option<Phase> {
        self.obs.phase
    }

    pub fn main1(&self) -> bool {
        self.obs.phase == Some(Phase::Main1)
    }

    pub fn my_lp(&self) -> i32 {
        self.obs.life_points[self.me as usize]
    }

    pub fn opp_lp(&self) -> i32 {
        self.obs.life_points[self.opp as usize]
    }

    pub fn deck_size(&self, controller: u8) -> u32 {
        self.obs.pile_size(controller, Location::Deck)
    }

    pub fn hand_size(&self, controller: u8) -> u32 {
        self.obs.pile_size(controller, Location::Hand)
    }

    /// The number an attacker has to exceed, or a guess when face-down.
    pub fn battle_stat(&self, card: &CardView) -> i32 {
        if !card.position.face_up && !card.known() {
            return UNKNOWN_MONSTER_STAT;
        }
        if card.position.attack {
            card.attack
        } else {
            card.defense
        }
    }

    pub fn opp_best_attack(&self) -> i32 {
        self.monsters(self.opp)
            .iter()
            .filter(|c| c.position.face_up)
            .map(|c| c.attack)
            .max()
            .unwrap_or(0)
    }

    pub fn my_best_attack(&self) -> i32 {
        self.monsters(self.me)
            .iter()
            .filter(|c| c.position.face_up)
            .map(|c| c.attack)
            .max()
            .unwrap_or(0)
    }

    /// A face-up Spell/Trap *of the opponent* that stops our monsters from
    /// attacking.  Our own locks are never removal targets.
    pub fn is_attack_lock(&self, card: &CardView) -> bool {
        card.at.controller == self.opp && Self::lock_card(self, card)
    }

    /// Our own face-up attack locks (a stall deck protects them).
    pub fn own_attack_locks(&self) -> Vec<&'a CardView> {
        self.spell_traps(self.me).into_iter().filter(|c| Self::lock_card(self, c)).collect()
    }

    fn lock_card(&self, card: &CardView) -> bool {
        card.position.face_up
            && card.at.location == Location::SpellTrapZone
            && card.code.map_or(false, |c| crate::staples::ATTACK_LOCKS.contains(&self.canonical(c)))
    }

    /// The opponent's face-up attack locks.
    pub fn attack_locks(&self) -> Vec<&'a CardView> {
        self.spell_traps(self.opp).into_iter().filter(|c| self.is_attack_lock(c)).collect()
    }

    /// What we know about a card beyond its printed stats (see
    /// [`knowledge`]), as things stand on the board.  Only a card we can see
    /// has a code, so nothing about the opponent's face-down cards leaks:
    /// they get no facts.
    pub fn facts(&self, card: &CardView) -> Facts {
        let Some(code) = card.code else { return Facts::NONE };
        let mut facts = knowledge::facts(self.canonical(code));
        let met = match facts.needs {
            Needs::Nothing => true,
            Needs::EmptyHand => self.hand_size(card.at.controller) == 0,
            Needs::EqualHands => self.hand_size(self.me) == self.hand_size(self.opp),
        };
        if !met {
            facts.attacked = [Attacked::PLAIN; 2];
            facts.immune = 0;
            facts.unaffected = 0;
            facts.negates_any = 0;
            facts.negates_aimed = 0;
            facts.negates_destruction = 0;
        }
        if self.monsters_banished() {
            if matches!(self.canonical(code), knowledge::GUSTO_GULLDO | knowledge::GUSTO_EGUL | knowledge::GUSTO_WINDA) {
                for attacked in &mut facts.attacked { attacked.payoff = 0; }
                facts.effect_payoff = 0;
            }
            if matches!(self.canonical(code), knowledge::DD_SURVIVOR | knowledge::DD_SCOUT_PLANE)
                && card.position.face_up {
                for attacked in &mut facts.attacked { attacked.payoff = 1200; }
                facts.effect_payoff = 1200;
            }
        }
        // Skill Drain leaves a face-up monster only what it does once it has
        // left the field.
        if card.at.location == Location::MonsterZone && card.position.face_up && self.effects_drained() {
            let left = |a: Attacked| Attacked {
                attacker: if a.when_destroyed { a.attacker } else { Fate::Unharmed },
                when_destroyed: a.when_destroyed,
                payoff: a.payoff,
                collateral: a.collateral,
                against: a.against,
                ..Attacked::PLAIN
            };
            facts = Facts {
                attacked: [left(facts.attacked[0]), left(facts.attacked[1])],
                effect_payoff: facts.effect_payoff,
                effect_collateral: facts.effect_collateral,
                ..Facts::NONE
            };
        }
        if self.gusto_reflects(card) || (self.is(card, 94004268) && card.position.face_up && !self.effects_drained()) {
            for attacked in &mut facts.attacked {
                attacked.no_damage = true;
                attacked.burn = knowledge::Burn::Reflected;
            }
        }
        if card.at.location == Location::MonsterZone && card.position.face_up
            && self.view_data(card).in_set(0x4) && !self.effects_drained()
            && self.face_up_on_field(card.at.controller, 15951532) {
            for attacked in &mut facts.attacked { attacked.survives = knowledge::ALWAYS; }
        }
        facts
    }

    /// Public replacement effects that stop monsters reaching the Graveyard.
    pub fn monsters_banished(&self) -> bool {
        [self.me, self.opp].iter().any(|&player| {
            self.face_up_on_field(player, knowledge::MACRO_COSMOS)
                || self.face_up_on_field(player, knowledge::DIMENSIONAL_FISSURE)
                || (!self.effects_drained() && self.face_up_on_field(player, knowledge::BANISHER_OF_THE_RADIANCE))
        })
    }

    /// Sphreez reflects damage for all its controller's face-up Gustos.
    /// It does not grant its own battle indestructibility to the others.
    pub fn gusto_reflects(&self, card: &CardView) -> bool {
        card.at.location == Location::MonsterZone && card.position.face_up
            && self.view_data(card).in_set(knowledge::SET_GUSTO)
            && !self.effects_drained()
            && self.face_up_on_field(card.at.controller, knowledge::DAIGUSTO_SPHREEZ)
    }

    /// A face-up Skill Drain, on either field: monsters on the field have
    /// no effects.
    pub fn effects_drained(&self) -> bool {
        self.face_up_on_field(self.me, knowledge::SKILL_DRAIN) || self.face_up_on_field(self.opp, knowledge::SKILL_DRAIN)
    }

    /// A monster whose Summon brings no effect of its own to the field: it
    /// has none (a Gemini monster has none yet), or a Skill Drain is face-up.
    pub fn effectless(&self, card: &CardView) -> bool {
        let data = self.view_data(card);
        self.effects_drained() || !data.is(types::EFFECT) || data.is(types::GEMINI)
    }

    /// What an attack on this monster meets.  `attacker`: the monster that
    /// attacks, or `None` for "whoever attacks" (facts that only hold
    /// against some attackers then do not count).
    pub fn attack_meets(&self, attacker: Option<&CardView>, target: &CardView) -> Meets {
        // Our own Set monster is known to us: it is flipped before damage calculation.
        let seen = target.position.face_up || (target.at.controller == self.me && target.known());
        if target.at.location != Location::MonsterZone || !seen {
            let facts = Attacked::PLAIN;
            return Meets { facts, stat: self.battle_stat(target), defending: !target.position.attack, survives: false };
        }
        let facts = self.facts(target);
        // A monster that switches position when attacked battles in the other one.
        let start = if target.position.attack { 0 } else { 1 };
        let switches = facts.attacked[start].switches;
        let end = if switches { 1 - start } else { start };
        let mut met = facts.attacked[end];
        let holds = match attacker {
            Some(a) => met.against.holds(a.attack + self.facts(a).striking.bonus, a.level, self.view_data(a).attribute),
            None => met.against == Against::All,
        };
        if !holds {
            met = Attacked::PLAIN;
        }
        met.switches = switches;
        let defending = end == 1;
        let stat = if defending { target.defense } else { target.attack } + met.stat;
        // Morphtronic Boarden in Defense Position shields the other Morphtronics.
        let boarden = self.view_data(target).in_set(knowledge::SET_MORPHTRONIC)
            && self.monsters(target.at.controller).iter().any(|c| {
                c.at != target.at && c.position.face_up && !c.position.attack && self.is(c, knowledge::MORPHTRONIC_BOARDEN)
            });
        let survives = boarden || met.survives == ALWAYS || met.survives as u32 > target.battles;
        Meets { facts: met, stat, defending, survives }
    }

    /// Battle cannot destroy this face-up monster as things stand, whoever
    /// attacks it and however often.
    pub fn battle_proof(&self, card: &CardView) -> bool {
        if card.at.location != Location::MonsterZone || !card.position.face_up {
            return false;
        }
        let met = self.attack_meets(None, card);
        met.survives && (met.facts.survives == ALWAYS || met.facts.survives == 0)
    }

    /// The kind of effect a card of ours has (see [`knowledge::kind`]).
    pub fn kind_of(&self, code: u32) -> u8 {
        let data = self.data(code);
        if data.is_spell() {
            kind::SPELL
        } else if data.is_trap() {
            kind::TRAP
        } else {
            kind::MONSTER
        }
    }

    /// Would an effect of one of these `kinds`, used by `player` on this
    /// card, do its work?  `aimed`: it targets the card.  `destroys`: its
    /// work is destroying it.  What stops it: the card cannot be targeted,
    /// its controller negates the effect, or the effect does nothing to it.
    pub fn reached_by(&self, card: &CardView, player: u8, kinds: u8, aimed: bool, destroys: bool) -> bool {
        if !card.position.face_up {
            return true;
        }
        let facts = self.facts(card);
        if player != card.at.controller {
            if aimed
                && card.at.location == Location::MonsterZone
                && self.view_data(card).in_set(knowledge::SET_TOON)
                && self.face_up_on_field(card.at.controller, knowledge::TOON_KINGDOM)
            {
                return false;
            }
            // A negation paid with a card from the hand needs one.
            let negates = !facts.negate_discards || self.hand_size(card.at.controller) > 0;
            let mut stopped = facts.negates_any;
            if aimed {
                stopped |= facts.untargetable;
            }
            if aimed && negates {
                stopped |= facts.negates_aimed;
            }
            if destroys && negates {
                stopped |= facts.negates_destruction;
            }
            stopped |= if destroys { facts.immune } else { facts.unaffected };
            if stopped & kinds != 0 {
                return false;
            }
        }
        if aimed && facts.coin_targeting_shield {
            // Heads shields it from its controller's effects, tails from the
            // opponent's; before the coin is known, from both.
            return match card.coin_effect.map(|e| e.result) {
                Some(Coin::Heads) => player != card.at.controller,
                Some(Coin::Tails) => player == card.at.controller,
                None => false,
            };
        }
        true
    }

    /// Would our card `by` do its work on this card?  See [`Ctx::reached_by`].
    pub fn reaches(&self, card: &CardView, by: u32, aimed: bool, destroys: bool) -> bool {
        self.reached_by(card, self.me, self.kind_of(by), aimed, destroys)
    }

    /// Would an effect of `player` that targets this card resolve?
    /// `spell_trap`: the effect is a Spell's or Trap's (either is assumed);
    /// otherwise a monster's.
    pub fn targetable_by(&self, card: &CardView, player: u8, spell_trap: bool) -> bool {
        let kinds = if spell_trap { kind::SPELL | kind::TRAP } else { kind::MONSTER };
        self.reached_by(card, player, kinds, true, false)
    }

    /// Would an effect of ours that targets this card resolve?
    pub fn targetable(&self, card: &CardView, spell_trap: bool) -> bool {
        self.targetable_by(card, self.me, spell_trap)
    }

    /// A face-up monster of the opponent whose controller negates, at no
    /// cost and every time, any effect of the kind our card `by` has: using
    /// the card now would waste it.
    pub fn wasted(&self, by: u32) -> bool {
        let kinds = self.kind_of(by);
        self.monsters(self.opp).iter().any(|c| c.position.face_up && self.facts(c).negates_any & kinds != 0)
    }

    /// Toon World on the field (Toon Kingdom is treated as it).
    pub fn is_toon_world(&self, card: &CardView) -> bool {
        card.code.map_or(false, |c| knowledge::TOON_WORLDS.contains(&self.canonical(c)))
    }

    /// Can the controller of this face-up monster negate an attack on it
    /// (Krebons for 800 LP, a Defense Position Morphtronic Boomboxen)?
    pub fn attack_negatable(&self, target: &CardView) -> bool {
        if !target.position.face_up {
            return false;
        }
        let owner = target.at.controller;
        let met = self.attack_meets(None, target).facts;
        let paid = met.negates > 0 && self.obs.life_points[owner as usize] > met.negate_cost;
        let boomboxen = self.view_data(target).in_set(knowledge::SET_MORPHTRONIC)
            && self
                .monsters(owner)
                .iter()
                .any(|c| c.position.face_up && !c.position.attack && self.is(c, knowledge::MORPHTRONIC_BOOMBOXEN));
        paid || boomboxen
    }

    /// What its own attack does to this monster besides the battle: `Some`
    /// when the target is gone whatever the result (`true`: before damage
    /// calculation, so there is no battle at all).
    pub fn strike_removes(&self, attacker: &CardView, target: &CardView) -> Option<bool> {
        let striking = self.facts(attacker).striking;
        let known = target.position.face_up || target.known();
        let hits = striking.target != Fate::Unharmed
            && known
            && striking.against.holds(target.attack, target.level, self.view_data(target).attribute);
        hits.then_some(striking.before_damage)
    }

    /// How much we want an opponent card gone by an effect that targets it
    /// (public information only).
    pub fn threat(&self, card: &CardView) -> i32 {
        self.removal_worth(card, true)
    }

    /// How much we want an opponent card gone by an effect that does not
    /// target it (Dark Hole, Raigeki, Smashing Ground): a monster battle
    /// cannot destroy counts even when no targeting effect could reach it.
    pub fn sweep_worth(&self, card: &CardView) -> i32 {
        self.removal_worth(card, false)
    }

    fn removal_worth(&self, card: &CardView, targeting: bool) -> i32 {
        // What the card is worth to the deck that plays it, once we can see it.
        let owner = card.code.map_or(0, |c| knowledge::owner_worth(self.canonical(c)));
        match card.at.location {
            Location::MonsterZone => {
                if !card.position.face_up {
                    return UNKNOWN_MONSTER_STAT;
                }
                let facts = self.facts(card);
                let mut worth = card.attack.max(card.defense).max(owner);
                if self.view_data(card).is_extra() {
                    worth += 300;
                }
                // Its controller negates what we play: it comes first.
                if facts.negates_any != 0 {
                    worth += 1000;
                }
                if targeting {
                    // Battle cannot remove it: an effect that can is the one to use.
                    if self.battle_proof(card) && self.targetable(card, false) {
                        worth += 2000;
                    }
                } else {
                    // No kind of effect destroys it.
                    if [kind::SPELL, kind::TRAP, kind::MONSTER].iter().all(|k| !self.reached_by(card, self.me, *k, false, true)) {
                        return 0;
                    }
                    if self.battle_proof(card) {
                        worth += 2000;
                    }
                    // Destroyed by an effect, it pays its controller back.
                    worth = (worth - facts.effect_payoff).max(worth.min(300));
                }
                worth
            }
            Location::SpellTrapZone => {
                if self.is_attack_lock(card) {
                    3000
                } else if card.position.face_up && self.is_toon_world(card) {
                    // Destroying Toon World destroys every Toon it carries.
                    let toons: i32 = self
                        .monsters(card.at.controller)
                        .iter()
                        .filter(|c| c.position.face_up && self.view_data(c).in_set(knowledge::SET_TOON))
                        .map(|c| c.attack.max(c.defense))
                        .sum();
                    owner.max(900 + toons)
                } else if card.position.face_up {
                    owner.max(900)
                } else {
                    1200
                }
            }
            // A revealed card in their hand (Trap Dustshoot): its printed stats.
            Location::Hand if card.known() && self.view_data(card).is_monster() => {
                let data = self.view_data(card);
                data.attack.max(data.defense).max(500)
            }
            _ => 500,
        }
    }

    /// What our card `by`, which destroys without targeting, would take off
    /// the opponent's field: the monsters it does destroy.
    pub fn swept_strength(&self, by: u32) -> i32 {
        self.monsters(self.opp).iter().filter(|c| self.reaches(c, by, false, true)).map(|c| self.sweep_worth(c)).sum()
    }

    pub fn field_strength(&self, controller: u8) -> i32 {
        self.monsters(controller)
            .iter()
            .map(|c| {
                if controller == self.opp {
                    self.sweep_worth(c)
                } else {
                    c.attack.max(c.defense).max(self.view_data(c).attack)
                }
            })
            .sum()
    }

    pub fn set_backrow(&self, controller: u8) -> Vec<&'a CardView> {
        self.spell_traps(controller)
            .into_iter()
            .filter(|c| !c.position.face_up)
            .collect()
    }

    pub fn free_monster_zones(&self, controller: u8) -> usize {
        5usize.saturating_sub(self.monsters(controller).iter().filter(|c| c.at.sequence < 5).count())
    }

    pub fn battle_attacker(&self) -> Option<&'a CardView> {
        self.obs.battle_attacker.and_then(|at| self.card(at))
    }

    pub fn battle_target(&self) -> Option<&'a CardView> {
        self.obs.battle_target.and_then(|at| self.card(at))
    }

    /// The opponent's declared attack, if one is in progress.
    pub fn incoming_attack(&self) -> Option<(&'a CardView, Option<&'a CardView>)> {
        let attacker = self.battle_attacker()?;
        (attacker.at.controller == self.opp).then(|| (attacker, self.battle_target()))
    }

    /// ATK the opponent's face-up Attack Position monsters could swing at an
    /// open field this turn.
    pub fn opp_attack_potential(&self) -> i32 {
        self.monsters(self.opp)
            .iter()
            .filter(|c| c.position.face_up && c.position.attack)
            .map(|c| c.attack)
            .sum()
    }

    /// Would this declared attack cost us a monster or a lot of life points?
    pub fn attack_hurts(&self, attacker: &CardView, target: Option<&CardView>) -> bool {
        match target {
            None => {
                attacker.attack >= 1500
                    || attacker.attack >= self.my_lp()
                    || (self.monsters(self.me).is_empty() && self.opp_attack_potential() >= 2000)
            }
            Some(t) => {
                // Its attack removes our monster whatever the numbers say:
                // a loss, unless ours only goes back to the hand.
                if self.strike_removes(attacker, t).is_some() {
                    return self.facts(attacker).striking.target != Fate::Returned || self.view_data(t).is_extra();
                }
                let met = self.attack_meets(Some(attacker), t);
                // Our monster rids us of the attacker before any damage.
                if met.facts.attacker != Fate::Unharmed && met.facts.before_damage {
                    return false;
                }
                let striking = self.facts(attacker).striking;
                let attack = met.facts.attacker_atk.apply(attacker.attack + striking.bonus);
                if met.defending {
                    attack > met.stat && (!met.survives || striking.piercing)
                } else {
                    attack >= met.stat && !(met.survives && met.facts.no_damage)
                }
            }
        }
    }
}
