//! `edopro-bot`: play EDOPro duels with a policy.
//!
//! Host a room in EDOPro (LAN mode), then:
//!
//! ```text
//! edopro-bot --policy blackwing --cards /path/to/cards.cdb
//! edopro-bot --replay edopro-logs/<duel>.trace --policy blackwing --cards /path/to/cards.cdb
//! ```

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ygo_policies::cards::CardDatabase;
use ygo_policies::registry;
use ygo_policies_edopro::log::{describe, read_trace, DuelLog, Entry, Names};
use ygo_policies_edopro::{play, protocol, ydk, Config, Outcome};
use ygo_policies_ocgcore::wire::msg;
use ygo_policies_ocgcore::{Seat, SqliteCards};

const USAGE: &str = "\
usage: edopro-bot --policy ID --cards CARDS.CDB [options]
       edopro-bot --replay TRACE --policy ID --cards CARDS.CDB
       edopro-bot --list

  --host HOST        EDOPro host (default 127.0.0.1)
  --port PORT        EDOPro port (default 7911)
  --password TEXT    room password
  --room-id N        room id (default 0)
  --name TEXT        player name (default \"[AI] <policy>\")
  --deck FILE.ydk    deck to register (default: the policy's own deck)
  --version HEX      EDOPro ClientVersion (default 0x000b0029: client 41.0, core 11.0)
  --duels N          duels to play, reconnecting between them (default 1)
  --go-second        take the second turn when we win rock-paper-scissors
  --wait SECONDS     keep retrying the connection this long (default 60)
  --log-dir DIR      write <time>-<policy>.log and .trace here (default edopro-logs)
  --no-log           do not write logs";

struct Args {
    policy: Option<String>,
    cards: Option<PathBuf>,
    host: String,
    port: u16,
    password: String,
    room_id: u32,
    name: Option<String>,
    deck: Option<PathBuf>,
    version: u32,
    duels: u32,
    go_first: bool,
    wait: u64,
    log_dir: Option<PathBuf>,
    replay: Option<PathBuf>,
    list: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        policy: None,
        cards: None,
        host: "127.0.0.1".into(),
        port: 7911,
        password: String::new(),
        room_id: 0,
        name: None,
        deck: None,
        version: protocol::DEFAULT_VERSION,
        duels: 1,
        go_first: true,
        wait: 60,
        log_dir: Some(PathBuf::from("edopro-logs")),
        replay: None,
        list: false,
    };
    let mut raw = std::env::args().skip(1);
    while let Some(flag) = raw.next() {
        let mut value = || raw.next().ok_or_else(|| format!("{flag} needs a value"));
        let number = |text: String| text.parse::<u64>().map_err(|e| format!("{text}: {e}"));
        match flag.as_str() {
            "--policy" => args.policy = Some(value()?),
            "--cards" => args.cards = Some(value()?.into()),
            "--host" => args.host = value()?,
            "--port" => args.port = number(value()?)? as u16,
            "--password" => args.password = value()?,
            "--room-id" => args.room_id = number(value()?)? as u32,
            "--name" => args.name = Some(value()?),
            "--deck" => args.deck = Some(value()?.into()),
            "--version" => {
                let text = value()?;
                args.version = u32::from_str_radix(text.trim_start_matches("0x"), 16).map_err(|e| format!("{text}: {e}"))?;
            }
            "--duels" => args.duels = number(value()?)? as u32,
            "--go-second" => args.go_first = false,
            "--wait" => args.wait = number(value()?)?,
            "--log-dir" => args.log_dir = Some(value()?.into()),
            "--no-log" => args.log_dir = None,
            "--replay" => args.replay = Some(value()?.into()),
            "--list" => args.list = true,
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown option {other}\n{USAGE}")),
        }
    }
    Ok(args)
}

/// The deck lists shipped with the policies.
fn decks_dir() -> PathBuf {
    std::env::var_os("YGO_POLICIES_DECKS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../decks"))
}

fn replay(trace: &Path, policy: &str, db: Arc<dyn CardDatabase>, names: &Names) -> Result<(), String> {
    let entries = read_trace(trace).map_err(|e| format!("{}: {e}", trace.display()))?;
    let mut seat: Option<Seat> = None;
    let (mut decisions, mut diverged) = (0, 0);
    let mut pending: Option<Vec<u8>> = None;
    for entry in entries {
        match entry {
            Entry::Received(message) => {
                if message.first() == Some(&msg::START) {
                    seat = Some(Seat::new(registry::create(policy, db.clone()).unwrap(), db.clone(), None));
                }
                let Some(seat) = seat.as_mut() else { continue };
                if let Some(response) = seat.feed(&message).map_err(|e| e.to_string())? {
                    let answer = seat.last_answer().unwrap();
                    println!("{}", describe(names, &answer.decision, &answer.decision.choices[answer.choice]));
                    decisions += 1;
                    pending = Some(response);
                }
            }
            Entry::Sent(recorded) => {
                if let Some(ours) = pending.take() {
                    if ours != recorded {
                        diverged += 1;
                        println!("   ^ the recorded duel answered differently ({recorded:02x?})");
                    }
                }
            }
        }
    }
    println!("{decisions} decisions replayed, {diverged} differ from the recording");
    Ok(())
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    if args.list {
        for entry in registry::POLICIES {
            println!("{:12} {}", entry.id, entry.deck);
        }
        return Ok(());
    }
    let policy = args.policy.clone().ok_or_else(|| format!("--policy is required\n{USAGE}"))?;
    let entry = registry::find(&policy).ok_or_else(|| format!("unknown policy {policy:?} (see --list)"))?;
    let cards = args.cards.clone().ok_or_else(|| format!("--cards is required\n{USAGE}"))?;
    let db: Arc<dyn CardDatabase> =
        Arc::new(SqliteCards::open(&cards).map_err(|e| format!("{}: {e}", cards.display()))?);
    let names = || Names::open(&cards).unwrap_or_default();
    if let Some(trace) = &args.replay {
        return replay(trace, &policy, db, &names());
    }
    let deck_path = args.deck.clone().unwrap_or_else(|| decks_dir().join(format!("{}.ydk", entry.deck)));
    let deck = ydk::load(&deck_path).map_err(|e| format!("{}: {e}", deck_path.display()))?;
    let config = Config {
        name: args.name.clone().unwrap_or_else(|| format!("[AI] {policy}")),
        room_id: args.room_id,
        password: args.password.clone(),
        version: args.version,
        deck,
        go_first: args.go_first,
    };
    let mut new_seat = || Seat::new(registry::create(&policy, db.clone()).unwrap(), db.clone(), None);
    for duel in 1..=args.duels {
        let started = Instant::now();
        let mut stream = loop {
            match TcpStream::connect((args.host.as_str(), args.port)) {
                Ok(stream) => break stream,
                Err(error) if started.elapsed() < Duration::from_secs(args.wait) => {
                    eprintln!("waiting for EDOPro at {}:{} ({error})", args.host, args.port);
                    std::thread::sleep(Duration::from_secs(2));
                }
                Err(error) => return Err(format!("cannot reach EDOPro at {}:{}: {error}", args.host, args.port)),
            }
        };
        let _ = stream.set_nodelay(true);
        println!("duel {duel}/{}: joined {}:{} with {} ({})", args.duels, args.host, args.port, policy, deck_path.display());
        let outcome = match &args.log_dir {
            Some(dir) => {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                let stem = dir.join(format!("{stamp}-{policy}"));
                let mut log = DuelLog::create(&stem, names()).map_err(|e| e.to_string())?;
                println!("logging to {}.log / .trace", stem.display());
                play(&mut stream, &config, &mut new_seat, &mut log)
            }
            None => play(&mut stream, &config, &mut new_seat, &mut ()),
        }
        .map_err(|e| format!("connection lost: {e}"))?;
        match outcome {
            Outcome::Finished { seat, winner, reason } => {
                let result = match (seat, winner) {
                    (_, 2) => "draw".to_string(),
                    (Some(seat), winner) if seat == winner => "the bot won".to_string(),
                    (Some(_), _) => "the bot lost".to_string(),
                    (None, winner) => format!("player {winner} won"),
                };
                println!("duel {duel}: {result} (reason {reason})");
            }
            Outcome::Ended(why) => println!("duel {duel}: {why}"),
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
