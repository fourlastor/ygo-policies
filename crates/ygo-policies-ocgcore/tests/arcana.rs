//! Public wire messages, through redaction, projection and the real policy.
use std::sync::Arc;
use ygo_policies::{
    cards::{CardData, MemoryCards},
    registry,
};
use ygo_policies_ocgcore::{
    wire::{msg, Loc, Writer},
    Seat,
};

const EMPEROR: u32 = 61175706;
const FOOL: u32 = 62892347;
const SECOND: u32 = 36562627;
const REVERSAL: u32 = 36690018;
const CALL: u32 = 99189322;
const CHARIOT: u32 = 34568403;

fn loc(player: u8, location: u8, sequence: u32) -> Loc {
    Loc {
        controller: player,
        location,
        sequence,
        position: 1,
    }
}

fn seat(player: u8) -> Seat {
    let mut db = MemoryCards::default();
    for code in [EMPEROR, FOOL, SECOND, REVERSAL, CALL, CHARIOT] {
        db.0.insert(
            code,
            CardData {
                code,
                kind: if matches!(code, REVERSAL | CALL) {
                    4
                } else if code == SECOND {
                    2
                } else {
                    0x21
                },
                attack: 1400,
                defense: 1400,
                level: 4,
                setcodes: vec![0x5],
                ..Default::default()
            },
        );
    }
    let db = Arc::new(db);
    Seat::new(
        registry::create("arcana", db.clone()).unwrap(),
        db,
        Some(player),
    )
}

fn move_card(seat: &mut Seat, code: u32, from: Loc, to: Loc) {
    seat.feed(
        &Writer::default()
            .u8(msg::MOVE)
            .u32(code)
            .loc(from)
            .loc(to)
            .u32(0)
            .0,
    )
    .unwrap();
}

fn chain(seat: &mut Seat, code: u32, at: Loc, size: u32) {
    seat.feed(
        &Writer::default()
            .u8(msg::CHAINING)
            .u32(code)
            .loc(at)
            .u8(at.controller)
            .u8(at.location)
            .u32(at.sequence)
            .u64((code as u64) << 4)
            .u32(size)
            .0,
    )
    .unwrap();
    seat.feed(&[msg::CHAINED, size as u8]).unwrap();
}

fn hint(seat: &mut Seat, at: Loc, kind: u8, description: u64) {
    seat.feed(
        &Writer::default()
            .u8(msg::CARD_HINT)
            .loc(at)
            .u8(kind)
            .u64(description)
            .0,
    )
    .unwrap();
}

fn redo(seat: &mut Seat, player: u8) -> i32 {
    let answer = seat
        .feed(
            &Writer::default()
                .u8(msg::SELECT_EFFECTYN)
                .u8(player)
                .u32(SECOND)
                .loc(loc(player, 8, 0))
                .u64(0)
                .0,
        )
        .unwrap()
        .unwrap();
    i32::from_le_bytes(answer.try_into().unwrap())
}

fn reversal(seat: &mut Seat, player: u8) -> i32 {
    activate(seat, player, REVERSAL)
}

fn activate(seat: &mut Seat, player: u8, code: u32) -> i32 {
    let answer = seat
        .feed(
            &Writer::default()
                .u8(msg::SELECT_CHAIN)
                .u8(player)
                .u8(0)
                .u8(0)
                .u32(0)
                .u32(0)
                .u32(1)
                .u32(code)
                .loc(loc(player, 8, 1))
                .u64(0)
                .u8(0)
                .0,
        )
        .unwrap()
        .unwrap();
    i32::from_le_bytes(answer.try_into().unwrap())
}

#[test]
fn toss_source_is_the_resolving_link_and_old_tosses_expire() {
    let mut s = seat(0);
    chain(&mut s, EMPEROR, loc(0, 4, 0), 1);
    chain(&mut s, FOOL, loc(0, 4, 1), 2);
    s.feed(&[msg::CHAIN_SOLVING, 1]).unwrap();
    s.feed(&[msg::TOSS_COIN, 0, 1, 1]).unwrap();
    assert_eq!(redo(&mut s, 0), 0); // Emperor heads, not Fool heads.
    s.feed(&[msg::TOSS_COIN, 0, 1, 0]).unwrap();
    assert_eq!(redo(&mut s, 0), 1);
    s.feed(&[msg::CHAIN_SOLVED, 1]).unwrap();
    assert_eq!(redo(&mut s, 0), 0);
    s.feed(&[msg::TOSS_COIN, 0, 1, 0]).unwrap(); // Unattributed toss.
    assert_eq!(redo(&mut s, 0), 0);
    assert!(ygo_policies_ocgcore::message::parse(&[msg::TOSS_COIN, 0, 2, 1]).is_err());
    assert!(ygo_policies_ocgcore::message::parse(&[msg::TOSS_COIN, 0, 1, 2]).is_err());
}

#[test]
fn registered_coin_survives_control_change_but_not_reset_or_slot_reuse() {
    let mut s = seat(0);
    let at = loc(0, 4, 0);
    move_card(&mut s, EMPEROR, Loc::default(), at);
    hint(&mut s, at, 6, 63);
    let effect = s.observation().unwrap().cards[0].coin_effect;
    assert!(effect.is_some());
    let theirs = loc(1, 4, 2);
    move_card(&mut s, EMPEROR, at, theirs);
    assert_eq!(s.observation().unwrap().cards[0].coin_effect, effect);
    s.feed(
        &Writer::default()
            .u8(msg::POS_CHANGE)
            .u32(EMPEROR)
            .u8(1)
            .u8(4)
            .u8(2)
            .u8(1)
            .u8(8)
            .0,
    )
    .unwrap();
    assert!(s.observation().unwrap().cards[0].coin_effect.is_none());
    move_card(&mut s, EMPEROR, theirs, loc(0, 16, 0));
    move_card(&mut s, EMPEROR, Loc::default(), at);
    assert!(s
        .observation()
        .unwrap()
        .cards
        .iter()
        .all(|c| c.coin_effect.is_none()));
}

fn target(s: &mut Seat, at: Loc) {
    s.feed(&Writer::default().u8(msg::BECOME_TARGET).u32(1).loc(at).0)
        .unwrap();
}

fn begin_chain(s: &mut Seat, code: u32, at: Loc) {
    s.feed(
        &Writer::default()
            .u8(msg::CHAINING)
            .u32(code)
            .loc(at)
            .u8(at.controller)
            .u8(at.location)
            .u32(at.sequence)
            .u64(0)
            .u32(1)
            .0,
    )
    .unwrap();
}

#[test]
fn silent_reversal_invalidates_the_old_hint_but_negation_does_not() {
    for negated in [false, true] {
        let mut s = seat(0);
        let at = loc(0, 4, 0);
        move_card(&mut s, EMPEROR, Loc::default(), at);
        hint(&mut s, at, 6, 63);
        begin_chain(&mut s, REVERSAL, loc(0, 8, 0));
        target(&mut s, at);
        s.feed(&[msg::CHAINED, 1]).unwrap();
        if negated {
            s.feed(&[msg::CHAIN_DISABLED, 1]).unwrap();
        }
        s.feed(&[msg::CHAIN_SOLVING, 1]).unwrap();
        s.feed(&[msg::CHAIN_SOLVED, 1]).unwrap();
        assert_eq!(
            s.observation().unwrap().cards[0].coin_effect.is_some(),
            negated
        );
    }
}

#[test]
fn arcana_call_uses_the_same_coin_with_the_donors_effect_and_restores_it() {
    let mut s = seat(0);
    let at = loc(0, 4, 0);
    let donor = loc(0, 16, 0);
    move_card(&mut s, EMPEROR, Loc::default(), at);
    move_card(&mut s, FOOL, Loc::default(), donor);
    hint(&mut s, at, 6, 63);
    assert_eq!(activate(&mut s, 0, CALL), 0);
    begin_chain(&mut s, CALL, loc(0, 8, 0));
    target(&mut s, at);
    target(&mut s, donor);
    s.feed(&[msg::CHAINED, 1]).unwrap();
    s.feed(&[msg::CHAIN_SOLVING, 1]).unwrap();
    move_card(&mut s, FOOL, donor, loc(0, 32, 0));
    hint(&mut s, at, 7, 63);
    hint(&mut s, at, 6, 63);
    let obs = s.observation().unwrap();
    let effect = obs
        .cards
        .iter()
        .find(|c| c.at.location == ygo_policies::model::Location::MonsterZone)
        .unwrap()
        .coin_effect
        .unwrap();
    assert_eq!(effect.code, FOOL);
    assert_eq!(effect.result, ygo_policies::model::Coin::Tails);
    s.feed(&[msg::CHAIN_SOLVED, 1]).unwrap();
    s.feed(&[msg::CHAIN_END]).unwrap();
    assert_eq!(reversal(&mut s, 0), -1); // Tails Fool is already good.
    hint(&mut s, at, 7, 63);
    hint(&mut s, at, 6, 63); // End-phase restoration.
    assert_eq!(reversal(&mut s, 0), 0); // Emperor's penalty is back.
}

fn select(s: &mut Seat, hint: u64, candidates: &[(u32, Loc)]) -> usize {
    s.feed(&Writer::default().u8(msg::HINT).u8(3).u8(0).u64(hint).0)
        .unwrap();
    let mut prompt = Writer::default()
        .u8(msg::SELECT_CARD)
        .u8(0)
        .u8(0)
        .u32(1)
        .u32(1)
        .u32(candidates.len() as u32);
    for (code, at) in candidates {
        prompt = prompt.u32(*code).loc(*at);
    }
    let answer = s.feed(&prompt.0).unwrap().unwrap();
    assert_eq!(u32::from_le_bytes(answer[4..8].try_into().unwrap()), 1);
    u32::from_le_bytes(answer[8..12].try_into().unwrap()) as usize
}

#[test]
fn arcana_call_selects_the_recipient_then_the_donor_without_banishing_itself() {
    let mut s = seat(0);
    let bad = loc(0, 4, 0);
    let good = loc(0, 4, 1);
    let donor = loc(0, 16, 0);
    move_card(&mut s, EMPEROR, Loc::default(), bad);
    move_card(&mut s, EMPEROR, Loc::default(), good);
    move_card(&mut s, FOOL, Loc::default(), donor);
    hint(&mut s, bad, 6, 63);
    hint(&mut s, good, 6, 62);
    assert_eq!(activate(&mut s, 0, CALL), 0);
    begin_chain(&mut s, CALL, loc(0, 8, 0));
    assert_eq!(select(&mut s, 516, &[(EMPEROR, good), (EMPEROR, bad)]), 1); // HINTMSG_FACEUP
    target(&mut s, bad);
    assert_eq!(select(&mut s, 503, &[(EMPEROR, bad), (FOOL, donor)]), 1); // HINTMSG_REMOVE
}

#[test]
fn chariot_is_set_without_retry_support_and_a_battle_flip_can_attack_later() {
    let mut s = seat(0);
    let hand = loc(0, 2, 0);
    move_card(&mut s, CHARIOT, Loc::default(), hand);
    s.feed(&[msg::NEW_TURN, 0]).unwrap();
    s.feed(&[msg::NEW_PHASE, 4, 0]).unwrap();
    // One normal summon, no specials/repositions, one monster Set, no S/T.
    let prompt = Writer::default()
        .u8(msg::SELECT_IDLECMD)
        .u8(0)
        .u32(1)
        .u32(CHARIOT)
        .u8(0)
        .u8(2)
        .u32(0)
        .u32(0)
        .u32(0)
        .u32(1)
        .u32(CHARIOT)
        .u8(0)
        .u8(2)
        .u32(0)
        .u32(0)
        .u32(0)
        .u8(0)
        .u8(1)
        .u8(0)
        .0;
    s.feed(&prompt).unwrap().unwrap();
    let answer = s.last_answer().unwrap();
    assert_eq!(
        answer.decision.choices[answer.choice].kind,
        ygo_policies::model::ChoiceKind::SetMonster
    );
    // Once a battle has flipped it, moving to attack does not toss a coin.
    let mut field = loc(0, 4, 0);
    field.position = 4;
    move_card(&mut s, CHARIOT, hand, field);
    s.feed(&[msg::NEW_TURN, 1]).unwrap();
    s.feed(&[msg::NEW_TURN, 0]).unwrap();
    let prompt = Writer::default()
        .u8(msg::SELECT_IDLECMD)
        .u8(0)
        .u32(0)
        .u32(0)
        .u32(1)
        .u32(CHARIOT)
        .u8(0)
        .u8(4)
        .u8(0)
        .u32(0)
        .u32(0)
        .u32(0)
        .u8(1)
        .u8(1)
        .u8(0)
        .0;
    s.feed(&prompt).unwrap().unwrap();
    let answer = s.last_answer().unwrap();
    assert_eq!(
        answer.decision.choices[answer.choice].kind,
        ygo_policies::model::ChoiceKind::ChangePosition
    );
}

#[test]
fn heads_fool_cannot_be_repaired_by_our_targeting_traps() {
    let mut s = seat(0);
    let at = loc(0, 4, 0);
    move_card(&mut s, FOOL, Loc::default(), at);
    move_card(&mut s, EMPEROR, Loc::default(), loc(0, 16, 0));
    hint(&mut s, at, 6, 62);
    assert_eq!(reversal(&mut s, 0), -1);
    assert_eq!(activate(&mut s, 0, CALL), -1);
}

#[test]
fn second_coin_toss_keeps_good_results_and_retries_bad_ones_from_either_seat() {
    for player in [0, 1] {
        for (code, good) in [(EMPEROR, 1), (FOOL, 0)] {
            for result in [0, 1] {
                let mut s = seat(player);
                let at = loc(player, 4, 0);
                move_card(&mut s, code, Loc::default(), at);
                chain(&mut s, code, at, 1);
                s.feed(&[msg::CHAIN_SOLVING, 1]).unwrap();
                s.feed(&[msg::TOSS_COIN, player, 1, result]).unwrap();
                assert_eq!(
                    redo(&mut s, player),
                    i32::from(result != good),
                    "{code}, result {result}, seat {player}"
                );
            }
        }
    }
}

#[test]
fn reversal_uses_the_public_registered_effect_not_an_unrelated_last_toss() {
    for (description, expected) in [(62, -1), (63, 0)] {
        let mut s = seat(0);
        let at = loc(0, 4, 0);
        move_card(&mut s, EMPEROR, Loc::default(), at);
        hint(&mut s, at, 6, description);
        s.feed(&[msg::TOSS_COIN, 1, 1, 1]).unwrap();
        assert_eq!(reversal(&mut s, 0), expected);
    }
}
