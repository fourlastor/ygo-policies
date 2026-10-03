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
        entry!("countdown", countdown::Countdown),
        entry!("verdict", verdict::Verdict),
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
}
