//! Read-only helpers over an [`Observation`] plus printed card data.

use crate::cards::{CardData, CardDatabase};
use crate::model::{CardRef, CardView, Location, Observation, Phase};

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

    /// How much we want an opponent card gone (public information only).
    pub fn threat(&self, card: &CardView) -> i32 {
        match card.at.location {
            Location::MonsterZone => {
                if !card.position.face_up {
                    return UNKNOWN_MONSTER_STAT;
                }
                let bonus = if self.view_data(card).is_extra() { 300 } else { 0 };
                card.attack.max(card.defense) + bonus
            }
            Location::SpellTrapZone => {
                if card.position.face_up {
                    900
                } else {
                    1200
                }
            }
            _ => 500,
        }
    }

    pub fn field_strength(&self, controller: u8) -> i32 {
        self.monsters(controller)
            .iter()
            .map(|c| {
                if controller == self.opp {
                    self.threat(c)
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
            Some(t) if !t.position.attack => attacker.attack > t.defense,
            Some(t) => attacker.attack >= t.attack,
        }
    }
}
