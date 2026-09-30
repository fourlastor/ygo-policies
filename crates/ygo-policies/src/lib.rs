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
        entry!("ojama", ojama::Ojama),
        entry!("watt", watt::Watt),
        entry!("pyramid", pyramid::Pyramid),
        entry!("arcana", arcana::Arcana),
        entry!("toon", toon::Toon),
        entry!("gravekeeper", gravekeeper::Gravekeeper),
        entry!("karakuri", karakuri::Karakuri),
        entry!("harpie", harpie::Harpie),
        entry!("fortune-lady", fortune_lady::FortuneLady),
        entry!("destiny-hero", destiny_hero::DestinyHero),
        entry!("six-samurai", six_samurai::SixSamurai),
        entry!("tele-dad", tele_dad::TeleDad),
        entry!("quickdraw-plant", quickdraw_plant::QuickdrawPlant),
        entry!("machina", machina::Machina),
        entry!("x-saber", x_saber::XSaber),
        entry!("draconic-might", draconic_might::DraconicMight),
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
            coin_toss: None,
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
            coin_effect: None,
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

    fn attack(code: u32, sequence: u32, direct: bool) -> Choice {
        Choice {
            kind: ChoiceKind::Attack,
            card: Some(Member {
                at: CardRef { controller: 0, location: Location::MonsterZone, sequence },
                code: Some(code),
                value: direct as i64,
                required: false,
            }),
            members: Vec::new(),
            description: 0,
            place: None,
        }
    }

    /// A monster that must attack leaves no way out of the Battle Phase: the
    /// least bad attack beats the engine's first choice.
    #[test]
    fn a_forced_battle_attacks() {
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards::default());
        let mut obs = observation();
        obs.phase = Some(Phase::BattleStep);
        obs.cards = vec![card(0, Location::MonsterZone, 0, 1, true), card(1, Location::MonsterZone, 0, 2, true)];
        obs.cards[1].attack = 3000;
        let decision = Decision {
            kind: DecisionKind::Battle,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices: vec![activate(3, Location::SpellTrapZone, 0), attack(1, 0, false)],
        };
        let mut policy = crate::registry::create("karakuri", db).unwrap();
        let index = policy.choose(&obs, &decision);
        assert_eq!(decision.choices[index].kind, ChoiceKind::Attack);
    }

    /// Main Phase 1 cannot end any other way than through the Battle Phase.
    #[test]
    fn a_turn_that_cannot_end_enters_battle() {
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards::default());
        let decision = Decision {
            kind: DecisionKind::Idle,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices: vec![activate(3, Location::SpellTrapZone, 0), choice(ChoiceKind::EnterBattle)],
        };
        let mut policy = crate::registry::create("karakuri", db).unwrap();
        let index = policy.choose(&observation(), &decision);
        assert_eq!(decision.choices[index].kind, ChoiceKind::EnterBattle);
    }

    /// "Attack directly?" follows the plan: direct unless a target was chosen.
    #[test]
    fn attacking_directly_follows_the_plan() {
        use crate::agent::{Memory, Turn};
        let db = MemoryCards::default();
        let obs = observation();
        let answer = |kind| Choice { description: crate::tactics::ATTACK_DIRECTLY, ..choice(kind) };
        let decision = Decision {
            kind: DecisionKind::YesNo,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices: vec![answer(ChoiceKind::Yes), answer(ChoiceKind::No)],
        };
        let mut memory = Memory::default();
        let t = Turn { ctx: crate::ctx::Ctx::new(&obs, &db), decision: &decision, memory: &mut memory };
        assert_eq!(crate::tactics::attack_directly(&t), Some(true));
        memory.intent = vec![(CardRef { controller: 1, location: Location::MonsterZone, sequence: 0 }, None)];
        let t = Turn { ctx: crate::ctx::Ctx::new(&obs, &db), decision: &decision, memory: &mut memory };
        assert_eq!(crate::tactics::attack_directly(&t), Some(false));
    }

    fn toggle(location: Location, sequence: u32, code: u32) -> Choice {
        Choice {
            kind: ChoiceKind::Toggle,
            card: Some(Member { at: CardRef { controller: 0, location, sequence }, code: Some(code), value: 0, required: false }),
            members: Vec::new(),
            description: 0,
            place: None,
        }
    }

    fn select_one(hint: Hint, choices: Vec<Choice>) -> Decision {
        Decision { kind: DecisionKind::SelectCards, hint, minimum: 1, maximum: 1, selected: Vec::new(), subject: None, choices }
    }

    /// Level Up! pays with the monster whose upgrade gains most, and never
    /// with Armed Dragon LV7, whose Level Up! would summon an LV5.
    #[test]
    fn level_up_never_levels_down() {
        const ARMED_DRAGON_LV7: u32 = 73879377;
        const HORUS_LV4: u32 = 75830094;
        const LEVEL_UP: u32 = 25290459;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards::default());
        let mut obs = observation();
        obs.cards = vec![
            card(0, Location::MonsterZone, 0, ARMED_DRAGON_LV7, true),
            card(0, Location::MonsterZone, 1, HORUS_LV4, true),
            card(0, Location::Hand, 0, LEVEL_UP, true),
        ];
        let mut policy = crate::registry::create("draconic-might", db).unwrap();
        let idle = Decision {
            kind: DecisionKind::Idle,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices: vec![activate(LEVEL_UP, Location::Hand, 0), choice(ChoiceKind::EndTurn)],
        };
        assert_eq!(policy.choose(&obs, &idle), 0, "Level Up! not played");
        let cost = select_one(
            Hint::ToGraveyard,
            vec![toggle(Location::MonsterZone, 0, ARMED_DRAGON_LV7), toggle(Location::MonsterZone, 1, HORUS_LV4)],
        );
        assert_eq!(policy.choose(&obs, &cost), 1, "Level Up! sent Armed Dragon LV7");
    }

    /// Red-Eyes Darkness Metal Dragon banishes the cheapest face-up Dragon,
    /// and never another REDMD: its effect is once per turn anyway.
    #[test]
    fn red_eyes_darkness_metal_banishes_the_cheapest_dragon() {
        use crate::cards::{races, types, CardData};
        const REDMD: u32 = 88264978;
        const PRIME_MATERIAL: u32 = 12298909;
        const MASKED: u32 = 39191307;
        let dragon = |code, attack, level| {
            (code, CardData { code, kind: types::MONSTER | types::EFFECT, attack, level, race: races::DRAGON, ..Default::default() })
        };
        let db: Arc<dyn crate::CardDatabase> =
            Arc::new(MemoryCards([dragon(REDMD, 2800, 10), dragon(PRIME_MATERIAL, 2400, 6), dragon(MASKED, 1400, 3)].into()));
        let mut obs = observation();
        obs.cards = vec![
            card(0, Location::MonsterZone, 0, REDMD, true),
            card(0, Location::MonsterZone, 1, PRIME_MATERIAL, true),
            card(0, Location::MonsterZone, 2, MASKED, true),
            card(0, Location::Hand, 0, REDMD, true),
        ];
        let mut policy = crate::registry::create("draconic-might", db.clone()).unwrap();
        let banish = select_one(
            Hint::Banish,
            (0..3).map(|sequence| toggle(Location::MonsterZone, sequence, obs.cards[sequence as usize].code.unwrap())).collect(),
        );
        assert_eq!(policy.choose(&obs, &banish), 2, "banished more than Masked Dragon");
        // With only the other REDMD to banish, the second one stays in the hand.
        obs.cards = vec![card(0, Location::MonsterZone, 0, REDMD, true), card(0, Location::Hand, 0, REDMD, true)];
        let special = Choice {
            kind: ChoiceKind::SpecialSummon,
            card: Some(Member { at: CardRef { controller: 0, location: Location::Hand, sequence: 0 }, code: Some(REDMD), value: 0, required: false }),
            members: Vec::new(),
            description: 0,
            place: None,
        };
        let idle = Decision {
            kind: DecisionKind::Idle,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices: vec![special, choice(ChoiceKind::EndTurn)],
        };
        let mut policy = crate::registry::create("draconic-might", db).unwrap();
        assert_eq!(idle.choices[policy.choose(&obs, &idle)].kind, ChoiceKind::EndTurn, "banished a REDMD for a REDMD");
    }

    #[test]
    fn registry_ids_are_unique() {
        let mut ids: Vec<_> = crate::registry::POLICIES.iter().map(|e| e.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), crate::registry::POLICIES.len());
    }
}
