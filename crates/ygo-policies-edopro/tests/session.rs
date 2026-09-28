//! A scripted EDOPro server: lobby handshake, then a short duel in which the
//! bot learns from `MSG_START` that it is seated second and answers its prompt.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use ygo_policies::cards::MemoryCards;
use ygo_policies::registry;
use ygo_policies_edopro::protocol::{ctos, read_packet, stoc, write_packet, DEFAULT_VERSION};
use ygo_policies_edopro::{play, ydk, Config, Outcome};
use ygo_policies_ocgcore::wire::{msg, Writer};
use ygo_policies_ocgcore::{start_message, Seat};

fn expect(stream: &mut TcpStream, opcode: u8) -> Vec<u8> {
    let (got, payload) = read_packet(stream).unwrap();
    assert_eq!(got, opcode, "unexpected client packet {got:#x}");
    payload
}

fn game(stream: &mut TcpStream, message: Vec<u8>) {
    write_packet(stream, stoc::GAME_MSG, &message).unwrap();
}

fn idle(player: u8) -> Vec<u8> {
    // No cards to use; Battle Phase and End Phase available.
    Writer::default().u8(msg::SELECT_IDLECMD).u8(player).u32(0).u32(0).u32(0).u32(0).u32(0).u32(0).u8(1).u8(1).u8(0).0
}

fn server(listener: TcpListener) {
    let (mut s, _) = listener.accept().unwrap();
    expect(&mut s, ctos::PLAYER_INFO);
    let join = expect(&mut s, ctos::JOIN_GAME);
    assert_eq!(u32::from_le_bytes(join[48..52].try_into().unwrap()), DEFAULT_VERSION);
    write_packet(&mut s, stoc::JOIN_GAME, &[0; 20]).unwrap();
    let deck = expect(&mut s, ctos::UPDATE_DECK);
    assert_eq!(u32::from_le_bytes(deck[0..4].try_into().unwrap()), 3);
    write_packet(&mut s, stoc::TYPE_CHANGE, &[0x01]).unwrap();
    expect(&mut s, ctos::HS_READY);
    write_packet(&mut s, stoc::DUEL_START, &[]).unwrap();
    write_packet(&mut s, stoc::SELECT_HAND, &[]).unwrap();
    let hand = expect(&mut s, ctos::HAND_RESULT);
    assert!((1..=3).contains(&hand[0]));
    write_packet(&mut s, stoc::SELECT_TP, &[]).unwrap();
    assert_eq!(expect(&mut s, ctos::TP_RESULT), vec![1]);

    // The opponent went first after all: the bot is duel seat 1.
    game(&mut s, start_message(1, [8000, 8000], [40, 40], [0, 0]));
    game(&mut s, Writer::default().u8(msg::NEW_TURN).u8(0).0);
    game(&mut s, Writer::default().u8(msg::NEW_PHASE).u16(0x04).0);
    game(&mut s, Writer::default().u8(msg::NEW_TURN).u8(1).0);
    game(&mut s, Writer::default().u8(msg::NEW_PHASE).u16(0x04).0);
    game(&mut s, idle(1));
    let response = expect(&mut s, ctos::RESPONSE);
    assert_eq!(response, 7i32.to_le_bytes().to_vec(), "nothing to attack with: end the turn");
    game(&mut s, Writer::default().u8(msg::WIN).u8(1).u8(0).0);
    s.flush().unwrap();
    // The bot hangs up without sending anything else.
    assert!(read_packet(&mut s).is_err(), "unexpected packet after the duel");
}

#[test]
fn joins_a_room_and_plays_its_own_seat() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let host = std::thread::spawn(move || server(listener));
    let config = Config {
        name: "[AI] test".into(),
        room_id: 0,
        password: String::new(),
        version: DEFAULT_VERSION,
        deck: ydk::parse("#main\n1\n2\n#extra\n3\n!side\n"),
        go_first: true,
    };
    let db = Arc::new(MemoryCards::default());
    let mut new_seat = || Seat::new(registry::create("blackwing", db.clone()).unwrap(), db.clone(), None);
    let mut stream = TcpStream::connect(address).unwrap();
    let outcome = play(&mut stream, &config, &mut new_seat, &mut ()).unwrap();
    drop(stream); // hang up, as edopro-bot does after a duel
    host.join().unwrap();
    assert_eq!(outcome, Outcome::Finished { seat: Some(1), winner: 1, reason: 0 });
}
