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
        /// Build the policy; `u64` seeds its tie-breaking.
        pub build: fn(Arc<dyn CardDatabase>, u64) -> Box<dyn Policy>,
    }

    macro_rules! entry {
        ($id:literal, $module:ident :: $strategy:ident) => {
            Entry {
                id: $id,
                deck: decks::$module::DECK,
                build: |db, seed| Box::new(Agent::seeded(decks::$module::$strategy::default(), db, seed)),
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
        entry!("crystal", crystal::Crystal),
        entry!("morphtronic", morphtronic::Morphtronic),
        entry!("dragunity", dragunity::Dragunity),
        entry!("spellcaster", spellcaster::Spellcaster),
    ];

    pub fn find(id: &str) -> Option<&'static Entry> {
        POLICIES.iter().find(|e| e.id == id)
    }

    pub fn create(id: &str, db: Arc<dyn CardDatabase>) -> Option<Box<dyn Policy>> {
        create_seeded(id, db, 0)
    }

    /// A policy whose equally good choices are broken by a generator seeded
    /// with `seed` (see [`crate::agent::TieBreak`]).
    pub fn create_seeded(id: &str, db: Arc<dyn CardDatabase>, seed: u64) -> Option<Box<dyn Policy>> {
        find(id).map(|e| (e.build)(db, seed))
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
            let mut policy = (entry.build)(db.clone(), 0);
            let index = policy.choose(&observation(), &decision);
            assert!(index < decision.choices.len(), "{} answered {index}", entry.id);
            // Nothing to attack with: end the turn rather than enter battle.
            assert_eq!(decision.choices[index].kind, ChoiceKind::EndTurn, "{}", entry.id);
        }
    }

    fn card(controller: u8, location: Location, sequence: u32, code: u32, face_up: bool) -> CardView {
        CardView {
            at: CardRef { controller, location, sequence },
            code: Some(code),
            position: Position { face_up, attack: true },
            attack: 1800,
            defense: 1000,
            level: 4,
            can_attack: true,
            counters: 0,
        }
    }

    fn activate(code: u32, location: Location, sequence: u32) -> Choice {
        Choice {
            kind: ChoiceKind::Activate,
            card: Some(Member { at: CardRef { controller: 0, location, sequence }, code: Some(code), value: 0, required: false }),
            members: Vec::new(),
            description: 0,
            place: None,
        }
    }

    /// Breaking the opponent's attack lock must never sweep our own lock away.
    #[test]
    fn lock_removal_spares_our_own_locks() {
        use crate::staples::{GIANT_TRUNADE, GRAVITY_BIND, HEAVY_STORM, MYSTICAL_SPACE_TYPHOON, SWORDS_OF_REVEALING_LIGHT};
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards::default());
        let mut obs = observation();
        obs.cards = vec![
            card(0, Location::MonsterZone, 0, 1, true),
            card(1, Location::SpellTrapZone, 0, GRAVITY_BIND, true),
            card(1, Location::SpellTrapZone, 1, 2, false),
        ];
        let decision = |choices: Vec<Choice>| Decision {
            kind: DecisionKind::Idle,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices,
        };
        let sweepers = decision(vec![
            activate(HEAVY_STORM, Location::Hand, 0),
            activate(GIANT_TRUNADE, Location::Hand, 1),
            choice(ChoiceKind::EnterBattle),
            choice(ChoiceKind::EndTurn),
        ]);
        let mut policy = crate::registry::create("burn", db.clone()).unwrap();
        // Without a lock of our own, sweeping their Gravity Bind is right.
        let index = policy.choose(&obs, &sweepers);
        assert_eq!(sweepers.choices[index].kind, ChoiceKind::Activate);
        // With our own Swords of Revealing Light face-up, neither sweeper is cast.
        obs.cards.push(card(0, Location::SpellTrapZone, 0, SWORDS_OF_REVEALING_LIGHT, true));
        let mut policy = crate::registry::create("burn", db.clone()).unwrap();
        let index = policy.choose(&obs, &sweepers);
        assert_ne!(sweepers.choices[index].kind, ChoiceKind::Activate, "swept our own lock");
        // A targeted MST is still aimed at their lock, not ours.
        let typhoon = decision(vec![activate(MYSTICAL_SPACE_TYPHOON, Location::Hand, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("burn", db).unwrap();
        let index = policy.choose(&obs, &typhoon);
        assert_eq!(typhoon.choices[index].kind, ChoiceKind::Activate);
    }

    #[test]
    fn ties_are_broken_at_random_and_reproducibly() {
        use crate::agent::TieBreak;
        let scored = [(1.0, 0), (0.5, 1), (1.0, 2), (1.0, 3)];
        let ties = TieBreak::new(7);
        let picks: Vec<usize> = (0..64).map(|_| ties.best(scored).unwrap().1).collect();
        assert!(picks.iter().all(|pick| [0, 2, 3].contains(pick)), "picked a lower score");
        for tied in [0, 2, 3] {
            assert!(picks.contains(&tied), "never picked {tied}");
        }
        let again = TieBreak::new(7);
        assert_eq!(picks, (0..64).map(|_| again.best(scored).unwrap().1).collect::<Vec<_>>());
        // A unique best never consumes a draw.
        let unique = TieBreak::new(7);
        assert_eq!(unique.best([(2.0, 9), (1.0, 8)]), Some((2.0, 9)));
        assert_eq!(unique.index(1_000_000), TieBreak::new(7).index(1_000_000));
    }

    #[test]
    fn zones_are_chosen_at_random_per_seed() {
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards::default());
        let decision = Decision {
            kind: DecisionKind::Place,
            hint: Hint::None,
            minimum: 1,
            maximum: 1,
            selected: Vec::new(),
            subject: None,
            choices: (0..5).map(|_| choice(ChoiceKind::Place)).collect(),
        };
        let answer = |seed| crate::registry::create_seeded("blackwing", db.clone(), seed).unwrap().choose(&observation(), &decision);
        let answers: Vec<usize> = (0..32).map(answer).collect();
        assert!(answers.iter().any(|a| *a != answers[0]), "every seed chose zone {}", answers[0]);
        assert_eq!(answers, (0..32).map(answer).collect::<Vec<_>>());
    }

    #[test]
    fn registry_ids_are_unique() {
        let mut ids: Vec<_> = crate::registry::POLICIES.iter().map(|e| e.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), crate::registry::POLICIES.len());
    }
}
