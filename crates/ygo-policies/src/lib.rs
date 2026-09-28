//! Rule-based, single-deck Yu-Gi-Oh! policies.
//!
//! * [`model`]: the engine-agnostic observation/decision types a policy sees.
//!   Engine front ends (e.g. `ygo-policies-ocgcore`) build them and are
//!   responsible for hiding everything the seat may not see.
//! * [`agent`]: the [`Policy`] trait, the per-deck [`Strategy`] hooks and the
//!   shared decision loop ([`Agent`]).
//! * [`tactics`] / [`staples`]: deck-independent play and shared staple cards.
//! * [`decks`]: one strategy per deck; [`registry`] builds them by id.

pub mod agent;
pub mod cards;
pub mod ctx;
pub mod decks;
pub mod model;
pub mod staples;
pub mod tactics;

pub use agent::{Agent, Policy, Strategy};
pub use cards::{CardData, CardDatabase};

pub mod registry {
    use std::sync::Arc;

    use crate::agent::{Agent, Policy};
    use crate::cards::CardDatabase;
    use crate::decks;

    /// A deck policy: stable id, the deck list it was written for, and a constructor.
    pub struct Entry {
        pub id: &'static str,
        pub deck: &'static str,
        pub build: fn(Arc<dyn CardDatabase>) -> Box<dyn Policy>,
    }

    macro_rules! entry {
        ($id:literal, $module:ident :: $strategy:ident) => {
            Entry {
                id: $id,
                deck: decks::$module::DECK,
                build: |db| Box::new(Agent::new(decks::$module::$strategy::default(), db)),
            }
        };
    }

    pub const POLICIES: &[Entry] = &[
        entry!("blackwing", blackwing::Blackwing),
        entry!("burn", burn::Burn),
        entry!("rock-block", rock_block::RockBlock),
        entry!("monarch", monarch::Monarch),
        entry!("lightsworn", lightsworn::Lightsworn),
        entry!("infernity", infernity::Infernity),
        entry!("gladiator", gladiator::Gladiator),
        entry!("heroes", heroes::Heroes),
        entry!("gishki", gishki::Gishki),
    ];

    pub fn find(id: &str) -> Option<&'static Entry> {
        POLICIES.iter().find(|e| e.id == id)
    }

    pub fn create(id: &str, db: Arc<dyn CardDatabase>) -> Option<Box<dyn Policy>> {
        find(id).map(|e| (e.build)(db))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::cards::MemoryCards;
    use crate::model::*;

    fn observation() -> Observation {
        Observation {
            me: 0,
            turn: 1,
            turn_player: Some(0),
            phase: Some(Phase::Main1),
            life_points: [8000, 8000],
            cards: Vec::new(),
            pile_sizes: Vec::new(),
            chain: Vec::new(),
            battle_attacker: None,
            battle_target: None,
            event_cards: Vec::new(),
            chain_known: true,
            can_attack_known: true,
        }
    }

    fn choice(kind: ChoiceKind) -> Choice {
        Choice { kind, card: None, members: Vec::new(), description: 0, place: None }
    }

    #[test]
    fn every_registered_policy_answers_within_range() {
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards::default());
        let decision = Decision {
            kind: DecisionKind::Idle,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices: vec![choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)],
        };
        for entry in crate::registry::POLICIES {
            let mut policy = (entry.build)(db.clone());
            let index = policy.choose(&observation(), &decision);
            assert!(index < decision.choices.len(), "{} answered {index}", entry.id);
            // Nothing to attack with: end the turn rather than enter battle.
            assert_eq!(decision.choices[index].kind, ChoiceKind::EndTurn, "{}", entry.id);
        }
    }

    #[test]
    fn registry_ids_are_unique() {
        let mut ids: Vec<_> = crate::registry::POLICIES.iter().map(|e| e.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), crate::registry::POLICIES.len());
    }
}
