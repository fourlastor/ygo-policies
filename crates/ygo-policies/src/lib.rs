//! Rule-based, single-deck Yu-Gi-Oh! policies.
//!
//! * [`model`]: the engine-agnostic observation/decision types a policy sees.
//!   Engine front ends (e.g. `ygo-policies-ocgcore`) build them and are
//!   responsible for hiding everything the seat may not see.
//! * [`agent`]: the [`Policy`] trait, the per-deck [`Strategy`] hooks and the
//!   shared decision loop ([`Agent`]).
//! * [`tactics`] / [`staples`]: deck-independent play and shared staple cards.
//! * [`knowledge`]: facts about particular cards that every deck plays around
//!   (what battle cannot destroy, what effects cannot reach).
//! * [`decks`]: one strategy per deck; [`registry`] builds them by id.

pub mod agent;
pub mod cards;
pub mod ctx;
pub mod decks;
pub mod knowledge;
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

    // Opt the built-in policies into copying without requiring custom Policy
    // or Strategy implementations to implement Clone. Immutable card data is shared.
    #[derive(Clone)]
    struct Forkable<P>(P);

    impl<P: Policy + Clone + 'static> Policy for Forkable<P> {
        fn choose(&mut self, obs: &crate::model::Observation, decision: &crate::model::Decision) -> usize {
            self.0.choose(obs, decision)
        }

        fn fork(&self) -> Option<Box<dyn Policy>> {
            Some(Box::new(self.clone()))
        }
    }

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
                build: |db, seed| Box::new(Forkable(Agent::seeded(decks::$module::$strategy::default(), db, seed))),
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
        entry!("countdown", countdown::Countdown),
        entry!("verdict", verdict::Verdict),
        entry!("exodia", exodia::Exodia),
        entry!("fabled", fabled::Fabled),
        entry!("counter-fairy", counter_fairy::CounterFairy),
        entry!("macro-dd", macro_dd::MacroDd),
        entry!("gusto", gusto::Gusto),
        entry!("agents", agents::Agents),
        entry!("scrap", scrap::Scrap),
        entry!("zombie", zombie::Zombie),
        entry!("herald", herald::Herald),
        entry!("fish", fish::Fish),
        entry!("cyber", cyber::Cyber),
        entry!("gemini", gemini::Gemini),
        entry!("psychic", psychic::Psychic),
        entry!("deckout", deckout::Deckout),
        entry!("chain-burn", chain_burn::ChainBurn),
        entry!("nurse", nurse::Nurse),
        entry!("benkei", benkei::Benkei),
        entry!("alien", alien::Alien),
        entry!("spirit", spirit::Spirit),
        entry!("naturia", naturia::Naturia),
        entry!("garden", garden::Garden),
        entry!("destiny-board", destiny_board::DestinyBoard),
        entry!("venom", venom::Venom),
        entry!("batteryman", batteryman::Batteryman),
        entry!("gem-knight", gem_knight::GemKnight),
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
            summon_used: false,
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
            battles: 0,
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

    fn monster(controller: u8, sequence: u32, code: Option<u32>, position: Position, attack: i32, defense: i32) -> CardView {
        CardView {
            at: CardRef { controller, location: Location::MonsterZone, sequence },
            code,
            position,
            attack,
            defense,
            level: 1,
            can_attack: true,
            battles: 0,
            counters: 0,
            coin_effect: None,
        }
    }

    /// Card facts come from the code the seat can see: the opponent's
    /// face-down monster is just an unknown monster, whatever it really is.
    #[test]
    fn face_down_cards_reveal_no_facts() {
        use crate::knowledge::{Facts, THE_FOOL};
        use crate::agent::Outcome;
        use crate::tactics::default_outcome;
        let db = MemoryCards::default();
        let mut obs = observation();
        let ours = monster(0, 0, Some(1), Position { face_up: true, attack: true }, 1800, 1000);
        // The Fool, Set: the redacted view has no code.
        let hidden = monster(1, 0, None, Position { face_up: false, attack: false }, 0, 0);
        obs.cards = vec![ours.clone(), hidden.clone()];
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        assert_eq!(ctx.facts(&hidden), Facts::NONE);
        assert!(!ctx.battle_proof(&hidden));
        // An unknown face-down monster: an 1800 attacker does not risk it.
        assert_eq!(default_outcome(&ctx, &ours, &hidden, 0), Outcome::Lose);
        // Flipped face-up, it is The Fool: battle cannot destroy it.
        let fool = monster(1, 0, Some(THE_FOOL), Position { face_up: true, attack: false }, 0, 0);
        obs.cards = vec![ours.clone(), fool.clone()];
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        assert!(ctx.battle_proof(&fool));
        assert_eq!(default_outcome(&ctx, &ours, &fool, 0), Outcome::Bounce);
    }

    fn printed(code: u32, kind: u32, attribute: u32) -> (u32, crate::cards::CardData) {
        (code, crate::cards::CardData { code, kind, attribute, ..Default::default() })
    }

    /// What the probes showed decides how an attack is expected to end.
    #[test]
    fn probed_facts_decide_battles() {
        use crate::agent::Outcome;
        use crate::cards::{attributes, types};
        use crate::tactics::default_outcome;
        const CATASTOR: u32 = 26593852;
        const DARK_RESONATOR: u32 = 97021916;
        const NISAMU: u32 = 3846170;
        let (earth, dark) = (1, 2);
        let db = MemoryCards(
            [printed(earth, types::MONSTER, attributes::EARTH), printed(dark, types::MONSTER, attributes::DARK)].into_iter().collect(),
        );
        let attack = Position { face_up: true, attack: true };
        let ours = monster(0, 0, Some(earth), attack, 3000, 1000);
        let ours_dark = monster(0, 1, Some(dark), attack, 3000, 1000);
        // Ally of Justice Catastor destroys the non-DARK monster it battles
        // before damage calculation, whichever of the two attacks.
        let catastor = monster(1, 0, Some(CATASTOR), attack, 2200, 1200);
        let mut resonator = monster(1, 1, Some(DARK_RESONATOR), attack, 1300, 300);
        let nisamu = monster(1, 2, Some(NISAMU), attack, 1400, 200);
        let mut obs = observation();
        obs.cards = vec![ours.clone(), ours_dark.clone(), catastor.clone(), resonator.clone(), nisamu.clone()];
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        assert_eq!(default_outcome(&ctx, &ours, &catastor, 0), Outcome::Lose);
        assert_eq!(default_outcome(&ctx, &ours_dark, &catastor, 0), Outcome::Win { trick: false });
        let our_catastor = monster(0, 2, Some(CATASTOR), attack, 2200, 1200);
        let their_big = monster(1, 3, Some(earth), attack, 3000, 1000);
        assert_eq!(default_outcome(&ctx, &our_catastor, &their_big, 0), Outcome::Win { trick: false });
        // Dark Resonator survives one battle a turn; a Karakuri attacked in
        // Attack Position battles in Defense Position.
        assert_eq!(default_outcome(&ctx, &ours, &resonator, 0), Outcome::Bounce);
        assert!(!ctx.battle_proof(&resonator));
        resonator.battles = 1;
        assert_eq!(default_outcome(&ctx, &ours, &resonator, 0), Outcome::Win { trick: false });
        let met = ctx.attack_meets(Some(&ours), &nisamu);
        assert!(met.defending && met.stat == 200 && met.facts.payoff > 0);
    }

    /// ...and which of our effects are worth using on a card.
    #[test]
    fn probed_facts_decide_what_effects_reach() {
        use crate::cards::types;
        use crate::staples::{BRIONAC, MIRROR_FORCE, SMASHING_GROUND};
        const WHITE_NIGHT_DRAGON: u32 = 79473793;
        const PRIME_MATERIAL_DRAGON: u32 = 12298909;
        const HORUS_LV8: u32 = 48229808;
        const SANGAN: u32 = 26202165;
        let db = MemoryCards(
            [printed(SMASHING_GROUND, types::SPELL, 0), printed(MIRROR_FORCE, types::TRAP, 0), printed(BRIONAC, types::MONSTER, 0)]
                .into_iter()
                .collect(),
        );
        let attack = Position { face_up: true, attack: true };
        let white_night = monster(1, 0, Some(WHITE_NIGHT_DRAGON), attack, 3000, 2500);
        let prime = monster(1, 1, Some(PRIME_MATERIAL_DRAGON), attack, 2400, 2000);
        let sangan = monster(1, 2, Some(SANGAN), attack, 1000, 600);
        let plain = monster(1, 3, Some(1), attack, 1000, 600);
        let mut obs = observation();
        obs.cards = vec![white_night.clone(), prime.clone(), sangan.clone(), plain.clone()];
        obs.pile_sizes = vec![(1, Location::Hand, 2)];
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        // White Night Dragon negates the Spells and Traps that target it:
        // one that does not target, or a monster's effect, still works.
        assert!(!ctx.targetable(&white_night, true) && ctx.targetable(&white_night, false));
        assert!(ctx.reaches(&white_night, SMASHING_GROUND, false, true));
        // Prime Material Dragon discards to negate what would destroy it,
        // not a bounce; with no card in hand it negates nothing.
        assert!(!ctx.reaches(&prime, SMASHING_GROUND, false, true));
        assert!(ctx.reaches(&prime, BRIONAC, true, false));
        assert_eq!(ctx.sweep_worth(&prime), 0);
        // Destroying Sangan by an effect pays its controller back.
        assert!(ctx.sweep_worth(&sangan) < ctx.sweep_worth(&plain));
        // Nothing here negates every Spell...
        assert!(!ctx.wasted(SMASHING_GROUND));
        obs.pile_sizes = vec![(1, Location::Hand, 0)];
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        assert!(ctx.reaches(&prime, SMASHING_GROUND, false, true));
        // ...Horus LV8 does, and only Spells; face-down it is not known to.
        let mut horus = monster(1, 4, Some(HORUS_LV8), attack, 3000, 1800);
        obs.cards.push(horus.clone());
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        assert!(ctx.wasted(SMASHING_GROUND) && !ctx.wasted(MIRROR_FORCE));
        horus.code = None;
        horus.position = Position { face_up: false, attack: false };
        obs.cards.pop();
        obs.cards.push(horus);
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        assert!(!ctx.wasted(SMASHING_GROUND));
    }

    /// A monster that sends its attacker back to the hand, and goes with it,
    /// is still worth attacking with a monster we can Summon again.
    #[test]
    fn a_bounce_is_not_a_card_lost() {
        use crate::agent::Outcome;
        use crate::cards::{attributes, types};
        use crate::tactics::default_outcome;
        const GRAND_MOLE: u32 = 80344569;
        let db = MemoryCards(
            [printed(1, types::MONSTER, attributes::EARTH), printed(GRAND_MOLE, types::MONSTER, attributes::EARTH)].into_iter().collect(),
        );
        let attack = Position { face_up: true, attack: true };
        let ours = monster(0, 0, Some(1), attack, 1800, 1000);
        let mole = monster(1, 0, Some(GRAND_MOLE), attack, 900, 300);
        let mut obs = observation();
        obs.cards = vec![ours.clone(), mole.clone()];
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        // Both go back to the hand before damage calculation...
        assert_eq!(default_outcome(&ctx, &ours, &mole, 0), Outcome::Trade);
        // ...which its own attack on our monster does too: no loss to prevent.
        assert!(!ctx.attack_hurts(&mole, Some(&ours)));
    }

    /// The Fool's coin decides whose targeting effects it negates; until the
    /// coin is known, nobody's are assumed to resolve.
    #[test]
    fn the_fool_coin_decides_who_cannot_target_it() {
        use crate::knowledge::THE_FOOL;
        let db = MemoryCards::default();
        let obs = observation();
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        let mut fool = monster(1, 0, Some(THE_FOOL), Position { face_up: true, attack: true }, 0, 0);
        assert!(!ctx.targetable(&fool, true));
        fool.coin_effect = Some(CoinEffect { code: THE_FOOL, result: Coin::Tails });
        assert!(!ctx.targetable(&fool, false));
        assert!(ctx.targetable_by(&fool, 1, false));
        fool.coin_effect = Some(CoinEffect { code: THE_FOOL, result: Coin::Heads });
        assert!(ctx.targetable(&fool, true));
        assert!(!ctx.targetable_by(&fool, 1, false));
    }

    /// Skill Drain leaves a face-up monster only what it does once it has
    /// left the field.
    #[test]
    fn skill_drain_switches_card_facts_off() {
        use crate::agent::Outcome;
        use crate::cards::{attributes, types};
        use crate::knowledge::SKILL_DRAIN;
        use crate::staples::SMASHING_GROUND;
        use crate::tactics::default_outcome;
        const CATASTOR: u32 = 26593852;
        const HORUS_LV8: u32 = 48229808;
        const SANGAN: u32 = 26202165;
        let db = MemoryCards(
            [printed(1, types::MONSTER, attributes::EARTH), printed(SMASHING_GROUND, types::SPELL, 0), printed(SKILL_DRAIN, types::TRAP, 0)]
                .into_iter()
                .collect(),
        );
        let attack = Position { face_up: true, attack: true };
        let ours = monster(0, 0, Some(1), attack, 3000, 1000);
        let catastor = monster(1, 0, Some(CATASTOR), attack, 2200, 1200);
        let horus = monster(1, 1, Some(HORUS_LV8), attack, 3000, 1800);
        let sangan = monster(1, 2, Some(SANGAN), attack, 1000, 600);
        let plain = monster(1, 3, Some(1), attack, 1000, 600);
        let mut obs = observation();
        obs.cards = vec![ours.clone(), catastor.clone(), horus, sangan.clone(), plain.clone()];
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        assert!(ctx.wasted(SMASHING_GROUND));
        assert_eq!(default_outcome(&ctx, &ours, &catastor, 0), Outcome::Lose);
        // Face-up on either field, Skill Drain makes them plain monsters...
        obs.cards.push(card(1, Location::SpellTrapZone, 0, SKILL_DRAIN, true));
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        assert!(ctx.effects_drained() && !ctx.wasted(SMASHING_GROUND));
        assert_eq!(default_outcome(&ctx, &ours, &catastor, 0), Outcome::Win { trick: false });
        // ...but Sangan still searches from the Graveyard.
        assert!(ctx.sweep_worth(&sangan) < ctx.sweep_worth(&plain));
        // Set, it does nothing yet.
        obs.cards.pop();
        obs.cards.push(card(1, Location::SpellTrapZone, 0, SKILL_DRAIN, false));
        let ctx = crate::ctx::Ctx::new(&obs, &db);
        assert!(!ctx.effects_drained() && ctx.wasted(SMASHING_GROUND));
    }

    /// Claudi-oh's Countdown spends one cover a turn, a Set Trap before a
    /// hand trap, and answers a negation it sees in the chain with the next.
    #[test]
    fn countdown_covers_a_turn_once_unless_negated() {
        use crate::cards::types;
        use crate::staples::SEVEN_TOOLS;
        const WABOKU: u32 = 12607053;
        const SWIFT_SCARECROW: u32 = 18964575;
        const SHI_EN: u32 = 29981921;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                printed(1, types::MONSTER, 0),
                printed(WABOKU, types::TRAP, 0),
                printed(SWIFT_SCARECROW, types::MONSTER | types::EFFECT, 0),
                printed(SEVEN_TOOLS, types::TRAP | types::COUNTER, 0),
                printed(SHI_EN, types::MONSTER | types::EFFECT | types::SYNCHRO, 0),
            ]
            .into_iter()
            .collect(),
        ));
        let prompt = |choices: Vec<Choice>| Decision {
            kind: DecisionKind::Chain { forced: false, triggers: false },
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices,
        };
        let both = prompt(vec![
            activate(SWIFT_SCARECROW, Location::Hand, 0),
            activate(WABOKU, Location::SpellTrapZone, 0),
            choice(ChoiceKind::Pass),
        ]);
        let hand_trap = prompt(vec![activate(SWIFT_SCARECROW, Location::Hand, 0), choice(ChoiceKind::Pass)]);
        let attack = Position { face_up: true, attack: true };
        let waboku = card(0, Location::SpellTrapZone, 0, WABOKU, false);
        let scarecrow = card(0, Location::Hand, 0, SWIFT_SCARECROW, false);
        let direct_attack = |attacker: &CardView, turn: u32| {
            let mut obs = observation();
            obs.turn = turn;
            obs.turn_player = Some(1);
            obs.phase = Some(Phase::BattleStep);
            obs.battle_attacker = Some(attacker.at);
            obs.cards = vec![attacker.clone(), waboku.clone(), scarecrow.clone()];
            obs
        };

        let mut policy = crate::registry::create("countdown", db.clone()).unwrap();
        let attacker = monster(1, 0, Some(1), attack, 2500, 1400);
        let mut obs = direct_attack(&attacker, 3);
        // A direct attack: Waboku, which could be destroyed before it is used.
        assert_eq!(both.choices[policy.choose(&obs, &both)].code(), Some(WABOKU));
        // The turn is covered: Swift Scarecrow stays in hand.
        obs.cards.remove(1);
        assert_eq!(hand_trap.choices[policy.choose(&obs, &hand_trap)].kind, ChoiceKind::Pass);
        // A Counter Trap answers Waboku: the next cover goes on top of it.
        let zone = |controller| CardRef { controller, location: Location::SpellTrapZone, sequence: 0 };
        obs.chain = vec![
            ChainLink { code: WABOKU, controller: 0, source: zone(0), targets: Vec::new() },
            ChainLink { code: SEVEN_TOOLS, controller: 1, source: zone(1), targets: Vec::new() },
        ];
        assert_eq!(hand_trap.choices[policy.choose(&obs, &hand_trap)].code(), Some(SWIFT_SCARECROW));

        // Under Shi En, which negates one Trap a turn, the hand trap goes first.
        let mut policy = crate::registry::create("countdown", db).unwrap();
        let shi_en = monster(1, 0, Some(SHI_EN), attack, 2500, 1400);
        let obs = direct_attack(&shi_en, 5);
        assert_eq!(both.choices[policy.choose(&obs, &both)].code(), Some(SWIFT_SCARECROW));
    }

    /// Claudi-oh's Countdown keeps its covers in hand and Sets one at a time:
    /// removal aimed at the back row then finds one card, not all of them.
    #[test]
    fn countdown_sets_one_cover_at_a_time() {
        use crate::cards::types;
        const WABOKU: u32 = 12607053;
        const THREATENING_ROAR: u32 = 36361633;
        const RAINBOW_LIFE: u32 = 34002992;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [WABOKU, THREATENING_ROAR, RAINBOW_LIFE].into_iter().map(|code| printed(code, types::TRAP, 0)).collect(),
        ));
        let mut policy = crate::registry::create("countdown", db).unwrap();
        let set = |code: u32, sequence: u32| Choice { kind: ChoiceKind::SetSpellTrap, ..activate(code, Location::Hand, sequence) };
        let idle = |choices: Vec<Choice>| Decision {
            kind: DecisionKind::Idle,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices,
        };
        let mut obs = observation();
        obs.turn = 2;
        // Rainbow Life needs a card to discard: Threatening Roar goes first.
        obs.cards = vec![card(0, Location::Hand, 0, RAINBOW_LIFE, false), card(0, Location::Hand, 1, THREATENING_ROAR, false)];
        let both = idle(vec![set(RAINBOW_LIFE, 0), set(THREATENING_ROAR, 1), choice(ChoiceKind::EndTurn)]);
        assert_eq!(both.choices[policy.choose(&obs, &both)].code(), Some(THREATENING_ROAR));
        // With one cover Set, the next stays in hand.
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, THREATENING_ROAR, false), card(0, Location::Hand, 0, WABOKU, false)];
        let another = idle(vec![set(WABOKU, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(another.choices[policy.choose(&obs, &another)].kind, ChoiceKind::EndTurn);
    }

    /// Two turns of a duel Claudi-oh's Verdict lost (Beat Claudi-oh's log,
    /// duel 10), under its own Skill Drain and with no monster of its own.
    #[test]
    fn free_traps_come_before_solemn_ones_and_torrential_waits_for_two() {
        use crate::cards::types;
        use crate::knowledge::SKILL_DRAIN;
        use crate::staples::{SOLEMN_WARNING, TORRENTIAL_TRIBUTE};
        const ABSOLUTE_ZERO: u32 = 40854197;
        const NEOS_ALIUS: u32 = 69884162;
        const ARMORED_BEE: u32 = 86915847;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                printed(SKILL_DRAIN, types::TRAP | types::CONTINUOUS, 0),
                printed(TORRENTIAL_TRIBUTE, types::TRAP, 0),
                printed(SOLEMN_WARNING, types::TRAP | types::COUNTER, 0),
                printed(ABSOLUTE_ZERO, types::MONSTER | types::EFFECT | types::FUSION, 0),
                printed(NEOS_ALIUS, types::MONSTER | types::EFFECT | types::GEMINI, 0),
                printed(ARMORED_BEE, types::MONSTER | types::EFFECT, 0),
            ]
            .into_iter()
            .collect(),
        ));
        let chain = |codes: &[u32]| Decision {
            kind: DecisionKind::Chain { forced: false, triggers: false },
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices: codes
                .iter()
                .enumerate()
                .map(|(sequence, code)| activate(*code, Location::SpellTrapZone, sequence as u32))
                .chain([choice(ChoiceKind::Pass)])
                .collect(),
        };
        let attack = Position { face_up: true, attack: true };
        let their_turn = |life_points: [i32; 2], hand: u32, theirs: Vec<CardView>| {
            let mut obs = observation();
            obs.turn = 8;
            obs.turn_player = Some(1);
            obs.life_points = life_points;
            obs.pile_sizes = vec![(1, Location::Hand, hand)];
            obs.cards = vec![
                card(0, Location::SpellTrapZone, 0, TORRENTIAL_TRIBUTE, false),
                card(0, Location::SpellTrapZone, 1, SOLEMN_WARNING, false),
                card(0, Location::SpellTrapZone, 2, SKILL_DRAIN, true),
            ];
            obs.cards.extend(theirs);
            obs
        };
        let mut policy = crate::registry::create("verdict", db).unwrap();
        let zero = monster(1, 0, Some(ABSOLUTE_ZERO), attack, 2500, 2000);
        let alius = monster(1, 1, Some(NEOS_ALIUS), attack, 1900, 1300);
        let bee = monster(1, 0, Some(ARMORED_BEE), attack, 1600, 1200);

        // Neos Alius is being Normal Summoned next to Absolute Zero.  Solemn
        // Warning would stop it for 2000 Life Points; Torrential Tribute, one
        // window later, destroys both for nothing.
        let negation = chain(&[SOLEMN_WARNING]);
        let mut obs = their_turn([6200, 2200], 1, vec![zero.clone(), alius.clone()]);
        obs.summon_used = true;
        obs.event_cards = vec![(alius.at, alius.code)];
        assert_eq!(negation.choices[policy.choose(&obs, &negation)].kind, ChoiceKind::Pass);
        let summoned = chain(&[TORRENTIAL_TRIBUTE]);
        assert_eq!(summoned.choices[policy.choose(&obs, &summoned)].code(), Some(TORRENTIAL_TRIBUTE));
        // Without Torrential Tribute behind it, Solemn Warning is the answer.
        obs.cards.remove(0);
        assert_eq!(negation.choices[policy.choose(&obs, &negation)].code(), Some(SOLEMN_WARNING));

        // Armored Bee (1600 ATK) is Flip Summoned at 1700 Life Points, their
        // Normal Summon still to come: the monster after it would end the
        // duel if Torrential Tribute went now, and both go if it waits.
        let mut obs = their_turn([1700, 2200], 2, vec![bee.clone()]);
        obs.turn = 12;
        obs.event_cards = vec![(bee.at, bee.code)];
        assert_eq!(summoned.choices[policy.choose(&obs, &summoned)].kind, ChoiceKind::Pass);
        // It does not wait for a Summon that cannot come...
        obs.summon_used = true;
        assert_eq!(summoned.choices[policy.choose(&obs, &summoned)].code(), Some(TORRENTIAL_TRIBUTE));
        // ...nor when what stands already ends the duel.
        obs.summon_used = false;
        obs.life_points = [1600, 2200];
        assert_eq!(summoned.choices[policy.choose(&obs, &summoned)].code(), Some(TORRENTIAL_TRIBUTE));
    }

    /// Solemn Judgment is paid with half our Life Points, whatever they are,
    /// and Solemn Warning with 2000 while that leaves us any: the price is
    /// weighed against what the answer saves, and no floor of Life Points
    /// keeps either Set.  (Draconic Might in Sands of the Duel, starting at
    /// 4000: Judgment stayed Set from turn 2 to the end of the duel, and
    /// Smashing Ground took Red-Eyes Darkness Metal Dragon on turn 3.)
    #[test]
    fn solemn_cards_weigh_their_price() {
        use crate::cards::{types, CardData};
        use crate::staples::{
            HEAVY_STORM, MIRROR_FORCE, MYSTICAL_SPACE_TYPHOON, SMASHING_GROUND, SOLEMN_JUDGMENT, SOLEMN_WARNING,
        };
        const REDMD: u32 = 88264978;
        const BOSS: u32 = 1;
        const BEATER: u32 = 2;
        let body = |code, attack| (code, CardData { code, kind: types::MONSTER | types::EFFECT, attack, level: 4, ..Default::default() });
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                printed(SOLEMN_JUDGMENT, types::TRAP | types::COUNTER, 0),
                printed(SOLEMN_WARNING, types::TRAP | types::COUNTER, 0),
                printed(MIRROR_FORCE, types::TRAP, 0),
                printed(SMASHING_GROUND, types::SPELL, 0),
                printed(HEAVY_STORM, types::SPELL, 0),
                printed(MYSTICAL_SPACE_TYPHOON, types::SPELL | types::QUICKPLAY, 0),
                body(REDMD, 2800),
                body(BOSS, 2800),
                body(BEATER, 1800),
            ]
            .into_iter()
            .collect(),
        ));
        // Their turn: our Set cards from the left, and what else is on the field.
        let board = |life: i32, set: &[u32], rest: Vec<CardView>| {
            let mut obs = observation();
            obs.turn = 3;
            obs.turn_player = Some(1);
            obs.life_points = [life, 8000];
            obs.cards = set.iter().enumerate().map(|(i, code)| card(0, Location::SpellTrapZone, i as u32, *code, false)).collect();
            obs.cards.extend(rest);
            obs
        };
        // The answer of `policy` with those cards Set: the card it activates.
        let answer = |policy: &str, obs: &Observation, set: &[u32]| {
            let window = Decision {
                kind: DecisionKind::Chain { forced: false, triggers: false },
                hint: Hint::None,
                minimum: 0,
                maximum: 0,
                selected: Vec::new(),
                subject: None,
                choices: set
                    .iter()
                    .enumerate()
                    .map(|(i, code)| activate(*code, Location::SpellTrapZone, i as u32))
                    .chain([choice(ChoiceKind::Pass)])
                    .collect(),
            };
            let mut policy = crate::registry::create(policy, db.clone()).unwrap();
            window.choices[policy.choose(obs, &window)].code()
        };
        let attack = Position { face_up: true, attack: true };
        let summon = |life: i32, set: &[u32], code: u32, strength: i32| {
            let summoned = monster(1, 0, Some(code), attack, strength, 0);
            let mut obs = board(life, set, vec![summoned.clone()]);
            obs.event_cards = vec![(summoned.at, summoned.code)];
            obs
        };
        let theirs = CardRef { controller: 1, location: Location::SpellTrapZone, sequence: 0 };
        let spell = |life: i32, set: &[u32], code: u32, targets: Vec<CardRef>, ours: Vec<CardView>| {
            let mut rest = vec![card(1, Location::SpellTrapZone, 0, code, true)];
            rest.extend(ours);
            let mut obs = board(life, set, rest);
            obs.chain = vec![ChainLink { code, controller: 1, source: theirs, targets }];
            obs
        };
        let judgment: &[u32] = &[SOLEMN_JUDGMENT];

        // A 2800 ATK monster is Summoned: negated at 2300 Life Points as at 8000.
        assert_eq!(answer("verdict", &summon(8000, judgment, BOSS, 2800), judgment), Some(SOLEMN_JUDGMENT));
        assert_eq!(answer("verdict", &summon(2300, judgment, BOSS, 2800), judgment), Some(SOLEMN_JUDGMENT));
        // An 1800 ATK monster is not worth 4000 Life Points, and is worth
        // 1800: its one attack would take as much.
        assert_eq!(answer("verdict", &summon(8000, judgment, BEATER, 1800), judgment), None);
        assert_eq!(answer("verdict", &summon(3700, judgment, BEATER, 1800), judgment), None);
        assert_eq!(answer("verdict", &summon(3600, judgment, BEATER, 1800), judgment), Some(SOLEMN_JUDGMENT));

        // Smashing Ground would take Red-Eyes Darkness Metal Dragon: worth
        // 2000 Life Points, not 4000.
        let redmd = || vec![monster(0, 0, Some(REDMD), attack, 2800, 2400)];
        assert_eq!(answer("draconic-might", &spell(4000, judgment, SMASHING_GROUND, vec![], redmd()), judgment), Some(SOLEMN_JUDGMENT));
        assert_eq!(answer("draconic-might", &spell(8000, judgment, SMASHING_GROUND, vec![], redmd()), judgment), None);

        // Heavy Storm takes the Trap itself either way: only what goes with
        // it counts, against the price.
        let backrow: &[u32] = &[SOLEMN_JUDGMENT, MIRROR_FORCE, SOLEMN_WARNING];
        assert_eq!(answer("verdict", &spell(2000, judgment, HEAVY_STORM, vec![], vec![]), judgment), None);
        assert_eq!(answer("verdict", &spell(6000, backrow, HEAVY_STORM, vec![], vec![]), judgment), Some(SOLEMN_JUDGMENT));
        assert_eq!(answer("verdict", &spell(8000, backrow, HEAVY_STORM, vec![], vec![]), judgment), None);
        // One Set card that is worth no more than the Trap is not saved with it.
        let aimed = vec![CardRef { controller: 0, location: Location::SpellTrapZone, sequence: 1 }];
        assert_eq!(answer("verdict", &spell(1000, backrow, MYSTICAL_SPACE_TYPHOON, aimed, vec![]), judgment), None);

        // Next to Solemn Warning, the cheaper of the two goes first.
        let both: &[u32] = &[SOLEMN_JUDGMENT, SOLEMN_WARNING];
        assert_eq!(answer("verdict", &summon(8000, both, BOSS, 2800), both), Some(SOLEMN_WARNING));
        assert_eq!(answer("verdict", &summon(3600, both, BOSS, 2800), both), Some(SOLEMN_JUDGMENT));
        // Solemn Warning alone: paid while it leaves us Life Points.
        let warning: &[u32] = &[SOLEMN_WARNING];
        assert_eq!(answer("verdict", &summon(2500, warning, BOSS, 2800), warning), Some(SOLEMN_WARNING));
        assert_eq!(answer("verdict", &summon(2000, warning, BOSS, 2800), warning), None);
    }

    /// Three rules the search found for Blackwing that hold for every deck,
    /// in the shared code: an attacker is not Set, a face-up Spell or Trap
    /// their deck runs on is destroyed at the first chance, and a Set card
    /// their Spell or Trap is about to destroy is used first.
    #[test]
    fn rules_every_deck_shares() {
        use crate::cards::{types, CardData};
        use crate::staples::{BOOK_OF_MOON, HEAVY_STORM, MYSTICAL_SPACE_TYPHOON, SWORDS_OF_REVEALING_LIGHT};
        const ATTACKER: u32 = 1;
        const WALL: u32 = 2;
        const THEIRS: u32 = 3;
        let body = |code, attack, defense| (code, CardData { code, kind: types::MONSTER | types::EFFECT, attack, defense, level: 4, ..Default::default() });
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                body(ATTACKER, 1700, 1000),
                body(WALL, 1700, 1600),
                body(THEIRS, 2500, 2000),
                printed(BOOK_OF_MOON, types::SPELL | types::QUICKPLAY, 0),
                printed(MYSTICAL_SPACE_TYPHOON, types::SPELL | types::QUICKPLAY, 0),
                printed(HEAVY_STORM, types::SPELL, 0),
                printed(SWORDS_OF_REVEALING_LIGHT, types::SPELL, 0),
            ]
            .into_iter()
            .collect(),
        ));
        let attack = Position { face_up: true, attack: true };
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("verdict", db.clone()).unwrap();
            let choice = &decision.choices[policy.choose(obs, decision)];
            (choice.kind, choice.code())
        };
        let idle = |choices: Vec<Choice>| Decision {
            kind: DecisionKind::Idle,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            choices,
        };
        let window = |codes: &[u32]| Decision {
            kind: DecisionKind::Chain { forced: false, triggers: false },
            choices: codes
                .iter()
                .enumerate()
                .map(|(i, code)| activate(*code, Location::SpellTrapZone, i as u32))
                .chain([choice(ChoiceKind::Pass)])
                .collect(),
            ..idle(Vec::new())
        };

        // Under a bigger monster, 1700 ATK with 1000 DEF is Summoned and
        // 1700 ATK with 1600 DEF is Set.
        let summon = |code: u32| {
            idle(vec![
                Choice { kind: ChoiceKind::NormalSummon, ..activate(code, Location::Hand, 0) },
                Choice { kind: ChoiceKind::SetMonster, ..activate(code, Location::Hand, 0) },
                choice(ChoiceKind::EndTurn),
            ])
        };
        let mut obs = observation();
        obs.turn = 3;
        obs.cards = vec![card(0, Location::Hand, 0, ATTACKER, false), monster(1, 0, Some(THEIRS), attack, 2500, 2000)];
        assert_eq!(chosen(&obs, &summon(ATTACKER)), (ChoiceKind::NormalSummon, Some(ATTACKER)));
        obs.cards[0] = card(0, Location::Hand, 0, WALL, false);
        assert_eq!(chosen(&obs, &summon(WALL)), (ChoiceKind::SetMonster, Some(WALL)));

        // Their Swords of Revealing Light is face-up: our Set Typhoon takes
        // it in their Standby Phase, without waiting for the End Phase.
        let mut obs = observation();
        obs.turn = 4;
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::Standby);
        obs.cards = vec![
            card(0, Location::SpellTrapZone, 0, MYSTICAL_SPACE_TYPHOON, false),
            card(1, Location::SpellTrapZone, 0, SWORDS_OF_REVEALING_LIGHT, true),
        ];
        assert_eq!(chosen(&obs, &window(&[MYSTICAL_SPACE_TYPHOON])).1, Some(MYSTICAL_SPACE_TYPHOON));

        // Their Heavy Storm takes our Set cards: Book of Moon first stops
        // their attacker, and Typhoon alone takes their Set card with it.
        obs.phase = Some(Phase::Main1);
        obs.cards = vec![
            card(0, Location::SpellTrapZone, 0, BOOK_OF_MOON, false),
            card(0, Location::SpellTrapZone, 1, MYSTICAL_SPACE_TYPHOON, false),
            card(1, Location::SpellTrapZone, 0, HEAVY_STORM, true),
            CardView { code: None, ..card(1, Location::SpellTrapZone, 1, 0, false) },
            monster(1, 0, Some(THEIRS), attack, 2500, 2000),
        ];
        let storm = CardRef { controller: 1, location: Location::SpellTrapZone, sequence: 0 };
        obs.chain = vec![ChainLink { code: HEAVY_STORM, controller: 1, source: storm, targets: Vec::new() }];
        assert_eq!(chosen(&obs, &window(&[BOOK_OF_MOON, MYSTICAL_SPACE_TYPHOON])).1, Some(BOOK_OF_MOON));
        let typhoon = Decision {
            choices: vec![activate(MYSTICAL_SPACE_TYPHOON, Location::SpellTrapZone, 1), choice(ChoiceKind::Pass)],
            ..window(&[])
        };
        assert_eq!(chosen(&obs, &typhoon).1, Some(MYSTICAL_SPACE_TYPHOON));
    }

    fn creature(code: u32, level: u32, attack: i32, defense: i32, set: u16) -> (u32, crate::cards::CardData) {
        use crate::cards::types;
        let kind = types::MONSTER | types::EFFECT;
        (code, crate::cards::CardData { code, kind, level, attack, defense, setcodes: vec![set], ..Default::default() })
    }

    fn decide(kind: DecisionKind, subject: Option<u32>, choices: Vec<Choice>) -> Decision {
        Decision { kind, hint: Hint::None, minimum: 0, maximum: 0, selected: Vec::new(), subject, choices }
    }

    fn from_hand(kind: ChoiceKind, code: u32, sequence: u32) -> Choice {
        Choice { kind, ..activate(code, Location::Hand, sequence) }
    }

    /// What a one-step search on top of the Blackwing pilot kept doing
    /// otherwise (`policy-bench search`), now the pilot's own rules.
    #[test]
    fn blackwing_plays_what_the_search_found() {
        use crate::cards::types;
        use crate::staples::{DARK_HOLE, MONSTER_REBORN};
        const SIROCCO: u32 = 75498415;
        const SHURA: u32 = 58820853;
        const BORA: u32 = 49003716;
        const JIN: u32 = 38562933;
        const BLACKWING: u16 = 0x33;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                creature(SIROCCO, 5, 2000, 900, BLACKWING),
                creature(SHURA, 4, 1800, 1200, BLACKWING),
                creature(BORA, 4, 1700, 800, BLACKWING),
                creature(JIN, 1, 600, 500, BLACKWING),
                creature(1, 4, 0, 0, 0),
                printed(DARK_HOLE, types::SPELL, 0),
                printed(MONSTER_REBORN, types::SPELL, 0),
            ]
            .into_iter()
            .collect(),
        ));
        let attack = Position { face_up: true, attack: true };
        let their = |attack_points: i32| monster(1, 0, Some(1), attack, attack_points, 1000);
        let summon = |code: u32| {
            vec![
                from_hand(ChoiceKind::NormalSummon, code, 0),
                from_hand(ChoiceKind::SetMonster, code, 0),
                choice(ChoiceKind::EndTurn),
            ]
        };
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("blackwing", db.clone()).unwrap();
            let choice = &decision.choices[policy.choose(obs, decision)];
            (choice.kind, choice.code())
        };

        // An attacker is Summoned face-up under a bigger monster; a small
        // Blackwing is still Set.
        let mut obs = observation();
        obs.turn = 3;
        obs.cards = vec![card(0, Location::Hand, 0, SHURA, false), their(2500)];
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summon(SHURA))), (ChoiceKind::NormalSummon, Some(SHURA)));
        obs.cards = vec![card(0, Location::Hand, 0, JIN, false), their(2500)];
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summon(JIN))), (ChoiceKind::SetMonster, Some(JIN)));

        // Sirocco needs no Tribute while our field is empty and theirs is
        // not: it comes down before Monster Reborn fills ours.
        obs.cards = vec![
            card(0, Location::Hand, 0, SIROCCO, false),
            card(0, Location::Hand, 1, MONSTER_REBORN, false),
            card(0, Location::Graveyard, 0, SHURA, true),
            their(2500),
        ];
        let mut choices = vec![activate(MONSTER_REBORN, Location::Hand, 1)];
        choices.extend(summon(SIROCCO));
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, choices)), (ChoiceKind::NormalSummon, Some(SIROCCO)));

        // Their one monster is smaller than what we Summon: Dark Hole stays
        // in hand.  Against a bigger one it is played.
        let dark_hole = || {
            let mut choices = vec![activate(DARK_HOLE, Location::Hand, 1)];
            choices.extend(summon(SHURA));
            decide(DecisionKind::Idle, None, choices)
        };
        let hand = [card(0, Location::Hand, 0, SHURA, false), card(0, Location::Hand, 1, DARK_HOLE, false)];
        obs.cards = hand.iter().cloned().chain([their(1500)]).collect();
        assert_eq!(chosen(&obs, &dark_hole()), (ChoiceKind::NormalSummon, Some(SHURA)));
        obs.cards = hand.iter().cloned().chain([their(2500)]).collect();
        assert_eq!(chosen(&obs, &dark_hole()), (ChoiceKind::Activate, Some(DARK_HOLE)));

        // Sirocco for a Tribute when its effect then gets one attacker over
        // their wall (2000 + Shura's 1800 against 3000), and not otherwise.
        let board = |wall: i32| {
            let mut obs = observation();
            obs.turn = 5;
            obs.cards = vec![
                card(0, Location::Hand, 0, SIROCCO, false),
                monster(0, 0, Some(BORA), attack, 1700, 800),
                monster(0, 1, Some(SHURA), attack, 1800, 1200),
                their(wall),
            ];
            obs
        };
        let mut choices = summon(SIROCCO);
        choices.insert(2, choice(ChoiceKind::EnterBattle));
        let tribute = decide(DecisionKind::Idle, None, choices);
        assert_eq!(chosen(&board(3000), &tribute), (ChoiceKind::NormalSummon, Some(SIROCCO)));
        assert_eq!(chosen(&board(1500), &tribute).0, ChoiceKind::EnterBattle);

        // Bora Special Summoned before the Battle Phase stands in Attack
        // Position, whatever they control.
        let position = decide(
            DecisionKind::Position,
            Some(BORA),
            vec![choice(ChoiceKind::Position(Position::FACE_UP_ATTACK)), choice(ChoiceKind::Position(Position::FACE_UP_DEFENSE))],
        );
        assert_eq!(chosen(&board(3000), &position).0, ChoiceKind::Position(Position::FACE_UP_ATTACK));
    }

    /// The same for the Monarch pilot.
    #[test]
    fn monarch_plays_what_the_search_found() {
        use crate::cards::types;
        use crate::staples::POT_OF_DUALITY;
        const THESTALOS: u32 = 26205777;
        const TREEBORN_FROG: u32 = 12538374;
        const BATTLE_FADER: u32 = 19665973;
        const SOUL_EXCHANGE: u32 = 68005187;
        const SPY: u32 = 24317029;
        const SWAP_FROG: u32 = 9126351;
        const ONE_FOR_ONE: u32 = 2295440;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                creature(THESTALOS, 6, 2400, 1000, 0),
                creature(TREEBORN_FROG, 1, 100, 100, 0),
                creature(BATTLE_FADER, 1, 0, 0, 0),
                creature(SPY, 4, 1200, 2000, 0),
                creature(SWAP_FROG, 2, 1000, 500, 0),
                creature(1, 4, 0, 0, 0),
                printed(SOUL_EXCHANGE, types::SPELL, 0),
                printed(ONE_FOR_ONE, types::SPELL, 0),
                printed(POT_OF_DUALITY, types::SPELL, 0),
            ]
            .into_iter()
            .collect(),
        ));
        let attack = Position { face_up: true, attack: true };
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("monarch", db.clone()).unwrap();
            let choice = &decision.choices[policy.choose(obs, decision)];
            (choice.kind, choice.code())
        };

        // With a Tribute of our own on the field, Soul Exchange still takes
        // theirs: the Monarch then removes a second card.
        let mut obs = observation();
        obs.turn = 3;
        obs.cards = vec![
            card(0, Location::Hand, 0, THESTALOS, false),
            card(0, Location::Hand, 1, SOUL_EXCHANGE, false),
            monster(0, 0, Some(TREEBORN_FROG), Position::FACE_UP_DEFENSE, 100, 100),
            monster(1, 0, Some(1), attack, 2800, 2000),
        ];
        let tribute = decide(
            DecisionKind::Idle,
            None,
            vec![
                from_hand(ChoiceKind::NormalSummon, THESTALOS, 0),
                activate(SOUL_EXCHANGE, Location::Hand, 1),
                choice(ChoiceKind::EndTurn),
            ],
        );
        assert_eq!(chosen(&obs, &tribute), (ChoiceKind::Activate, Some(SOUL_EXCHANGE)));

        // Battle Fader is a hand trap: it is never Set.
        obs.cards = vec![card(0, Location::Hand, 0, BATTLE_FADER, false)];
        let fader = decide(
            DecisionKind::Idle,
            None,
            vec![
                from_hand(ChoiceKind::NormalSummon, BATTLE_FADER, 0),
                from_hand(ChoiceKind::SetMonster, BATTLE_FADER, 0),
                choice(ChoiceKind::EndTurn),
            ],
        );
        assert_eq!(chosen(&obs, &fader).0, ChoiceKind::EndTurn);

        // Treeborn Frog's revival would keep Pot of Duality in hand for the
        // turn: declined, unless a Monarch waits for the Tribute.
        let revive = decide(DecisionKind::YesNo, Some(TREEBORN_FROG), vec![choice(ChoiceKind::Yes), choice(ChoiceKind::No)]);
        obs.phase = Some(Phase::Standby);
        obs.cards = vec![card(0, Location::Hand, 0, POT_OF_DUALITY, false), card(0, Location::Graveyard, 0, TREEBORN_FROG, true)];
        assert_eq!(chosen(&obs, &revive).0, ChoiceKind::No);
        obs.cards.push(card(0, Location::Hand, 1, THESTALOS, false));
        assert_eq!(chosen(&obs, &revive).0, ChoiceKind::Yes);

        // A Set Gravekeeper's Spy is Flip Summoned before the Monarch takes
        // its Tribute: its effect brings a second Spy.
        let mut obs = observation();
        obs.turn = 3;
        obs.cards = vec![
            card(0, Location::Hand, 0, THESTALOS, false),
            monster(0, 0, Some(SPY), Position { face_up: false, attack: false }, 1200, 2000),
            monster(1, 0, Some(1), attack, 1500, 1000),
        ];
        let flip = Choice { kind: ChoiceKind::ChangePosition, ..activate(SPY, Location::MonsterZone, 0) };
        let summon = decide(
            DecisionKind::Idle,
            None,
            vec![from_hand(ChoiceKind::NormalSummon, THESTALOS, 0), flip, choice(ChoiceKind::EndTurn)],
        );
        assert_eq!(chosen(&obs, &summon), (ChoiceKind::ChangePosition, Some(SPY)));

        // Only a Monarch in hand and no monster of ours: no Summon is offered
        // yet, and One for One makes the Tribute.  Not once the Normal Summon
        // is used.
        obs.cards = vec![
            card(0, Location::Hand, 0, THESTALOS, false),
            card(0, Location::Hand, 1, ONE_FOR_ONE, false),
            monster(1, 0, Some(1), attack, 1500, 1000),
        ];
        let fodder = decide(DecisionKind::Idle, None, vec![activate(ONE_FOR_ONE, Location::Hand, 1), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &fodder), (ChoiceKind::Activate, Some(ONE_FOR_ONE)));
        obs.summon_used = true;
        assert_eq!(chosen(&obs, &fodder).0, ChoiceKind::EndTurn);

        // No Monarch in hand: Swap Frog returns the one on the field, to be
        // Tribute Summoned again for its effect (a card of their hand).
        let mut obs = observation();
        obs.turn = 5;
        obs.pile_sizes = vec![(1, Location::Hand, 3)];
        obs.cards = vec![
            monster(0, 0, Some(THESTALOS), attack, 2400, 1000),
            monster(0, 1, Some(SWAP_FROG), attack, 1000, 500),
            monster(1, 0, Some(1), attack, 1500, 1000),
        ];
        let again = decide(
            DecisionKind::Idle,
            None,
            vec![activate(SWAP_FROG, Location::MonsterZone, 1), choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)],
        );
        assert_eq!(chosen(&obs, &again), (ChoiceKind::Activate, Some(SWAP_FROG)));
        obs.summon_used = true;
        assert_ne!(chosen(&obs, &again).0, ChoiceKind::Activate);
    }

    /// A Set card of Blackwing's that their Spell or Trap is about to
    /// destroy is chained while it still can be: Icarus Attack takes two of
    /// their cards with it, Threatening Roar still stops this turn's attacks.
    #[test]
    fn blackwing_uses_a_set_card_before_it_is_destroyed() {
        use crate::cards::types;
        use crate::staples::{HEAVY_STORM, MYSTICAL_SPACE_TYPHOON, THREATENING_ROAR};
        const ICARUS_ATTACK: u32 = 53567095;
        const SHURA: u32 = 58820853;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                creature(SHURA, 4, 1800, 1200, 0x33),
                creature(1, 4, 1900, 1000, 0),
                printed(ICARUS_ATTACK, types::TRAP, 0),
                printed(THREATENING_ROAR, types::TRAP, 0),
                printed(MYSTICAL_SPACE_TYPHOON, types::SPELL | types::QUICKPLAY, 0),
                printed(HEAVY_STORM, types::SPELL, 0),
            ]
            .into_iter()
            .collect(),
        ));
        let attack = Position { face_up: true, attack: true };
        let icarus = CardRef { controller: 0, location: Location::SpellTrapZone, sequence: 0 };
        let roar = CardRef { controller: 0, location: Location::SpellTrapZone, sequence: 1 };
        let theirs = CardRef { controller: 1, location: Location::SpellTrapZone, sequence: 0 };
        // Their turn: they activate `code` at `targets`, with a monster and a Set card of their own.
        let board = |phase: Phase, code: u32, targets: Vec<CardRef>| {
            let mut obs = observation();
            obs.turn = 4;
            obs.turn_player = Some(1);
            obs.phase = Some(phase);
            obs.cards = vec![
                card(0, Location::SpellTrapZone, 0, ICARUS_ATTACK, false),
                card(0, Location::SpellTrapZone, 1, THREATENING_ROAR, false),
                monster(0, 0, Some(SHURA), attack, 1800, 1200),
                monster(1, 0, Some(1), attack, 1900, 1000),
                card(1, Location::SpellTrapZone, 0, code, true),
                CardView { code: None, ..card(1, Location::SpellTrapZone, 1, 0, false) },
            ];
            obs.chain = vec![ChainLink { code, controller: 1, source: theirs, targets }];
            obs
        };
        let window = decide(
            DecisionKind::Chain { forced: false, triggers: false },
            None,
            vec![
                activate(ICARUS_ATTACK, Location::SpellTrapZone, 0),
                activate(THREATENING_ROAR, Location::SpellTrapZone, 1),
                choice(ChoiceKind::Pass),
            ],
        );
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("blackwing", db.clone()).unwrap();
            window.choices[policy.choose(obs, &window)].code()
        };
        // Their Typhoon at Icarus Attack: it goes now, at two cards of theirs.
        assert_eq!(chosen(&board(Phase::Main1, MYSTICAL_SPACE_TYPHOON, vec![icarus])), Some(ICARUS_ATTACK));
        // At Threatening Roar, before their attacks: it still stops them.
        assert_eq!(chosen(&board(Phase::Main1, MYSTICAL_SPACE_TYPHOON, vec![roar])), Some(THREATENING_ROAR));
        // In their End Phase it has nothing left to stop.
        assert_eq!(chosen(&board(Phase::End, MYSTICAL_SPACE_TYPHOON, vec![roar])), None);
        // Heavy Storm takes both: Icarus Attack first.
        assert_eq!(chosen(&board(Phase::Main1, HEAVY_STORM, vec![])), Some(ICARUS_ATTACK));
    }

    /// Claudi-oh's Verdict Summons Beast King Barbaros without Tributes
    /// when the engine asks which way.
    #[test]
    fn verdict_summons_barbaros_without_tributes() {
        const BARBAROS: u64 = 78651105;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards::default());
        let mut policy = crate::registry::create("verdict", db).unwrap();
        let option = |description: u64| Choice { description, ..choice(ChoiceKind::Option) };
        let decision = Decision {
            kind: DecisionKind::Option,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: Vec::new(),
            subject: None,
            // 1: the ordinary Tribute Summon.
            choices: vec![option(1), option(BARBAROS << 20)],
        };
        assert_eq!(policy.choose(&observation(), &decision), 1);
    }

    /// A Quick-Play Spell answers their turn only when Set.  Most decks kept
    /// Book of Moon, Shrink and Enemy Controller in hand: their own rule said
    /// "Traps", and the shared default (Traps and Quick-Play Spells) never ran.
    #[test]
    fn quick_play_spells_are_set() {
        use crate::cards::types;
        use crate::staples::{BOOK_OF_MOON, ENEMY_CONTROLLER, MYSTICAL_SPACE_TYPHOON, SHRINK};
        const D_TIME: u32 = 99075257;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [BOOK_OF_MOON, ENEMY_CONTROLLER, MYSTICAL_SPACE_TYPHOON, SHRINK]
                .into_iter()
                .map(|code| printed(code, types::SPELL | types::QUICKPLAY, 0))
                .chain([printed(D_TIME, types::TRAP, 0)])
                .collect(),
        ));
        let chosen = |id: &str, obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create(id, db.clone()).unwrap();
            decision.choices[policy.choose(obs, decision)].kind
        };
        let set = |code: u32| decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SetSpellTrap, code, 0), choice(ChoiceKind::EndTurn)]);
        let mut obs = observation();
        obs.phase = Some(Phase::Main2);
        for (id, code) in [
            ("fortune-lady", BOOK_OF_MOON),
            ("fortune-lady", SHRINK),
            ("destiny-hero", BOOK_OF_MOON),
            ("gladiator", SHRINK),
            ("karakuri", ENEMY_CONTROLLER),
            ("tele-dad", ENEMY_CONTROLLER),
            ("machina", MYSTICAL_SPACE_TYPHOON),
            ("x-saber", BOOK_OF_MOON),
        ] {
            obs.cards = vec![card(0, Location::Hand, 0, code, false)];
            assert_eq!(chosen(id, &obs, &set(code)), ChoiceKind::SetSpellTrap, "{id} {code}");
        }
        // A deck still says no where it has a reason: D - Time has no
        // Elemental HERO to work with.
        obs.cards = vec![card(0, Location::Hand, 0, D_TIME, false)];
        assert_eq!(chosen("destiny-hero", &obs, &set(D_TIME)), ChoiceKind::EndTurn);
    }

    /// Direct attacks go weakest first, and from the other side an answer
    /// to one attacker waits for the strongest.
    #[test]
    fn the_weakest_attacks_first_and_the_answer_waits_for_the_strongest() {
        use crate::cards::types;
        use crate::staples::{DIMENSIONAL_PRISON, DUST_TORNADO};
        const NECROVALLEY: u32 = 47355498;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                printed(DIMENSIONAL_PRISON, types::TRAP, 0),
                printed(DUST_TORNADO, types::TRAP, 0),
                printed(NECROVALLEY, types::SPELL | types::FIELD, 0),
                creature(1, 4, 2800, 2400, 0),
                creature(2, 4, 1700, 1300, 0),
            ]
            .into_iter()
            .collect(),
        ));
        let chosen = |id: &str, obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create(id, db.clone()).unwrap();
            decision.choices[policy.choose(obs, decision)].clone()
        };
        let attacking = Position { face_up: true, attack: true };

        // 2800 and 1700 ATK on an open field: the 1700 attacks first, and
        // whatever it brings out the 2800 can still meet.  When one attack
        // ends the duel, that one goes.
        let mut obs = observation();
        obs.turn = 3;
        obs.phase = Some(Phase::BattleStep);
        obs.cards = vec![monster(0, 0, Some(1), attacking, 2800, 2400), monster(0, 1, Some(2), attacking, 1700, 1300)];
        let battle = decide(DecisionKind::Battle, None, vec![attack(1, 0, true), attack(2, 1, true), choice(ChoiceKind::EnterMain2)]);
        assert_eq!(chosen("draconic-might", &obs, &battle).code(), Some(2));
        obs.life_points = [8000, 2500];
        assert_eq!(chosen("draconic-might", &obs, &battle).code(), Some(1));

        // Their 1700 attacks first, their 2800 can still attack: Dimensional
        // Prison waits for the 2800.
        let window = |code: u32| {
            decide(DecisionKind::Chain { forced: false, triggers: false }, None, vec![activate(code, Location::SpellTrapZone, 0), choice(ChoiceKind::Pass)])
        };
        let mut obs = observation();
        obs.turn = 4;
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::BattleStep);
        obs.cards = vec![
            card(0, Location::SpellTrapZone, 0, DIMENSIONAL_PRISON, false),
            monster(1, 0, Some(2), attacking, 1700, 1300),
            monster(1, 1, Some(1), attacking, 2800, 2400),
        ];
        let their = |sequence: u32| Some(CardRef { controller: 1, location: Location::MonsterZone, sequence });
        obs.battle_attacker = their(0);
        assert_eq!(chosen("fortune-lady", &obs, &window(DIMENSIONAL_PRISON)).kind, ChoiceKind::Pass);
        obs.battle_attacker = their(1);
        assert_eq!(chosen("fortune-lady", &obs, &window(DIMENSIONAL_PRISON)).code(), Some(DIMENSIONAL_PRISON));
        // An attack that ends the duel is stopped, whoever follows.
        obs.battle_attacker = their(0);
        obs.life_points = [1500, 8000];
        assert_eq!(chosen("fortune-lady", &obs, &window(DIMENSIONAL_PRISON)).code(), Some(DIMENSIONAL_PRISON));

        // Dust Tornado takes a face-up Spell their deck runs on at the first
        // chance, as Typhoon does.
        let mut obs = observation();
        obs.turn = 4;
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::Standby);
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, DUST_TORNADO, false), card(1, Location::SpellTrapZone, 5, NECROVALLEY, true)];
        assert_eq!(chosen("fortune-lady", &obs, &window(DUST_TORNADO)).code(), Some(DUST_TORNADO));
    }

    /// Infernity plays toward an empty hand (`benchmarks/game-run-rules.md`):
    /// what a recorded duel of the game showed, and what a search on top of
    /// the pilot kept doing otherwise.
    #[test]
    fn infernity_plays_toward_an_empty_hand() {
        use crate::cards::types;
        use crate::knowledge::SKILL_DRAIN;
        const ARCHFIEND: u32 = 99177923;
        const BEETLE: u32 = 49080532;
        const MIRAGE: u32 = 86197239;
        const AVENGER: u32 = 85475641;
        const PLAGUESPREADER: u32 = 33420078;
        const LAUNCHER: u32 = 66957584;
        const GUARDIAN: u32 = 51566770;
        const NECROMANCER: u32 = 56209279;
        const STYGIAN_PATROL: u32 = 13521194;
        const INFERNITY_FORCE: u32 = 18712704;
        const CATASTOR: u32 = 26593852;
        const INFERNITY: u16 = 0xb;
        let tuner = |card: (u32, crate::cards::CardData)| (card.0, crate::cards::CardData { kind: card.1.kind | types::TUNER, ..card.1 });
        let synchro = |card: (u32, crate::cards::CardData)| (card.0, crate::cards::CardData { kind: card.1.kind | types::SYNCHRO, ..card.1 });
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                creature(ARCHFIEND, 4, 1800, 1200, INFERNITY),
                tuner(creature(BEETLE, 2, 1200, 0, INFERNITY)),
                creature(MIRAGE, 1, 0, 0, INFERNITY),
                tuner(creature(AVENGER, 1, 0, 0, INFERNITY)),
                tuner(creature(PLAGUESPREADER, 2, 400, 200, 0)),
                creature(GUARDIAN, 4, 1200, 1700, INFERNITY),
                creature(NECROMANCER, 3, 0, 2000, INFERNITY),
                creature(STYGIAN_PATROL, 4, 1600, 1200, 0),
                synchro(creature(CATASTOR, 5, 2200, 1200, 0)),
                creature(1, 4, 1900, 1000, 0),
                printed(LAUNCHER, types::SPELL | types::CONTINUOUS, 0),
                printed(INFERNITY_FORCE, types::TRAP, 0),
                printed(SKILL_DRAIN, types::TRAP | types::CONTINUOUS, 0),
            ]
            .into_iter()
            .collect(),
        ));
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("infernity", db.clone()).unwrap();
            let choice = &decision.choices[policy.choose(obs, decision)];
            (choice.kind, choice.code())
        };
        let hand = |codes: &[u32]| -> Vec<CardView> { codes.iter().enumerate().map(|(i, code)| card(0, Location::Hand, i as u32, *code, false)).collect() };
        let graveyard = |codes: &[u32]| -> Vec<CardView> { codes.iter().enumerate().map(|(i, code)| card(0, Location::Graveyard, i as u32, *code, true)).collect() };
        let summons = |codes: &[u32]| -> Vec<Choice> {
            codes
                .iter()
                .enumerate()
                .flat_map(|(i, code)| [from_hand(ChoiceKind::NormalSummon, *code, i as u32), from_hand(ChoiceKind::SetMonster, *code, i as u32)])
                .chain([choice(ChoiceKind::EndTurn)])
                .collect()
        };
        let launcher = card(0, Location::SpellTrapZone, 0, LAUNCHER, true);
        let mut obs = observation();
        obs.turn = 8;
        obs.life_points = [1600, 8000];

        // The recorded turn: Launcher on the field, Avenger and Plaguespreader
        // Zombie in hand, two Beetles in the Graveyard.  Launcher sends Avenger
        // away, the Zombie takes the Normal Summon face-up, and the hand is
        // empty for Launcher to bring the Beetles back.
        obs.cards = [hand(&[AVENGER, PLAGUESPREADER]), graveyard(&[BEETLE, BEETLE]), vec![launcher.clone()]].concat();
        let mut choices = vec![activate(LAUNCHER, Location::SpellTrapZone, 0)];
        choices.extend(summons(&[AVENGER, PLAGUESPREADER]));
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, choices)), (ChoiceKind::Activate, Some(LAUNCHER)));
        obs.cards = [hand(&[PLAGUESPREADER]), graveyard(&[BEETLE, BEETLE, AVENGER]), vec![launcher]].concat();
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summons(&[PLAGUESPREADER]))), (ChoiceKind::NormalSummon, Some(PLAGUESPREADER)));

        // Avenger alone, nothing to follow: face-down, not 0 ATK in Attack Position.
        obs.cards = hand(&[AVENGER]);
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summons(&[AVENGER]))), (ChoiceKind::SetMonster, Some(AVENGER)));

        // Mirage needs an empty hand: alone it is Summoned for the two in the
        // Graveyard; beside Archfiend, Archfiend takes the Normal Summon.
        obs.cards = [hand(&[MIRAGE]), graveyard(&[BEETLE, ARCHFIEND])].concat();
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summons(&[MIRAGE]))), (ChoiceKind::NormalSummon, Some(MIRAGE)));
        obs.cards = [hand(&[MIRAGE, ARCHFIEND]), graveyard(&[BEETLE, ARCHFIEND])].concat();
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summons(&[MIRAGE, ARCHFIEND]))), (ChoiceKind::NormalSummon, Some(ARCHFIEND)));

        // What a search on top of the pilot kept doing otherwise.
        // Stygian Street Patrol in the Graveyard puts Mirage on the field
        // without the Normal Summon.
        obs.cards = [hand(&[MIRAGE]), graveyard(&[STYGIAN_PATROL, BEETLE, ARCHFIEND])].concat();
        let mut choices = vec![activate(STYGIAN_PATROL, Location::Graveyard, 0)];
        choices.extend(summons(&[MIRAGE]));
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, choices)), (ChoiceKind::Activate, Some(STYGIAN_PATROL)));

        // A small Tuner beside a non-Tuner of ours is Summoned for the
        // Synchro Summon, whatever stays in hand.
        let field = |code: u32| card(0, Location::MonsterZone, 0, code, true);
        obs.cards = [hand(&[MIRAGE, AVENGER]), vec![field(GUARDIAN), card(0, Location::Extra, 0, CATASTOR, false)]].concat();
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summons(&[MIRAGE, AVENGER]))), (ChoiceKind::NormalSummon, Some(AVENGER)));

        // Guardian under a stronger monster goes face-down.
        obs.cards = hand(&[GUARDIAN]);
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summons(&[GUARDIAN]))), (ChoiceKind::NormalSummon, Some(GUARDIAN)));
        obs.cards.push(card(1, Location::MonsterZone, 0, 1, true));
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summons(&[GUARDIAN]))), (ChoiceKind::SetMonster, Some(GUARDIAN)));

        // Launcher is given for two monsters, not for one.
        let revive = decide(DecisionKind::Idle, None, vec![activate(LAUNCHER, Location::SpellTrapZone, 0), choice(ChoiceKind::EndTurn)]);
        let launcher = card(0, Location::SpellTrapZone, 0, LAUNCHER, true);
        obs.cards = [graveyard(&[BEETLE]), vec![launcher.clone()]].concat();
        assert_eq!(chosen(&obs, &revive).0, ChoiceKind::EndTurn);
        obs.cards = [graveyard(&[BEETLE, ARCHFIEND]), vec![launcher]].concat();
        assert_eq!(chosen(&obs, &revive), (ChoiceKind::Activate, Some(LAUNCHER)));

        // Archfiend's search, the Normal Summon spent: Infernity Force can be
        // Set at once, a monster would stay in hand.  With nothing of the
        // kind left in the Deck the effect is declined.
        let ask = |kind: ChoiceKind| Choice { kind, ..activate(ARCHFIEND, Location::MonsterZone, 0) };
        let search = decide(DecisionKind::YesNo, Some(ARCHFIEND), vec![ask(ChoiceKind::Yes), ask(ChoiceKind::No)]);
        obs.summon_used = true;
        obs.cards = [vec![field(ARCHFIEND)], graveyard(&[LAUNCHER, INFERNITY_FORCE])].concat();
        let mut policy = crate::registry::create("infernity", db.clone()).unwrap();
        assert_eq!(search.choices[policy.choose(&obs, &search)].kind, ChoiceKind::Yes);
        let pick = select_one(Hint::AddToHand, vec![toggle(Location::Deck, 0, MIRAGE), toggle(Location::Deck, 1, INFERNITY_FORCE)]);
        assert_eq!(pick.choices[policy.choose(&obs, &pick)].code(), Some(INFERNITY_FORCE));
        obs.cards = [vec![field(ARCHFIEND)], graveyard(&[LAUNCHER, INFERNITY_FORCE, INFERNITY_FORCE])].concat();
        assert_eq!(chosen(&obs, &search).0, ChoiceKind::No);

        // What a second search showed, on the pilot with all of the above.
        // Beetle with another monster left in hand has no effect: an attacker
        // takes the Normal Summon, or Beetle waits face-down.  As the last
        // card it is Summoned.
        obs.summon_used = false;
        obs.cards = hand(&[BEETLE, ARCHFIEND, MIRAGE]);
        let three = decide(DecisionKind::Idle, None, summons(&[BEETLE, ARCHFIEND, MIRAGE]));
        assert_eq!(chosen(&obs, &three), (ChoiceKind::NormalSummon, Some(ARCHFIEND)));
        obs.cards = hand(&[BEETLE, MIRAGE]);
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summons(&[BEETLE, MIRAGE]))), (ChoiceKind::SetMonster, Some(BEETLE)));
        obs.cards = hand(&[BEETLE]);
        assert_eq!(chosen(&obs, &decide(DecisionKind::Idle, None, summons(&[BEETLE]))), (ChoiceKind::NormalSummon, Some(BEETLE)));

        // Skill Drain leaves a Summoned Necromancer in Attack Position with
        // 0 ATK: it is Set.
        obs.cards = [hand(&[NECROMANCER]), graveyard(&[BEETLE])].concat();
        let necromancer = decide(DecisionKind::Idle, None, summons(&[NECROMANCER]));
        assert_eq!(chosen(&obs, &necromancer), (ChoiceKind::NormalSummon, Some(NECROMANCER)));
        obs.cards.push(card(1, Location::SpellTrapZone, 0, SKILL_DRAIN, true));
        assert_eq!(chosen(&obs, &necromancer), (ChoiceKind::SetMonster, Some(NECROMANCER)));

        // A face-down Guardian turns face-up once the hand is empty: nothing
        // destroys it then, and Infernity Force answers an attack on it.
        let face_down = CardView { position: Position { face_up: false, attack: false }, ..field(GUARDIAN) };
        let flip = Choice { kind: ChoiceKind::ChangePosition, ..activate(GUARDIAN, Location::MonsterZone, 0) };
        let turn_up = decide(DecisionKind::Idle, None, vec![flip, choice(ChoiceKind::EndTurn)]);
        obs.cards = vec![face_down.clone()];
        assert_eq!(chosen(&obs, &turn_up), (ChoiceKind::ChangePosition, Some(GUARDIAN)));
        obs.cards = [vec![face_down], hand(&[MIRAGE])].concat();
        assert_eq!(chosen(&obs, &turn_up).0, ChoiceKind::EndTurn);
    }

    /// Forbidden Chalice (`benchmarks/game-run-rules.md`): two decks held it
    /// and never played it.  It negates the effect a monster of theirs
    /// activates on the field, and its 400 ATK turn a battle of ours around.
    #[test]
    fn forbidden_chalice_is_played() {
        use crate::cards::types;
        use crate::staples::FORBIDDEN_CHALICE;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                printed(FORBIDDEN_CHALICE, types::SPELL | types::QUICKPLAY, 0),
                creature(1, 4, 1800, 1200, 0),
                creature(2, 6, 2000, 1000, 0),
                creature(3, 6, 2400, 1000, 0),
            ]
            .into_iter()
            .collect(),
        ));
        let chosen = |id: &str, obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create(id, db.clone()).unwrap();
            decision.choices[policy.choose(obs, decision)].kind
        };
        let window = decide(
            DecisionKind::Chain { forced: false, triggers: false },
            None,
            vec![activate(FORBIDDEN_CHALICE, Location::SpellTrapZone, 0), choice(ChoiceKind::Pass)],
        );
        let attacking = Position { face_up: true, attack: true };
        let chalice = card(0, Location::SpellTrapZone, 0, FORBIDDEN_CHALICE, false);
        for id in ["infernity", "lightsworn"] {
            // Their monster activates its effect on the field: negated.  With
            // nothing to answer, the card is kept.
            let mut obs = observation();
            obs.turn = 4;
            obs.turn_player = Some(1);
            let theirs = monster(1, 0, Some(3), attacking, 2400, 1000);
            obs.cards = vec![chalice.clone(), theirs.clone()];
            assert_eq!(chosen(id, &obs, &window), ChoiceKind::Pass, "{id}");
            obs.chain = vec![ChainLink { code: 3, controller: 1, source: theirs.at, targets: Vec::new() }];
            assert_eq!(chosen(id, &obs, &window), ChoiceKind::Activate, "{id}");

            // Our 1800 attacks their 2000: 400 ATK win the battle.  Against
            // 2400 they would not.
            let mut obs = observation();
            obs.turn = 3;
            obs.phase = Some(Phase::BattleStep);
            let ours = monster(0, 0, Some(1), attacking, 1800, 1200);
            obs.cards = vec![chalice.clone(), ours.clone(), monster(1, 0, Some(2), attacking, 2000, 1000)];
            obs.battle_attacker = Some(ours.at);
            obs.battle_target = Some(CardRef { controller: 1, location: Location::MonsterZone, sequence: 0 });
            assert_eq!(chosen(id, &obs, &window), ChoiceKind::Activate, "{id}");
            obs.cards[2] = monster(1, 0, Some(3), attacking, 2400, 1000);
            assert_eq!(chosen(id, &obs, &window), ChoiceKind::Pass, "{id}");
        }
        // Lightsworn kept it in hand: it is Set like the deck's Traps.
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, FORBIDDEN_CHALICE, false)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SetSpellTrap, FORBIDDEN_CHALICE, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen("lightsworn", &obs, &idle), ChoiceKind::SetSpellTrap);
    }

    /// What a search on top of the Fortune Lady pilot kept doing otherwise:
    /// the Normal Summon is never left unused, and a Tribute Summon never
    /// gives a grown Lady for a smaller one.
    #[test]
    fn fortune_lady_uses_its_normal_summon() {
        const LIGHT: u32 = 34471458;
        const FIRE: u32 = 71870152;
        const WATER: u32 = 29088922;
        const DARK: u32 = 55586621;
        const EARTH: u32 = 82971335;
        const LADY: u16 = 0x31;
        // A Fortune Lady's ATK is "?": its Level decides.
        let lady = |code: u32, level: u32| creature(code, level, 0, 0, LADY);
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [lady(LIGHT, 1), lady(FIRE, 2), lady(WATER, 4), lady(DARK, 5), lady(EARTH, 6), creature(1, 4, 1700, 1000, 0)].into_iter().collect(),
        ));
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("fortune-lady", db.clone()).unwrap();
            let choice = &decision.choices[policy.choose(obs, decision)];
            (choice.kind, choice.code())
        };
        let hand = |codes: &[u32]| -> Vec<CardView> { codes.iter().enumerate().map(|(i, code)| card(0, Location::Hand, i as u32, *code, false)).collect() };
        let summons = |codes: &[u32]| -> Decision {
            let choices = codes
                .iter()
                .enumerate()
                .flat_map(|(i, code)| [from_hand(ChoiceKind::NormalSummon, *code, i as u32), from_hand(ChoiceKind::SetMonster, *code, i as u32)])
                .chain([choice(ChoiceKind::EndTurn)])
                .collect();
            decide(DecisionKind::Idle, None, choices)
        };
        let attacking = Position { face_up: true, attack: true };

        // Light and Fire in hand, nothing on the field: the pilot used to end
        // its turn.  Light is Summoned face-up; Fire alone is Set.
        let mut obs = observation();
        obs.cards = hand(&[LIGHT, FIRE]);
        assert_eq!(chosen(&obs, &summons(&[LIGHT, FIRE])), (ChoiceKind::NormalSummon, Some(LIGHT)));
        obs.cards = hand(&[FIRE]);
        assert_eq!(chosen(&obs, &summons(&[FIRE])), (ChoiceKind::SetMonster, Some(FIRE)));

        // Earth (2400 on arrival) is not Summoned for an Earth grown to 2800,
        // and is for a Water of 1200.
        obs.turn = 5;
        obs.cards = [hand(&[EARTH]), vec![monster(0, 0, Some(EARTH), attacking, 2800, 2800)]].concat();
        assert_eq!(chosen(&obs, &summons(&[EARTH])).0, ChoiceKind::EndTurn);
        obs.cards = [hand(&[EARTH]), vec![monster(0, 0, Some(WATER), attacking, 1200, 1200)]].concat();
        assert_eq!(chosen(&obs, &summons(&[EARTH])), (ChoiceKind::NormalSummon, Some(EARTH)));
        // Dark before Earth when it wins a battle: its effect brings the Tribute back.
        obs.cards = [hand(&[EARTH, DARK]), vec![monster(0, 0, Some(WATER), attacking, 1200, 1200), monster(1, 0, Some(1), attacking, 1700, 1000)]].concat();
        assert_eq!(chosen(&obs, &summons(&[EARTH, DARK])), (ChoiceKind::NormalSummon, Some(DARK)));
    }

    /// The same for Draconic Might: Dark Hole waits for two monsters or a
    /// big one, and Armed Dragon LV5 is Tribute Summoned under a bigger
    /// monster when its effect destroys that monster at once.
    #[test]
    fn draconic_might_plays_what_the_search_found() {
        use crate::cards::types;
        use crate::staples::DARK_HOLE;
        const ARMED_DRAGON_LV5: u32 = 46384672;
        const ARMED_DRAGON_LV7: u32 = 73879377;
        const TWIN_HEADED_BEHEMOTH: u32 = 43586926;
        let db: Arc<dyn crate::CardDatabase> = Arc::new(MemoryCards(
            [
                creature(ARMED_DRAGON_LV5, 5, 2400, 1700, 0),
                creature(ARMED_DRAGON_LV7, 7, 2800, 1000, 0),
                creature(TWIN_HEADED_BEHEMOTH, 3, 1500, 1200, 0),
                creature(1, 4, 1800, 1000, 0),
                creature(2, 4, 1500, 1000, 0),
                creature(3, 8, 2800, 2000, 0),
                printed(DARK_HOLE, types::SPELL, 0),
            ]
            .into_iter()
            .collect(),
        ));
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("draconic-might", db.clone()).unwrap();
            let choice = &decision.choices[policy.choose(obs, decision)];
            (choice.kind, choice.code())
        };
        let attacking = Position { face_up: true, attack: true };
        let their = |sequence: u32, code: u32, attack: i32| monster(1, sequence, Some(code), attacking, attack, 1000);

        // Dark Hole: not for one monster of 1800, for two, or for one of 2800.
        let dark_hole = decide(DecisionKind::Idle, None, vec![activate(DARK_HOLE, Location::Hand, 0), choice(ChoiceKind::EndTurn)]);
        let mut obs = observation();
        obs.turn = 2;
        obs.cards = vec![card(0, Location::Hand, 0, DARK_HOLE, false), their(0, 1, 1800)];
        assert_eq!(chosen(&obs, &dark_hole).0, ChoiceKind::EndTurn);
        obs.cards.push(their(1, 2, 1500));
        assert_eq!(chosen(&obs, &dark_hole), (ChoiceKind::Activate, Some(DARK_HOLE)));
        obs.cards = vec![card(0, Location::Hand, 0, DARK_HOLE, false), their(0, 3, 2800)];
        assert_eq!(chosen(&obs, &dark_hole), (ChoiceKind::Activate, Some(DARK_HOLE)));

        // Armed Dragon LV5 with LV7 in hand to send: their 2800 goes.  Without
        // a monster to send, 2400 ATK is not Summoned under 2800.
        let summon = decide(
            DecisionKind::Idle,
            None,
            vec![from_hand(ChoiceKind::NormalSummon, ARMED_DRAGON_LV5, 0), from_hand(ChoiceKind::SetMonster, ARMED_DRAGON_LV5, 0), choice(ChoiceKind::EndTurn)],
        );
        let behemoth = monster(0, 0, Some(TWIN_HEADED_BEHEMOTH), attacking, 1500, 1200);
        obs.turn = 6;
        obs.cards = vec![
            card(0, Location::Hand, 0, ARMED_DRAGON_LV5, false),
            card(0, Location::Hand, 1, ARMED_DRAGON_LV7, false),
            behemoth.clone(),
            their(0, 3, 2800),
        ];
        assert_eq!(chosen(&obs, &summon), (ChoiceKind::NormalSummon, Some(ARMED_DRAGON_LV5)));
        obs.cards = vec![card(0, Location::Hand, 0, ARMED_DRAGON_LV5, false), behemoth, their(0, 3, 2800)];
        assert_eq!(chosen(&obs, &summon).0, ChoiceKind::EndTurn);
    }
    #[test]
    fn ojama_blue_searches_an_engine_before_hurricane() {
        use crate::cards::types;
        const BLUE: u32 = 64627453;
        const COUNTRY: u32 = 90011152;
        const MAGIC: u32 = 24643836;
        const HURRICANE: u32 = 8251996;
        let db = Arc::new(MemoryCards([
            creature(BLUE, 2, 0, 1000, 0xf),
            printed(COUNTRY, types::SPELL | types::FIELD, 0xf),
            printed(MAGIC, types::SPELL, 0xf),
            printed(HURRICANE, types::SPELL, 0xf),
        ].into_iter().collect()));
        let mut policy = crate::registry::create("ojama", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Graveyard, 0, BLUE, true)];
        let trigger = decide(DecisionKind::Chain { forced: true, triggers: true }, None,
            vec![activate(BLUE, Location::Graveyard, 0)]);
        policy.choose(&obs, &trigger);
        let mut search = select_one(Hint::AddToHand, vec![
            toggle(Location::Deck, 0, COUNTRY), toggle(Location::Deck, 1, MAGIC), toggle(Location::Deck, 2, HURRICANE),
        ]);
        search.minimum = 2;
        search.maximum = 2;
        assert_eq!(search.choices[policy.choose(&obs, &search)].code(), Some(COUNTRY));
        // The first selected card is still in the deck during a sequential
        // prompt. Its offered member must count toward the second choice.
        let selected = search.choices.remove(0).card.unwrap();
        search.selected.push(selected.at);
        for c in &mut search.choices { c.members = vec![selected, c.card.unwrap()]; }
        assert_eq!(search.choices[policy.choose(&obs, &search)].code(), Some(MAGIC));
        // Once all three names are available, Hurricane is the payoff.
        obs.cards.extend([12482652, 42941100, 79335209].into_iter().enumerate()
            .map(|(i, code)| card(0, Location::Hand, i as u32, code, false)));
        search.selected.clear();
        for c in &mut search.choices { c.members.clear(); }
        assert_eq!(search.choices[policy.choose(&obs, &search)].code(), Some(HURRICANE));
    }

    #[test]
    fn ojama_country_revives_fusions_and_exposes_the_stat_swap() {
        use crate::cards::types;
        const COUNTRY: u32 = 90011152;
        const BLUE: u32 = 64627453;
        const GREEN: u32 = 12482652;
        const KNIGHT: u32 = 40391316;
        let db = Arc::new(MemoryCards([
            creature(BLUE, 2, 0, 1000, 0xf), creature(GREEN, 2, 0, 1000, 0xf),
            (KNIGHT, crate::cards::CardData { kind: types::MONSTER | types::FUSION, defense: 2500,
                ..creature(KNIGHT, 5, 0, 2500, 0xf).1 }),
            printed(COUNTRY, types::SPELL | types::FIELD, 0xf),
        ].into_iter().collect()));
        let mut policy = crate::registry::create("ojama", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 5, COUNTRY, true),
            card(0, Location::Hand, 0, GREEN, false),
            card(0, Location::Graveyard, 0, KNIGHT, true),
            card(0, Location::Graveyard, 1, GREEN, true)];
        let revive = decide(DecisionKind::Idle, None, vec![activate(COUNTRY, Location::SpellTrapZone, 5), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &revive), 0, "revive a fusion even with only one Ojama in hand");
        let target = select_one(Hint::SpecialSummon, vec![toggle(Location::Graveyard, 1, GREEN), toggle(Location::Graveyard, 0, KNIGHT)]);
        assert_eq!(target.choices[policy.choose(&obs, &target)].code(), Some(KNIGHT));
        obs.cards = vec![card(0, Location::SpellTrapZone, 5, COUNTRY, true),
            monster(0, 0, Some(BLUE), Position::FACE_DOWN_DEFENSE, 0, 1000)];
        let flip = decide(DecisionKind::Idle, None, vec![Choice { kind: ChoiceKind::ChangePosition,
            ..activate(BLUE, Location::MonsterZone, 0) }, choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &flip), 0);
        obs.cards.remove(0);
        assert_eq!(policy.choose(&obs, &flip), 1, "without Country Blue stays face down");
    }

    #[test]
    fn destiny_doom_lord_removes_a_lone_wall_without_turning_away_first() {
        const DOOM: u32 = 41613948;
        let db = Arc::new(MemoryCards([creature(DOOM, 3, 600, 800, 0xc008), creature(1, 4, 1800, 1000, 0)].into_iter().collect()));
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("destiny-hero", db.clone()).unwrap();
            decision.choices[policy.choose(obs, decision)].kind
        };
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, DOOM, false), monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1800, 1000)];
        let summon = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SetMonster, DOOM, 0),
            from_hand(ChoiceKind::NormalSummon, DOOM, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &summon), ChoiceKind::NormalSummon);
        obs.cards.push(monster(1, 1, Some(1), Position::FACE_UP_ATTACK, 1800, 1000));
        assert_eq!(chosen(&obs, &summon), ChoiceKind::SetMonster, "do not expose 600 ATK to a second attacker");
        obs.cards.pop();
        obs.cards[0] = monster(0, 0, Some(DOOM), Position::FACE_UP_ATTACK, 600, 800);
        let effect = decide(DecisionKind::Idle, None, vec![Choice { kind: ChoiceKind::ChangePosition,
            ..activate(DOOM, Location::MonsterZone, 0) }, activate(DOOM, Location::MonsterZone, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &effect), ChoiceKind::Activate, "the effect requires Attack Position");
        obs.cards[0].position = Position::FACE_DOWN_DEFENSE;
        let flip = decide(DecisionKind::Idle, None, vec![effect.choices[0].clone(), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &flip), ChoiceKind::ChangePosition);
        obs.phase = Some(Phase::Main2);
        assert_eq!(chosen(&obs, &flip), ChoiceKind::EndTurn);
    }

    #[test]
    fn destiny_defender_stalls_and_can_complete_three_tributes() {
        const DEFENDER: u32 = 54749427;
        const DIAMOND: u32 = 13093792;
        const DOGMA: u32 = 17132130;
        let db = Arc::new(MemoryCards([creature(DEFENDER, 4, 100, 2700, 0xc008), creature(DIAMOND, 4, 1400, 1600, 0xc008),
            creature(DOGMA, 8, 3400, 3400, 0xc008), creature(1, 4, 1900, 1000, 0)].into_iter().collect()));
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("destiny-hero", db.clone()).unwrap();
            let c = &decision.choices[policy.choose(obs, decision)]; (c.kind, c.code())
        };
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, DEFENDER, false), card(0, Location::Hand, 1, DIAMOND, false),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000)];
        let summon = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, DIAMOND, 1),
            from_hand(ChoiceKind::SetMonster, DEFENDER, 0), from_hand(ChoiceKind::NormalSummon, DEFENDER, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &summon), (ChoiceKind::SetMonster, Some(DEFENDER)));
        obs.cards[2].attack = 3000;
        assert_ne!(chosen(&obs, &summon).1, Some(DEFENDER), "2700 DEF cannot hold this monster");
        obs.cards = vec![card(0, Location::Hand, 0, DEFENDER, false), card(0, Location::Hand, 1, DOGMA, false),
            monster(0, 0, Some(DIAMOND), Position::FACE_UP_ATTACK, 1400, 1600),
            monster(0, 1, Some(DIAMOND), Position::FACE_UP_ATTACK, 1400, 1600)];
        let third = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, DEFENDER, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &third), (ChoiceKind::NormalSummon, Some(DEFENDER)));
    }

    #[test]
    fn destiny_uses_graveyard_setup_and_d_chain_with_a_payoff() {
        use crate::cards::types;
        const DUNKER: u32 = 93431862;
        const MALICIOUS: u32 = 9411399;
        const DASHER: u32 = 81866673;
        const DIAMOND: u32 = 13093792;
        const CHAIN: u32 = 43405287;
        let db = Arc::new(MemoryCards([
            creature(DUNKER, 4, 1200, 1700, 0xc008), creature(MALICIOUS, 6, 800, 800, 0xc008),
            creature(DASHER, 6, 2100, 1000, 0xc008), creature(DIAMOND, 4, 1400, 1600, 0xc008),
            creature(1, 4, 1700, 1000, 0), printed(CHAIN, types::TRAP | types::EQUIP, 0),
        ].into_iter().collect()));
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("destiny-hero", db.clone()).unwrap();
            decision.choices[policy.choose(obs, decision)].kind
        };
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(DUNKER), Position::FACE_UP_ATTACK, 1200, 1700),
            card(0, Location::Hand, 0, MALICIOUS, false)];
        let burn = decide(DecisionKind::Idle, None, vec![activate(DUNKER, Location::MonsterZone, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &burn), ChoiceKind::Activate, "Malicious works from the graveyard even without lethal burn");
        obs.cards[1].code = Some(DIAMOND);
        assert_eq!(chosen(&obs, &burn), ChoiceKind::EndTurn, "do not burn a useful normal summon for 500");
        obs.cards[1].code = Some(DASHER);
        obs.cards.push(card(0, Location::Graveyard, 0, MALICIOUS, true));
        let body = decide(DecisionKind::Idle, None, vec![activate(MALICIOUS, Location::Graveyard, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &body), ChoiceKind::Activate, "Dasher can use the tribute");
        obs.cards[1].code = Some(DIAMOND);
        assert_eq!(chosen(&obs, &body), ChoiceKind::EndTurn);

        obs.cards = vec![monster(0, 0, Some(DIAMOND), Position::FACE_UP_ATTACK, 1400, 1600),
            card(0, Location::SpellTrapZone, 0, CHAIN, false)];
        let equip = decide(DecisionKind::Chain { forced: false, triggers: false }, None,
            vec![activate(CHAIN, Location::SpellTrapZone, 0), choice(ChoiceKind::Pass)]);
        assert_eq!(chosen(&obs, &equip), ChoiceKind::Activate, "a permanent equip also improves direct attacks");
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::Battle);
        obs.cards.push(monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1700, 1000));
        obs.battle_attacker = Some(obs.cards[2].at);
        obs.battle_target = Some(obs.cards[0].at);
        assert_eq!(chosen(&obs, &equip), ChoiceKind::Activate);
        obs.cards[2].attack = 2000;
        assert_eq!(chosen(&obs, &equip), ChoiceKind::Pass, "500 ATK does not save this battle");
    }

    #[test]
    fn watt_cash_in_cube_only_when_the_permanent_bonus_is_larger() {
        use crate::cards::{races, types};
        const CUBE: u32 = 65612454;
        const GIRAFFE: u32 = 402568;
        let mut giraffe = creature(GIRAFFE, 4, 1200, 100, 0xe);
        giraffe.1.race = races::THUNDER;
        let db = Arc::new(MemoryCards([giraffe, printed(CUBE, types::SPELL | types::EQUIP, 0xe)].into_iter().collect()));
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("watt", db.clone()).unwrap();
            decision.choices[policy.choose(obs, decision)].kind
        };
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(GIRAFFE), Position::FACE_UP_ATTACK, 1300, 100),
            card(0, Location::SpellTrapZone, 0, CUBE, true), card(0, Location::Graveyard, 0, GIRAFFE, true)];
        let effect = decide(DecisionKind::Idle, None, vec![activate(CUBE, Location::SpellTrapZone, 0),
            choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &effect), ChoiceKind::Activate);
        // Ten Thunder monsters already give 1000 ATK from the equip.
        obs.cards.extend((1..10).map(|i| card(0, Location::Graveyard, i, GIRAFFE, true)));
        assert_ne!(chosen(&obs, &effect), ChoiceKind::Activate);
        obs.cards.truncate(3);
        obs.phase = Some(Phase::Main2);
        assert_ne!(chosen(&obs, &effect), ChoiceKind::Activate);
    }

    #[test]
    fn watt_direct_attack_does_not_leave_a_monster_target_in_memory() {
        const GIRAFFE: u32 = 402568;
        let db = Arc::new(MemoryCards([creature(GIRAFFE, 4, 1200, 100, 0xe), creature(1, 4, 1000, 1000, 0)].into_iter().collect()));
        let mut policy = crate::registry::create("watt", db).unwrap();
        let mut obs = observation();
        obs.phase = Some(Phase::Battle);
        obs.cards = vec![monster(0, 0, Some(GIRAFFE), Position::FACE_UP_ATTACK, 1200, 100),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1000, 1000)];
        let attack = decide(DecisionKind::Battle, None, vec![attack(GIRAFFE, 0, true), choice(ChoiceKind::EnterMain2)]);
        assert_eq!(policy.choose(&obs, &attack), 0);
        let direct = decide(DecisionKind::YesNo, None, [ChoiceKind::Yes, ChoiceKind::No].into_iter()
            .map(|k| Choice { description: crate::tactics::ATTACK_DIRECTLY, ..choice(k) }).collect());
        assert_eq!(direct.choices[policy.choose(&obs, &direct)].kind, ChoiceKind::Yes);
    }

    #[test]
    fn toon_table_searches_world_once_then_playable_monsters() {
        use crate::cards::types;
        const TABLE: u32 = 89997728;
        const WORLD: u32 = 15259703;
        const ELF: u32 = 42386471;
        const GIRL: u32 = 90960358;
        const DRAGON: u32 = 53183600;
        const SHEEP: u32 = 73915052;
        let db = Arc::new(MemoryCards([printed(TABLE, types::SPELL, 0x62), printed(WORLD, types::SPELL | types::CONTINUOUS, 0x62),
            creature(ELF, 4, 1900, 900, 0x62), creature(GIRL, 6, 2000, 1700, 0x62),
            creature(DRAGON, 8, 3000, 2500, 0x62), creature(SHEEP, 1, 0, 0, 0)].into_iter().collect()));
        let mut policy = crate::registry::create("toon", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, TABLE, false)];
        let activation = decide(DecisionKind::Idle, None, vec![activate(TABLE, Location::Hand, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &activation), 0);
        let search = select_one(Hint::AddToHand, [WORLD, ELF, GIRL, DRAGON].into_iter().enumerate()
            .map(|(i, code)| toggle(Location::Deck, i as u32, code)).collect());
        assert_eq!(search.choices[policy.choose(&obs, &search)].code(), Some(WORLD));
        obs.cards.push(card(0, Location::Hand, 1, WORLD, false));
        assert_eq!(search.choices[policy.choose(&obs, &search)].code(), Some(ELF), "do not collect redundant Worlds or two-tribute monsters");
        obs.cards.push(monster(0, 0, Some(SHEEP), Position::FACE_UP_DEFENSE, 0, 0));
        assert_eq!(search.choices[policy.choose(&obs, &search)].code(), Some(GIRL), "one cheap tribute enables the Girl");
        obs.cards[2] = monster(0, 0, Some(ELF), Position::FACE_UP_ATTACK, 1900, 900);
        assert_eq!(search.choices[policy.choose(&obs, &search)].code(), Some(ELF), "the existing attacker is not cheap tribute fodder");
    }

    #[test]
    fn toon_defense_is_ready_early_but_declines_lethal_redirection() {
        use crate::cards::types;
        const DEFENSE: u32 = 43509019;
        const ELF: u32 = 42386471;
        let db = Arc::new(MemoryCards([printed(DEFENSE, types::TRAP | types::CONTINUOUS, 0x62),
            creature(ELF, 4, 1900, 900, 0x62), creature(1, 4, 2300, 1000, 0)].into_iter().collect()));
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("toon", db.clone()).unwrap();
            decision.choices[policy.choose(obs, decision)].kind
        };
        let mut obs = observation();
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::Draw);
        obs.cards = vec![monster(0, 0, Some(ELF), Position::FACE_UP_ATTACK, 1900, 900),
            card(0, Location::SpellTrapZone, 0, DEFENSE, false), monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 2300, 1000)];
        let chain = decide(DecisionKind::Chain { forced: false, triggers: false }, None,
            vec![activate(DEFENSE, Location::SpellTrapZone, 0), choice(ChoiceKind::Pass)]);
        assert_eq!(chosen(&obs, &chain), ChoiceKind::Activate);
        obs.cards[1].position.face_up = true;
        obs.phase = Some(Phase::BattleStart);
        obs.battle_attacker = Some(obs.cards[2].at);
        obs.battle_target = Some(obs.cards[0].at);
        let trigger = decide(DecisionKind::YesNo, Some(DEFENSE), vec![choice(ChoiceKind::Yes), choice(ChoiceKind::No)]);
        assert_eq!(chosen(&obs, &trigger), ChoiceKind::Yes);
        obs.life_points[0] = 1700;
        assert_eq!(chosen(&obs, &trigger), ChoiceKind::No, "400 battle damage is survivable; 2300 direct damage is not");
        assert_eq!(chosen(&obs, &chain), ChoiceKind::Pass, "the chain and yes/no prompts must use the same rule");
        obs.life_points[0] = 8000;
        obs.cards[2].attack = 1800;
        assert_eq!(chosen(&obs, &trigger), ChoiceKind::No, "keep a battle the Toon wins");
    }


    #[test]
    fn watt_sets_small_blockers_instead_of_ending_with_them_in_hand() {
        for (code, level, atk, def) in [(32548609, 3, 0, 100), (27324313, 3, 1000, 500),
            (5554990, 3, 300, 0), (24996659, 3, 600, 100)] {
            let db = Arc::new(MemoryCards([creature(code, level, atk, def, 0xe), creature(1, 4, 1900, 1000, 0)].into_iter().collect()));
            let mut policy = crate::registry::create("watt", db).unwrap();
            let mut obs = observation();
            obs.cards = vec![card(0, Location::Hand, 0, code, false),
                monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000)];
            let decision = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, code, 0),
                from_hand(ChoiceKind::SetMonster, code, 0), choice(ChoiceKind::EndTurn)]);
            assert_eq!(decision.choices[policy.choose(&obs, &decision)].kind, ChoiceKind::SetMonster, "block with {code}");
        }
    }

    #[test]
    fn watt_turns_direct_attackers_toward_larger_blockers() {
        for code in [402568, 81896771] {
            let db = Arc::new(MemoryCards([creature(code, 4, 1200, 100, 0xe), creature(1, 4, 1900, 1000, 0)].into_iter().collect()));
            for position in [Position::FACE_UP_DEFENSE, Position::FACE_DOWN_DEFENSE] {
                let mut policy = crate::registry::create("watt", db.clone()).unwrap();
                let mut obs = observation();
                obs.cards = vec![monster(0, 0, Some(code), position, 1200, 100),
                    monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000)];
                obs.cards[0].can_attack = false;
                let decision = decide(DecisionKind::Idle, None, vec![Choice { kind: ChoiceKind::ChangePosition,
                    ..activate(code, Location::MonsterZone, 0) }, choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
                assert_eq!(decision.choices[policy.choose(&obs, &decision)].kind, ChoiceKind::ChangePosition);
                obs.phase = Some(Phase::Main2);
                assert_ne!(decision.choices[policy.choose(&obs, &decision)].kind, ChoiceKind::ChangePosition);
            }
        }
    }

    #[test]
    fn watt_dragonfly_recruits_replacement_blockers_under_pressure() {
        const FLY: u32 = 97885363;
        const LEMUR: u32 = 45801022;
        const GIRAFFE: u32 = 402568;
        let db = Arc::new(MemoryCards([creature(FLY, 2, 900, 100, 0xe), creature(LEMUR, 2, 800, 100, 0xe),
            creature(GIRAFFE, 4, 1200, 100, 0xe), creature(1, 4, 1900, 1000, 0)].into_iter().collect()));
        let mut policy = crate::registry::create("watt", db).unwrap();
        let mut obs = observation();
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::Damage);
        obs.cards = vec![card(0, Location::Graveyard, 0, FLY, true),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000),
            monster(1, 1, Some(1), Position::FACE_UP_ATTACK, 1900, 1000)];
        let trigger = decide(DecisionKind::YesNo, Some(FLY), vec![Choice { kind: ChoiceKind::Yes,
            ..activate(FLY, Location::Graveyard, 0) }, choice(ChoiceKind::No)]);
        assert_eq!(policy.choose(&obs, &trigger), 0);
        let search = select_one(Hint::SpecialSummon, [GIRAFFE, LEMUR, FLY].into_iter().enumerate()
            .map(|(i, code)| toggle(Location::Deck, i as u32, code)).collect());
        assert_eq!(search.choices[policy.choose(&obs, &search)].code(), Some(FLY));
        let without_fly = select_one(Hint::SpecialSummon, [GIRAFFE, LEMUR].into_iter().enumerate()
            .map(|(i, code)| toggle(Location::Deck, i as u32, code)).collect());
        assert_eq!(without_fly.choices[policy.choose(&obs, &without_fly)].code(), Some(LEMUR));
        obs.phase = Some(Phase::End);
        assert_eq!(search.choices[policy.choose(&obs, &search)].code(), Some(GIRAFFE));
        obs.phase = Some(Phase::Damage);
        obs.turn_player = Some(0);
        assert_eq!(search.choices[policy.choose(&obs, &search)].code(), Some(GIRAFFE));
    }

    #[test]
    fn watt_summons_in_defense_on_the_opponents_turn() {
        const GIRAFFE: u32 = 402568;
        let db = Arc::new(MemoryCards([creature(GIRAFFE, 4, 1200, 100, 0xe)].into_iter().collect()));
        let mut policy = crate::registry::create("watt", db).unwrap();
        let mut obs = observation();
        obs.turn_player = Some(1);
        let decision = decide(DecisionKind::Position, Some(GIRAFFE), vec![choice(ChoiceKind::Position(Position::FACE_UP_ATTACK)),
            choice(ChoiceKind::Position(Position::FACE_UP_DEFENSE))]);
        assert_eq!(decision.choices[policy.choose(&obs, &decision)].kind, ChoiceKind::Position(Position::FACE_UP_DEFENSE));
        obs.turn_player = Some(0);
        assert_eq!(decision.choices[policy.choose(&obs, &decision)].kind, ChoiceKind::Position(Position::FACE_UP_ATTACK));
    }

    #[test]
    fn watt_prefers_replacement_blockers_but_takes_lethal_direct_damage() {
        const FLY: u32 = 97885363;
        const LEMUR: u32 = 45801022;
        const GIRAFFE: u32 = 402568;
        let db = Arc::new(MemoryCards([creature(FLY, 2, 900, 100, 0xe), creature(LEMUR, 2, 800, 100, 0xe),
            creature(GIRAFFE, 4, 1200, 100, 0xe), creature(1, 4, 1900, 1000, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, FLY, false), card(0, Location::Hand, 1, LEMUR, false),
            card(0, Location::Hand, 2, GIRAFFE, false), monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000)];
        let mut choices: Vec<_> = [FLY, LEMUR, GIRAFFE].into_iter().enumerate().flat_map(|(i, code)|
            [from_hand(ChoiceKind::NormalSummon, code, i as u32), from_hand(ChoiceKind::SetMonster, code, i as u32)]).collect();
        choices.extend([choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
        let decision = decide(DecisionKind::Idle, None, choices);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("watt", db.clone()).unwrap();
            let c = &decision.choices[policy.choose(obs, &decision)];
            (c.kind, c.code())
        };
        assert_eq!(chosen(&obs), (ChoiceKind::SetMonster, Some(FLY)));
        obs.life_points[1] = 1200;
        assert_eq!(chosen(&obs), (ChoiceKind::NormalSummon, Some(GIRAFFE)));
        obs.life_points[1] = 2200;
        obs.cards.push(monster(0, 0, Some(GIRAFFE), Position::FACE_UP_ATTACK, 1000, 100));
        assert_eq!(chosen(&obs), (ChoiceKind::NormalSummon, Some(GIRAFFE)), "count direct damage already on the board");
        obs.phase = Some(Phase::Main2);
        assert_eq!(chosen(&obs), (ChoiceKind::SetMonster, Some(FLY)), "a summon after battle cannot finish this turn");
    }

    #[test]
    fn arcana_sets_up_solidarity_for_hidden_fairies_and_counts_its_boost() {
        use crate::cards::{races, types};
        const EMPEROR: u32 = 61175706;
        const SOLIDARITY: u32 = 86780027;
        let mut fairy = creature(EMPEROR, 4, 1400, 1400, 0x5);
        fairy.1.race = races::FAIRY;
        let db = Arc::new(MemoryCards([fairy, creature(1, 4, 1900, 1000, 0),
            printed(SOLIDARITY, types::SPELL | types::CONTINUOUS, 0)].into_iter().collect()));
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("arcana", db.clone()).unwrap();
            decision.choices[policy.choose(obs, decision)].kind
        };
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(EMPEROR), Position::FACE_DOWN_DEFENSE, 1400, 1400),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000),
            card(0, Location::Hand, 0, SOLIDARITY, false), card(0, Location::Graveyard, 0, EMPEROR, true)];
        let setup = decide(DecisionKind::Idle, None, vec![activate(SOLIDARITY, Location::Hand, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &setup), ChoiceKind::Activate, "a set Fairy still needs the boost when it flips");
        obs.cards[2] = card(0, Location::SpellTrapZone, 0, SOLIDARITY, true);
        let flip = decide(DecisionKind::Idle, None, vec![Choice { kind: ChoiceKind::ChangePosition,
            ..activate(EMPEROR, Location::MonsterZone, 0) }, choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
        assert_eq!(chosen(&obs, &flip), ChoiceKind::ChangePosition, "2200 ATK beats 1900");
        let summon = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SetMonster, EMPEROR, 0),
            from_hand(ChoiceKind::NormalSummon, EMPEROR, 0), choice(ChoiceKind::EndTurn)]);
        obs.cards[0] = card(0, Location::Hand, 0, EMPEROR, false);
        assert_eq!(chosen(&obs, &summon), ChoiceKind::NormalSummon);
        obs.cards.pop();
        assert_eq!(chosen(&obs, &summon), ChoiceKind::SetMonster, "an empty graveyard does not enable Solidarity");
        obs.cards.push(card(0, Location::Graveyard, 0, EMPEROR, true));
        obs.cards.push(card(0, Location::Graveyard, 1, 1, true));
        assert_eq!(chosen(&obs, &summon), ChoiceKind::SetMonster, "a second original race disables the boost");
    }

    #[test]
    fn arcana_ruler_can_replace_an_outclassed_board() {
        const LOVERS: u32 = 97574404;
        const RULER: u32 = 69831560;
        let db = Arc::new(MemoryCards([creature(LOVERS, 4, 1600, 1600, 0x5),
            creature(RULER, 10, 4000, 4000, 0x5), creature(1, 4, 2000, 1000, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = (0..3).map(|i| monster(0, i, Some(LOVERS), Position::FACE_DOWN_DEFENSE, 1600, 1600)).collect();
        obs.cards.push(monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 2000, 1000));
        obs.cards.push(card(0, Location::Hand, 0, RULER, false));
        let decision = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SpecialSummon, RULER, 0), choice(ChoiceKind::EndTurn)]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("arcana", db.clone()).unwrap();
            decision.choices[policy.choose(obs, &decision)].kind
        };
        assert_eq!(chosen(&obs), ChoiceKind::SpecialSummon, "the three sets cost 4800 but cannot break this board");
        obs.cards[3].attack = 4500;
        assert_eq!(chosen(&obs), ChoiceKind::EndTurn, "4000 ATK would not solve the stall");
        obs.cards[3].attack = 1500;
        for c in &mut obs.cards[..3] { c.position = Position::FACE_UP_ATTACK; }
        assert_eq!(chosen(&obs), ChoiceKind::EndTurn, "keep three attackers that already beat the opponent");
    }

    #[test]
    fn burn_cannon_waits_past_two_standbys_but_takes_lethal() {
        use crate::cards::types;
        const CANNON: u32 = 38992735;
        let db = Arc::new(MemoryCards([printed(CANNON, types::SPELL | types::CONTINUOUS, 0)].into_iter().collect()));
        for lethal in [false, true] {
            let mut policy = crate::registry::create("burn", db.clone()).unwrap();
            let mut obs = observation();
            obs.cards = vec![card(0, Location::SpellTrapZone, 0, CANNON, true)];
            let decision = decide(DecisionKind::Idle, None, vec![activate(CANNON, Location::SpellTrapZone, 0), choice(ChoiceKind::EndTurn)]);
            assert_eq!(decision.choices[policy.choose(&obs, &decision)].kind, ChoiceKind::EndTurn);
            obs.turn = 5;
            if lethal { obs.life_points[1] = 2000; }
            assert_eq!(decision.choices[policy.choose(&obs, &decision)].kind,
                if lethal { ChoiceKind::Activate } else { ChoiceKind::EndTurn });
            if !lethal {
                obs.turn = 9;
                assert_eq!(decision.choices[policy.choose(&obs, &decision)].kind, ChoiceKind::EndTurn);
                obs.turn = 17;
                assert_eq!(decision.choices[policy.choose(&obs, &decision)].kind, ChoiceKind::Activate);
            }
        }
    }

    #[test]
    fn burn_turtle_recruits_another_blocker_before_exposing_princess() {
        const TURTLE: u32 = 60806437;
        const PRINCESS: u32 = 64752646;
        let db = Arc::new(MemoryCards([creature(TURTLE, 4, 1400, 1200, 0),
            creature(PRINCESS, 4, 1300, 1500, 0), creature(1, 4, 1900, 1000, 0)].into_iter().collect()));
        let mut policy = crate::registry::create("burn", db).unwrap();
        let mut obs = observation();
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::Damage);
        obs.cards = vec![card(0, Location::Graveyard, 0, TURTLE, true),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000),
            monster(1, 1, Some(1), Position::FACE_UP_ATTACK, 1900, 1000)];
        let trigger = decide(DecisionKind::YesNo, Some(TURTLE), vec![Choice { kind: ChoiceKind::Yes,
            ..activate(TURTLE, Location::Graveyard, 0) }, choice(ChoiceKind::No)]);
        assert_eq!(policy.choose(&obs, &trigger), 0);
        let recruit = select_one(Hint::SpecialSummon, vec![toggle(Location::Deck, 0, PRINCESS), toggle(Location::Deck, 1, TURTLE)]);
        assert_eq!(recruit.choices[policy.choose(&obs, &recruit)].code(), Some(TURTLE));
        obs.turn_player = Some(0);
        assert_eq!(recruit.choices[policy.choose(&obs, &recruit)].code(), Some(PRINCESS));
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::End);
        assert_eq!(recruit.choices[policy.choose(&obs, &recruit)].code(), Some(PRINCESS));
    }

    #[test]
    fn burn_turtle_attacks_when_clear_and_sets_when_outclassed() {
        const TURTLE: u32 = 60806437;
        let db = Arc::new(MemoryCards([creature(TURTLE, 4, 1400, 1200, 0), creature(1, 4, 1900, 1000, 0)].into_iter().collect()));
        let decision = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SetMonster, TURTLE, 0),
            from_hand(ChoiceKind::NormalSummon, TURTLE, 0), choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("burn", db.clone()).unwrap();
            decision.choices[policy.choose(obs, &decision)].kind
        };
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, TURTLE, false)];
        assert_eq!(chosen(&obs), ChoiceKind::NormalSummon);
        obs.phase = Some(Phase::Main2);
        assert_eq!(chosen(&obs), ChoiceKind::SetMonster);
        obs.phase = Some(Phase::Main1);
        obs.cards.push(monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000));
        assert_eq!(chosen(&obs), ChoiceKind::SetMonster);
    }

    #[test]
    fn spellcaster_flips_for_draw_removal_and_useful_counters() {
        use crate::cards::types;
        const SEER: u32 = 82099401;
        const VINDICTIVE: u32 = 45141844;
        const MANDRAGOLA: u32 = 7802006;
        const CITADEL: u32 = 39910367;
        let db = Arc::new(MemoryCards([printed(SEER, types::MONSTER | types::EFFECT | types::FLIP, 0),
            printed(VINDICTIVE, types::MONSTER | types::EFFECT | types::FLIP, 0),
            printed(MANDRAGOLA, types::MONSTER | types::EFFECT | types::FLIP, 0),
            printed(CITADEL, types::SPELL | types::FIELD, 0)].into_iter().collect()));
        for (code, needs_target) in [(SEER, false), (VINDICTIVE, true), (MANDRAGOLA, true)] {
            let mut obs = observation();
            obs.cards = vec![monster(0, 0, Some(code), Position::FACE_DOWN_DEFENSE, 400, 400)];
            let flip = decide(DecisionKind::Idle, None, vec![Choice { kind: ChoiceKind::ChangePosition,
                ..activate(code, Location::MonsterZone, 0) }, choice(ChoiceKind::EndTurn)]);
            let chosen = |obs: &Observation| {
                let mut policy = crate::registry::create("spellcaster", db.clone()).unwrap();
                flip.choices[policy.choose(obs, &flip)].kind
            };
            assert_eq!(chosen(&obs), if needs_target { ChoiceKind::EndTurn } else { ChoiceKind::ChangePosition });
            if code == VINDICTIVE {
                obs.cards.push(monster(1, 0, None, Position::FACE_DOWN_DEFENSE, 0, 0));
            } else if code == MANDRAGOLA {
                obs.cards.push(card(0, Location::SpellTrapZone, 5, CITADEL, true));
            }
            assert_eq!(chosen(&obs), ChoiceKind::ChangePosition);
            obs.phase = Some(Phase::Main2);
            assert_eq!(chosen(&obs), ChoiceKind::ChangePosition, "the effect still helps after battle");
        }
    }

    #[test]
    fn spellcaster_equips_attackers_even_when_they_already_win() {
        use crate::cards::types;
        const NUZZLER: u32 = 99597615;
        let db = Arc::new(MemoryCards([creature(1, 4, 1800, 1000, 0),
            printed(NUZZLER, types::SPELL | types::EQUIP, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(1), Position::FACE_UP_ATTACK, 1800, 1000),
            card(0, Location::Hand, 0, NUZZLER, false)];
        let equip = decide(DecisionKind::Idle, None, vec![activate(NUZZLER, Location::Hand, 0),
            choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("spellcaster", db.clone()).unwrap();
            equip.choices[policy.choose(obs, &equip)].kind
        };
        assert_eq!(chosen(&obs), ChoiceKind::Activate, "700 more direct damage is useful");
        obs.cards.push(monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1500, 1000));
        assert_eq!(chosen(&obs), ChoiceKind::Activate, "increase damage over a weaker attacker");
        obs.cards[2].attack = 2500;
        assert_ne!(chosen(&obs), ChoiceKind::Activate, "the boost does not win this battle");
        obs.cards[2].attack = 1500;
        obs.phase = Some(Phase::Main2);
        assert_ne!(chosen(&obs), ChoiceKind::Activate, "save it until the next attack opportunity");
    }

    #[test]
    fn pyramid_keeps_sphinx_face_up_through_open_main_phase_windows() {
        const GUARDIAN: u32 = 40659562;
        let db = Arc::new(MemoryCards([creature(GUARDIAN, 5, 1700, 2400, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(GUARDIAN), Position::FACE_UP_ATTACK, 1700, 2400)];
        let window = decide(DecisionKind::Chain { forced: false, triggers: false }, None,
            vec![activate(GUARDIAN, Location::MonsterZone, 0), choice(ChoiceKind::Pass)]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("pyramid", db.clone()).unwrap();
            window.choices[policy.choose(obs, &window)].kind
        };
        assert_eq!(chosen(&obs), ChoiceKind::Pass, "no EnterBattle option in a chain window does not mean battle is over");
        obs.phase = Some(Phase::Main2);
        assert_eq!(chosen(&obs), ChoiceKind::Activate, "reset for next turn after attacking");
        obs.phase = Some(Phase::Main1);
        obs.cards[0].can_attack = false;
        assert_eq!(chosen(&obs), ChoiceKind::Activate, "no attack is being sacrificed");
    }

    #[test]
    fn pyramid_turtle_recruits_reaper_in_defense_against_large_attackers() {
        const TURTLE: u32 = 77044671;
        const REAPER: u32 = 23205979;
        const MUMMY: u32 = 70821187;
        let db = Arc::new(MemoryCards([creature(TURTLE, 4, 1200, 1400, 0),
            creature(REAPER, 3, 300, 200, 0), creature(MUMMY, 4, 1800, 1500, 0),
            creature(1, 4, 1900, 1000, 0)].into_iter().collect()));
        let mut policy = crate::registry::create("pyramid", db).unwrap();
        let mut obs = observation();
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::Damage);
        obs.cards = vec![card(0, Location::Graveyard, 0, TURTLE, true),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000)];
        let trigger = decide(DecisionKind::YesNo, Some(TURTLE), vec![Choice { kind: ChoiceKind::Yes,
            ..activate(TURTLE, Location::Graveyard, 0) }, choice(ChoiceKind::No)]);
        assert_eq!(policy.choose(&obs, &trigger), 0);
        let recruit = select_one(Hint::SpecialSummon, vec![toggle(Location::Deck, 0, MUMMY), toggle(Location::Deck, 1, REAPER)]);
        assert_eq!(recruit.choices[policy.choose(&obs, &recruit)].code(), Some(REAPER));
        let position = decide(DecisionKind::Position, Some(REAPER), vec![choice(ChoiceKind::Position(Position::FACE_UP_ATTACK)),
            choice(ChoiceKind::Position(Position::FACE_UP_DEFENSE))]);
        assert_eq!(position.choices[policy.choose(&obs, &position)].kind, ChoiceKind::Position(Position::FACE_UP_DEFENSE));
        obs.turn_player = Some(0);
        assert_eq!(recruit.choices[policy.choose(&obs, &recruit)].code(), Some(MUMMY));
        obs.turn_player = Some(1);
        obs.cards[1].attack = 1700;
        assert_eq!(recruit.choices[policy.choose(&obs, &recruit)].code(), Some(MUMMY));
    }

    #[test]
    fn pyramid_does_not_tribute_its_large_sphinxes_for_weaker_ones() {
        const ANDRO: u32 = 15013468;
        const GUARDIAN: u32 = 40659562;
        const HIERACO: u32 = 82260502;
        const TURTLE: u32 = 77044671;
        let db = Arc::new(MemoryCards([creature(ANDRO, 10, 3000, 2500, 0),
            creature(GUARDIAN, 5, 1700, 2400, 0), creature(HIERACO, 6, 2400, 1200, 0),
            creature(TURTLE, 4, 1200, 1400, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(ANDRO), Position::FACE_UP_ATTACK, 3000, 2500),
            card(0, Location::Hand, 0, GUARDIAN, false), card(0, Location::Hand, 1, HIERACO, false)];
        let summon = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SetMonster, GUARDIAN, 0),
            from_hand(ChoiceKind::NormalSummon, HIERACO, 1), choice(ChoiceKind::EndTurn)]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("pyramid", db.clone()).unwrap();
            summon.choices[policy.choose(obs, &summon)].kind
        };
        assert_eq!(chosen(&obs), ChoiceKind::EndTurn);
        obs.cards.push(monster(0, 1, Some(TURTLE), Position::FACE_UP_DEFENSE, 1200, 1400));
        assert_eq!(chosen(&obs), ChoiceKind::SetMonster, "a smaller tribute makes the summon worthwhile");
    }

    #[test]
    fn spellcaster_unites_to_break_a_wall_without_reducing_open_field_damage() {
        use crate::cards::{races, types};
        const UNITE: u32 = 36045450;
        let mut body = creature(1, 4, 1800, 1000, 0);
        body.1.race = races::SPELLCASTER;
        let db = Arc::new(MemoryCards([body, printed(UNITE, types::SPELL, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(1), Position::FACE_UP_ATTACK, 1800, 1000),
            monster(0, 1, Some(1), Position::FACE_UP_ATTACK, 1600, 1000),
            card(0, Location::Hand, 0, UNITE, false),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 2800, 1000)];
        let unite = decide(DecisionKind::Idle, None, vec![activate(UNITE, Location::Hand, 0),
            choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("spellcaster", db.clone()).unwrap();
            unite.choices[policy.choose(obs, &unite)].kind
        };
        assert_eq!(chosen(&obs), ChoiceKind::Activate);
        obs.cards[3].attack = 3300;
        assert_ne!(chosen(&obs), ChoiceKind::Activate, "3000 still cannot beat their monster");
        obs.cards.pop();
        assert_ne!(chosen(&obs), ChoiceKind::Activate, "keep 3400 direct damage rather than limit it to 3000");
        obs.cards[1].attack = 800;
        assert_eq!(chosen(&obs), ChoiceKind::Activate, "3000 improves on 2600 direct damage");
        obs.phase = Some(Phase::Main2);
        assert_ne!(chosen(&obs), ChoiceKind::Activate);
    }

    #[test]
    fn pyramid_energy_selects_attack_to_win_battle_and_defense_to_save_a_blocker() {
        use crate::cards::types;
        const ENERGY: u32 = 76754619;
        let db = Arc::new(MemoryCards([creature(1, 4, 1800, 1700, 0),
            printed(ENERGY, types::SPELL | types::QUICKPLAY, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.phase = Some(Phase::Damage);
        obs.cards = vec![monster(0, 0, Some(1), Position::FACE_UP_ATTACK, 1800, 1700),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000),
            card(0, Location::SpellTrapZone, 0, ENERGY, false)];
        obs.battle_attacker = Some(obs.cards[0].at);
        obs.battle_target = Some(obs.cards[1].at);
        let window = decide(DecisionKind::Chain { forced: false, triggers: false }, None,
            vec![activate(ENERGY, Location::SpellTrapZone, 0), choice(ChoiceKind::Pass)]);
        let option = decide(DecisionKind::Option, None, vec![Choice { description: (ENERGY as u64) << 4,
            ..choice(ChoiceKind::Option) }, Choice { description: ((ENERGY as u64) << 4) | 1,
            ..choice(ChoiceKind::Option) }]);
        let answer = |obs: &Observation| {
            let mut policy = crate::registry::create("pyramid", db.clone()).unwrap();
            let kind = window.choices[policy.choose(obs, &window)].kind;
            (kind, policy.choose(obs, &option))
        };
        assert_eq!(answer(&obs), (ChoiceKind::Activate, 0));
        obs.cards[1].attack = 2000;
        assert_eq!(answer(&obs).0, ChoiceKind::Pass, "do not spend Energy just to trade");
        obs.cards[1].attack = 1900;
        obs.turn_player = Some(1);
        obs.cards[0].position = Position::FACE_UP_DEFENSE;
        obs.battle_attacker = Some(obs.cards[1].at);
        obs.battle_target = Some(obs.cards[0].at);
        assert_eq!(answer(&obs), (ChoiceKind::Activate, 1));
    }

    #[test]
    fn spellcaster_counts_mage_power_from_hand_and_from_its_set_zone() {
        use crate::cards::types;
        const MAGE: u32 = 83746708;
        const CITADEL: u32 = 39910367;
        let db = Arc::new(MemoryCards([creature(1, 4, 1800, 1000, 0),
            printed(MAGE, types::SPELL | types::EQUIP, 0), printed(CITADEL, types::SPELL | types::FIELD, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(1), Position::FACE_UP_ATTACK, 1800, 1000),
            card(0, Location::Hand, 0, MAGE, false), card(0, Location::SpellTrapZone, 5, CITADEL, true),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 2700, 1000)];
        let chosen = |obs: &Observation, location| {
            let mut policy = crate::registry::create("spellcaster", db.clone()).unwrap();
            let decision = decide(DecisionKind::Idle, None, vec![activate(MAGE, location, 0),
                choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
            decision.choices[policy.choose(obs, &decision)].kind
        };
        assert_eq!(chosen(&obs, Location::Hand), ChoiceKind::Activate, "Citadel plus the entering equip give 1000 ATK");
        obs.cards[1] = card(0, Location::SpellTrapZone, 0, MAGE, false);
        assert_eq!(chosen(&obs, Location::SpellTrapZone), ChoiceKind::Activate);
        obs.cards[3].attack = 2800;
        assert_ne!(chosen(&obs, Location::SpellTrapZone), ChoiceKind::Activate, "do not count a set equip twice");
    }

    #[test]
    fn crystal_recruits_ruby_for_two_companions_with_room_for_the_swarm() {
        const PEGASUS: u32 = 7093411;
        const RUBY: u32 = 32710364;
        const TIGER: u32 = 95600067;
        let db = Arc::new(MemoryCards([creature(PEGASUS, 4, 1800, 1200, 0x1034),
            creature(RUBY, 3, 300, 300, 0x1034), creature(TIGER, 4, 1600, 1000, 0x1034)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, PEGASUS, true), card(0, Location::SpellTrapZone, 1, TIGER, true)];
        let deck = select_one(Hint::SpecialSummon, vec![toggle(Location::Deck, 0, PEGASUS), toggle(Location::Deck, 1, RUBY)]);
        let chosen = |obs: &Observation, decision: &Decision| {
            let mut policy = crate::registry::create("crystal", db.clone()).unwrap();
            decision.choices[policy.choose(obs, decision)].code()
        };
        assert_eq!(chosen(&obs, &deck), Some(RUBY), "Ruby can summon both stored Beasts");
        obs.cards.extend((0..3).map(|i| monster(0, i, Some(TIGER), Position::FACE_UP_ATTACK, 1600, 1000)));
        assert_eq!(chosen(&obs, &deck), Some(PEGASUS), "two free zones cannot hold Ruby and two companions");
        obs.cards.truncate(2);
        obs.cards[1].code = Some(RUBY);
        let backrow = select_one(Hint::SpecialSummon, vec![toggle(Location::SpellTrapZone, 0, PEGASUS), toggle(Location::SpellTrapZone, 1, RUBY)]);
        assert_eq!(chosen(&obs, &backrow), Some(PEGASUS), "Ruby cannot count itself among its companions");
    }

    #[test]
    fn crystal_tree_spends_one_counter_and_counts_its_own_freed_zone() {
        use crate::cards::types;
        const TREE: u32 = 47408488;
        const TIGER: u32 = 95600067;
        let db = Arc::new(MemoryCards([printed(TREE, types::SPELL | types::CONTINUOUS, 0),
            creature(TIGER, 4, 1600, 1000, 0x1034)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, TREE, true)];
        obs.cards[0].counters = 1;
        let spend = decide(DecisionKind::Idle, None, vec![activate(TREE, Location::SpellTrapZone, 0), choice(ChoiceKind::EndTurn)]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("crystal", db.clone()).unwrap();
            spend.choices[policy.choose(obs, &spend)].kind
        };
        assert_eq!(chosen(&obs), ChoiceKind::Activate);
        obs.cards.extend((1..4).map(|i| card(0, Location::SpellTrapZone, i, TIGER, true)));
        obs.cards[0].counters = 2;
        assert_eq!(chosen(&obs), ChoiceKind::Activate, "one empty zone plus Tree's own zone fit two Beasts");
        obs.cards[0].counters = 0;
        let empty = decide(DecisionKind::Idle, None, vec![choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("crystal", db).unwrap();
        assert_eq!(policy.choose(&obs, &empty), 0);
    }

    #[test]
    fn crystal_abundance_clears_two_opposing_cards() {
        use crate::cards::types;
        const ABUNDANCE: u32 = 72881007;
        const TIGER: u32 = 95600067;
        let db = Arc::new(MemoryCards([printed(ABUNDANCE, types::SPELL, 0),
            creature(TIGER, 4, 1600, 1000, 0x1034)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = (0..4).map(|i| card(0, Location::SpellTrapZone, i, TIGER, true)).collect();
        obs.cards.extend([card(0, Location::Hand, 0, ABUNDANCE, false),
            monster(1, 0, None, Position::FACE_DOWN_DEFENSE, 0, 0),
            CardView { code: None, ..card(1, Location::SpellTrapZone, 0, 0, false) }]);
        let clear = decide(DecisionKind::Idle, None, vec![activate(ABUNDANCE, Location::Hand, 0), choice(ChoiceKind::EndTurn)]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("crystal", db.clone()).unwrap();
            clear.choices[policy.choose(obs, &clear)].kind
        };
        assert_eq!(chosen(&obs), ChoiceKind::Activate, "their identities are unnecessary for a field-wide send");
        obs.cards.pop();
        assert_eq!(chosen(&obs), ChoiceKind::EndTurn, "preserve the four stored Beasts against a single card");
    }

    #[test]
    fn crystal_release_adds_damage_on_an_open_field() {
        use crate::cards::types;
        const RELEASE: u32 = 10004783;
        const PEGASUS: u32 = 7093411;
        let db = Arc::new(MemoryCards([printed(RELEASE, types::SPELL | types::EQUIP, 0),
            creature(PEGASUS, 4, 1800, 1200, 0x1034)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, RELEASE, false),
            monster(0, 0, Some(PEGASUS), Position::FACE_UP_ATTACK, 1800, 1200)];
        let equip = decide(DecisionKind::Idle, None, vec![activate(RELEASE, Location::Hand, 0),
            choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("crystal", db.clone()).unwrap();
            equip.choices[policy.choose(obs, &equip)].kind
        };
        assert_eq!(chosen(&obs), ChoiceKind::Activate);
        obs.cards.push(monster(1, 0, Some(PEGASUS), Position::FACE_UP_ATTACK, 2700, 1200));
        assert_ne!(chosen(&obs), ChoiceKind::Activate, "the boost still cannot win this battle");
        obs.cards.pop();
        obs.phase = Some(Phase::Main2);
        assert_ne!(chosen(&obs), ChoiceKind::Activate);
    }

    #[test]
    fn crystal_uses_a_spare_ruby_but_prefers_pegasus() {
        const RUBY: u32 = 32710364;
        const PEGASUS: u32 = 7093411;
        let db = Arc::new(MemoryCards([creature(RUBY, 3, 300, 300, 0x1034),
            creature(PEGASUS, 4, 1800, 1200, 0x1034)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, RUBY, false)];
        let mut summon = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, RUBY, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("crystal", db.clone()).unwrap();
        assert_eq!(summon.choices[policy.choose(&obs, &summon)].code(), Some(RUBY));
        obs.cards.push(card(0, Location::Hand, 1, PEGASUS, false));
        summon.choices.push(from_hand(ChoiceKind::NormalSummon, PEGASUS, 1));
        let mut policy = crate::registry::create("crystal", db).unwrap();
        assert_eq!(summon.choices[policy.choose(&obs, &summon)].code(), Some(PEGASUS));
    }

    #[test]
    fn crystal_malefic_can_live_under_the_opponents_field_spell() {
        use crate::cards::types;
        const MALEFIC: u32 = 598988;
        const FIELD: u32 = 1;
        let db = Arc::new(MemoryCards([creature(MALEFIC, 10, 4000, 0, 0),
            printed(FIELD, types::SPELL | types::FIELD, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, MALEFIC, false), card(1, Location::SpellTrapZone, 5, FIELD, true)];
        let summon = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SpecialSummon, MALEFIC, 0), choice(ChoiceKind::EndTurn)]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("crystal", db.clone()).unwrap();
            summon.choices[policy.choose(obs, &summon)].kind
        };
        assert_eq!(chosen(&obs), ChoiceKind::SpecialSummon);
        obs.cards[1].position.face_up = false;
        obs.cards[1].code = None;
        assert_eq!(chosen(&obs), ChoiceKind::EndTurn, "an unknown set Field Spell does not sustain Malefic");
        obs.cards[1].position.face_up = true;
        obs.cards[1].code = Some(FIELD);
        obs.cards[1].at.sequence = 0;
        assert_eq!(chosen(&obs), ChoiceKind::EndTurn, "a Spell in the ordinary backrow is insufficient");
    }

    #[test]
    fn morphtronic_prioritizes_scopen_with_a_partner_then_celfon() {
        const SCOPEN: u32 = 10591919;
        const RADION: u32 = 55119278;
        const CELFON: u32 = 93542102;
        const REMOTEN: u32 = 57108202;
        let db = Arc::new(MemoryCards([creature(SCOPEN, 3, 800, 1400, 0x26),
            creature(RADION, 4, 1000, 900, 0x26), creature(CELFON, 1, 100, 100, 0x26),
            creature(REMOTEN, 3, 300, 1200, 0x26)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = [SCOPEN, RADION, CELFON, REMOTEN].into_iter().enumerate()
            .map(|(i, c)| card(0, Location::Hand, i as u32, c, false)).collect();
        let make_decision = |obs: &Observation| decide(DecisionKind::Idle, None,
            obs.cards.iter().map(|c| from_hand(ChoiceKind::NormalSummon, c.code.unwrap(), c.at.sequence))
                .chain([choice(ChoiceKind::EndTurn)]).collect());
        let chosen = |obs: &Observation| {
            let decision = make_decision(obs);
            let mut policy = crate::registry::create("morphtronic", db.clone()).unwrap();
            decision.choices[policy.choose(obs, &decision)].code()
        };
        assert_eq!(chosen(&obs), Some(SCOPEN), "Scopen can bring Radion from hand");
        obs.cards.remove(1);
        assert_eq!(chosen(&obs), Some(CELFON), "without a Level 4 partner, prefer Celfon's recruitment");
    }

    #[test]
    fn morphtronic_power_tool_starts_in_attack_to_use_its_equips() {
        const POWER_TOOL: u32 = 2403771;
        let db = Arc::new(MemoryCards([creature(POWER_TOOL, 7, 2300, 2500, 0), creature(1, 6, 2600, 1000, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 2600, 1000)];
        let position = decide(DecisionKind::Position, Some(POWER_TOOL), vec![choice(ChoiceKind::Position(Position::FACE_UP_ATTACK)),
            choice(ChoiceKind::Position(Position::FACE_UP_DEFENSE))]);
        let chosen = |obs: &Observation| {
            let mut policy = crate::registry::create("morphtronic", db.clone()).unwrap();
            position.choices[policy.choose(obs, &position)].kind
        };
        assert_eq!(chosen(&obs), ChoiceKind::Position(Position::FACE_UP_ATTACK));
        obs.turn_player = Some(1);
        assert_eq!(chosen(&obs), ChoiceKind::Position(Position::FACE_UP_DEFENSE));
    }

    #[test]
    fn morphtronic_equips_cord_before_changing_position_and_aims_at_enemy_backrow() {
        use crate::cards::types;
        const CORD: u32 = 70423794;
        const BOOMBOXEN: u32 = 92720564;
        let db = Arc::new(MemoryCards([printed(CORD, types::SPELL | types::EQUIP, 0),
            creature(BOOMBOXEN, 4, 1200, 400, 0x26)].into_iter().collect()));
        let mut obs = observation();
        obs.phase = Some(Phase::Main2);
        obs.cards = vec![monster(0, 0, Some(BOOMBOXEN), Position::FACE_UP_ATTACK, 1200, 400),
            card(0, Location::Hand, 0, CORD, false), CardView { code: None, ..card(1, Location::SpellTrapZone, 0, 0, false) }];
        let idle = decide(DecisionKind::Idle, None, vec![activate(CORD, Location::Hand, 0),
            Choice { kind: ChoiceKind::ChangePosition, ..activate(BOOMBOXEN, Location::MonsterZone, 0) }, choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("morphtronic", db).unwrap();
        assert_eq!(idle.choices[policy.choose(&obs, &idle)].code(), Some(CORD));
        obs.cards[1] = card(0, Location::SpellTrapZone, 0, CORD, true);
        let change = decide(DecisionKind::Idle, None, idle.choices[1..].to_vec());
        assert_eq!(change.choices[policy.choose(&obs, &change)].kind, ChoiceKind::ChangePosition);
        obs.cards[0].position = Position::FACE_UP_DEFENSE;
        let trigger = decide(DecisionKind::Chain { forced: true, triggers: true }, None,
            vec![activate(CORD, Location::SpellTrapZone, 0)]);
        assert_eq!(policy.choose(&obs, &trigger), 0);
        let target = select_one(Hint::Destroy, vec![toggle(Location::SpellTrapZone, 0, CORD), Choice {
            card: Some(Member { at: obs.cards[2].at, code: None, value: 0, required: false }), ..choice(ChoiceKind::Toggle) }]);
        assert_eq!(policy.choose(&obs, &target), 1, "target their unknown set card, not our own equip");
    }

    #[test]
    fn morphtronic_does_not_plan_to_equip_cord_to_power_tool() {
        use crate::cards::types;
        const CORD: u32 = 70423794;
        const POWER_TOOL: u32 = 2403771;
        const RADION: u32 = 55119278;
        let db = Arc::new(MemoryCards([printed(CORD, types::SPELL | types::EQUIP, 0),
            creature(POWER_TOOL, 7, 2300, 2500, 0), creature(RADION, 4, 1000, 900, 0x26)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(POWER_TOOL), Position::FACE_UP_DEFENSE, 2300, 2500),
            monster(0, 1, Some(RADION), Position::FACE_UP_ATTACK, 1800, 900),
            card(0, Location::Hand, 0, CORD, false), CardView { code: None, ..card(1, Location::SpellTrapZone, 0, 0, false) }];
        // Cord can legally equip Radion, but only Power Tool can change position now.
        let idle = decide(DecisionKind::Idle, None, vec![activate(CORD, Location::Hand, 0),
            Choice { kind: ChoiceKind::ChangePosition, ..activate(POWER_TOOL, Location::MonsterZone, 0) }, choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("morphtronic", db).unwrap();
        assert_eq!(idle.choices[policy.choose(&obs, &idle)].kind, ChoiceKind::ChangePosition);
    }
    #[test]
    fn gishki_searches_the_missing_ritual_piece() {
        use crate::cards::types;
        const OGRE: u32 = 57272170;
        const MIRROR: u32 = 46159582;
        const SHADOW: u32 = 29888389;
        let mut ritual = creature(OGRE, 8, 2800, 2800, 0x3a);
        ritual.1.kind |= types::RITUAL;
        let db = Arc::new(MemoryCards([
            ritual, printed(MIRROR, types::SPELL | types::RITUAL, 0),
            creature(SHADOW, 4, 1200, 1000, 0x3a),
        ].into_iter().collect()));
        let mut policy = crate::registry::create("gishki", db).unwrap();
        let mut obs = observation();
        let search = select_one(Hint::AddToHand, vec![toggle(Location::Deck, 0, OGRE), toggle(Location::Deck, 1, MIRROR), toggle(Location::Deck, 2, SHADOW)]);
        obs.cards = vec![card(0, Location::Hand, 0, OGRE, false)];
        assert_eq!(policy.choose(&obs, &search), 1);
        obs.cards = vec![card(0, Location::Hand, 0, MIRROR, false)];
        assert_eq!(policy.choose(&obs, &search), 0);
        obs.cards.push(card(0, Location::Hand, 1, OGRE, false));
        assert_eq!(policy.choose(&obs, &search), 2);
    }

    #[test]
    fn gishki_recovers_a_spare_ritual_monster() {
        use crate::cards::types;
        let mut ritual = creature(57272170, 8, 2800, 2800, 0x3a);
        ritual.1.kind |= types::RITUAL;
        let db = Arc::new(MemoryCards([ritual, printed(46159582, types::SPELL | types::RITUAL, 0)].into_iter().collect()));
        let mut policy = crate::registry::create("gishki", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, 57272170, false), card(0, Location::Graveyard, 0, 46159582, true), card(0, Location::Graveyard, 1, 21496848, true)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(46159582, Location::Graveyard, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 0);
    }

    #[test]
    fn gishki_flips_ariel_for_its_search() {
        use crate::cards::types;
        let mut ariel = creature(92784374, 4, 1000, 1800, 0x3a);
        ariel.1.kind |= types::FLIP;
        let mut policy = crate::registry::create("gishki", Arc::new(MemoryCards([ariel].into_iter().collect()))).unwrap();
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(92784374), Position::FACE_DOWN_DEFENSE, 1000, 1800), monster(1, 0, None, Position::FACE_UP_ATTACK, 2400, 1000)];
        let flip = Choice { kind: ChoiceKind::ChangePosition, ..activate(92784374, Location::MonsterZone, 0) };
        let idle = decide(DecisionKind::Idle, None, vec![flip, choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards[0].position = Position::FACE_UP_DEFENSE;
        assert_eq!(policy.choose(&obs, &idle), 1);
    }

    #[test]
    fn karakuri_flips_sazank_only_with_an_enemy_target() {
        let mut policy = crate::registry::create("karakuri", Arc::new(MemoryCards([creature(93724592, 3, 1200, 1200, 0x11)].into_iter().collect()))).unwrap();
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(93724592), Position::FACE_DOWN_DEFENSE, 1200, 1200), monster(1, 0, None, Position::FACE_UP_ATTACK, 2500, 1000)];
        let flip = Choice { kind: ChoiceKind::ChangePosition, ..activate(93724592, Location::MonsterZone, 0) };
        let idle = decide(DecisionKind::Idle, None, vec![flip, choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards[1].position = Position::FACE_DOWN_DEFENSE;
        assert_eq!(policy.choose(&obs, &idle), 1);
        obs.cards.pop();
        assert_eq!(policy.choose(&obs, &idle), 1);
    }

    #[test]
    fn karakuri_normal_summons_the_tuner_for_a_shogun() {
        use crate::cards::{types, races};
        let mut cards = vec![creature(39118197, 4, 1800, 600, 0x11), creature(66625883, 3, 500, 1600, 0x11), creature(70271583, 4, 600, 1800, 0x11), creature(30230789, 2, 500, 1500, 0x11)];
        for (_, card) in &mut cards { card.race = races::MACHINE; }
        cards[1].1.kind |= types::TUNER;
        cards[2].1.kind |= types::TUNER;
        let mut policy = crate::registry::create("karakuri", Arc::new(MemoryCards(cards.into_iter().collect()))).unwrap();
        let mut obs = observation();
        let mut body = monster(0, 0, Some(39118197), Position::FACE_UP_ATTACK, 1800, 600);
        body.level = 4;
        obs.cards = vec![body, card(0, Location::Hand, 0, 30230789, false), card(0, Location::Hand, 1, 66625883, false)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, 30230789, 0), from_hand(ChoiceKind::NormalSummon, 66625883, 1), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 1);
        obs.cards.remove(0);
        assert_eq!(policy.choose(&obs, &idle), 0);
    }

    #[test]
    fn gishki_uses_tetrogre_in_its_main_phase() {
        let mut policy = crate::registry::create("gishki", Arc::new(MemoryCards::default())).unwrap();
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(21496848), Position::FACE_UP_ATTACK, 2600, 2100)];
        let effect = activate(21496848, Location::MonsterZone, 0);
        let idle = decide(DecisionKind::Idle, None, vec![effect.clone(), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.turn_player = Some(1);
        let chain = decide(DecisionKind::Chain { forced: false, triggers: false }, None, vec![effect, choice(ChoiceKind::Pass)]);
        assert_eq!(policy.choose(&obs, &chain), 1);
    }

    #[test]
    fn karakuri_position_targets_consider_the_resulting_position() {
        let db = Arc::new(MemoryCards([creature(30230789, 2, 500, 1500, 0x11), creature(23874409, 7, 2600, 1900, 0x11)].into_iter().collect()));
        let mut policy = crate::registry::create("karakuri", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(30230789), Position::FACE_UP_ATTACK, 500, 1500), monster(0, 1, Some(23874409), Position::FACE_UP_ATTACK, 2600, 1900), monster(1, 0, None, Position::FACE_UP_DEFENSE, 3300, 3200)];
        let mut enemy = toggle(Location::MonsterZone, 0, 0);
        enemy.card.as_mut().unwrap().at.controller = 1;
        enemy.card.as_mut().unwrap().code = None;
        let target = select_one(Hint::Other(528), vec![toggle(Location::MonsterZone, 0, 30230789), toggle(Location::MonsterZone, 1, 23874409), enemy]);
        // Put the compulsory weak attacker in Defense, rather than strengthen their defender.
        assert_eq!(policy.choose(&obs, &target), 0);
        obs.cards[2].position = Position::FACE_UP_ATTACK;
        obs.cards[2].defense = 1000;
        assert_eq!(policy.choose(&obs, &target), 2);
        // An unrevealed monster gets no score based on its hidden identity or stats.
        obs.cards[2].position = Position::FACE_DOWN_DEFENSE;
        obs.cards[2].attack = 0;
        obs.cards[2].defense = 0;
        obs.cards[1].position = Position::FACE_UP_DEFENSE;
        assert_eq!(policy.choose(&obs, &target), 1);
    }

    #[test]
    fn gishki_can_use_meditation_in_its_own_open_main_window() {
        let mut policy = crate::registry::create("gishki", Arc::new(MemoryCards::default())).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, 46337945, false)];
        let window = decide(DecisionKind::Chain { forced: false, triggers: false }, None, vec![activate(46337945, Location::SpellTrapZone, 0), choice(ChoiceKind::Pass)]);
        assert_eq!(policy.choose(&obs, &window), 0);
        obs.turn_player = Some(1);
        assert_eq!(policy.choose(&obs, &window), 1);
        obs.phase = Some(Phase::End);
        assert_eq!(policy.choose(&obs, &window), 0);
    }

    #[test]
    fn karakuri_special_summons_cyber_dragon_before_its_normal_summon() {
        let db = Arc::new(MemoryCards([creature(70095154, 5, 2100, 1600, 0), creature(30230789, 2, 500, 1500, 0x11)].into_iter().collect()));
        let mut policy = crate::registry::create("karakuri", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, 70095154, false), card(0, Location::Hand, 1, 30230789, false), monster(1, 0, None, Position::FACE_UP_ATTACK, 2500, 1000)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SpecialSummon, 70095154, 0), from_hand(ChoiceKind::NormalSummon, 30230789, 1), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 0);
    }

    #[test]
    fn quickdraw_discards_level_eater_for_its_junk_warrior_line() {
        const QUICKDRAW: u32 = 20932152;
        const EATER: u32 = 57421866;
        const DANDY: u32 = 15341821;
        const WARRIOR: u32 = 60800381;
        let db = Arc::new(MemoryCards([creature(QUICKDRAW, 5, 700, 1400, 0),
            creature(EATER, 1, 600, 0, 0), creature(DANDY, 3, 300, 300, 0),
            creature(WARRIOR, 5, 2300, 1300, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, QUICKDRAW, false), card(0, Location::Hand, 1, EATER, false),
            card(0, Location::Hand, 2, DANDY, false), card(0, Location::Extra, 0, WARRIOR, false)];
        let summon = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SpecialSummon, QUICKDRAW, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("quickdraw-plant", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &summon), 0);
        let cost = select_one(Hint::ToGraveyard, vec![toggle(Location::Hand, 1, EATER), toggle(Location::Hand, 2, DANDY)]);
        assert_eq!(policy.choose(&obs, &cost), 0, "the planned combo needs Eater, despite Dandylion's usual discard priority");
        obs.cards.retain(|c| c.at.location != Location::Extra);
        let mut policy = crate::registry::create("quickdraw-plant", db).unwrap();
        assert_eq!(policy.choose(&obs, &summon), 1, "do not spend a card without the payoff in our Extra Deck");
    }

    #[test]
    fn quickdraw_uses_hand_and_graveyard_tuners_for_small_synchros() {
        use crate::cards::types;
        const BULB: u32 = 67441435;
        const TOKEN: u32 = 15341822;
        const FORMULA: u32 = 50091196;
        let mut bulb = creature(BULB, 1, 100, 100, 0);
        bulb.1.kind |= types::TUNER;
        let mut formula = creature(FORMULA, 2, 200, 1500, 0);
        formula.1.kind |= types::SYNCHRO | types::TUNER;
        let db = Arc::new(MemoryCards([bulb, formula, creature(TOKEN, 1, 0, 0, 0)].into_iter().collect()));
        for location in [Location::Hand, Location::Graveyard] {
            let action = if location == Location::Hand { from_hand(ChoiceKind::NormalSummon, BULB, 0) }
                else { activate(BULB, location, 0) };
            let idle = decide(DecisionKind::Idle, None, vec![action, choice(ChoiceKind::EndTurn)]);
            let mut obs = observation();
            obs.cards = vec![card(0, location, 0, BULB, false), card(0, Location::Extra, 0, FORMULA, false),
                CardView { level: 1, ..monster(0, 0, Some(TOKEN), Position::FACE_UP_DEFENSE, 0, 0) }];
            let mut policy = crate::registry::create("quickdraw-plant", db.clone()).unwrap();
            assert_eq!(policy.choose(&obs, &idle), 0, "Formula is a useful draw and Synchro Tuner");
            obs.cards.retain(|c| c.at.location != Location::Extra);
            let mut policy = crate::registry::create("quickdraw-plant", db.clone()).unwrap();
            assert_eq!(policy.choose(&obs, &idle), 1, "save the tuner without a Synchro payoff");
        }
    }

    #[test]
    fn quickdraw_junk_synchron_revives_a_non_tuner() {
        use crate::cards::types;
        const JUNK: u32 = 63977008;
        const BULB: u32 = 67441435;
        const DOPPEL: u32 = 53855409;
        let mut bulb = creature(BULB, 1, 100, 100, 0);
        bulb.1.kind |= types::TUNER;
        let db = Arc::new(MemoryCards([bulb, creature(DOPPEL, 2, 800, 800, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Graveyard, 0, BULB, true), card(0, Location::Graveyard, 1, DOPPEL, true)];
        let select = select_one(Hint::SpecialSummon, vec![toggle(Location::Graveyard, 0, BULB), toggle(Location::Graveyard, 1, DOPPEL)]);
        let mut policy = crate::registry::create("quickdraw-plant", db).unwrap();
        assert_eq!(policy.choose(&obs, &select), 0, "ordinary revivals keep their existing ranking");
        let trigger = decide(DecisionKind::Chain { forced: true, triggers: true }, None, vec![activate(JUNK, Location::MonsterZone, 0)]);
        assert_eq!(policy.choose(&obs, &trigger), 0);
        assert_eq!(policy.choose(&obs, &select), 1, "Junk is already a tuner; negating Bulb does not remove its tuner type");
    }

    #[test]
    fn x_saber_uses_one_for_one_before_its_normal_summon() {
        const ONE_FOR_ONE: u32 = 2295440;
        const AIRBELLUM: u32 = 90508760;
        let db = Arc::new(MemoryCards([creature(AIRBELLUM, 3, 1600, 200, 0x100d)].into_iter().collect()));
        let mut policy = crate::registry::create("x-saber", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, ONE_FOR_ONE, false), card(0, Location::Hand, 1, AIRBELLUM, false)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(ONE_FOR_ONE, Location::Hand, 0),
            from_hand(ChoiceKind::NormalSummon, AIRBELLUM, 1), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 0);
    }

    #[test]
    fn x_saber_flips_a_second_body_for_faultroll() {
        const FAULTROLL: u32 = 51808422;
        const EMMERSBLADE: u32 = 42737833;
        const PASHUUL: u32 = 23093604;
        let db = Arc::new(MemoryCards([creature(FAULTROLL, 6, 2400, 1800, 0x100d),
            creature(EMMERSBLADE, 3, 1300, 800, 0x100d), creature(PASHUUL, 2, 100, 0, 0x100d)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, FAULTROLL, false),
            monster(0, 0, Some(EMMERSBLADE), Position::FACE_UP_ATTACK, 1300, 800),
            monster(0, 1, Some(PASHUUL), Position::FACE_DOWN_DEFENSE, 100, 0),
            monster(1, 0, None, Position::FACE_UP_ATTACK, 3000, 1000)];
        let flip = decide(DecisionKind::Idle, None, vec![Choice { kind: ChoiceKind::ChangePosition,
            ..activate(PASHUUL, Location::MonsterZone, 1) }, choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("x-saber", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &flip), 0);
        obs.cards.retain(|c| c.at.location != Location::Hand);
        let mut policy = crate::registry::create("x-saber", db).unwrap();
        assert_eq!(policy.choose(&obs, &flip), 1, "do not expose a weak set monster without the Faultroll payoff");
    }

    #[test]
    fn x_saber_searches_a_small_body_when_faultroll_is_already_in_hand() {
        const FAULTROLL: u32 = 51808422;
        const AIRBELLUM: u32 = 90508760;
        let db = Arc::new(MemoryCards([creature(FAULTROLL, 6, 2400, 1800, 0x100d),
            creature(AIRBELLUM, 3, 1600, 200, 0x100d)].into_iter().collect()));
        let mut policy = crate::registry::create("x-saber", db).unwrap();
        let search = select_one(Hint::AddToHand, vec![toggle(Location::Deck, 0, FAULTROLL), toggle(Location::Deck, 1, AIRBELLUM)]);
        let mut obs = observation();
        assert_eq!(policy.choose(&obs, &search), 0);
        obs.cards.push(card(0, Location::Hand, 0, FAULTROLL, false));
        assert_eq!(policy.choose(&obs, &search), 1);
    }

    #[test]
    fn x_saber_sets_pashuul_without_a_synchro_play() {
        let mut pashuul = creature(23093604, 2, 100, 0, 0x100d);
        pashuul.1.kind |= crate::cards::types::TUNER;
        let db = Arc::new(MemoryCards([pashuul].into_iter().collect()));
        let mut policy = crate::registry::create("x-saber", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, 23093604, false), monster(1, 0, None, Position::FACE_UP_ATTACK, 2500, 1000)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, 23093604, 0),
            from_hand(ChoiceKind::SetMonster, 23093604, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 1);
    }

    #[test]
    fn x_saber_negates_small_summons_with_saber_hole() {
        const SABER_HOLE: u32 = 44901281;
        const LONEFIRE: u32 = 48686504;
        let db = Arc::new(MemoryCards([creature(LONEFIRE, 3, 500, 1400, 0)].into_iter().collect()));
        let mut policy = crate::registry::create("x-saber", db).unwrap();
        let mut obs = observation();
        obs.turn_player = Some(1);
        let summoned = monster(1, 0, Some(LONEFIRE), Position::FACE_UP_ATTACK, 500, 1400);
        obs.event_cards = vec![(summoned.at, summoned.code)];
        obs.cards = vec![summoned, card(0, Location::SpellTrapZone, 0, SABER_HOLE, false)];
        let window = decide(DecisionKind::Chain { forced: false, triggers: false }, None,
            vec![activate(SABER_HOLE, Location::SpellTrapZone, 0), choice(ChoiceKind::Pass)]);
        assert_eq!(policy.choose(&obs, &window), 0, "small engine monsters are worth stopping too");
        obs.event_cards.clear();
        assert_eq!(policy.choose(&obs, &window), 1, "require an opposing summon event");
    }

    #[test]
    fn dragunity_uses_vajrayana_to_extend_with_phalanx() {
        use crate::cards::types;
        const PHALANX: u32 = 59755122;
        const VAJRAYANA: u32 = 21249921;
        const GAIA: u32 = 97204936;
        let mut phalanx = creature(PHALANX, 2, 500, 1100, 0x29);
        phalanx.1.kind |= types::TUNER;
        let mut vajrayana = creature(VAJRAYANA, 6, 1900, 1200, 0x29);
        vajrayana.1.kind |= types::SYNCHRO;
        let mut gaia = creature(GAIA, 6, 2600, 800, 0);
        gaia.1.kind |= types::SYNCHRO;
        let db = Arc::new(MemoryCards([phalanx, vajrayana, gaia].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::MonsterZone, 0, PHALANX, true),
            card(0, Location::Extra, 0, VAJRAYANA, false), card(0, Location::Extra, 1, GAIA, false)];
        let idle = decide(DecisionKind::Idle, None, vec![
            Choice { kind: ChoiceKind::SpecialSummon, ..activate(VAJRAYANA, Location::Extra, 0) },
            Choice { kind: ChoiceKind::SpecialSummon, ..activate(GAIA, Location::Extra, 1) }, choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("dragunity", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards[0] = card(0, Location::Graveyard, 0, PHALANX, true);
        let mut policy = crate::registry::create("dragunity", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0, "Phalanx can also be equipped from an earlier turn");
        obs.cards.remove(0);
        let mut policy = crate::registry::create("dragunity", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "without Phalanx, keep Gaia's higher immediate value");
    }

    #[test]
    fn dragunity_discards_its_first_phalanx_for_the_equip_engine() {
        const PHALANX: u32 = 59755122;
        const JAVELIN: u32 = 80549379;
        let db = Arc::new(MemoryCards([creature(PHALANX, 2, 500, 1100, 0x29),
            creature(JAVELIN, 2, 1200, 800, 0x29)].into_iter().collect()));
        let mut policy = crate::registry::create("dragunity", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, PHALANX, false), card(0, Location::Hand, 1, JAVELIN, false)];
        let discard = select_one(Hint::Discard, vec![toggle(Location::Hand, 0, PHALANX), toggle(Location::Hand, 1, JAVELIN)]);
        assert_eq!(policy.choose(&obs, &discard), 0);
        obs.cards.push(card(0, Location::Graveyard, 0, PHALANX, true));
        assert_eq!(policy.choose(&obs, &discard), 1, "once stocked, preserve the more useful hand card");
    }

    #[test]
    fn dragunity_uses_ravine_with_one_card_left_to_discard() {
        const RAVINE: u32 = 62265044;
        let mut policy = crate::registry::create("dragunity", Arc::new(MemoryCards::default())).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 5, RAVINE, true), card(0, Location::Hand, 0, 59755122, false)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(RAVINE, Location::SpellTrapZone, 5), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards.pop();
        assert_eq!(policy.choose(&obs, &idle), 1);
    }

    #[test]
    fn dragunity_draws_with_a_single_non_phalanx_tuner() {
        const CONSONANCE: u32 = 39701395;
        const AKLYS: u32 = 36870345;
        let mut aklys = creature(AKLYS, 2, 1000, 800, 0x29);
        aklys.1.kind |= crate::cards::types::TUNER;
        let db = Arc::new(MemoryCards([aklys].into_iter().collect()));
        let mut policy = crate::registry::create("dragunity", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, CONSONANCE, false), card(0, Location::Hand, 1, AKLYS, false)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(CONSONANCE, Location::Hand, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards.pop();
        assert_eq!(policy.choose(&obs, &idle), 1);
    }

    #[test]
    fn dragunity_uses_icarus_only_with_two_opposing_cards() {
        const ICARUS: u32 = 53567095;
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, ICARUS, false),
            monster(0, 0, Some(28183605), Position::FACE_UP_ATTACK, 1700, 1200),
            monster(1, 0, None, Position::FACE_UP_ATTACK, 2500, 1000),
            CardView { code: None, ..card(1, Location::SpellTrapZone, 0, 0, false) }];
        for kind in [DecisionKind::Idle, DecisionKind::Chain { forced: false, triggers: false }] {
            let fallback = if kind == DecisionKind::Idle { ChoiceKind::EndTurn } else { ChoiceKind::Pass };
            let decision = decide(kind, None, vec![activate(ICARUS, Location::SpellTrapZone, 0), choice(fallback)]);
            let mut policy = crate::registry::create("dragunity", Arc::new(MemoryCards::default())).unwrap();
            assert_eq!(policy.choose(&obs, &decision), 0);
            let mut one_target = obs.clone();
            one_target.cards.pop();
            assert_eq!(policy.choose(&one_target, &decision), 1, "do not pay to destroy one of our own cards");
        }
    }

    #[test]
    fn gladiator_hand_searches_differ_from_tag_in_choices() {
        const LAQUARI: u32 = 78868776;
        const MURMILLO: u32 = 5975022;
        const BESTIARI: u32 = 41470137;
        let db = Arc::new(MemoryCards([creature(LAQUARI, 4, 1800, 400, 0x1019),
            creature(MURMILLO, 2, 800, 400, 0x1019), creature(BESTIARI, 4, 1500, 800, 0x1019),
            creature(25924653, 4, 1700, 300, 0x1019)].into_iter().collect()));
        let mut policy = crate::registry::create("gladiator", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![monster(1, 0, None, Position::FACE_UP_ATTACK, 2500, 1000)];
        let choices = vec![toggle(Location::Deck, 0, LAQUARI), toggle(Location::Deck, 1, MURMILLO), toggle(Location::Deck, 2, BESTIARI)];
        let hand = select_one(Hint::AddToHand, choices.clone());
        assert_eq!(policy.choose(&obs, &hand), 0, "Laquari is playable from hand; Murmillo's removal needs a tag-in");
        let tag = select_one(Hint::SpecialSummon, choices);
        assert_eq!(policy.choose(&obs, &tag), 1, "a real tag-in can use Murmillo's removal");
        obs.cards.push(monster(0, 0, Some(25924653), Position::FACE_UP_ATTACK, 1700, 300));
        assert_eq!(policy.choose(&obs, &hand), 2, "Bestiari completes contact Fusion with our existing Gladiator");
    }

    #[test]
    fn gladiator_uses_prisma_when_its_copied_name_enables_a_combo() {
        const PRISMA: u32 = 89312388;
        const EQUESTE: u32 = 57731460;
        const TEST_TIGER: u32 = 92373006;
        const GYZARUS: u32 = 48156348;
        const HERAKLINOS: u32 = 27346636;
        let db = Arc::new(MemoryCards([creature(PRISMA, 4, 1700, 1100, 0),
            creature(EQUESTE, 4, 1600, 1200, 0x1019)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(PRISMA), Position::FACE_UP_ATTACK, 1700, 1100)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(PRISMA, Location::MonsterZone, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("gladiator", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1);
        obs.cards.push(monster(0, 1, Some(EQUESTE), Position::FACE_UP_ATTACK, 1600, 1200));
        assert_eq!(policy.choose(&obs, &idle), 0);
        let reveal = select_one(Hint::Confirm, vec![toggle(Location::Extra, 0, GYZARUS), toggle(Location::Extra, 1, HERAKLINOS)]);
        assert_eq!(policy.choose(&obs, &reveal), 0, "copy Bestiari by revealing Gyzarus");
        obs.cards.pop();
        obs.cards.push(card(0, Location::Hand, 0, TEST_TIGER, false));
        let mut policy = crate::registry::create("gladiator", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0, "the copied name also enables Test Tiger");
    }

    #[test]
    fn gladiator_normal_summons_murmillo_to_enable_test_tiger() {
        const MURMILLO: u32 = 5975022;
        const TEST_TIGER: u32 = 92373006;
        let db = Arc::new(MemoryCards([creature(MURMILLO, 2, 800, 400, 0x1019)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, MURMILLO, false), card(0, Location::Hand, 1, TEST_TIGER, false)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, MURMILLO, 0),
            from_hand(ChoiceKind::SetMonster, MURMILLO, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("gladiator", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards.pop();
        let mut policy = crate::registry::create("gladiator", db).unwrap();
        assert_ne!(policy.choose(&obs, &idle), 0, "keep the normal veto without a Test Tiger follow-up");
    }

    #[test]
    fn gladiator_prefers_gyzarus_removal_when_both_contact_fusions_are_offered() {
        let mut obs = observation();
        obs.cards = vec![monster(1, 0, None, Position::FACE_UP_ATTACK, 2500, 1000)];
        let idle = decide(DecisionKind::Idle, None, vec![
            Choice { kind: ChoiceKind::SpecialSummon, ..activate(27346636, Location::Extra, 0) },
            Choice { kind: ChoiceKind::SpecialSummon, ..activate(48156348, Location::Extra, 1) }, choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("gladiator", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1);
        obs.cards.clear();
        assert_eq!(policy.choose(&obs, &idle), 0, "Heraklinos remains the choice on an empty opposing field");
    }

    #[test]
    fn dragunity_prefers_legionnaire_with_aklys_against_a_large_monster() {
        const DUX: u32 = 28183605;
        const LEGIONNAIRE: u32 = 54578613;
        const AKLYS: u32 = 36870345;
        let db = Arc::new(MemoryCards([creature(DUX, 4, 1500, 1000, 0x29),
            creature(LEGIONNAIRE, 3, 1200, 800, 0x29), creature(AKLYS, 2, 1000, 800, 0x29)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, DUX, false), card(0, Location::Hand, 1, LEGIONNAIRE, false),
            card(0, Location::Graveyard, 0, AKLYS, true), monster(1, 0, None, Position::FACE_UP_ATTACK, 2500, 1000)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, DUX, 0),
            from_hand(ChoiceKind::NormalSummon, LEGIONNAIRE, 1), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("dragunity", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1);
        obs.cards.last_mut().unwrap().attack = 2000;
        let mut policy = crate::registry::create("dragunity", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
    }

    #[test]
    fn gladiator_uses_darius_and_bestiari_to_prepare_gyzarus() {
        const DARIUS: u32 = 25924653;
        const BESTIARI: u32 = 41470137;
        const MURMILLO: u32 = 5975022;
        const LAQUARI: u32 = 78868776;
        const GYZARUS: u32 = 48156348;
        let db = Arc::new(MemoryCards([creature(DARIUS, 4, 1700, 300, 0x1019),
            creature(BESTIARI, 4, 1500, 800, 0x1019), creature(MURMILLO, 2, 800, 400, 0x1019),
            creature(LAQUARI, 4, 1800, 400, 0x1019)].into_iter().collect()));
        let mut policy = crate::registry::create("gladiator", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Graveyard, 0, BESTIARI, true), card(0, Location::Graveyard, 1, LAQUARI, true),
            card(0, Location::Extra, 0, GYZARUS, false), monster(1, 0, None, Position::FACE_UP_ATTACK, 2500, 1000)];
        let tag = select_one(Hint::SpecialSummon, vec![toggle(Location::Deck, 0, DARIUS), toggle(Location::Deck, 1, MURMILLO)]);
        assert_eq!(policy.choose(&obs, &tag), 0);
        obs.cards.push(monster(0, 0, Some(DARIUS), Position::FACE_UP_ATTACK, 1700, 300));
        let trigger = decide(DecisionKind::Chain { forced: true, triggers: true }, None, vec![activate(DARIUS, Location::MonsterZone, 0)]);
        assert_eq!(policy.choose(&obs, &trigger), 0);
        let revive = select_one(Hint::SpecialSummon, vec![toggle(Location::Graveyard, 0, BESTIARI), toggle(Location::Graveyard, 1, LAQUARI)]);
        assert_eq!(policy.choose(&obs, &revive), 0, "Darius plus Bestiari can contact fuse; Darius plus Laquari cannot");
    }

    #[test]
    fn gladiator_tags_into_equeste_to_recover_war_chariot() {
        const CHARIOT: u32 = 96216229;
        const EQUESTE: u32 = 57731460;
        const BESTIARI: u32 = 41470137;
        let db = Arc::new(MemoryCards([creature(EQUESTE, 4, 1600, 1200, 0x1019),
            creature(BESTIARI, 4, 1500, 800, 0x1019)].into_iter().collect()));
        let mut policy = crate::registry::create("gladiator", db).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Graveyard, 0, CHARIOT, true),
            CardView { code: None, ..card(1, Location::SpellTrapZone, 0, 0, false) }];
        let tag = select_one(Hint::SpecialSummon, vec![toggle(Location::Deck, 0, EQUESTE), toggle(Location::Deck, 1, BESTIARI)]);
        assert_eq!(policy.choose(&obs, &tag), 0);
        obs.cards.push(card(0, Location::Hand, 0, CHARIOT, false));
        assert_eq!(policy.choose(&obs, &tag), 1, "with Chariot already in hand, resume the usual tag priorities");
    }

    #[test]
    fn harpie_waits_for_opposing_backrow_before_using_hunting_ground() {
        const GROUND: u32 = 75782277;
        for location in [Location::Hand, Location::SpellTrapZone] {
            let mut policy = crate::registry::create("harpie", Arc::new(MemoryCards::default())).unwrap();
            let mut obs = observation();
            obs.cards = vec![card(0, location, 0, GROUND, false)];
            let idle = decide(DecisionKind::Idle, None, vec![activate(GROUND, location, 0), choice(ChoiceKind::EndTurn)]);
            assert_eq!(policy.choose(&obs, &idle), 1, "save the mandatory destruction effect for an opposing target");
            obs.cards.push(CardView { code: None, ..card(1, Location::SpellTrapZone, 0, 0, false) });
            assert_eq!(policy.choose(&obs, &idle), 0, "an unknown set card is a public target; its identity is unnecessary");
        }
    }

    #[test]
    fn harpie_keeps_queen_as_a_monster_without_backrow_to_hunt() {
        const QUEEN: u32 = 75064463;
        let db = Arc::new(MemoryCards([creature(QUEEN, 4, 1900, 1200, 0x64)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, QUEEN, false)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(QUEEN, Location::Hand, 0),
            from_hand(ChoiceKind::NormalSummon, QUEEN, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("harpie", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1);
        obs.cards.push(CardView { code: None, ..card(1, Location::SpellTrapZone, 0, 0, false) });
        let mut policy = crate::registry::create("harpie", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
    }

    #[test]
    fn harpie_falcon_returns_set_backrow_before_an_attacker() {
        const LADY: u32 = 91932350;
        const PRISON: u32 = 70342110;
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(LADY), Position::FACE_UP_ATTACK, 1600, 1400),
            card(0, Location::SpellTrapZone, 0, PRISON, false)];
        let cost = select_one(Hint::ReturnToHand, vec![toggle(Location::MonsterZone, 0, LADY),
            toggle(Location::SpellTrapZone, 0, PRISON)]);
        let mut policy = crate::registry::create("harpie", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(policy.choose(&obs, &cost), 1, "keep the other attacker for this Battle Phase");
    }

    #[test]
    fn harpie_destroys_hunting_ground_before_its_own_live_backrow() {
        const GROUND: u32 = 75782277;
        const PARTY: u32 = 77778835;
        let mut policy = crate::registry::create("harpie", Arc::new(MemoryCards::default())).unwrap();
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 5, GROUND, true), card(0, Location::SpellTrapZone, 0, PARTY, true)];
        let trigger = decide(DecisionKind::Chain { forced: true, triggers: true }, None,
            vec![activate(GROUND, Location::SpellTrapZone, 5)]);
        assert_eq!(policy.choose(&obs, &trigger), 0);
        let target = select_one(Hint::Destroy, vec![toggle(Location::SpellTrapZone, 0, PARTY),
            toggle(Location::SpellTrapZone, 5, GROUND)]);
        assert_eq!(policy.choose(&obs, &target), 1, "destroying Party would also destroy its revived Ladies");
    }

    #[test]
    fn harpie_revives_a_single_lady_in_main_phase() {
        const PARTY: u32 = 77778835;
        const QUEEN: u32 = 75064463;
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, PARTY, false), card(0, Location::Graveyard, 0, QUEEN, true),
            card(0, Location::Hand, 0, 75782277, false)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(PARTY, Location::SpellTrapZone, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("harpie", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards.pop();
        let mut policy = crate::registry::create("harpie", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "Party needs a discard");
    }

    #[test]
    fn harpie_party_rebuilds_an_empty_field_or_revives_multiple_ladies() {
        const PARTY: u32 = 77778835;
        const QUEEN: u32 = 75064463;
        const LADY: u32 = 91932350;
        let mut obs = observation();
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::Draw);
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, PARTY, false), card(0, Location::Graveyard, 0, QUEEN, true),
            card(0, Location::Hand, 0, 75782277, false)];
        let window = decide(DecisionKind::Chain { forced: false, triggers: false }, None,
            vec![activate(PARTY, Location::SpellTrapZone, 0), choice(ChoiceKind::Pass)]);
        let mut policy = crate::registry::create("harpie", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(policy.choose(&obs, &window), 0, "rebuild before an attack is declared");
        obs.cards.push(monster(0, 0, Some(LADY), Position::FACE_UP_ATTACK, 1600, 1400));
        let mut policy = crate::registry::create("harpie", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(policy.choose(&obs, &window), 1, "one revival can wait when we already have a defender");
        obs.cards.push(card(0, Location::Graveyard, 1, LADY, true));
        let mut policy = crate::registry::create("harpie", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(policy.choose(&obs, &window), 0, "a group revival is worth using in other windows too");
    }

    #[test]
    fn rock_block_summons_barbaros_beside_an_existing_monster_without_tributes() {
        const BARBAROS: u32 = 78651105;
        let db = Arc::new(MemoryCards([creature(BARBAROS, 8, 3000, 1200, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, BARBAROS, false),
            monster(0, 0, Some(45041488), Position::FACE_UP_ATTACK, 1900, 1200)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, BARBAROS, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("rock-block", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        let options = decide(DecisionKind::Option, None, vec![
            Choice { description: 1, ..choice(ChoiceKind::Option) },
            Choice { description: (BARBAROS as u64) << 20, ..choice(ChoiceKind::Option) },
            Choice { description: ((BARBAROS as u64) << 20) | 1, ..choice(ChoiceKind::Option) }]);
        assert_eq!(policy.choose(&obs, &options), 1, "select the actual no-tribute procedure, regardless of option ordering");
        for seq in 1..5 { obs.cards.push(monster(0, seq, Some(45041488), Position::FACE_UP_ATTACK, 1900, 1200)); }
        let mut policy = crate::registry::create("rock-block", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "a full field cannot use the no-tribute summon");
    }

    #[test]
    fn rock_block_still_preserves_rocks_for_upkeep_under_skill_drain() {
        const GUARDIAN: u32 = 45041488;
        const RAI_OH: u32 = 71564252;
        const DRAIN: u32 = 82732705;
        let mut guardian = creature(GUARDIAN, 4, 1900, 1200, 0x1d);
        guardian.1.race = crate::cards::races::ROCK;
        let db = Arc::new(MemoryCards([guardian, creature(RAI_OH, 4, 1900, 800, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, DRAIN, true), card(0, Location::Hand, 0, GUARDIAN, false),
            card(0, Location::Hand, 1, RAI_OH, false)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, GUARDIAN, 0),
            from_hand(ChoiceKind::NormalSummon, RAI_OH, 1), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("rock-block", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "Skill Drain does not switch off the upkeep cost");
        obs.cards.push(card(0, Location::Hand, 2, GUARDIAN, false));
        let mut policy = crate::registry::create("rock-block", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0, "the second Rock can pay upkeep after the summon");
    }

    #[test]
    fn rock_block_oppression_negates_the_opponent_but_not_our_own_summon() {
        const OPPRESSION: u32 = 93016201;
        let mut obs = observation();
        obs.life_points[0] = 900;
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, OPPRESSION, false),
            monster(1, 0, Some(44508094), Position::FACE_UP_ATTACK, 2500, 2000)];
        obs.event_cards = vec![(obs.cards[1].at, obs.cards[1].code)];
        let window = decide(DecisionKind::Chain { forced: false, triggers: false }, None,
            vec![Choice { description: (OPPRESSION as u64) << 20, ..activate(OPPRESSION, Location::SpellTrapZone, 0) }, choice(ChoiceKind::Pass)]);
        let mut policy = crate::registry::create("rock-block", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(policy.choose(&obs, &window), 0, "the opponent may summon during our turn, with Oppression still set");
        obs.life_points[0] = 800;
        let mut policy = crate::registry::create("rock-block", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(policy.choose(&obs, &window), 1, "leave Life Points after paying the cost");
        obs.life_points[0] = 8000;
        obs.cards[1].at.controller = 0;
        obs.event_cards = vec![(obs.cards[1].at, obs.cards[1].code)];
        for turn_player in [0, 1] {
            obs.turn_player = Some(turn_player);
            let mut policy = crate::registry::create("rock-block", Arc::new(MemoryCards::default())).unwrap();
            assert_eq!(policy.choose(&obs, &window), 1, "never mistake a negation option for merely flipping Oppression face-up");
        }
    }

    #[test]
    fn tele_dad_mills_plaguespreader_instead_of_stranding_both_malicious() {
        const MALICIOUS: u32 = 9411399;
        const PLAGUE: u32 = 33420078;
        let db = Arc::new(MemoryCards([creature(MALICIOUS, 6, 800, 800, 0),
            creature(PLAGUE, 2, 400, 200, 0)].into_iter().collect()));
        let send = select_one(Hint::ToGraveyard, vec![toggle(Location::Deck, 0, MALICIOUS), toggle(Location::Deck, 1, PLAGUE)]);
        let mut obs = observation();
        let mut policy = crate::registry::create("tele-dad", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &send), 0);
        for location in [Location::Hand, Location::Graveyard, Location::MonsterZone, Location::Banished] {
            obs.cards = vec![card(0, location, 0, MALICIOUS, true)];
            let mut policy = crate::registry::create("tele-dad", db.clone()).unwrap();
            assert_eq!(policy.choose(&obs, &send), 1, "another Malicious is already visible outside the Deck");
        }
    }

    #[test]
    fn rock_block_uses_grand_mole_against_large_monsters_unless_drained() {
        const MOLE: u32 = 80344569;
        const RAI: u32 = 71564252;
        let db = Arc::new(MemoryCards([creature(MOLE, 3, 900, 300, 0), creature(RAI, 4, 1900, 800, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, MOLE, false), card(0, Location::Hand, 1, RAI, false),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 2500, 2000)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, MOLE, 0),
            from_hand(ChoiceKind::NormalSummon, RAI, 1), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("rock-block", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards.push(card(1, Location::SpellTrapZone, 0, 82732705, true));
        let mut policy = crate::registry::create("rock-block", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "the bounce cannot resolve under Skill Drain");
    }

    #[test]
    fn rock_block_ad_changer_exposes_weak_defense_before_battle() {
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1200),
            monster(1, 0, Some(2), Position::FACE_UP_ATTACK, 2500, 1000),
            card(0, Location::Graveyard, 0, 96146814, true)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(96146814, Location::Graveyard, 0),
            choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("rock-block", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards[1].defense = 2500;
        let mut policy = crate::registry::create("rock-block", Arc::new(MemoryCards::default())).unwrap();
        assert_ne!(policy.choose(&obs, &idle), 0, "do not rotate an equally impenetrable defender");
    }


    #[test]
    fn lightsworn_revives_wulf_as_an_attacker_instead_of_valuing_it_as_a_discard() {
        const WULF: u32 = 58996430;
        const LUMINA: u32 = 95503687;
        let db = Arc::new(MemoryCards([creature(WULF, 4, 2100, 300, 0x38), creature(LUMINA, 3, 1000, 1000, 0x38)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Graveyard, 0, WULF, true), card(0, Location::Graveyard, 1, LUMINA, true)];
        let revive = select_one(Hint::SpecialSummon, vec![toggle(Location::Graveyard, 0, WULF), toggle(Location::Graveyard, 1, LUMINA)]);
        let mut policy = crate::registry::create("lightsworn", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &revive), 0);
        obs.cards = vec![card(0, Location::Hand, 0, WULF, false), card(0, Location::Hand, 1, LUMINA, false)];
        let discard = select_one(Hint::Discard, vec![toggle(Location::Hand, 0, WULF), toggle(Location::Hand, 1, LUMINA)]);
        let mut policy = crate::registry::create("lightsworn", db).unwrap();
        assert_eq!(policy.choose(&obs, &discard), 0, "Wulf is still the cheaper hand discard");
    }

    #[test]
    fn lightsworn_revives_plaguespreader_only_with_a_hand_card_and_synchro_route() {
        const PLAGUE: u32 = 33420078;
        const JAIN: u32 = 96235275;
        const GAIA: u32 = 97204936;
        let mut plague = creature(PLAGUE, 2, 400, 200, 0);
        plague.1.kind |= crate::cards::types::TUNER;
        let mut gaia = creature(GAIA, 6, 2600, 800, 0);
        gaia.1.kind |= crate::cards::types::SYNCHRO;
        let db = Arc::new(MemoryCards([plague, gaia, creature(JAIN, 4, 1800, 1200, 0x38)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(JAIN), Position::FACE_UP_ATTACK, 1800, 1200),
            card(0, Location::Graveyard, 0, PLAGUE, true), card(0, Location::Extra, 0, GAIA, false), card(0, Location::Hand, 0, 1, false)];
        obs.cards[0].level = 4;
        obs.pile_sizes = vec![(0, Location::Deck, 20)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(PLAGUE, Location::Graveyard, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("lightsworn", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards.pop();
        let mut policy = crate::registry::create("lightsworn", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "the revival needs a card to return to the Deck");
        obs.cards.pop();
        obs.cards.push(card(0, Location::Hand, 0, 1, false));
        let mut policy = crate::registry::create("lightsworn", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "do not revive without a useful Synchro route");
    }

    #[test]
    fn lightsworn_summons_judgment_dragon_before_adding_more_millers() {
        const JD: u32 = 57774843;
        const LYLA: u32 = 22624373;
        let db = Arc::new(MemoryCards([creature(JD, 8, 3000, 2600, 0), creature(LYLA, 4, 1700, 200, 0x38)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, LYLA, false), card(0, Location::Hand, 1, JD, false)];
        obs.pile_sizes = vec![(0, Location::Deck, 30)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, LYLA, 0),
            from_hand(ChoiceKind::SpecialSummon, JD, 1), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("lightsworn", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1);
        obs.pile_sizes = vec![(0, Location::Deck, 1)];
        let mut policy = crate::registry::create("lightsworn", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 2, "early sequencing must still respect the deck-out budget");
    }

    #[test]
    fn heroes_summons_stratos_before_spending_it_as_fusion_material() {
        const STRATOS: u32 = 40044918;
        let db = Arc::new(MemoryCards([creature(STRATOS, 4, 1800, 300, 0x3008)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, STRATOS, false), card(0, Location::Hand, 1, 27847700, false)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(27847700, Location::Hand, 1),
            from_hand(ChoiceKind::NormalSummon, STRATOS, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("heroes", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "resolve Stratos before using the Fusion spell");
    }

    #[test]
    fn heroes_values_absolute_zeros_leaving_field_effect_when_choosing_a_fusion() {
        const ZERO: u32 = 40854197;
        const GAIA: u32 = 16304628;
        let db = Arc::new(MemoryCards([creature(ZERO, 8, 2500, 2000, 0x3008), creature(GAIA, 6, 2200, 2600, 0x3008)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 2500, 2000)];
        let fusion = select_one(Hint::SpecialSummon, vec![toggle(Location::Extra, 0, GAIA), toggle(Location::Extra, 1, ZERO)]);
        let mut policy = crate::registry::create("heroes", db).unwrap();
        assert_eq!(policy.choose(&obs, &fusion), 1);
    }

    #[test]
    fn lightsworn_flips_ryko_for_removal_but_preserves_its_deck_budget() {
        const RYKO: u32 = 21502796;
        let mut ryko = creature(RYKO, 2, 200, 100, 0x38);
        ryko.1.kind |= crate::cards::types::FLIP;
        let mut art = ryko.clone();
        art.0 += 1;
        art.1.code = art.0;
        art.1.alias = RYKO;
        let db = Arc::new(MemoryCards([ryko, art].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(RYKO + 1), Position::FACE_DOWN_DEFENSE, 200, 100),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 2500, 2000)];
        obs.pile_sizes = vec![(0, Location::Deck, 20)];
        let idle = decide(DecisionKind::Idle, None, vec![Choice { kind: ChoiceKind::ChangePosition, ..activate(RYKO + 1, Location::MonsterZone, 0) }, choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("lightsworn", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0, "recognize alternate passcodes too");
        obs.pile_sizes = vec![(0, Location::Deck, 3)];
        let mut policy = crate::registry::create("lightsworn", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "do not initiate another three-card mill near deck-out");
        obs.pile_sizes = vec![(0, Location::Deck, 20)];
        obs.cards.pop();
        let mut policy = crate::registry::create("lightsworn", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "leave Ryko set when there is no opposing target");
    }

    #[test]
    fn heroes_preserves_a_winning_fusion_but_fuses_when_it_is_outmatched() {
        const TORNADO: u32 = 3642509;
        let mut tornado = creature(TORNADO, 8, 2800, 2200, 0x3008);
        tornado.1.kind |= crate::cards::types::FUSION;
        let db = Arc::new(MemoryCards([tornado].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(TORNADO), Position::FACE_UP_ATTACK, 2800, 2200),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1800, 1000)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(45906428, Location::Hand, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("heroes", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "do not spend another Fusion just to replace a winning body");
        obs.cards[1].attack = 3000;
        let mut policy = crate::registry::create("heroes", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0, "an opposing larger monster still justifies another Fusion");
    }

    #[test]
    fn lightsworn_uses_compulsory_on_an_extra_deck_monster_before_it_attacks() {
        const STARDUST: u32 = 44508094;
        const CED: u32 = 94192409;
        let mut stardust = creature(STARDUST, 8, 2500, 2000, 0);
        stardust.1.kind |= crate::cards::types::SYNCHRO;
        let db = Arc::new(MemoryCards([stardust, creature(1, 4, 1900, 1000, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.turn_player = Some(1);
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, CED, false),
            monster(1, 0, Some(STARDUST), Position::FACE_UP_ATTACK, 2500, 2000)];
        let window = decide(DecisionKind::Chain { forced: false, triggers: false }, None,
            vec![activate(CED, Location::SpellTrapZone, 0), choice(ChoiceKind::Pass)]);
        let mut policy = crate::registry::create("lightsworn", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &window), 0);
        obs.cards[1].code = Some(1);
        let mut policy = crate::registry::create("lightsworn", db).unwrap();
        assert_eq!(policy.choose(&obs, &window), 1, "keep the trap instead of returning a reusable Normal Summon without a threat");
    }

    #[test]
    fn lightsworn_judgment_dragon_accepts_two_turns_of_deck_but_not_one() {
        const JD: u32 = 57774843;
        let db = Arc::new(MemoryCards([creature(JD, 8, 3000, 2600, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, JD, false)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SpecialSummon, JD, 0), choice(ChoiceKind::EndTurn)]);
        obs.pile_sizes = vec![(0, Location::Deck, 11)];
        let mut policy = crate::registry::create("lightsworn", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.pile_sizes = vec![(0, Location::Deck, 10)];
        let mut policy = crate::registry::create("lightsworn", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "a nonlethal summon still needs two safe turns of Deck");
    }

    #[test]
    fn lightsworn_uses_judgment_wipe_for_a_lone_threat_without_spending_a_better_board() {
        const JD: u32 = 57774843;
        let db = Arc::new(MemoryCards([creature(JD, 8, 3000, 2600, 0), creature(1, 4, 1900, 1000, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(JD), Position::FACE_UP_ATTACK, 3000, 2600),
            monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000)];
        obs.pile_sizes = vec![(0, Location::Deck, 20)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(JD, Location::MonsterZone, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("lightsworn", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards.push(monster(0, 1, Some(1), Position::FACE_UP_ATTACK, 1900, 1000));
        let mut policy = crate::registry::create("lightsworn", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "do not sacrifice an equally valuable ally for the wipe");
    }
    #[test]
    fn samurai_kageki_brings_a_tuner_before_another_attacker() {
        const KAGEKI: u32 = 2511717;
        const KAGEMUSHA: u32 = 1498130;
        const ENISHI: u32 = 75116619;
        let mut tuner = creature(KAGEMUSHA, 2, 400, 1800, 0x3d);
        tuner.1.kind |= crate::cards::types::TUNER;
        let db = Arc::new(MemoryCards([tuner, creature(KAGEKI, 3, 200, 2000, 0x3d), creature(ENISHI, 4, 1700, 700, 0x3d)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(KAGEKI), Position::FACE_UP_ATTACK, 200, 2000)];
        let summon = select_one(Hint::SpecialSummon, vec![toggle(Location::Hand, 0, ENISHI), toggle(Location::Hand, 1, KAGEMUSHA)]);
        let mut policy = crate::registry::create("six-samurai", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &summon), 1);
        obs.cards.push(monster(0, 1, Some(KAGEMUSHA), Position::FACE_UP_DEFENSE, 400, 1800));
        let mut policy = crate::registry::create("six-samurai", db).unwrap();
        assert_eq!(policy.choose(&obs, &summon), 0, "do not fill the board with redundant Tuners");
    }

    #[test]
    fn machina_summons_cyber_dragon_before_filling_its_empty_field() {
        const CYBER: u32 = 70095154;
        const FORTRESS: u32 = 5556499;
        const GEARFRAME: u32 = 42940404;
        let db = Arc::new(MemoryCards([creature(CYBER, 5, 2100, 1600, 0), creature(FORTRESS, 7, 2500, 1600, 0), creature(GEARFRAME, 4, 1800, 0, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, CYBER, false), card(0, Location::Hand, 1, FORTRESS, false), card(0, Location::Hand, 2, GEARFRAME, false), monster(1, 0, Some(1), Position::FACE_UP_ATTACK, 1900, 1000)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::SpecialSummon, FORTRESS, 1), from_hand(ChoiceKind::NormalSummon, GEARFRAME, 2), from_hand(ChoiceKind::SpecialSummon, CYBER, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("machina", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 2);
    }

    #[test]
    fn machina_discards_fortress_before_spending_two_searching_machines() {
        const FORTRESS: u32 = 5556499;
        const GEARFRAME: u32 = 42940404;
        const GADGET: u32 = 86445415;
        let db = Arc::new(MemoryCards([creature(FORTRESS, 7, 2500, 1600, 0), creature(GEARFRAME, 4, 1800, 0, 0), creature(GADGET, 4, 1300, 1500, 0)].into_iter().collect()));
        let obs = observation();
        let mut discard = select_one(Hint::Discard, vec![toggle(Location::Hand, 0, GEARFRAME), toggle(Location::Hand, 1, GADGET), toggle(Location::Hand, 2, FORTRESS)]);
        discard.kind = DecisionKind::SelectSum;
        let mut policy = crate::registry::create("machina", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &discard), 2);
        let search = select_one(Hint::AddToHand, vec![toggle(Location::Deck, 0, GEARFRAME), toggle(Location::Deck, 1, FORTRESS)]);
        let mut policy = crate::registry::create("machina", db).unwrap();
        assert_eq!(policy.choose(&obs, &search), 1, "cheap discard does not reduce Fortress search priority");
    }

    #[test]
    fn samurai_activates_another_united_before_the_next_summon() {
        const UNITED: u32 = 72345736;
        const KAGEKI: u32 = 2511717;
        let db = Arc::new(MemoryCards([creature(KAGEKI, 3, 200, 2000, 0x3d)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, UNITED, true), card(0, Location::Hand, 0, UNITED, false)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, KAGEKI, 1), activate(UNITED, Location::Hand, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("six-samurai", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1);
    }

    #[test]
    fn samurai_searches_a_tuner_or_a_free_summon_using_the_public_normal_summon_state() {
        const KAGEKI: u32 = 2511717;
        const KAGEMUSHA: u32 = 1498130;
        const KIZAN: u32 = 49721904;
        let mut tuner = creature(KAGEMUSHA, 2, 400, 1800, 0x3d);
        tuner.1.kind |= crate::cards::types::TUNER;
        let db = Arc::new(MemoryCards([tuner, creature(KAGEKI, 3, 200, 2000, 0x3d), creature(KIZAN, 4, 1800, 500, 0x3d)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::Hand, 0, KAGEKI, false)];
        let search = select_one(Hint::AddToHand, vec![toggle(Location::Deck, 0, KAGEKI), toggle(Location::Deck, 1, KAGEMUSHA), toggle(Location::Deck, 2, KIZAN)]);
        let mut policy = crate::registry::create("six-samurai", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &search), 1, "Kageki in hand can bring the missing Tuner");
        obs.cards = vec![monster(0, 0, Some(KAGEKI), Position::FACE_UP_ATTACK, 200, 2000)];
        obs.cards[0].level = 3;
        obs.summon_used = true;
        let mut policy = crate::registry::create("six-samurai", db).unwrap();
        assert_eq!(policy.choose(&obs, &search), 2, "after the Normal Summon, take a Samurai that can Special Summon itself");
    }

    #[test]
    fn samurai_normal_summons_a_tuner_only_when_it_connects_to_its_extra_deck() {
        const KIZAN: u32 = 49721904;
        const SQUIRE: u32 = 33883834;
        const SHI_EN: u32 = 29981921;
        let mut tuner = creature(SQUIRE, 1, 100, 100, 0x3d);
        tuner.1.kind |= crate::cards::types::TUNER;
        let mut shi_en = creature(SHI_EN, 5, 2500, 1400, 0x3d);
        shi_en.1.kind |= crate::cards::types::SYNCHRO;
        let db = Arc::new(MemoryCards([tuner, shi_en, creature(KIZAN, 4, 1800, 500, 0x3d)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(KIZAN), Position::FACE_UP_ATTACK, 1800, 500), card(0, Location::Extra, 0, SHI_EN, false)];
        obs.cards[0].level = 4;
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, SQUIRE, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("six-samurai", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.cards.pop();
        let mut policy = crate::registry::create("six-samurai", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "do not summon the fragile hand protector without a Synchro route");
    }

    #[test]
    fn samurai_revives_kagemusha_to_make_a_synchro_instead_of_waiting_for_a_tuner() {
        const KAGEMUSHA: u32 = 1498130;
        const KAGEKI: u32 = 2511717;
        const SHI_EN: u32 = 29981921;
        const RETURN: u32 = 46874015;
        const DOUBLE: u32 = 21007444;
        let mut tuner = creature(KAGEMUSHA, 2, 400, 1800, 0x3d);
        tuner.1.kind |= crate::cards::types::TUNER;
        let mut shi_en = creature(SHI_EN, 5, 2500, 1400, 0x3d);
        shi_en.1.kind |= crate::cards::types::SYNCHRO;
        let db = Arc::new(MemoryCards([tuner, shi_en, creature(KAGEKI, 3, 200, 2000, 0x3d)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(KAGEKI), Position::FACE_UP_ATTACK, 200, 2000), card(0, Location::Graveyard, 0, KAGEMUSHA, true), card(0, Location::Extra, 0, SHI_EN, false)];
        obs.cards[0].level = 3;
        let revive = decide(DecisionKind::Idle, None, vec![activate(RETURN, Location::SpellTrapZone, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("six-samurai", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &revive), 0);
        obs.cards[0] = card(0, Location::Graveyard, 1, KAGEKI, true);
        let pair = decide(DecisionKind::Idle, None, vec![activate(DOUBLE, Location::SpellTrapZone, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("six-samurai", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &pair), 0);
        obs.life_points[0] = 4000;
        let mut policy = crate::registry::create("six-samurai", db).unwrap();
        assert_eq!(policy.choose(&obs, &pair), 1, "keep the existing LP reserve for the two-body revival");
    }

    #[test]
    fn gravekeeper_recognizes_visionarys_single_gravekeeper_tribute() {
        const VISIONARY: u32 = 3825890;
        const SPY: u32 = 24317029;
        let db = Arc::new(MemoryCards([creature(VISIONARY, 8, 2000, 1800, 0x2e), creature(SPY, 4, 1200, 2000, 0x2e)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(SPY), Position::FACE_UP_ATTACK, 1200, 2000)];
        let idle = decide(DecisionKind::Idle, None, vec![from_hand(ChoiceKind::NormalSummon, VISIONARY, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("gravekeeper", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0, "one Gravekeeper pays the alternative Tribute procedure");
        let options = decide(DecisionKind::Option, Some(VISIONARY), vec![Choice { description: 1, ..choice(ChoiceKind::Option) }, Choice { description: (VISIONARY as u64) << 4, ..choice(ChoiceKind::Option) }]);
        assert_eq!(policy.choose(&obs, &options), 1, "use the one-Tribute procedure when both are offered");
        obs.cards.clear();
        let mut policy = crate::registry::create("gravekeeper", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1);
    }

    #[test]
    fn gravekeeper_deploys_its_last_hand_monster_before_royal_tribute() {
        const VALLEY: u32 = 47355498;
        const TRIBUTE: u32 = 72405967;
        const SPEAR: u32 = 63695531;
        let db = Arc::new(MemoryCards([creature(SPEAR, 4, 1500, 1000, 0x2e)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![card(0, Location::SpellTrapZone, 5, VALLEY, true), card(0, Location::Hand, 0, SPEAR, false)];
        obs.pile_sizes = vec![(1, Location::Hand, 5)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(TRIBUTE, Location::Hand, 1), from_hand(ChoiceKind::NormalSummon, SPEAR, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("gravekeeper", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1);
        obs.summon_used = true;
        obs.cards[1] = monster(0, 0, Some(SPEAR), Position::FACE_UP_ATTACK, 2000, 1500);
        let idle = decide(DecisionKind::Idle, None, vec![activate(TRIBUTE, Location::Hand, 0), choice(ChoiceKind::EndTurn)]);
        assert_eq!(policy.choose(&obs, &idle), 0);
    }

    #[test]
    fn gravekeeper_does_not_cycle_treasure_into_its_own_necrovalley_lock() {
        const VALLEY: u32 = 47355498;
        const TREASURE: u32 = 63571750;
        const CHIEF: u32 = 62473983;
        let db = Arc::new(MemoryCards::default());
        let mut obs = observation();
        obs.turn_player = Some(1);
        obs.phase = Some(Phase::End);
        obs.chain_known = true;
        obs.cards = vec![card(0, Location::SpellTrapZone, 0, TREASURE, false), card(0, Location::SpellTrapZone, 5, VALLEY, true)];
        let chain = decide(DecisionKind::Chain { forced: false, triggers: false }, None, vec![activate(TREASURE, Location::SpellTrapZone, 0), choice(ChoiceKind::Pass)]);
        let mut policy = crate::registry::create("gravekeeper", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &chain), 1);
        obs.cards.push(monster(0, 0, Some(CHIEF), Position::FACE_UP_ATTACK, 2400, 1700));
        let mut policy = crate::registry::create("gravekeeper", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &chain), 0, "Chief exempts our Graveyard");
        obs.cards.retain(|c| c.code != Some(CHIEF) && c.code != Some(VALLEY));
        let mut policy = crate::registry::create("gravekeeper", db).unwrap();
        assert_eq!(policy.choose(&obs, &chain), 0, "recovery remains available without Necrovalley");
    }

    #[test]
    fn machina_keeps_its_own_machines_but_contact_fuses_with_the_opponents() {
        const CYBER: u32 = 70095154;
        const FORTRESS: u32 = 5556499;
        const GEARFRAME: u32 = 42940404;
        const CHIMERATECH: u32 = 79229522;
        let mut entries = [creature(CYBER, 5, 2100, 1600, 0), creature(FORTRESS, 7, 2500, 1600, 0), creature(GEARFRAME, 4, 1800, 0, 0)];
        for e in &mut entries { e.1.race = crate::cards::races::MACHINE; }
        let db = Arc::new(MemoryCards(entries.into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(CYBER), Position::FACE_UP_ATTACK, 2100, 1600), monster(0, 1, Some(FORTRESS), Position::FACE_UP_ATTACK, 2500, 1600), monster(0, 2, Some(GEARFRAME), Position::FACE_UP_ATTACK, 1800, 0)];
        let idle = decide(DecisionKind::Idle, None, vec![Choice { kind: ChoiceKind::SpecialSummon, ..activate(CHIMERATECH, Location::Extra, 0) }, choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("machina", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1, "do not compress three useful attackers into one");
        obs.cards.push(monster(1, 0, Some(FORTRESS), Position::FACE_UP_ATTACK, 2500, 1600));
        let mut policy = crate::registry::create("machina", db).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
    }

    #[test]
    fn machina_uses_troopers_boost_with_a_battle_phase_and_a_deck_reserve() {
        const TROOPER: u32 = 85087012;
        let db = Arc::new(MemoryCards([creature(TROOPER, 3, 400, 400, 0)].into_iter().collect()));
        let mut obs = observation();
        obs.cards = vec![monster(0, 0, Some(TROOPER), Position::FACE_UP_ATTACK, 400, 400)];
        obs.pile_sizes = vec![(0, Location::Deck, 7)];
        let idle = decide(DecisionKind::Idle, None, vec![activate(TROOPER, Location::MonsterZone, 0), choice(ChoiceKind::EnterBattle), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("machina", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 0);
        obs.pile_sizes[0].2 = 6;
        let mut policy = crate::registry::create("machina", db.clone()).unwrap();
        assert_eq!(policy.choose(&obs, &idle), 1);
        obs.pile_sizes[0].2 = 7;
        let no_battle = decide(DecisionKind::Idle, None, vec![activate(TROOPER, Location::MonsterZone, 0), choice(ChoiceKind::EndTurn)]);
        let mut policy = crate::registry::create("machina", db).unwrap();
        assert_eq!(policy.choose(&obs, &no_battle), 1);
    }
}
