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
fn four_registered_lists_are_legal_and_have_scripts() {
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
    for id in ["fabled", "counter-fairy", "macro-dd", "gusto"] {
        let entry = registry::find(id).unwrap();
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
