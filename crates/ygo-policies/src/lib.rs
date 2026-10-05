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
}
