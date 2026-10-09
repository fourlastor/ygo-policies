//! Reproducible matchups, round robins, and paired policy comparisons.
mod engine;
mod knowledge;
mod probe;
mod replay;
mod report;
mod ygo_agent;
use engine::{Asked, Core, Deck, PlayOptions, PolicyLibrary, Result, SearchOptions};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
};

pub const EXISTING: &str = "blackwing,burn,rock-block,monarch,lightsworn,infernity,gladiator,gishki,crystal,morphtronic,dragunity,spellcaster,heroes";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Job {
    a: String,
    b: String,
    seed: u64,
    seat: usize,
}

fn names(spec: &str, catalog: &std::collections::HashMap<String, String>) -> Result<Vec<String>> {
    let mut result: Vec<_> = match spec {
        "all" => catalog.keys().filter(|n| n.as_str() != "ygo-agent" && !BENCHMARK_PLAYERS.contains(&n.as_str())).cloned().collect(),
        "existing" => EXISTING.split(',').map(str::to_owned).collect(),
        _ => spec.split(',').map(str::to_owned).collect(),
    };
    result.sort();
    result.dedup();
    for name in &result {
        if !catalog.contains_key(name) {
            return Err(format!("Unknown policy: {name}"));
        }
    }
    Ok(result)
}

/// The policies of the ygo-agent benchmark, beside the roster.
const BENCHMARK_PLAYERS: [&str; 4] = ["blue-eyes", "blue-eyes-improved", "first", "random"];

/// `first`: the policy that takes seat 0, and so the first turn, in every
/// game of its pairs.  Otherwise each pair alternates seats.
fn jobs(pilots: &[String], opponents: &[String], mode: &str, games: usize, seed: u64, first: Option<&str>) -> Vec<Job> {
    let mut pairs = BTreeSet::new();
    for a in pilots {
        for b in opponents {
            // A pair of the same name is a matchup's, and among the benchmark's
            // own players a search's: the pilot with a search against itself.
            if a == b && mode != "matchup" && !(mode == "search" && BENCHMARK_PLAYERS.contains(&a.as_str())) {
                continue;
            }
            // A searching pilot keeps its side of each pair, as a compared one does.
            if mode != "compare" && mode != "search" && a > b {
                pairs.insert((b.clone(), a.clone()));
            } else {
                pairs.insert((a.clone(), b.clone()));
            }
        }
    }
    pairs
        .into_iter()
        .flat_map(|(a, b)| {
            // Names, not list indices, determine the seed: filtering/reordering a
            // tournament preserves every retained game's shuffle and engine RNG.
            let mut pair = [a.as_str(), b.as_str()];
            pair.sort();
            let salt = fingerprint(format!("{}\0{}", pair[0], pair[1]).as_bytes());
            let fixed = first.map(|p| if a == p { 0 } else { 1 });
            (0..games).map(move |i| Job {
                a: a.clone(),
                b: b.clone(),
                seed: seed.wrapping_add(salt).wrapping_add(i as u64) & 0x7fffffff,
                seat: fixed.unwrap_or(i % 2),
            })
        })
        .collect()
}

/// The monsters to probe: `--only CODE,CODE`, or every allowed monster of
/// `--pool` (an EDOPro lflist; default `data/wc2011.lflist.conf`).
fn pool(args: &BTreeMap<String, String>, root: &Path, cards: &Path) -> Result<Vec<u32>> {
    let db = rusqlite::Connection::open_with_flags(cards, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
    let monster = |code: u32| -> bool {
        db.query_row("select type from datas where id = ?", [code], |row| row.get::<_, i64>(0)).map_or(false, |kind| kind & 1 != 0)
    };
    let mut codes = Vec::new();
    if let Some(only) = args.get("--only") {
        for code in only.split(',') {
            codes.push(code.trim().parse().map_err(|_| format!("Invalid card code: {code}"))?);
        }
        return Ok(codes);
    }
    let path = args.get("--pool").map(PathBuf::from).unwrap_or_else(|| root.join("data/wc2011.lflist.conf"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(Ok(code)), Some(Ok(limit))) = (parts.next().map(str::parse::<u32>), parts.next().map(str::parse::<i32>)) {
            if limit > 0 && monster(code) {
                codes.push(code);
            }
        }
    }
    codes.sort_unstable();
    codes.dedup();
    Ok(codes)
}

/// Probe every monster, each worker on its own engine.  In `codes` order.
fn examine_all(codes: &[u32], workers: usize, core: Option<&Path>, cards: &Path, scripts: &Path) -> Result<Vec<probe::Examined>> {
    let cursor = std::sync::atomic::AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut results: Vec<Option<probe::Examined>> = vec![None; codes.len()];
    std::thread::scope(|scope| -> Result<()> {
        for _ in 0..workers.max(1) {
            let tx = tx.clone();
            let cursor = &cursor;
            scope.spawn(move || {
                let work = || -> Result<()> {
                    let mut lab = probe::Lab::new(Core::open(core, cards, scripts)?)?;
                    loop {
                        let index = cursor.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(code) = codes.get(index) else { break };
                        let examined = lab.examine(*code).map_err(|e| format!("{code}: {e}"))?;
                        tx.send(Ok((index, examined))).map_err(|e| e.to_string())?;
                    }
                    Ok(())
                };
                if let Err(e) = work() {
                    let _ = tx.send(Err(e));
                }
            });
        }
        drop(tx);
        let mut done = 0;
        for result in rx {
            let (index, examined) = result?;
            results[index] = Some(examined);
            done += 1;
            if done % 500 == 0 {
                eprintln!("{done}/{} monsters", codes.len());
            }
        }
        Ok(())
    })?;
    results.into_iter().collect::<Option<Vec<_>>>().ok_or_else(|| "Incomplete probe run".to_string())
}

/// A finished game with the score of the player in `seat`, or its error.
fn scored(game: Result<Value>, seat: usize) -> Value {
    match game {
        Ok(mut game) => {
            game["score"] = json!(match game["winner"].as_u64() {
                Some(winner) if winner < 2 =>
                    if winner == seat as u64 {
                        1.0
                    } else {
                        0.0
                    },
                _ => 0.5,
            });
            game
        }
        Err(e) => json!({"error": e}),
    }
}

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, b| {
        (hash ^ *b as u64).wrapping_mul(0x100000001b3)
    })
}

fn file_identity(path: &Path) -> Value {
    match std::fs::read(path) {
        Ok(bytes) => {
            json!({"path": path, "bytes": bytes.len(), "fnv1a64": format!("{:016x}", fingerprint(&bytes))})
        }
        Err(e) => json!({"path": path, "error": e.to_string()}),
    }
}

fn git_revision(path: &Path) -> Value {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    json!({"path": path, "commit": git(&["rev-parse", "HEAD"]), "status": git(&["status", "--porcelain"])})
}

fn help() {
    println!(
        "policy-bench <matchup|round-robin|compare|search|rank|knowledge|probe|replay> [OPTIONS]\n\
  matchup --policies arcana --opponents blackwing,monarch\n\
  search --policies monarch --opponents existing\n\
  round-robin --policies all --games 256 --markdown DECK-TIER-LIST.md\n\
  compare --policies arcana --opponents existing --baseline old.so --candidate new.so\n\
  rank --input tournament.jsonl --markdown DECK-TIER-LIST.md\n\
  knowledge --workers 8 --report data/wc2011-monster-facts.md\n\
  probe --only 26593852 --output catastor.jsonl\n\
  replay --input beat-claudio.sqlite --duel 7 --recheck true\n\
\nOptions: --output results.jsonl --games 64 --seed 730000 --workers 4\n\
  --library PATH (matchup/round-robin; defaults to target/release/libygo_policies.so)\n\
  --core PATH (optional external OCGCore; default builds the pinned vendor engine)\n\
  --cards PATH --scripts DIR --decks DIR (default to this checkout's vendor/decks)\n\
  --reference existing (ranking reference pool; all or comma-separated ids also work)\n\
  --limit 4096 --trace true --markdown PATH\n\
  --first POLICY (that policy goes first in every game; default: seats alternate)\n\
  --lp 8000,4000 (starting Life Points of the first and second player)\n\
  --rules mr1|mr5 (default mr1)\n\
  --server http://127.0.0.1:3013 (required for ygo-agent; MR5 matchup or search)\n\
  --stand-in POLICY (search against ygo-agent: the policy that plays the model's seat in the\n\
  try-outs, which never ask the server; default: the searching policy itself; ygo-agent:\n\
  copies of the model's own seat, every decision of theirs a request)\n\
  --carried true (ygo-agent: the seat carries the duel's memory and asks /v1/predict; the\n\
  server keeps no duel, and with --stand-in ygo-agent it is always so)\n\
  matchup --policies blue-eyes --opponents random,first --rules mr5 --games 1000\n\
  matchup --policies ygo-agent --opponents ygo-agent --rules mr5 --server URL\n\
Deck lists and libraries are never modified. Each pair alternates seats.\n\
Compare keeps opponents on the baseline library and runs both versions on identical seeds.\n\
\nknowledge stages every monster of a card pool against the engine and writes what it\n\
shows as the policies' card facts (crates/ygo-policies/src/knowledge/pool.rs):\n\
  --pool FILE (an EDOPro lflist; default data/wc2011.lflist.conf) or --only CODE,CODE\n\
  --rust PATH (the table) --report PATH (the same in words, with what needs review)\n\
  --output PATH (the staged attacks this build's tactics call wrong)\n\
probe writes what the engine did, monster by monster, as JSON lines (--output).\n\
search plays each game twice on the same seed: the pilot alone, and the pilot with a\n\
one-step search at each of its decisions (see the bench README).\n\
  --worlds 8 --confirm 32 --final 96 (worlds tried after each stage)\n\
  --z 1.645 (how far ahead of the pilot's answer an alternative must be)\n\
  --strict false (also search while the other player has a face-down monster, which\n\
  the worlds cannot deal again: the search then sees what it is)\n\
  --foresight true (not a player: tries every alternative in the world as it is, hidden\n\
  cards and draws to come included, and leaves the pilot's answer when it loses and another\n\
  wins; what it still loses, no single change of answer could have won)\n\
  --log true (write every decision examined: the situation, how each alternative did in\n\
  the first-stage worlds, the playouts it took and what was chosen)\n\
  --log full (the same, with the alternatives also tried where the pilot's answer wins\n\
  every first-stage world: the games are the same, the playouts up to a fifth more)\n\
  --validate true (search nothing; check that snapshots and rebuilt seats replay the duel)\n\
  --record true (also for matchup, compare and round-robin: write each duel as a record that\n\
  replay plays again; a deviation's `decision` is its answer's place in the record's responses)\n\
replay plays a duel from Beat Claudi-oh's database again (--input, and --duel ID, default\n\
the last one) and tells it turn by turn with nothing hidden (to --output, else printed).\n\
  --input also takes a .json file holding one record, or the .jsonl of a run made with\n\
  --record true: --duel is then the row (the first is 0, default the last) and --which the\n\
  duel of that row (search, baseline, candidate or game; default the searched one).\n\
  --recheck true also asks the policy that played, as --library has it now, at each of its\n\
  decisions, and says where it would answer otherwise."
    );
}

fn run() -> Result<()> {
    let mut iter = std::env::args().skip(1);
    let Some(mode) = iter.next() else {
        help();
        return Ok(());
    };
    if mode == "--help" {
        help();
        return Ok(());
    }
    if !["matchup", "round-robin", "compare", "search", "rank", "probe", "knowledge", "replay"].contains(&mode.as_str()) {
        return Err(format!("Unknown mode {mode}; use --help"));
    }
    let mut args = BTreeMap::new();
    while let Some(key) = iter.next() {
        if key == "--help" {
            help();
            return Ok(());
        }
        if ![
            "--core",
            "--cards",
            "--scripts",
            "--decks",
            "--baseline",
            "--candidate",
            "--library",
            "--output",
            "--input",
            "--policies",
            "--opponents",
            "--reference",
            "--games",
            "--seed",
            "--workers",
            "--limit",
            "--trace",
            "--markdown",
            "--first",
            "--lp",
            "--only",
            "--pool",
            "--rust",
            "--report",
            "--duel",
            "--recheck",
            "--worlds",
            "--confirm",
            "--final",
            "--z",
            "--validate",
            "--strict",
            "--foresight",
            "--log",
            "--record",
            "--which",
            "--rules",
            "--server",
            "--stand-in",
            "--carried",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("Unknown argument: {key}"));
        }
        let value = iter
            .next()
            .ok_or_else(|| format!("Missing value for {key}"))?;
        if args.insert(key.clone(), value).is_some() {
            return Err(format!("Repeated argument: {key}"));
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = |key: &str, default: &str| {
        args.get(key)
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join(default))
    };
    let required = |key: &str| -> Result<PathBuf> {
        args.get(key)
            .map(PathBuf::from)
            .ok_or_else(|| format!("Required: {key}"))
    };
    let number = |key: &str, default| -> Result<usize> {
        args.get(key).map_or(Ok(default), |v| {
            v.parse().map_err(|_| format!("Invalid {key}"))
        })
    };
    let output = args
        .get("--output")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("benchmark.jsonl"));
    let markdown = args.get("--markdown").map(PathBuf::from);
    if mode == "rank" {
        let input = required("--input")?;
        let text = std::fs::read_to_string(&input).map_err(|e| e.to_string())?;
        let rows: Vec<Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).map_err(|e| e.to_string()))
            .collect::<Result<_>>()?;
        let metadata: Value = serde_json::from_slice(
            &std::fs::read(input.with_extension("metadata.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if rows.iter().any(|r| r["a"] == r["b"]
            || [r["a"].as_str().unwrap_or(""), r["b"].as_str().unwrap_or("")]
                .iter().any(|n| *n == "ygo-agent" || BENCHMARK_PLAYERS.contains(n))) {
            return ygo_agent::summary(&rows, &metadata, &input.with_extension("summary.json"));
        }
        return report::ranking(
            &rows,
            &metadata,
            &input.with_extension("summary.json"),
            markdown.as_deref(),
        );
    }
    let cards = path("--cards", "vendor/BabelCdb/cards.cdb");
    let scripts = path("--scripts", "vendor/CardScripts");
    let decks = path("--decks", "decks");
    let core_path = args.get("--core").map(PathBuf::from);
    if mode == "replay" {
        // A duel from Beat Claudi-oh's log, played again and told.
        let duel = args.get("--duel").map(|v| v.parse().map_err(|_| "Invalid --duel".to_string())).transpose()?;
        let record = replay::load(&required("--input")?, duel, args.get("--which").map(String::as_str))?;
        let mut core = Core::open(core_path.as_deref(), &cards, &scripts)?;
        // The policy that played, as it is built now, asked at each of its decisions.
        let library = match args.get("--recheck").map(String::as_str).unwrap_or("false") {
            "true" => {
                let name = format!("target/release/{}ygo_policies{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX);
                Some(PolicyLibrary::open(&path("--library", &name))?)
            }
            "false" => None,
            _ => return Err("Invalid --recheck".into()),
        };
        let asked = match (&library, &record.policy) {
            (Some(library), Some((policy, seat))) => Some(Asked { library, policy, cards: &cards, seat: *seat }),
            (Some(_), None) => return Err("--recheck needs a duel from the database: a bare replay does not say which policy played it".into()),
            (None, _) => None,
        };
        let story = replay::narrate(&mut core, &record, &replay::names(&cards)?, asked.as_ref())?;
        match args.get("--output") {
            Some(path) => std::fs::write(path, story).map_err(|e| format!("{path}: {e}"))?,
            None => print!("{story}"),
        }
        return Ok(());
    }
    if mode == "probe" {
        // What the engine shows about each monster of a pool, as JSON lines.
        let codes = pool(&args, &root, &cards)?;
        let workers = number("--workers", 4)?;
        let examined = examine_all(&codes, workers, core_path.as_deref(), &cards, &scripts)?;
        let mut file = std::fs::File::create(&output).map_err(|e| format!("{}: {e}", output.display()))?;
        for e in &examined {
            writeln!(file, "{}", probe::examined_json(e)).map_err(|e| e.to_string())?;
        }
        eprintln!("{} monsters probed -> {}", examined.len(), output.display());
        return Ok(());
    }
    if mode == "knowledge" {
        // The facts the policies use, from what the probes show.
        let codes = pool(&args, &root, &cards)?;
        let workers = number("--workers", 4)?;
        let examined = examine_all(&codes, workers, core_path.as_deref(), &cards, &scripts)?;
        let db = rusqlite::Connection::open_with_flags(&cards, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
        let name = |code: u32| -> String {
            db.query_row("select name from texts where id = ?", [code], |row| row.get::<_, String>(0)).unwrap_or_else(|_| code.to_string())
        };
        let derived: BTreeMap<u32, knowledge::Derived> = examined.iter().map(|e| (e.code, knowledge::derive(e))).collect();
        let pool_name = args.get("--pool").map(|p| format!("`{p}`")).unwrap_or_else(|| "`data/wc2011.lflist.conf`".into());
        let known = derived.values().filter(|d| d.facts != ygo_policies::knowledge::Facts::NONE).count();
        if !args.contains_key("--only") {
            let rust = path("--rust", "crates/ygo-policies/src/knowledge/pool.rs");
            std::fs::write(&rust, knowledge::table(&derived, &name, &pool_name)).map_err(|e| format!("{}: {e}", rust.display()))?;
            eprintln!("{} monsters probed, {known} with something to know -> {}", derived.len(), rust.display());
        }
        if let Some(report) = args.get("--report") {
            std::fs::write(report, knowledge::report(&examined, &derived, &name)).map_err(|e| format!("{report}: {e}"))?;
        }
        // How well the tactics built into this binary call the battles staged.
        let (all, special, misses) = knowledge::agreement(&examined);
        let share = |n: [usize; 2]| if n[0] == 0 { 0.0 } else { 100.0 * n[1] as f64 / n[0] as f64 };
        eprintln!("attacks staged: {}, called right by this build's tactics: {} ({:.1}%)", all[0], all[1], share(all));
        eprintln!("  of the {} that printed stats alone do not explain: {} ({:.1}%)", special[0], special[1], share(special));
        if let Some(path) = args.get("--output") {
            // The attacks called wrong, for a look at what the facts still miss.
            let mut text = String::new();
            for (code, miss) in &misses {
                text.push_str(&format!("{} ({code}): {miss}\n", name(*code)));
            }
            std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))?;
        }
        if args.contains_key("--only") {
            for e in &examined {
                let d = &derived[&e.code];
                println!("{} {}", e.code, name(e.code));
                for line in knowledge::describe(&d.facts) {
                    println!("  {line}");
                }
                for note in &d.notes {
                    println!("  note: {note}");
                }
            }
            for (code, miss) in misses.iter().take(8) {
                println!("  miss: {} {miss}", name(*code));
            }
        }
        return Ok(());
    }
    let library = path(
        "--library",
        &format!(
            "target/release/{}ygo_policies{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        ),
    );
    let baseline = if mode == "compare" {
        required("--baseline")?
    } else {
        library.clone()
    };
    let candidate = if mode == "compare" {
        required("--candidate")?
    } else {
        library
    };
    let mut catalog = PolicyLibrary::open(&baseline)?.catalog;
    catalog.insert("ygo-agent".into(), "BlueEyes".into());
    if mode != "compare" && (args.contains_key("--baseline") || args.contains_key("--candidate")) {
        return Err("--baseline/--candidate require compare mode".into());
    }
    let mut candidate_catalog = PolicyLibrary::open(&candidate)?.catalog;
    candidate_catalog.insert("ygo-agent".into(), "BlueEyes".into());
    let pilots = names(
        args.get("--policies").map(String::as_str).unwrap_or("all"),
        &catalog,
    )?;
    let opponents = if mode == "round-robin" {
        pilots.clone()
    } else {
        names(
            args.get("--opponents")
                .map(String::as_str)
                .unwrap_or("existing"),
            &catalog,
        )?
    };
    if mode == "round-robin" && args.contains_key("--opponents") {
        return Err(
            "round-robin uses --policies for both sides; use matchup for --opponents".into(),
        );
    }
    for pilot in &pilots {
        if candidate_catalog.get(pilot) != catalog.get(pilot) {
            return Err(format!("Candidate deck/catalog differs for {pilot}"));
        }
    }
    let reference = names(
        args.get("--reference")
            .map(String::as_str)
            .unwrap_or("existing"),
        &catalog,
    )?;
    let games = number("--games", 64)?;
    let workers = number("--workers", 4)?;
    let seed = number("--seed", 730000)? as u64;
    let limit = number("--limit", 4096)?;
    let trace = match args.get("--trace").map(String::as_str).unwrap_or("false") {
        "true" => true,
        "false" => false,
        _ => return Err("--trace must be true or false".into()),
    };
    if games == 0 || games % 2 != 0 || workers == 0 || limit == 0 {
        return Err("games must be positive and even; workers and limit must be positive".into());
    }
    let first = args.get("--first").map(String::as_str);
    let life_points: [u32; 2] = match args.get("--lp") {
        None => [8000, 8000],
        Some(v) => match v.split(',').map(|n| n.trim().parse::<u32>()).collect::<std::result::Result<Vec<_>, _>>() {
            Ok(lp) if lp.len() == 2 && lp.iter().all(|&n| n > 0) => [lp[0], lp[1]],
            _ => return Err("--lp takes two positive numbers: FIRST,SECOND".into()),
        },
    };
    let jobs = jobs(&pilots, &opponents, mode.as_str(), games, seed, first);
    let stages = [number("--worlds", 8)?, number("--confirm", 32)?, number("--final", 96)?];
    let z: f64 = args.get("--z").map_or(Ok(1.645), |v| v.parse().map_err(|_| "Invalid --z".to_string()))?;
    let flag = |key: &str, default: bool| -> Result<bool> {
        match args.get(key).map(String::as_str) {
            None => Ok(default),
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            Some(_) => Err(format!("{key} must be true or false")),
        }
    };
    let (validate, strict, foresight) = (flag("--validate", false)?, flag("--strict", true)?, flag("--foresight", false)?);
    let (log, log_all) = match args.get("--log").map(String::as_str) {
        None | Some("false") => (false, false),
        Some("true") => (true, false),
        Some("full") => (true, true),
        Some(_) => return Err("--log must be true, false or full".into()),
    };
    let record = flag("--record", false)?;
    let flags = match args.get("--rules").map(String::as_str).unwrap_or("mr1") {
        "mr1" => engine::DuelOptions::MASTER_RULE_1,
        "mr5" => engine::DuelOptions::MASTER_RULE_5,
        _ => return Err("--rules must be mr1 or mr5".into()),
    };
    let has_agent = pilots.iter().chain(&opponents).any(|p| p == "ygo-agent");
    let server = args.get("--server").map(String::as_str);
    if has_agent && (!["matchup", "search"].contains(&mode.as_str()) || server.is_none() || flags != engine::DuelOptions::MASTER_RULE_5) {
        return Err("ygo-agent requires matchup or search, --rules mr5 --server http://HOST:PORT".into());
    }
    // In a search the model is the other player, and a policy stands in for it in the try-outs.
    if has_agent && mode == "search" && jobs.iter().any(|j| j.a == "ygo-agent") {
        return Err("search against ygo-agent: --policies names the policy that searches, --opponents the model".into());
    }
    let stand_in = args.get("--stand-in").map(String::as_str);
    if let Some(name) = stand_in {
        if !has_agent || mode != "search" {
            return Err("--stand-in is for a search against ygo-agent".into());
        }
        if !catalog.contains_key(name) {
            return Err(format!("--stand-in {name}: not a policy of the library"));
        }
    }
    // The model in its own place in the try-outs: its seat is copied for each,
    // which only a seat that carries the duel's memory can be.
    let itself = stand_in == Some("ygo-agent");
    let carried = flag("--carried", false)? || itself;
    if carried && !has_agent {
        return Err("--carried is for ygo-agent".into());
    }
    if mode == "search" && (stages[0] == 0 || stages[0] > stages[1] || stages[1] > stages[2]) {
        return Err("search needs --worlds <= --confirm <= --final".into());
    }
    if let Some(first) = first {
        if mode != "matchup" && mode != "round-robin" || jobs.iter().any(|j| j.a != first && j.b != first) {
            return Err(format!("--first {first}: every pair must include it (matchup or round-robin with it)"));
        }
    }
    if jobs.is_empty() {
        return Err("No distinct matchups selected".into());
    }
    let mut deck_map = BTreeMap::new();
    let mut deck_ids = BTreeMap::new();
    for name in pilots.iter().chain(&opponents) {
        let file = decks.join(format!("{}.ydk", catalog[name]));
        deck_map.insert(name.clone(), Deck::load(&file)?);
        deck_ids.insert(name.clone(), file_identity(&file));
    }
    let metadata = json!({"mode": mode, "arguments": args, "games_per_matchup": games, "seed": seed,
        "decision_limit": limit, "life_points": life_points, "first": first,
        "rules": if flags == engine::DuelOptions::MASTER_RULE_5 { "MasterRule5 (0x2E800)" } else { "MasterRule1 (0xD0700)" },
        "shuffle": "SplitMix64 Fisher-Yates v1; stable pair-name seed", "policy_seed": "engine seed + seat",
        "engine": core_path.as_ref().map(|p| file_identity(p)).unwrap_or_else(|| json!({"linked": true})),
        "cards": file_identity(&cards), "scripts": git_revision(&scripts), "core": git_revision(&root.join("vendor/ocgcore")),
        "lua": git_revision(&root.join("vendor/ocgcore/lua/src")), "checkout": git_revision(&root),
        "baseline": file_identity(&baseline), "candidate": file_identity(&candidate), "stand_in": stand_in, "carried": carried,
        "decks": deck_ids, "catalog": catalog, "reference": reference, "planned_jobs": jobs.len()});
    // Refuse accidental truncation of an earlier run; choose another output name.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .map_err(|e| format!("{}: {e}", output.display()))?;
    std::fs::write(
        output.with_extension("metadata.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let cursor = std::sync::atomic::AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut results = Vec::new();
    let mut completed = 0;
    let mut search_scores = [0.0; 2];
    std::thread::scope(|scope| -> Result<()> {
        for _ in 0..workers {
            let tx = tx.clone();
            let (jobs, cursor, cards, scripts, baseline, candidate, core_path, deck_map, mode) = (
                &jobs, &cursor, &cards, &scripts, &baseline, &candidate, &core_path, &deck_map,
                &mode,
            );
            scope.spawn(move || {
                let work = || -> Result<()> {
                    let core = Core::open(core_path.as_deref(), cards, scripts)?;
                    let baseline = PolicyLibrary::open(baseline)?;
                    let candidate = PolicyLibrary::open(candidate)?;
                    loop {
                        let index = cursor.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(job) = jobs.get(index) else { break };
                        let (a, b, seed, seat) =
                            (job.a.as_str(), job.b.as_str(), job.seed, job.seat);
                        let names = if seat == 0 { [a, b] } else { [b, a] };
                        let decks = [deck_map[names[0]].clone(), deck_map[names[1]].clone()];
                        let mut row = json!({"a": a, "b": b, "seed": seed, "seat": seat});
                        if mode == "search" {
                            let policies = [&candidate, &candidate];
                            let options = PlayOptions { seed, limit, trace: false, life_points, record, flags };
                            // Against the model the plain game is the one `matchup` plays, and in the
                            // searched one the model answers its seat while a policy stands in for it
                            // in the try-outs.
                            let model = names.contains(&"ygo-agent");
                            row["baseline"] = scored(
                                if model {
                                    ygo_agent::play(&core, &decks, &candidate, names, cards, options, server.unwrap(), carried)
                                } else {
                                    engine::play(&core, &decks, policies, names, cards, options)
                                },
                                seat,
                            );
                            let started = std::time::Instant::now();
                            let search = SearchOptions { searcher: seat, stages, z, validate, strict, foresight, log, log_all };
                            let searched = || -> Result<Value> {
                                let outside = if model {
                                    Some(engine::Outside {
                                        seat: ygo_agent::seat(server.unwrap(), 1 - seat as u8, cards, carried)?,
                                        // With the model in its own place, the searching policy only sits at the table.
                                        stand_in: if itself { a } else { stand_in.unwrap_or(a) },
                                        itself,
                                    })
                                } else {
                                    None
                                };
                                engine::play_searching(&core, &decks, policies, names, cards, options, search, outside)
                            };
                            row["search"] = scored(searched(), seat);
                            row["search"]["seconds"] = json!(started.elapsed().as_secs_f64());
                            tx.send(Ok(row)).map_err(|e| e.to_string())?;
                            continue;
                        }
                        let versions = if mode == "compare" {
                            vec![("baseline", &baseline), ("candidate", &candidate)]
                        } else {
                            vec![("game", &candidate)]
                        };
                        for (label, library) in versions {
                            let policies = if mode != "compare" {
                                [library, library]
                            } else if seat == 0 {
                                [library, &baseline]
                            } else {
                                [&baseline, library]
                            };
                            let options = PlayOptions { seed, limit, trace, life_points, record, flags };
                            let game = if names.contains(&"ygo-agent") {
                                ygo_agent::play(&core, &decks, library, names, cards, options, server.unwrap(), carried)
                            } else {
                                engine::play(&core, &decks, policies, names, cards, options)
                            };
                            row[label] = scored(game, seat);
                        }
                        tx.send(Ok(row)).map_err(|e| e.to_string())?;
                    }
                    Ok(())
                };
                if let Err(e) = work() {
                    let _ = tx.send(Err(e));
                }
            });
        }
        drop(tx);
        let started = std::time::Instant::now();
        for result in rx {
            let result = result?;
            writeln!(file, "{result}").map_err(|e| e.to_string())?;
            completed += 1;
            if mode == "search" {
                for (sum, label) in search_scores.iter_mut().zip(["baseline", "search"]) {
                    *sum += result[label]["score"].as_f64().unwrap_or(0.0);
                }
                if completed % 16 == 0 || completed == jobs.len() {
                    let rate = |index: usize| 100.0 * search_scores[index] / completed as f64;
                    eprintln!(
                        "{}/{} games, {:.0} s: pilot {:.1}%, searching {:.1}%",
                        completed,
                        jobs.len(),
                        started.elapsed().as_secs_f64(),
                        rate(0),
                        rate(1)
                    );
                }
                // Already written in full; search needs only running totals.
            } else {
                results.push(result);
                if completed % games.max(128) == 0 || completed == jobs.len() {
                    eprintln!(
                        "{}/{} {}",
                        completed,
                        jobs.len(),
                        if mode == "compare" { "pairs" } else { "games" }
                    );
                }
            }
        }
        Ok(())
    })?;
    if completed != jobs.len() {
        return Err("Incomplete run".into());
    }
    if mode == "search" {
        // The rows hold both games of each seed; their analysis is left to the reader.
        Ok(())
    } else if mode == "compare" {
        report::comparison(&results, &output.with_extension("summary.json"))
    } else if mode == "matchup" && (pilots.iter().chain(&opponents)
        .any(|n| n == "ygo-agent" || BENCHMARK_PLAYERS.contains(&n.as_str()))
        || results.iter().any(|r| r["a"] == r["b"])) {
        ygo_agent::summary(&results, &metadata, &output.with_extension("summary.json"))
    } else {
        report::ranking(
            &results,
            &metadata,
            &output.with_extension("summary.json"),
            markdown.as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn improved_blue_eyes_remains_outside_the_wc2011_roster() {
        let catalog = [("blue-eyes", "BlueEyes"), ("blue-eyes-improved", "BlueEyes"), ("monarch", "Emperor, Arise!")]
            .into_iter().map(|(id, deck)| (id.to_owned(), deck.to_owned())).collect();
        assert_eq!(super::names("all", &catalog).unwrap(), vec!["monarch"]);
        assert_eq!(super::names("blue-eyes,blue-eyes-improved", &catalog).unwrap().len(), 2);
    }
    use super::*;
    #[test]
    fn round_robin_is_unique_balanced_and_subset_stable() {
        let names = vec!["a".into(), "b".into(), "c".into()];
        let all = jobs(&names, &names, "round-robin", 4, 7, None);
        assert_eq!(all.len(), 12);
        let subset = jobs(&names[..1], &names[1..2], "matchup", 4, 7, None);
        assert_eq!(
            subset,
            all.iter()
                .filter(|j| j.a == "a" && j.b == "b")
                .cloned()
                .collect::<Vec<_>>()
        );
        assert_eq!(subset.iter().filter(|j| j.seat == 0).count(), 2);
        assert!(all.iter().all(|j| j.a < j.b));
    }

    #[test]
    fn mirrors_have_one_job_per_deal_and_alternate_the_named_player() {
        let players=vec!["ygo-agent".into()];
        let mirror=jobs(&players,&players,"matchup",10,7,None);
        assert_eq!(mirror.len(),10);
        assert_eq!(mirror.iter().filter(|j|j.seat==0).count(),5);
        assert!(mirror.iter().all(|j|j.a==j.b));
        assert_eq!(mirror, jobs(&players,&players,"matchup",10,7,None));
    }

    #[test]
    fn a_search_pairs_a_benchmark_player_with_itself_on_the_deals_of_its_matchup() {
        let pilot = vec!["blue-eyes".to_owned()];
        let mirror = jobs(&pilot, &pilot, "search", 4, 7, None);
        assert_eq!(mirror.len(), 4);
        assert_eq!(mirror, jobs(&pilot, &pilot, "matchup", 4, 7, None));
        // A policy of the roster is not searched against itself, as before,
        // and the policy that searches keeps its side whatever its name.
        let roster = vec!["monarch".to_owned()];
        assert!(jobs(&roster, &roster, "search", 4, 7, None).is_empty());
        let other = vec!["blackwing".to_owned()];
        assert!(jobs(&roster, &other, "search", 4, 7, None).iter().all(|j| j.a == "monarch" && j.b == "blackwing"));
        // Against the model the deals are those of the matchup too.
        let model = vec!["ygo-agent".to_owned()];
        assert_eq!(jobs(&pilot, &model, "search", 4, 7, None), jobs(&model, &pilot, "matchup", 4, 7, None));
    }
}
