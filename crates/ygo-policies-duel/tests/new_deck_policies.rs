//! Focused decisions using the actual card database, plus list/registry checks.
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use ygo_policies::{
    agent::{member_score, Memory, Turn},
    cards::{types, CardDatabase},
    ctx::Ctx,
    decks::{fabled::Fabled, macro_dd::MacroDd},
    knowledge,
    model::*,
    registry,
};
use ygo_policies_ocgcore::SqliteCards;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn db() -> Arc<dyn CardDatabase> {
    Arc::new(SqliteCards::open(root().join("vendor/BabelCdb/cards.cdb")).unwrap())
}
fn obs() -> Observation {
    Observation {
        me: 0,
        turn: 2,
        turn_player: Some(0),
        phase: Some(Phase::Main1),
        life_points: [8000, 8000],
        cards: vec![],
        pile_sizes: vec![(0, Location::Deck, 30)],
        chain: vec![],
        battle_attacker: None,
        battle_target: None,
        event_cards: vec![],
        summon_used: false,
        chain_known: true,
        can_attack_known: true,
        coin_toss: None,
    }
}
fn card(db: &dyn CardDatabase, p: u8, loc: Location, seq: u32, code: u32) -> CardView {
    let d = db.card(code).unwrap();
    CardView {
        at: CardRef {
            controller: p,
            location: loc,
            sequence: seq,
        },
        code: Some(code),
        position: Position::FACE_UP_ATTACK,
        attack: d.attack,
        defense: d.defense,
        level: d.level,
        can_attack: true,
        battles: 0,
        counters: 0,
        coin_effect: None,
    }
}
fn choice(kind: ChoiceKind, card: Option<&CardView>) -> Choice {
    Choice {
        kind,
        card: card.map(|c| Member {
            at: c.at,
            code: c.code,
            value: 0,
            required: false,
        }),
        members: vec![],
        description: 0,
        place: None,
    }
}
fn decision(kind: DecisionKind, choices: Vec<Choice>) -> Decision {
    Decision {
        kind,
        hint: Hint::None,
        minimum: 0,
        maximum: 0,
        selected: vec![],
        subject: None,
        choices,
    }
}
fn picks(id: &str, db: Arc<dyn CardDatabase>, o: &Observation, d: &Decision) -> usize {
    registry::create_seeded(id, db, 7).unwrap().choose(o, d)
}

#[test]
fn new_registered_lists_are_legal_and_have_scripts() {
    let db = db();
    let limits: HashMap<u32, usize> =
        std::fs::read_to_string(root().join("data/wc2011.lflist.conf"))
            .unwrap()
            .lines()
            .filter_map(|s| {
                let mut x = s.split_whitespace();
                Some((x.next()?.parse().ok()?, x.next()?.parse().ok()?))
            })
            .collect();
    for entry in registry::POLICIES.iter().skip(32) {
        let id = entry.id;
        let text =
            std::fs::read_to_string(root().join("decks").join(format!("{}.ydk", entry.deck)))
                .unwrap();
        let mut piles = [vec![], vec![], vec![]];
        let mut p = 0;
        for s in text.lines() {
            match s {
                "#main" => p = 0,
                "#extra" => p = 1,
                "!side" => p = 2,
                _ => {
                    if let Ok(code) = s.parse::<u32>() {
                        piles[p].push(code);
                    }
                }
            }
        }
        assert_eq!(piles[0].len(), 40, "{id}");
        assert!(piles[1].len() <= 15 && piles[2].len() <= 15);
        let mut counts = HashMap::new();
        for (pile, codes) in piles.iter().enumerate() {
            for &code in codes {
                let d = db.card(code).unwrap();
                let canonical = if d.alias == 0 { code } else { d.alias };
                let n = counts.entry(canonical).or_insert(0);
                *n += 1;
                assert!(
                    *n <= *limits.get(&canonical).unwrap_or(&0),
                    "{id}: {code} outside pool or over limit"
                );
                if pile < 2 {
                    assert_eq!(d.is_extra(), pile == 1, "{id}: {code} in wrong pile");
                }
                if d.is(types::EFFECT) || d.is_spell() || d.is_trap() {
                    assert!(
                        root()
                            .join(format!("vendor/CardScripts/official/c{canonical}.lua"))
                            .exists(),
                        "{id}: missing script for {code}"
                    );
                }
            }
        }
        let p = (entry.build)(db.clone(), 7);
        assert!(p.fork().is_some(), "{id} must be forkable");
    }
}

#[test]
fn fabled_discards_for_triggers_but_does_not_treat_sending_as_discarding() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 82888408),
        card(db.as_ref(), 0, Location::Hand, 1, 97651498),
    ];
    let mut d = decision(DecisionKind::SelectCards, vec![]);
    d.hint = Hint::Discard;
    let mut memory = Memory::default();
    memory.last_activated = Some(29905795);
    let m = choice(ChoiceKind::Toggle, Some(&o.cards[0])).card.unwrap();
    let score = member_score(
        &Fabled,
        &Turn {
            ctx: Ctx::new(&o, db.as_ref()),
            decision: &d,
            memory: &mut memory,
        },
        &m,
    );
    assert!(score > 0.0);
    d.hint = Hint::ToGraveyard;
    assert!(
        member_score(
            &Fabled,
            &Turn {
                ctx: Ctx::new(&o, db.as_ref()),
                decision: &d,
                memory: &mut memory
            },
            &m
        ) < 0.0
    );
    d.hint = Hint::Discard;
    o.cards
        .push(card(db.as_ref(), 1, Location::SpellTrapZone, 0, 30241314));
    assert!(
        member_score(
            &Fabled,
            &Turn {
                ctx: Ctx::new(&o, db.as_ref()),
                decision: &d,
                memory: &mut memory
            },
            &m
        ) < 0.0
    );
}

#[test]
fn fabled_uses_ragin_with_an_empty_hand() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Extra, 0, 47395382),
        card(db.as_ref(), 0, Location::Extra, 1, 43385557),
    ];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::SpecialSummon, Some(&o.cards[1])),
            choice(ChoiceKind::SpecialSummon, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("fabled", db, &o, &d), 1);
}

#[test]
fn catsith_is_not_discarded_when_our_removal_takes_its_only_safe_target() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 56399890),
        card(db.as_ref(), 0, Location::Hand, 1, 82888408),
        card(db.as_ref(), 1, Location::MonsterZone, 0, 89631139),
    ];
    let mut d = decision(DecisionKind::SelectCards, vec![]);
    d.hint = Hint::Discard;
    let mut memory = Memory::default();
    memory.last_activated = Some(63356631);
    memory.intent = vec![(o.cards[2].at, o.cards[2].code)];
    let t = Turn {
        ctx: Ctx::new(&o, db.as_ref()),
        decision: &d,
        memory: &mut memory,
    };
    let score = |n| {
        member_score(
            &Fabled,
            &t,
            &choice(ChoiceKind::Toggle, Some(&o.cards[n])).card.unwrap(),
        )
    };
    assert!(score(1) > score(0));
    assert!(score(0) < -5000.0);
}

#[test]
fn fabled_can_stack_its_independent_discard_triggers() {
    let db = db();
    let mut o = obs();
    let c = card(db.as_ref(), 0, Location::Graveyard, 0, 82888408);
    o.cards.push(c.clone());
    o.chain.push(ChainLink {
        code: 18282103,
        controller: 0,
        source: c.at,
        targets: vec![],
    });
    let d = decision(
        DecisionKind::Chain {
            forced: false,
            triggers: true,
        },
        vec![
            choice(ChoiceKind::Activate, Some(&c)),
            choice(ChoiceKind::Pass, None),
        ],
    );
    assert_eq!(picks("fabled", db, &o, &d), 0);
}

#[test]
fn counter_fairies_negate_the_opponents_link_only() {
    let db = db();
    let mut o = obs();
    let mut trap = card(db.as_ref(), 0, Location::SpellTrapZone, 0, 81066751);
    trap.position.face_up = false;
    o.cards = vec![
        trap.clone(),
        card(db.as_ref(), 0, Location::SpellTrapZone, 5, 56433456),
    ];
    o.chain.push(ChainLink {
        code: 53129443,
        controller: 1,
        source: CardRef {
            controller: 1,
            location: Location::SpellTrapZone,
            sequence: 0,
        },
        targets: vec![],
    });
    let d = decision(
        DecisionKind::Chain {
            forced: false,
            triggers: false,
        },
        vec![
            choice(ChoiceKind::Activate, Some(&trap)),
            choice(ChoiceKind::Pass, None),
        ],
    );
    assert_eq!(picks("counter-fairy", db.clone(), &o, &d), 0);
    o.chain[0].controller = 0;
    o.chain[0].source.controller = 0;
    assert_eq!(picks("counter-fairy", db, &o, &d), 1);
}

#[test]
fn counter_fairies_search_sanctuary_and_accept_vandalgyon_from_hand() {
    let db = db();
    let mut o = obs();
    let z = card(db.as_ref(), 0, Location::Hand, 0, 12171659);
    o.cards.push(z.clone());
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::NormalSummon, Some(&z)),
            choice(ChoiceKind::Activate, Some(&z)),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("counter-fairy", db.clone(), &o, &d), 1);
    let v = card(db.as_ref(), 0, Location::Hand, 1, 24857466);
    o.cards.push(v.clone());
    let d = decision(
        DecisionKind::Chain {
            forced: false,
            triggers: true,
        },
        vec![
            choice(ChoiceKind::Activate, Some(&v)),
            choice(ChoiceKind::Pass, None),
        ],
    );
    assert_eq!(picks("counter-fairy", db, &o, &d), 0);
}

#[test]
fn macro_activates_banishment_before_tributing_survivor() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 81674782),
        card(db.as_ref(), 0, Location::Hand, 1, 9748752),
        card(db.as_ref(), 0, Location::MonsterZone, 0, 48092532),
        card(db.as_ref(), 1, Location::MonsterZone, 0, 44508094),
    ];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::NormalSummon, Some(&o.cards[1])),
            choice(ChoiceKind::Activate, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("macro-dd", db, &o, &d), 1);
}

#[test]
fn macro_does_not_summon_a_monarch_into_its_own_empty_opposing_field() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 9748752),
        card(db.as_ref(), 0, Location::MonsterZone, 0, 48092532),
    ];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::NormalSummon, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("macro-dd", db, &o, &d), 1);
}

#[test]
fn allure_banishes_scout_plane_not_survivor_for_a_free_return() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 3773196),
        card(db.as_ref(), 0, Location::Hand, 1, 48092532),
    ];
    let mut d = decision(DecisionKind::SelectCards, vec![]);
    d.hint = Hint::Banish;
    let mut memory = Memory::default();
    memory.last_activated = Some(1475311);
    let t = Turn {
        ctx: Ctx::new(&o, db.as_ref()),
        decision: &d,
        memory: &mut memory,
    };
    let scores: Vec<_> = o
        .cards
        .iter()
        .map(|c| {
            member_score(
                &MacroDd,
                &t,
                &choice(ChoiceKind::Toggle, Some(c)).card.unwrap(),
            )
        })
        .collect();
    assert!(scores[0] > scores[1]);
}

#[test]
fn gusto_summons_sphreez_before_generic_synchros() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Extra, 0, 29552709),
        card(db.as_ref(), 0, Location::Extra, 1, 50321796),
    ];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::SpecialSummon, Some(&o.cards[1])),
            choice(ChoiceKind::SpecialSummon, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("gusto", db, &o, &d), 1);
}

#[test]
fn gusto_rams_only_when_public_sphreez_reflects_damage() {
    let db = db();
    let mut o = obs();
    o.phase = Some(Phase::BattleStep);
    o.cards = vec![
        card(db.as_ref(), 0, Location::MonsterZone, 0, 29552709),
        card(db.as_ref(), 0, Location::MonsterZone, 1, 91662792),
        card(db.as_ref(), 1, Location::MonsterZone, 0, 89631139),
    ]; // Blue-Eyes, no hidden information
    let d = decision(
        DecisionKind::Battle,
        vec![
            choice(ChoiceKind::Attack, Some(&o.cards[1])),
            choice(ChoiceKind::EnterMain2, None),
        ],
    );
    assert_eq!(picks("gusto", db.clone(), &o, &d), 0);
    o.cards[0].position.face_up = false;
    assert_eq!(picks("gusto", db.clone(), &o, &d), 1);
    o.cards[0].position.face_up = true;
    o.cards
        .push(card(db.as_ref(), 1, Location::SpellTrapZone, 0, 82732705));
    assert_eq!(picks("gusto", db, &o, &d), 1);
}

#[test]
fn every_policy_sees_gusto_reflection_but_only_sphreez_is_indestructible() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 1, Location::MonsterZone, 0, 29552709),
        card(db.as_ref(), 1, Location::MonsterZone, 1, 91662792),
    ];
    let ctx = Ctx::new(&o, db.as_ref());
    assert!(ctx.battle_proof(&o.cards[0]));
    assert!(!ctx.battle_proof(&o.cards[1]));
    assert_eq!(
        ctx.facts(&o.cards[1]).attacked[0].burn,
        knowledge::Burn::Reflected
    );
    o.cards[0].code = None;
    assert_eq!(
        Ctx::new(&o, db.as_ref()).facts(&o.cards[1]).attacked[0].burn,
        knowledge::Burn::None
    );
}

#[test]
fn gusto_does_not_sacrifice_recruiters_into_damage_prevention() {
    let db = db();
    let mut o = obs();
    o.phase = Some(Phase::BattleStep);
    o.cards = vec![
        card(db.as_ref(), 0, Location::MonsterZone, 0, 29552709),
        card(db.as_ref(), 0, Location::MonsterZone, 1, 91662792),
        card(db.as_ref(), 1, Location::MonsterZone, 0, 69031175),
    ]; // Blackwing Armor Master
    let d = decision(
        DecisionKind::Battle,
        vec![
            choice(ChoiceKind::Attack, Some(&o.cards[1])),
            choice(ChoiceKind::EnterMain2, None),
        ],
    );
    assert_eq!(picks("gusto", db, &o, &d), 1);
}

#[test]
fn gusto_turns_a_legal_defense_position_recruiter_toward_reflected_damage() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::MonsterZone, 0, 29552709),
        card(db.as_ref(), 0, Location::MonsterZone, 1, 65277087),
        card(db.as_ref(), 1, Location::MonsterZone, 0, 89631139),
    ];
    o.cards[1].position = Position::FACE_UP_DEFENSE;
    o.cards[1].can_attack = false;
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::ChangePosition, Some(&o.cards[1])),
            choice(ChoiceKind::EnterBattle, None),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("gusto", db, &o, &d), 0);
}

#[test]
fn opponents_avoid_lethal_reflected_damage() {
    let db = db();
    let mut o = obs();
    o.phase = Some(Phase::BattleStep);
    o.life_points[0] = 1000;
    o.cards = vec![
        card(db.as_ref(), 0, Location::MonsterZone, 0, 89631139),
        card(db.as_ref(), 1, Location::MonsterZone, 0, 29552709),
        card(db.as_ref(), 1, Location::MonsterZone, 1, 91662792),
    ];
    let d = decision(
        DecisionKind::Battle,
        vec![
            choice(ChoiceKind::Attack, Some(&o.cards[0])),
            choice(ChoiceKind::EnterMain2, None),
        ],
    );
    assert_eq!(picks("blackwing", db, &o, &d), 1);
}

#[test]
fn unicore_knowledge_tracks_public_hand_counts_and_skill_drain() {
    let db = db();
    let mut o = obs();
    o.cards
        .push(card(db.as_ref(), 1, Location::MonsterZone, 0, 44155002));
    o.pile_sizes = vec![(0, Location::Hand, 2), (1, Location::Hand, 2)];
    assert_eq!(
        Ctx::new(&o, db.as_ref()).facts(&o.cards[0]).negates_any,
        knowledge::kind::ALL
    );
    o.pile_sizes[1].2 = 3;
    assert_eq!(Ctx::new(&o, db.as_ref()).facts(&o.cards[0]).negates_any, 0);
    o.pile_sizes[1].2 = 2;
    o.cards
        .push(card(db.as_ref(), 0, Location::SpellTrapZone, 0, 82732705));
    assert_eq!(Ctx::new(&o, db.as_ref()).facts(&o.cards[0]).negates_any, 0);
}

#[test]
fn banishment_turns_off_gusto_recruitment_and_enables_dd_returns() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::MonsterZone, 0, 65277087),
        card(db.as_ref(), 1, Location::MonsterZone, 0, 48092532),
    ];
    assert!(Ctx::new(&o, db.as_ref()).facts(&o.cards[0]).attacked[0].payoff > 0);
    o.cards
        .push(card(db.as_ref(), 1, Location::SpellTrapZone, 0, 81674782));
    let ctx = Ctx::new(&o, db.as_ref());
    assert_eq!(ctx.facts(&o.cards[0]).attacked[0].payoff, 0);
    assert!(ctx.facts(&o.cards[1]).attacked[0].payoff > 0);
}

#[test]
fn piercing_into_a_defending_gusto_still_reflects_lethal_damage() {
    let db = db();
    let mut o = obs();
    o.phase = Some(Phase::BattleStep);
    o.life_points[0] = 1000;
    o.cards = vec![
        card(db.as_ref(), 0, Location::MonsterZone, 0, 49003716), // Bora: piercing
        card(db.as_ref(), 1, Location::MonsterZone, 0, 29552709),
        card(db.as_ref(), 1, Location::MonsterZone, 1, 91662792),
    ];
    o.cards[2].position = Position::FACE_UP_DEFENSE;
    let d = decision(
        DecisionKind::Battle,
        vec![
            choice(ChoiceKind::Attack, Some(&o.cards[0])),
            choice(ChoiceKind::EnterMain2, None),
        ],
    );
    assert_eq!(picks("blackwing", db, &o, &d), 1);
}

#[test]
fn agents_search_venus_before_redundant_earth() {
    use ygo_policies::decks::agents::Agents;
    let db = db();
    let o = obs();
    let mut d = decision(DecisionKind::SelectCards, vec![]);
    d.hint = Hint::AddToHand;
    let mut memory = Memory::default();
    memory.last_activated = Some(91188343);
    let t = Turn {
        ctx: Ctx::new(&o, db.as_ref()),
        decision: &d,
        memory: &mut memory,
    };
    let member = |code| Member {
        at: CardRef {
            controller: 0,
            location: Location::Deck,
            sequence: 0,
        },
        code: Some(code),
        value: 0,
        required: false,
    };
    assert!(
        member_score(&Agents, &t, &member(64734921)) > member_score(&Agents, &t, &member(91188343))
    );
}

#[test]
fn scrapstorm_is_held_under_macro() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 48445393),
        card(db.as_ref(), 0, Location::MonsterZone, 0, 83135907),
    ];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::Activate, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("scrap", db.clone(), &o, &d), 0);
    o.cards
        .push(card(db.as_ref(), 1, Location::SpellTrapZone, 0, 30241314));
    assert_eq!(picks("scrap", db, &o, &d), 1);
}

#[test]
fn zombie_master_spends_mezuki_before_a_live_master() {
    use ygo_policies::decks::zombie::Zombie;
    let db = db();
    let o = obs();
    let mut d = decision(DecisionKind::SelectCards, vec![]);
    d.hint = Hint::ToGraveyard;
    let mut memory = Memory::default();
    memory.last_activated = Some(17259470);
    let t = Turn {
        ctx: Ctx::new(&o, db.as_ref()),
        decision: &d,
        memory: &mut memory,
    };
    let member = |code| Member {
        at: CardRef {
            controller: 0,
            location: Location::Hand,
            sequence: 0,
        },
        code: Some(code),
        value: 0,
        required: false,
    };
    assert!(
        member_score(&Zombie, &t, &member(92826944)) > member_score(&Zombie, &t, &member(17259470))
    );
}

#[test]
fn herald_dawn_recovers_fairy_instead_of_treating_recovery_as_a_cost() {
    use ygo_policies::decks::herald::Herald;
    let db = db();
    let o = obs();
    let mut d = decision(DecisionKind::SelectCards, vec![]);
    d.hint = Hint::ReturnToHand;
    let mut memory = Memory::default();
    memory.last_activated = Some(27383110);
    let t = Turn {
        ctx: Ctx::new(&o, db.as_ref()),
        decision: &d,
        memory: &mut memory,
    };
    let m = Member {
        at: CardRef {
            controller: 0,
            location: Location::Graveyard,
            sequence: 0,
        },
        code: Some(95492061),
        value: 0,
        required: false,
    };
    assert!(member_score(&Herald, &t, &m) > 0.0);
}

#[test]
fn fish_recruitment_prefers_an_oyster_after_selecting_a_tuner() {
    use ygo_policies::decks::fish::Fish;
    let db = db();
    let o = obs();
    let eel = card(db.as_ref(), 0, Location::Deck, 0, 37953640);
    let oyster = card(db.as_ref(), 0, Location::Deck, 1, 83239739);
    let eel2 = card(db.as_ref(), 0, Location::Deck, 2, 37953640);
    let mut c = choice(ChoiceKind::Toggle, Some(&oyster));
    c.members
        .push(choice(ChoiceKind::Toggle, Some(&eel)).card.unwrap());
    let mut d = decision(DecisionKind::SelectCards, vec![c]);
    d.selected = vec![eel.at];
    d.hint = Hint::SpecialSummon;
    let mut memory = Memory::default();
    memory.last_activated = Some(88307361);
    let t = Turn {
        ctx: Ctx::new(&o, db.as_ref()),
        decision: &d,
        memory: &mut memory,
    };
    assert!(
        member_score(
            &Fish,
            &t,
            &choice(ChoiceKind::Toggle, Some(&oyster)).card.unwrap()
        ) > member_score(
            &Fish,
            &t,
            &choice(ChoiceKind::Toggle, Some(&eel2)).card.unwrap()
        )
    );
}

#[test]
fn cyber_holds_power_bond_without_a_safe_attack_window() {
    let db = db();
    let mut o = obs();
    o.cards = vec![card(db.as_ref(), 0, Location::Hand, 0, 37630732)];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::Activate, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("cyber", db.clone(), &o, &d), 1);
    let mut battle = d.clone();
    battle.choices.push(choice(ChoiceKind::EnterBattle, None));
    assert_eq!(picks("cyber", db.clone(), &o, &battle), 0);
    o.life_points[0] = 2000;
    assert_ne!(picks("cyber", db, &o, &battle), 0);
}

#[test]
fn gemini_resummons_gigaplant_to_unlock_its_effect() {
    let db = db();
    let mut o = obs();
    o.cards = vec![card(db.as_ref(), 0, Location::MonsterZone, 0, 53257892)];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::NormalSummon, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("gemini", db, &o, &d), 0);
}

#[test]
fn psychic_jumper_does_not_give_away_a_boss_for_a_weak_monster() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::MonsterZone, 0, 52430902),
        card(db.as_ref(), 0, Location::MonsterZone, 1, 40101111),
        card(db.as_ref(), 1, Location::MonsterZone, 0, 39552864),
    ];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::Activate, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("psychic", db, &o, &d), 1);
}

#[test]
fn nurse_waits_for_simochi_to_resolve_before_gift_card() {
    use ygo_policies::{decks::nurse::Nurse, Strategy};
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::SpellTrapZone, 0, 40633297),
        card(db.as_ref(), 0, Location::SpellTrapZone, 1, 39526584),
    ];
    let d = decision(
        DecisionKind::Chain {
            forced: false,
            triggers: false,
        },
        vec![
            choice(ChoiceKind::Activate, Some(&o.cards[1])),
            choice(ChoiceKind::Pass, None),
        ],
    );
    let mut memory = Memory::default();
    o.chain.push(ChainLink {
        code: 40633297,
        controller: 0,
        source: o.cards[0].at,
        targets: vec![],
    });
    let mut strategy = Nurse;
    assert!(
        strategy
            .chain(
                &Turn {
                    ctx: Ctx::new(&o, db.as_ref()),
                    decision: &d,
                    memory: &mut memory
                },
                0
            )
            .unwrap()
            .score
            <= 0.0
    );
    o.chain.clear();
    assert!(
        strategy
            .chain(
                &Turn {
                    ctx: Ctx::new(&o, db.as_ref()),
                    decision: &d,
                    memory: &mut memory
                },
                0
            )
            .unwrap()
            .score
            > 0.0
    );
}

#[test]
fn chain_burn_extends_its_own_chain_with_a_different_draw_card() {
    let db = db();
    let mut o = obs();
    o.turn_player = Some(1);
    o.cards = vec![card(db.as_ref(), 0, Location::SpellTrapZone, 0, 83968380)];
    o.chain.push(ChainLink {
        code: 27053506,
        controller: 0,
        source: CardRef {
            controller: 0,
            location: Location::SpellTrapZone,
            sequence: 1,
        },
        targets: vec![],
    });
    let d = decision(
        DecisionKind::Chain {
            forced: false,
            triggers: false,
        },
        vec![
            choice(ChoiceKind::Activate, Some(&o.cards[0])),
            choice(ChoiceKind::Pass, None),
        ],
    );
    assert_eq!(picks("chain-burn", db, &o, &d), 0);
}

#[test]
fn hidden_armory_waits_for_a_body_instead_of_locking_out_ben_kei() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 52105192),
        card(db.as_ref(), 0, Location::Hand, 1, 84430950),
    ];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::Activate, Some(&o.cards[0])),
            choice(ChoiceKind::NormalSummon, Some(&o.cards[1])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("benkei", db, &o, &d), 1);
}

#[test]
fn deckout_flips_its_needle_worm_with_taiyou() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 38699854),
        card(db.as_ref(), 0, Location::MonsterZone, 0, 81843628),
    ];
    o.cards[1].position = Position::FACE_DOWN_DEFENSE;
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::Activate, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("deckout", db, &o, &d), 0);
}

#[test]
fn naturia_only_tributes_bamboo_over_a_naturia() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 20174189),
        card(db.as_ref(), 0, Location::MonsterZone, 0, 39552864),
    ];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::NormalSummon, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("naturia", db.clone(), &o, &d), 1);
    o.cards[1] = card(db.as_ref(), 0, Location::MonsterZone, 0, 33866130);
    assert_eq!(picks("naturia", db, &o, &d), 0);
}

#[test]
fn alien_recovers_swords_with_golgar() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::MonsterZone, 0, 68319538),
        card(db.as_ref(), 0, Location::SpellTrapZone, 0, 72302403),
    ];
    let mut c = choice(ChoiceKind::Activate, Some(&o.cards[0]));
    c.description = (68319538u64) << 20;
    let d = decision(
        DecisionKind::Idle,
        vec![c, choice(ChoiceKind::EndTurn, None)],
    );
    assert_eq!(picks("alien", db, &o, &d), 0);
}

#[test]
fn spirit_holds_dark_dust_when_it_would_wipe_our_better_board() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 89111398),
        card(db.as_ref(), 0, Location::MonsterZone, 0, 44508094),
    ];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::NormalSummon, Some(&o.cards[0])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("spirit", db, &o, &d), 1);
}

#[test]
fn garden_is_activated_after_summoning_the_initial_attacker() {
    let db = db();
    let mut o = obs();
    o.cards = vec![
        card(db.as_ref(), 0, Location::Hand, 0, 71645242),
        card(db.as_ref(), 0, Location::Hand, 1, 20546916),
    ];
    let d = decision(
        DecisionKind::Idle,
        vec![
            choice(ChoiceKind::Activate, Some(&o.cards[0])),
            choice(ChoiceKind::NormalSummon, Some(&o.cards[1])),
            choice(ChoiceKind::EndTurn, None),
        ],
    );
    assert_eq!(picks("garden", db, &o, &d), 1);
}
