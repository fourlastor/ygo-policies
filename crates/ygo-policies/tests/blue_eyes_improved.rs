use std::sync::Arc;
use ygo_policies::{cards::MemoryCards, model::*, registry};

const BLUE: u32 = 89631139;
const ALTERNATIVE: u32 = 38517737;
const ANCIENTS: u32 = 71039903;
const RETURN: u32 = 6853254;
const TRADE: u32 = 38120068;
const PRIME: u32 = 31801517;
const AZURE: u32 = 40908371;
const ARMOR: u32 = 39030163;

fn observation() -> Observation {
    Observation {
        me: 0,
        turn: 3,
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

fn card(code: u32, location: Location, sequence: u32) -> CardView {
    CardView {
        at: CardRef {
            controller: 0,
            location,
            sequence,
        },
        code: Some(code),
        position: Position::FACE_UP_ATTACK,
        attack: 3000,
        defense: 2500,
        level: 8,
        can_attack: false,
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
            required: false,
            value: 0,
        }),
        members: Vec::new(),
        description: 0,
        place: None,
    }
}

fn idle(choices: Vec<Choice>) -> Decision {
    Decision {
        kind: DecisionKind::Idle,
        hint: Hint::None,
        minimum: 0,
        maximum: 0,
        selected: Vec::new(),
        subject: None,
        choices,
    }
}

#[test]
fn recovery_precedes_revival_and_targets_the_missing_summon_piece() {
    for (held, missing) in [(ALTERNATIVE, BLUE), (BLUE, ALTERNATIVE)] {
        let mut obs = observation();
        obs.cards = vec![
            card(held, Location::Hand, 0),
            card(RETURN, Location::Hand, 1),
            card(ANCIENTS, Location::Graveyard, 0),
            card(missing, Location::Graveyard, 1),
            card(held, Location::Graveyard, 2),
        ];
        let d = idle(vec![
            choice(ChoiceKind::Activate, Some(&obs.cards[1])),
            choice(ChoiceKind::Activate, Some(&obs.cards[2])),
            choice(ChoiceKind::EndTurn, None),
        ]);
        let mut p =
            registry::create("blue-eyes-improved", Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(p.choose(&obs, &d), 1);
        let mut target = idle(vec![
            choice(ChoiceKind::Toggle, Some(&obs.cards[4])),
            choice(ChoiceKind::Toggle, Some(&obs.cards[3])),
        ]);
        target.kind = DecisionKind::SelectCards;
        target.hint = Hint::AddToHand;
        target.minimum = 1;
        target.maximum = 1;
        assert_eq!(p.choose(&obs, &target), 1);
    }
}

#[test]
fn alternative_summons_before_trade_in_can_discard_its_reveal() {
    let mut obs = observation();
    obs.cards = vec![
        card(BLUE, Location::Hand, 0),
        card(ALTERNATIVE, Location::Hand, 1),
        card(TRADE, Location::Hand, 2),
    ];
    let d = idle(vec![
        choice(ChoiceKind::Activate, Some(&obs.cards[2])),
        choice(ChoiceKind::SpecialSummon, Some(&obs.cards[1])),
        choice(ChoiceKind::EndTurn, None),
    ]);
    for (id, expected) in [("blue-eyes", 0), ("blue-eyes-improved", 1)] {
        let mut p = registry::create(id, Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(p.choose(&obs, &d), expected, "{id}");
    }
}

#[test]
fn prime_breaks_a_defense_wall_but_does_not_replace_attackers_for_a_small_target() {
    let mut obs = observation();
    obs.cards = vec![
        card(BLUE, Location::MonsterZone, 0),
        card(ALTERNATIVE, Location::MonsterZone, 1),
        card(PRIME, Location::Extra, 0),
    ];
    let mut wall = card(AZURE, Location::MonsterZone, 0);
    wall.at.controller = 1;
    wall.position = Position::FACE_UP_DEFENSE;
    wall.attack = 2500;
    wall.defense = 3000;
    obs.cards.push(wall);
    let d = idle(vec![
        choice(ChoiceKind::SpecialSummon, Some(&obs.cards[2])),
        choice(ChoiceKind::EnterBattle, None),
    ]);
    for (id, expected) in [("blue-eyes", 1), ("blue-eyes-improved", 0)] {
        let mut p = registry::create(id, Arc::new(MemoryCards::default())).unwrap();
        assert_eq!(p.choose(&obs, &d), expected, "{id}");
    }
    obs.cards[3].defense = 2000;
    let mut p = registry::create("blue-eyes-improved", Arc::new(MemoryCards::default())).unwrap();
    assert_eq!(p.choose(&obs, &d), 1);
}

#[test]
fn full_armor_uses_the_two_material_prime_route() {
    let mut obs = observation();
    obs.cards = vec![
        card(BLUE, Location::MonsterZone, 0),
        card(BLUE, Location::MonsterZone, 1),
        card(BLUE, Location::MonsterZone, 2),
        card(ARMOR, Location::Extra, 0),
        card(PRIME, Location::Extra, 1),
    ];
    let mut opponent = card(BLUE, Location::MonsterZone, 0);
    opponent.at.controller = 1;
    obs.cards.push(opponent);
    let d = idle(vec![
        choice(ChoiceKind::SpecialSummon, Some(&obs.cards[3])),
        choice(ChoiceKind::SpecialSummon, Some(&obs.cards[4])),
        choice(ChoiceKind::EndTurn, None),
    ]);
    let mut p = registry::create("blue-eyes-improved", Arc::new(MemoryCards::default())).unwrap();
    assert_eq!(
        p.choose(&obs, &d),
        1,
        "keep the third dragon instead of using three materials"
    );
    obs.cards[0].code = Some(PRIME);
    let mut p = registry::create("blue-eyes-improved", Arc::new(MemoryCards::default())).unwrap();
    assert_eq!(p.choose(&obs, &d), 0, "overlay the existing Prime");
    obs.cards[0].code = Some(BLUE);
    let without_prime = idle(vec![
        choice(ChoiceKind::SpecialSummon, Some(&obs.cards[3])),
        choice(ChoiceKind::EndTurn, None),
    ]);
    let mut p = registry::create("blue-eyes-improved", Arc::new(MemoryCards::default())).unwrap();
    assert_eq!(p.choose(&obs, &without_prime), 0, "keep the direct route when Prime cannot be summoned");
}
