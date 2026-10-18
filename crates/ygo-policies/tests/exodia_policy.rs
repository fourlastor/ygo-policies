use std::sync::Arc;
use ygo_policies::{cards::MemoryCards, model::*, registry};

const HEAD: u32 = 33396948;
const HEART: u32 = 35762283;
const RELOAD: u32 = 22589918;
const FADER: u32 = 19665973;
const DUALITY: u32 = 98645731;

fn observation() -> Observation {
    Observation {
        me: 0, turn: 1, turn_player: Some(0), phase: Some(Phase::Main1),
        life_points: [8000, 8000], cards: Vec::new(),
        pile_sizes: vec![(0, Location::Deck, 30)], chain: Vec::new(),
        battle_attacker: None, battle_target: None, event_cards: Vec::new(),
        summon_used: false, chain_known: true, can_attack_known: true, coin_toss: None,
    }
}
fn member(code: u32, location: Location, sequence: u32) -> Member {
    Member { at: CardRef { controller: 0, location, sequence }, code: Some(code), value: 0, required: false }
}
fn choice(kind: ChoiceKind, card: Option<Member>) -> Choice {
    Choice { kind, card, members: Vec::new(), description: 0, place: None }
}
fn decision(kind: DecisionKind, choices: Vec<Choice>) -> Decision {
    Decision { kind, hint: Hint::None, minimum: 0, maximum: 0, selected: Vec::new(), subject: None, choices }
}
fn card(code: u32, location: Location, sequence: u32) -> CardView {
    CardView {
        at: member(code, location, sequence).at, code: Some(code),
        position: Position::FACE_DOWN_DEFENSE, attack: 0, defense: 0, level: 1,
        can_attack: false, battles: 0, counters: 0, coin_effect: None,
    }
}

#[test]
fn pieces_and_hand_protection_are_never_normal_summoned_or_set() {
    let mut obs = observation();
    obs.cards = vec![card(HEAD, Location::Hand, 0), card(FADER, Location::Hand, 1)];
    let d = decision(DecisionKind::Idle, vec![
        choice(ChoiceKind::NormalSummon, Some(member(HEAD, Location::Hand, 0))),
        choice(ChoiceKind::SetMonster, Some(member(HEAD, Location::Hand, 0))),
        choice(ChoiceKind::NormalSummon, Some(member(FADER, Location::Hand, 1))),
        choice(ChoiceKind::SetMonster, Some(member(FADER, Location::Hand, 1))),
        choice(ChoiceKind::EndTurn, None),
    ]);
    let mut p = registry::create("exodia", Arc::new(MemoryCards::default())).unwrap();
    assert_eq!(p.choose(&obs, &d), 4);
}

#[test]
fn end_phase_discards_spare_cards_instead_of_exodia() {
    let mut obs = observation();
    obs.phase = Some(Phase::End);
    obs.cards = vec![card(HEAD, Location::Hand, 0), card(FADER, Location::Hand, 1)];
    let mut d = decision(DecisionKind::SelectCards, vec![
        choice(ChoiceKind::Toggle, Some(member(HEAD, Location::Hand, 0))),
        choice(ChoiceKind::Toggle, Some(member(FADER, Location::Hand, 1))),
    ]);
    d.hint = Hint::Discard; d.minimum = 1; d.maximum = 1;
    let mut p = registry::create("exodia", Arc::new(MemoryCards::default())).unwrap();
    assert_eq!(p.choose(&obs, &d), 1);
}

#[test]
fn duality_takes_the_winning_fifth_piece() {
    let mut obs = observation();
    obs.cards = [70903634, 7902349, 8124921, 44519536].into_iter().enumerate()
        .map(|(i, c)| card(c, Location::Hand, i as u32)).collect();
    let mut p = registry::create("exodia", Arc::new(MemoryCards::default())).unwrap();
    let activate = decision(DecisionKind::Idle, vec![
        choice(ChoiceKind::Activate, Some(member(DUALITY, Location::Hand, 4))),
        choice(ChoiceKind::EndTurn, None),
    ]);
    assert_eq!(p.choose(&obs, &activate), 0);
    let mut select = decision(DecisionKind::SelectCards, vec![
        choice(ChoiceKind::Toggle, Some(member(FADER, Location::Deck, 0))),
        choice(ChoiceKind::Toggle, Some(member(HEAD, Location::Deck, 1))),
    ]);
    select.hint = Hint::AddToHand; select.minimum = 1; select.maximum = 1;
    assert_eq!(p.choose(&obs, &select), 1);
}

#[test]
fn underdog_keeps_drawing_after_three_activations() {
    let mut obs = observation();
    obs.phase = Some(Phase::Draw);
    let mut heart = card(HEART, Location::SpellTrapZone, 0);
    heart.position = Position::FACE_UP_ATTACK;
    obs.cards.push(heart);
    let d = decision(DecisionKind::Chain { forced: false, triggers: true }, vec![
        choice(ChoiceKind::Activate, Some(member(HEART, Location::SpellTrapZone, 0))),
        choice(ChoiceKind::Pass, None),
    ]);
    let mut p = registry::create("exodia", Arc::new(MemoryCards::default())).unwrap();
    for remaining in (1..=12).rev() {
        obs.pile_sizes = vec![(0, Location::Deck, remaining)];
        assert_eq!(p.choose(&obs, &d), 0, "legal draw with {remaining} cards remaining");
    }
    obs.pile_sizes = vec![(0, Location::Deck, 0)];
    assert_eq!(p.choose(&obs, &d), 1, "do not draw from an empty deck");
}


#[test]
fn multiple_hearts_chain_without_overdrawing() {
    let mut obs = observation();
    obs.phase = Some(Phase::Draw);
    obs.pile_sizes = vec![(0, Location::Deck, 2)];
    let d = decision(DecisionKind::Chain { forced: false, triggers: true }, vec![
        choice(ChoiceKind::Activate, Some(member(HEART, Location::SpellTrapZone, 2))),
        choice(ChoiceKind::Pass, None),
    ]);
    let link = |sequence| ChainLink { code: HEART, controller: 0,
        source: member(HEART, Location::SpellTrapZone, sequence).at, targets: Vec::new() };
    let mut p = registry::create("exodia", Arc::new(MemoryCards::default())).unwrap();
    obs.chain.push(link(0));
    assert_eq!(p.choose(&obs, &d), 0, "a second Heart should chain above our first");
    obs.chain.push(link(1));
    assert_eq!(p.choose(&obs, &d), 1, "two pending draws already exhaust the Deck");
}

#[test]
fn reload_restarts_the_draw_phase_after_hearts_resolve() {
    let mut obs = observation();
    let mut heart = card(HEART, Location::SpellTrapZone, 0);
    heart.position = Position::FACE_UP_ATTACK;
    obs.cards = vec![heart, card(HEAD, Location::Hand, 0),
        card(RELOAD, Location::Hand, 1), card(70903634, Location::Hand, 2)];
    let d = decision(DecisionKind::Chain { forced: false, triggers: false }, vec![
        choice(ChoiceKind::Activate, Some(member(RELOAD, Location::Hand, 1))),
        choice(ChoiceKind::Pass, None),
    ]);
    let mut p = registry::create("exodia", Arc::new(MemoryCards::default())).unwrap();
    assert_eq!(p.choose(&obs, &d), 1, "keep Reload for the Draw Phase");
    obs.phase = Some(Phase::Draw);
    obs.turn_player = Some(1);
    assert_eq!(p.choose(&obs, &d), 1, "our Heart does not draw on the opponent's turn");
    obs.turn_player = Some(0);
    obs.chain.push(ChainLink { code: HEART, controller: 0,
        source: member(HEART, Location::SpellTrapZone, 0).at, targets: Vec::new() });
    assert_eq!(p.choose(&obs, &d), 1, "finish the active draw sequence first");
    obs.chain.clear();
    assert_eq!(p.choose(&obs, &d), 0);
}

#[test]
fn attack_locks_and_hearts_leave_room_for_reload() {
    const PEACE: u32 = 44656491;
    const LEVEL: u32 = 3136426;
    const DRAIN: u32 = 82732705;
    let mut obs = observation();
    obs.cards = [HEART, HEART, PEACE, DRAIN].into_iter().enumerate().map(|(i, c)| {
        let mut v = card(c, Location::SpellTrapZone, i as u32);
        v.position = Position::FACE_UP_ATTACK;
        v
    }).collect();
    obs.cards.push(card(HEART, Location::Hand, 0));
    obs.cards.push(card(LEVEL, Location::Hand, 1));
    let d = decision(DecisionKind::Idle, vec![
        choice(ChoiceKind::Activate, Some(member(HEART, Location::Hand, 0))),
        choice(ChoiceKind::Activate, Some(member(LEVEL, Location::Hand, 1))),
        choice(ChoiceKind::EndTurn, None),
    ]);
    let mut p = registry::create("exodia", Arc::new(MemoryCards::default())).unwrap();
    assert_eq!(p.choose(&obs, &d), 2);
}

#[test]
fn other_strategies_keep_the_repeated_action_guard() {
    use ygo_policies::agent::{Agent, Policy, Strategy};
    struct DefaultStrategy;
    impl Strategy for DefaultStrategy {}
    let mut obs = observation();
    obs.phase = Some(Phase::Draw);
    let mut heart = card(HEART, Location::SpellTrapZone, 0);
    heart.position = Position::FACE_UP_ATTACK;
    obs.cards.push(heart);
    let d = decision(DecisionKind::Chain { forced: false, triggers: true }, vec![
        choice(ChoiceKind::Activate, Some(member(HEART, Location::SpellTrapZone, 0))),
        choice(ChoiceKind::Pass, None),
    ]);
    let mut p = Agent::new(DefaultStrategy, Arc::new(MemoryCards::default()));
    for _ in 0..3 { assert_eq!(p.choose(&obs, &d), 0); }
    assert_eq!(p.choose(&obs, &d), 1);
}
