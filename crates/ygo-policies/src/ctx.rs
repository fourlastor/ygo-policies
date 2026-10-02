//! Read-only helpers over an [`Observation`] plus printed card data.

use crate::cards::{CardData, CardDatabase};
use crate::knowledge::{self, BattleProof, Facts};
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
    /// [`knowledge`]).  Only a card we can see has a code, so nothing about
    /// the opponent's face-down cards leaks: they get no facts.
    pub fn facts(&self, card: &CardView) -> Facts {
        card.code.map_or(Facts::NONE, |c| knowledge::facts(self.canonical(c)))
    }

    /// Battle cannot destroy this face-up monster as things stand.
    pub fn battle_proof(&self, card: &CardView) -> bool {
        if card.at.location != Location::MonsterZone || !card.position.face_up {
            return false;
        }
        match self.facts(card).battle_proof {
            BattleProof::Always => true,
            BattleProof::EmptyHand => self.hand_size(card.at.controller) == 0,
            // Morphtronic Boarden in Defense Position shields the other Morphtronics.
            BattleProof::No => {
                self.view_data(card).in_set(knowledge::SET_MORPHTRONIC)
                    && self.monsters(card.at.controller).iter().any(|c| {
                        c.at != card.at
                            && c.position.face_up
                            && !c.position.attack
                            && self.is(c, knowledge::MORPHTRONIC_BOARDEN)
                    })
            }
        }
    }

    /// Would an effect of `player` that targets this card resolve?
    /// `spell_trap`: the effect is a Spell's or Trap's.
    pub fn targetable_by(&self, card: &CardView, player: u8, spell_trap: bool) -> bool {
        if !card.position.face_up {
            return true;
        }
        if player != card.at.controller
            && card.at.location == Location::MonsterZone
            && self.view_data(card).in_set(knowledge::SET_TOON)
            && self.face_up_on_field(card.at.controller, knowledge::TOON_KINGDOM)
        {
            return false;
        }
        let facts = self.facts(card);
        if spell_trap && facts.spell_trap_shield {
            return false;
        }
        if facts.coin_targeting_shield {
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

    /// Would an effect of ours that targets this card resolve?
    pub fn targetable(&self, card: &CardView, spell_trap: bool) -> bool {
        self.targetable_by(card, self.me, spell_trap)
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
        let paid = self.facts(target).negates_attacks_for.map_or(false, |cost| self.obs.life_points[owner as usize] > cost);
        let boomboxen = self.view_data(target).in_set(knowledge::SET_MORPHTRONIC)
            && self
                .monsters(owner)
                .iter()
                .any(|c| c.position.face_up && !c.position.attack && self.is(c, knowledge::MORPHTRONIC_BOOMBOXEN));
        paid || boomboxen
    }

    /// The stat an attack on this monster meets, and whether it then defends:
    /// a Karakuri switches position when selected as an attack target, and
    /// some monsters lose ATK when attacked.
    pub fn attacked_stance(&self, target: &CardView) -> (i32, bool) {
        if !target.position.face_up {
            return (self.battle_stat(target), !target.position.attack);
        }
        let facts = self.facts(target);
        let attack = match facts.switches_when_attacked {
            knowledge::Switch::No => target.position.attack,
            knowledge::Switch::ToDefense => false,
            knowledge::Switch::Either => !target.position.attack,
        };
        if attack {
            (target.attack - facts.attacked_malus, false)
        } else {
            (target.defense, true)
        }
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
                let bonus = if self.view_data(card).is_extra() { 300 } else { 0 };
                // Battle cannot remove it: an effect that can is the one to use.
                let reachable = !targeting || self.targetable(card, false);
                let wall = if self.battle_proof(card) && reachable { 2000 } else { 0 };
                card.attack.max(card.defense).max(owner) + bonus + wall
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
                let attack = attacker.attack + self.facts(attacker).attack_bonus;
                let (stat, defending) = self.attacked_stance(t);
                if defending {
                    attack > stat && !self.battle_proof(t)
                } else {
                    attack >= stat
                }
            }
        }
    }
}
