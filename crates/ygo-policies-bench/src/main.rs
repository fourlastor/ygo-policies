//! Reproducible matchups, round robins, and paired policy comparisons.
use ygo_policies_bench::engine;
mod knowledge;
mod probe;
mod replay;
mod report;
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
        "all" => catalog.keys().cloned().collect(),
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

/// `first`: the policy that takes seat 0, and so the first turn, in every
/// game of its pairs.  Otherwise each pair alternates seats.
fn jobs(pilots: &[String], opponents: &[String], mode: &str, games: usize, seed: u64, first: Option<&str>) -> Vec<Job> {
    let mut pairs = BTreeSet::new();
    for a in pilots {
        for b in opponents {
            if a == b {
                continue;
            }
            if mode != "compare" && a > b {
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
        "policy-bench <matchup|round-robin|compare|search|generic|rank|knowledge|probe|replay> [OPTIONS]\n\
  matchup --policies arcana --opponents blackwing,monarch\n\
  search --policies monarch --opponents existing\n\
  round-robin --policies all --games 256 --markdown DECK-TIER-LIST.md\n\
  compare --policies arcana --opponents existing --baseline old.so --candidate new.so\n\
  generic --policies all --baseline frozen.so --library candidate.so --games 1024\n\
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
generic mirrors each selected specialist deck: generic versus the frozen baseline specialist.\n\
  --plain true (use the generic policy without engine search)\n\
  --effects true|false (public initial-effect metadata; default true)\n\
  --method rollout|beam|mcts (default rollout; planners preserve executable continuations)\n\
  --beam-width 8 --beam-actions 6 --beam-nodes 128 --beam-decisions 256 --beam-replies 1\n\
  Beam/MCTS: --horizon 0 validates to terminal; nonzero validates at the requested turn horizon.\n\
  --mcts-iterations 128 --mcts-worlds 4 --mcts-actions 6 --mcts-nodes 1024\n\
  --mcts-decisions 512 --mcts-replies 1 --mcts-terminal false (proposal return mode); --mcts-validate-roots false\n\
  --mcts-adversarial-opponent false (pessimistic opponent UCT proposals; shared fixed budget)\n\
  --opponent-model generic|specialist (default generic; specialist is a benchmark diagnostic)\n\
  --worlds 4 --max-candidates 6 --max-playouts 24 --horizon 0 --millis 200\n\
  --depth 1|2 (default 1; depth 2 adds nested sampled rollout search; ignored by --plain true)\n\
  --millis 0 uses only deterministic work limits; --horizon 0 rolls out to the duel limit.\n\
  --exceptions countdown (comma-separated ids exempted from the 55% target, still reported)\n\
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
    if !["matchup", "round-robin", "compare", "search", "generic", "rank", "probe", "knowledge", "replay"].contains(&mode.as_str()) {
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
            "--plain",
            "--max-candidates",
            "--max-playouts",
            "--horizon",
            "--depth",
            "--millis",
            "--exceptions",
            "--effects",
            "--method",
            "--beam-width",
            "--beam-actions",
            "--beam-nodes",
            "--beam-decisions",
            "--beam-replies",
            "--mcts-iterations",
            "--mcts-actions",
            "--mcts-nodes",
            "--mcts-decisions",
            "--mcts-replies",
            "--mcts-terminal",
            "--mcts-worlds",
            "--mcts-validate-roots",
            "--mcts-adversarial-opponent",
            "--opponent-model",
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
    if mode != "generic" {
        for key in ["--effects", "--method", "--beam-width", "--beam-actions", "--beam-nodes", "--beam-decisions", "--beam-replies", "--mcts-iterations", "--mcts-actions", "--mcts-nodes", "--mcts-decisions", "--mcts-replies", "--mcts-terminal", "--mcts-worlds", "--mcts-validate-roots", "--mcts-adversarial-opponent", "--opponent-model"] {
            if args.contains_key(key) { return Err(format!("{key} is supported only by generic mode")); }
        }
    }
    if mode != "generic" && args.contains_key("--depth") {
        return Err("--depth is supported only by generic mode".into());
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
    if mode == "generic" {
        return generic_run(&args, &root, &cards, &scripts, &decks, core_path.as_deref(), &output);
    }
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
    let catalog = PolicyLibrary::open(&baseline)?.catalog;
    if mode != "compare" && (args.contains_key("--baseline") || args.contains_key("--candidate")) {
        return Err("--baseline/--candidate require compare mode".into());
    }
    let candidate_catalog = PolicyLibrary::open(&candidate)?.catalog;
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
    // A searching pilot keeps its side of each pair, as a compared one does.
    let pairing = if mode == "search" { "compare" } else { mode.as_str() };
    let jobs = jobs(&pilots, &opponents, pairing, games, seed, first);
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
        "decision_limit": limit, "life_points": life_points, "first": first, "rules": "MasterRule1 (0xD0700)",
        "shuffle": "SplitMix64 Fisher-Yates v1; stable pair-name seed", "policy_seed": "engine seed + seat",
        "engine": core_path.as_ref().map(|p| file_identity(p)).unwrap_or_else(|| json!({"linked": true})),
        "cards": file_identity(&cards), "scripts": git_revision(&scripts), "core": git_revision(&root.join("vendor/ocgcore")),
        "lua": git_revision(&root.join("vendor/ocgcore/lua/src")), "checkout": git_revision(&root),
        "baseline": file_identity(&baseline), "candidate": file_identity(&candidate),
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
                            let options = PlayOptions { seed, limit, trace: false, life_points, record };
                            row["baseline"] = scored(engine::play(&core, &decks, policies, names, cards, options), seat);
                            let started = std::time::Instant::now();
                            let search = SearchOptions { searcher: seat, stages, z, validate, strict, foresight, log, log_all };
                            row["search"] = scored(engine::play_searching(&core, &decks, policies, names, cards, options, search), seat);
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
                            row[label] = scored(
                                engine::play(&core, &decks, policies, names, cards, PlayOptions { seed, limit, trace, life_points, record }),
                                seat,
                            );
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
    } else {
        report::ranking(
            &results,
            &metadata,
            &output.with_extension("summary.json"),
            markdown.as_deref(),
        )
    }
}

/// The candidate's deck is identical to its opponent's. Names and seeds identify
/// the deck, independently of the selected roster, worker count or search budget.
fn generic_jobs(pilots: &[String], games: usize, seed: u64) -> Vec<Job> {
    let mut pilots = pilots.to_vec();
    pilots.sort();
    pilots.dedup();
    pilots
        .into_iter()
        .flat_map(|name| {
            let salt = fingerprint(format!("generic-mirror\0{name}").as_bytes());
            (0..games).map(move |i| Job {
                a: "generic".into(),
                b: name.clone(),
                seed: seed.wrapping_add(salt).wrapping_add(i as u64) & 0x7fffffff,
                seat: i % 2,
            })
        })
        .collect()
}

fn generic_wilson(wins: usize, games: usize) -> Option<[f64; 2]> {
    if games == 0 {
        return None;
    }
    let n = games as f64;
    let p = wins as f64 / n;
    let z2 = 1.96_f64.powi(2);
    let center = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
    let half = 1.96 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / (1.0 + z2 / n);
    Some([(center - half).max(0.0), (center + half).min(1.0)])
}

fn generic_latency(samples: &mut [f64]) -> Value {
    if samples.is_empty() {
        return Value::Null;
    }
    samples.sort_by(f64::total_cmp);
    let quantile = |p: f64| {
        samples[((samples.len() as f64 * p).ceil() as usize)
            .saturating_sub(1)
            .min(samples.len() - 1)]
    };
    json!({"samples": samples.len(), "mean": samples.iter().sum::<f64>() / samples.len() as f64,
        "p50": quantile(0.50), "p95": quantile(0.95), "p99": quantile(0.99),
        "max": samples[samples.len() - 1], "total": samples.iter().sum::<f64>()})
}

/// Errors and capped duels are visible separately and can never qualify a deck.
/// Raw win rate counts a draw as zero wins. Its interval is per matchup, not a
/// simultaneous confidence guarantee over every deck in the roster.
fn generic_stats(rows: &[&Value], expected: usize) -> Value {
    let mut wins = 0usize;
    let mut losses = 0usize;
    let mut draws = 0usize;
    let mut failures = 0usize;
    let mut caps = 0usize;
    let mut failed_playouts = 0u64;
    let mut playouts = 0u64;
    let mut search_counts: BTreeMap<&str, u64> = ["searched", "failed_searches", "budget_exhausted",
        "incomplete_worlds", "truncated_playouts", "in_chain", "facing_set",
        "inner_playouts", "inner_searches", "unresolved_playouts", "unsafe_selections", "soft_budget_overruns", "proposal_nodes", "proposal_leaves", "proposal_improvements", "proposal_completed_prefixes", "proposal_responses", "plan_evaluations", "plan_applied", "plan_mismatches", "live_plan_applied", "live_plan_mismatches", "mcts_simulations", "mcts_completed", "mcts_nodes", "mcts_steps", "mcts_tree_hits", "mcts_opponent_nodes", "mcts_opponent_actions", "mcts_opponent_tree_hits", "mcts_cutoffs", "mcts_live_hits", "mcts_live_missing", "mcts_frozen_actions", "mcts_accepted", "mcts_root_actions", "mcts_visited_root_actions"]
        .into_iter().map(|key| (key, 0)).collect();
    let mut samples = Vec::new();
    let mut search_samples = Vec::new();
    let mut seconds = 0.0;
    let mut seats = [0usize; 2];
    for row in rows {
        let game = &row["game"];
        let seat = row["seat"].as_u64().unwrap_or(2);
        if seat < 2 {
            seats[seat as usize] += 1;
        }
        seconds += row["seconds"].as_f64().unwrap_or(0.0);
        failed_playouts += game["failed_playouts"].as_u64().unwrap_or(0);
        playouts += game["playouts"].as_u64().unwrap_or(0);
        for (key, count) in &mut search_counts {
            *count += game[*key].as_u64().unwrap_or(0);
        }
        if let Some(values) = game["decision_ms"].as_array() {
            samples.extend(
                values
                    .iter()
                    .filter_map(Value::as_f64)
                    .filter(|v| v.is_finite() && *v >= 0.0),
            );
        }
        if let Some(values) = game["search_decision_ms"].as_array() {
            search_samples.extend(values.iter().filter_map(Value::as_f64).filter(|v| v.is_finite() && *v >= 0.0));
        }
        if game["error"].is_string() || seat > 1 {
            failures += 1;
        } else if game["limit"] == true {
            caps += 1;
        } else {
            match game["winner"].as_u64() {
                Some(winner) if winner == seat => wins += 1,
                Some(winner) if winner < 2 => losses += 1,
                Some(_) => draws += 1,
                None => failures += 1,
            }
        }
    }
    let completed = wins + losses + draws;
    let rate = (completed > 0).then(|| wins as f64 / completed as f64);
    let interval = generic_wilson(wins, completed);
    let complete = rows.len() == expected && failures == 0 && caps == 0
        && failed_playouts == 0 && search_counts["failed_searches"] == 0;
    let mut summary = json!({"attempted": rows.len(), "planned": expected, "completed": completed,
        "wins": wins, "losses": losses, "draws": draws, "failures": failures, "caps": caps,
        "win_rate": rate, "wilson_95": interval, "target": 0.55,
        "meets_observed_target": complete && rate.is_some_and(|p| p >= 0.55),
        "lower_95_meets_target": complete && interval.is_some_and(|ci| ci[0] >= 0.55),
        "complete": complete, "candidate_seats": seats, "playouts": playouts,
        "failed_playouts": failed_playouts, "failed_searches": search_counts["failed_searches"],
        "searched": search_counts["searched"], "budget_exhausted": search_counts["budget_exhausted"],
        "incomplete_worlds": search_counts["incomplete_worlds"], "truncated_playouts": search_counts["truncated_playouts"],
        "in_chain": search_counts["in_chain"], "facing_set": search_counts["facing_set"],
        "inner_playouts": search_counts["inner_playouts"], "inner_searches": search_counts["inner_searches"],
        "unresolved_playouts": search_counts["unresolved_playouts"], "unsafe_selections": search_counts["unsafe_selections"],
        "decision_latency_ms": generic_latency(&mut samples),
        "search_latency_ms": generic_latency(&mut search_samples),
        "soft_budget_overruns": search_counts["soft_budget_overruns"],
        "duel_seconds_sum": seconds,
        "mean_duel_seconds": (!rows.is_empty()).then(|| seconds / rows.len() as f64)});
    for (key, value) in search_counts { summary[key] = json!(value); }
    summary
}

fn generic_validate_schedule(stats: &mut Value, rows: &[&Value], expected: &[Job]) {
    let expected: BTreeSet<_> = expected.iter().map(|j| (j.b.as_str(), j.seed, j.seat as u64)).collect();
    let mut seen = BTreeSet::new();
    let mut duplicates = 0usize;
    let mut unexpected = 0usize;
    for row in rows {
        let key = match (row["opponent"].as_str(), row["seed"].as_u64(), row["seat"].as_u64()) {
            (Some(opponent), Some(seed), Some(seat)) => (opponent, seed, seat),
            _ => { unexpected += 1; continue; }
        };
        if !expected.contains(&key) { unexpected += 1; }
        if !seen.insert(key) { duplicates += 1; }
    }
    let missing = expected.difference(&seen).count();
    stats["schedule"] = json!({"missing": missing, "duplicates": duplicates, "unexpected": unexpected});
    if missing != 0 || duplicates != 0 || unexpected != 0 {
        stats["complete"] = json!(false);
        stats["meets_observed_target"] = json!(false);
        stats["lower_95_meets_target"] = json!(false);
    }
}

fn generic_summary(
    rows: &[Value],
    pilots: &[String],
    games: usize,
    seed: u64,
    exceptions: &BTreeSet<String>,
    elapsed: f64,
) -> Value {
    let all: Vec<_> = rows.iter().collect();
    let mut matchups = Vec::new();
    for pilot in pilots {
        let selected: Vec<_> = rows
            .iter()
            .filter(|row| row["opponent"] == pilot.as_str())
            .collect();
        let mut stats = generic_stats(&selected, games);
        generic_validate_schedule(&mut stats, &selected, &generic_jobs(std::slice::from_ref(pilot), games, seed));
        stats["opponent"] = json!(pilot);
        stats["exception"] = json!(exceptions.contains(pilot));
        matchups.push(stats);
    }
    let mut overall = generic_stats(&all, pilots.len() * games);
    generic_validate_schedule(&mut overall, &all, &generic_jobs(pilots, games, seed));
    let required: Vec<_> = matchups.iter().filter(|m| m["exception"] != true).collect();
    let all_meet =
        overall["complete"] == true && !required.is_empty() && required.iter().all(|m| m["meets_observed_target"] == true);
    let failed: Vec<_> = required
        .iter()
        .filter(|m| m["meets_observed_target"] != true)
        .map(|m| m["opponent"].clone())
        .collect();
    json!({"mode": "generic", "candidate_policy": "generic", "target": 0.55,
        "scoring": "raw wins / completed games; draws are not wins; caps and errors prevent qualification",
        "interval_scope": "per-matchup Wilson 95%; not simultaneous across the roster",
        "exceptions": exceptions, "required_matchups": required.len(),
        "all_required_meet_observed_target": all_meet, "below_target_or_incomplete": failed,
        "wall_seconds": elapsed, "overall": overall, "matchups": matchups})
}

fn generic_run(
    args: &BTreeMap<String, String>,
    root: &Path,
    cards: &Path,
    scripts: &Path,
    decks: &Path,
    core_path: Option<&Path>,
    output: &Path,
) -> Result<()> {
    for key in args.keys() {
        if ![
            "--core",
            "--cards",
            "--scripts",
            "--decks",
            "--baseline",
            "--library",
            "--output",
            "--policies",
            "--games",
            "--seed",
            "--workers",
            "--limit",
            "--trace",
            "--record",
            "--plain",
            "--worlds",
            "--max-candidates",
            "--max-playouts",
            "--horizon",
            "--depth",
            "--millis",
            "--strict",
            "--exceptions",
            "--effects",
            "--method",
            "--beam-width",
            "--beam-actions",
            "--beam-nodes",
            "--beam-decisions",
            "--beam-replies",
            "--mcts-iterations",
            "--mcts-actions",
            "--mcts-nodes",
            "--mcts-decisions",
            "--mcts-replies",
            "--mcts-terminal",
            "--mcts-worlds",
            "--mcts-validate-roots",
            "--mcts-adversarial-opponent",
            "--opponent-model",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("{key} is not supported by generic mode"));
        }
    }
    let number = |key: &str, default: usize| -> Result<usize> {
        args.get(key).map_or(Ok(default), |v| {
            v.parse().map_err(|_| format!("Invalid {key}"))
        })
    };
    let flag = |key: &str, default: bool| -> Result<bool> {
        match args.get(key).map(String::as_str) {
            None => Ok(default),
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            _ => Err(format!("{key} must be true or false")),
        }
    };
    let baseline = args
        .get("--baseline")
        .map(PathBuf::from)
        .ok_or("generic requires --baseline FROZEN_LIBRARY")?;
    let candidate = args.get("--library").map(PathBuf::from).unwrap_or_else(|| {
        root.join(format!(
            "target/release/{}ygo_policies{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        ))
    });
    let catalog = PolicyLibrary::open(&baseline)?.catalog;
    let pilots = names(
        args.get("--policies").map(String::as_str).unwrap_or("all"),
        &catalog,
    )?;
    let games = number("--games", 64)?;
    let workers = number("--workers", 4)?;
    let limit = number("--limit", 4096)?;
    let seed = number("--seed", 2400000)? as u64;
    let plain = flag("--plain", false)?;
    let effects = flag("--effects", true)?;
    let strict = flag("--strict", true)?;
    if !strict {
        return Err("generic requires --strict true: face-down monsters cannot be resampled safely".into());
    }
    let trace = flag("--trace", false)?;
    let record = flag("--record", false)?;
    let worlds = number("--worlds", 4)?;
    let max_candidates = number("--max-candidates", 6)?;
    let method = args.get("--method").map(String::as_str).unwrap_or("rollout");
    if !["rollout", "beam", "mcts"].contains(&method) { return Err("--method must be rollout, beam or mcts".into()); }
    let planner = if method == "beam" {
        Some(engine::PlannerOptions { width: number("--beam-width", 8)?,
            actions: number("--beam-actions", 6)?, nodes: number("--beam-nodes", 128)?,
            decisions: number("--beam-decisions", 256)?, replies: number("--beam-replies", 1)? })
    } else { None };
    if planner.is_some_and(|p| p.width == 0 || p.actions == 0 || p.nodes == 0 || p.decisions == 0) {
        return Err("beam width/actions/nodes/decisions must be positive".into());
    }
    let mcts_worlds = number("--mcts-worlds", 4)?;
    let mcts_validate_roots = flag("--mcts-validate-roots", false)?;
    let adversarial_opponent = flag("--mcts-adversarial-opponent", false)?;
    let opponent_model = args.get("--opponent-model").map(String::as_str).unwrap_or("generic");
    let specialist_opponent = match opponent_model {
        "generic" => false,
        "specialist" => true,
        _ => return Err("--opponent-model must be generic or specialist".into()),
    };
    if adversarial_opponent && (method != "mcts" || specialist_opponent || plain) {
        return Err("--mcts-adversarial-opponent true requires MCTS search with the generic opponent model".into());
    }
    if specialist_opponent && plain {
        return Err("--opponent-model specialist requires search".into());
    }
    let mcts = if method == "mcts" {
        Some(engine::MctsOptions { iterations: number("--mcts-iterations", 128)?,
            actions: number("--mcts-actions", 6)?, nodes: number("--mcts-nodes", 1024)?,
            decisions: number("--mcts-decisions", 512)?, replies: number("--mcts-replies", 1)?,
            terminal: flag("--mcts-terminal", false)?, adversarial_opponent })
    } else { None };
    if mcts.is_some_and(|p| p.iterations == 0 || p.actions == 0 || p.nodes == 0 || p.decisions == 0)
        || mcts.is_some() && mcts_worlds == 0 {
        return Err("MCTS iterations/actions/nodes/decisions/worlds must be positive".into());
    }
    let max_playouts = if plain {
        0
    } else {
        number("--max-playouts", if planner.is_some() { 1024 } else if mcts.is_some() { 256 } else { 24 })?
    };
    let horizon = number("--horizon", 0)?;
    let depth = number("--depth", 1)?;
    if (planner.is_some() || mcts.is_some()) && depth != 1 { return Err("--depth applies only to rollout search".into()); }
    if !(1..=2).contains(&depth) {
        return Err("generic --depth must be 1 or 2".into());
    }
    let millis = number("--millis", 200)? as u64;
    if games == 0 || games % 2 != 0 || workers == 0 || limit == 0 || pilots.is_empty() {
        return Err(
            "generic requires positive even games, positive workers/limit and at least one policy"
                .into(),
        );
    }
    if !plain && (worlds == 0 || max_candidates < 2 || max_playouts == 0) {
        return Err(
            "generic search requires positive worlds/playouts and at least two candidates".into(),
        );
    }
    let exceptions: BTreeSet<_> = args
        .get("--exceptions")
        .map(String::as_str)
        .unwrap_or("countdown")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    for exception in &exceptions {
        if !catalog.contains_key(exception) {
            return Err(format!("Unknown exception policy: {exception}"));
        }
    }
    let candidate_library = PolicyLibrary::open(&candidate)?;
    // Generic deliberately has no default deck/catalog entry: creation is separate
    // from deck choice and the exact same policy handles every selected deck.
    candidate_library.seat("generic", cards, 0, seed)?;
    drop(candidate_library);
    let mut deck_map = BTreeMap::new();
    let mut deck_ids = BTreeMap::new();
    for name in &pilots {
        let file = decks.join(format!("{}.ydk", catalog[name]));
        deck_map.insert(name.clone(), Deck::load(&file)?);
        deck_ids.insert(name.clone(), file_identity(&file));
    }
    let jobs = generic_jobs(&pilots, games, seed);
    let metadata = json!({"mode": "generic", "arguments": args, "candidate_policy": "generic",
        "matchup": "same-deck generic versus frozen specialist; seats alternate",
        "games_per_matchup": games, "planned_jobs": jobs.len(), "seed": seed, "workers": workers,
        "shuffle": "SplitMix64 Fisher-Yates v1; stable generic-mirror/deck-name seed",
        "policy_seed": "engine seed + seat", "rules": "MasterRule1 (0xD0700)",
        "decision_limit": limit, "life_points": [8000,8000], "target": 0.55, "exceptions": exceptions,
        "plain": plain, "effect_profiles": effects, "search": {"opponent_model": opponent_model, "worlds": worlds, "max_candidates": max_candidates,
            "max_playouts": max_playouts, "horizon": horizon, "depth": depth, "millis": millis, "strict": strict,
            "method": method, "mcts": mcts.map(|p| json!({"iterations":p.iterations,"actions":p.actions,"nodes":p.nodes,"decisions":p.decisions,"replies":p.replies,"terminal":p.terminal,"worlds":mcts_worlds,"validate_roots":mcts_validate_roots,"adversarial_opponent":p.adversarial_opponent})), "validation": if horizon == 0 { "terminal" } else { "heuristic" }, "beam": planner.map(|p| json!({"width":p.width,"actions":p.actions,"nodes":p.nodes,"decisions":p.decisions,"replies":p.replies}))},
        "timing_reproducibility": if millis == 0 || plain { "deterministic work cap" } else { "wall-time budget may change choices under machine load" },
        "engine": core_path.map(file_identity).unwrap_or_else(|| json!({"linked": true})),
        "executable": std::env::current_exe().ok().map(|p| file_identity(&p)),
        "cards": file_identity(cards), "scripts": git_revision(scripts),
        "core": git_revision(&root.join("vendor/ocgcore")), "lua": git_revision(&root.join("vendor/ocgcore/lua/src")),
        "checkout": git_revision(root), "baseline": file_identity(&baseline), "candidate": file_identity(&candidate),
        "decks": deck_ids, "catalog": catalog, "policies": pilots});
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|e| format!("{}: {e}", output.display()))?;
    std::fs::write(
        output.with_extension("metadata.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let cursor = std::sync::atomic::AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::channel();
    let started = std::time::Instant::now();
    let mut rows = Vec::new();
    std::thread::scope(|scope| -> Result<()> {
        for _ in 0..workers {
            let tx = tx.clone();
            let (jobs, cursor, baseline, candidate, deck_map) =
                (&jobs, &cursor, &baseline, &candidate, &deck_map);
            scope.spawn(move || {
                let work = || -> Result<()> {
                    let core = Core::open(core_path, cards, scripts)?;
                    let baseline = PolicyLibrary::open(baseline)?;
                    let candidate = PolicyLibrary::open(candidate)?;
                    loop {
                        let index = cursor.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(job) = jobs.get(index) else { break };
                        let seat = job.seat;
                        let specialist = job.b.as_str();
                        let decks = [deck_map[specialist].clone(), deck_map[specialist].clone()];
                        let (libraries, names) = if seat == 0 {
                            ([&candidate, &baseline], ["generic", specialist])
                        } else { ([&baseline, &candidate], [specialist, "generic"]) };
                        let options = PlayOptions { seed: job.seed, limit, trace, life_points: [8000; 2], record };
                        let fast = engine::FastOptions { searcher: seat, worlds, max_candidates, max_playouts, horizon, depth, millis, strict, effects, planner, mcts, mcts_worlds, mcts_validate_roots, specialist_opponent };
                        let game_started = std::time::Instant::now();
                        let game = match engine::play_fast(&core, &decks, libraries, names, cards, options, fast) {
                            Ok(game) => game, Err(error) => json!({"error": error}),
                        };
                        let row = json!({"mode": "generic", "opponent": specialist, "candidate_policy": "generic",
                            "deck": specialist, "seed": job.seed, "seat": seat, "plain": plain,
                            "seconds": game_started.elapsed().as_secs_f64(), "game": game});
                        tx.send(Ok(row)).map_err(|e| e.to_string())?;
                    }
                    Ok(())
                };
                if let Err(error) = work() { let _ = tx.send(Err(error)); }
            });
        }
        drop(tx);
        for row in rx {
            let row = row?;
            writeln!(file, "{row}").map_err(|e| e.to_string())?;
            rows.push(row);
            if rows.len() % games.max(64) == 0 || rows.len() == jobs.len() {
                let wins = rows
                    .iter()
                    .filter(|r| r["game"]["winner"] == r["seat"] && r["game"]["limit"] != true)
                    .count();
                eprintln!(
                    "{}/{} generic games, {:.1}% wins across attempted games, {:.1}s",
                    rows.len(),
                    jobs.len(),
                    100.0 * wins as f64 / rows.len() as f64,
                    started.elapsed().as_secs_f64()
                );
            }
        }
        Ok(())
    })?;
    let summary = generic_summary(
        &rows,
        &pilots,
        games,
        seed,
        &exceptions,
        started.elapsed().as_secs_f64(),
    );
    std::fs::write(
        output.with_extension("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"overall": summary["overall"],
        "all_required_meet_observed_target": summary["all_required_meet_observed_target"],
        "below_target_or_incomplete": summary["below_target_or_incomplete"]}))
        .unwrap()
    );
    if summary["overall"]["complete"] != true {
        return Err(
            "Generic benchmark incomplete or contains errors, caps or failed searches/playouts; see summary"
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_mirrors_are_balanced_unique_and_subset_stable() {
        let all = generic_jobs(&["b".into(), "a".into(), "a".into()], 8, 42);
        assert_eq!(all.len(), 16);
        let subset = generic_jobs(&["b".into()], 8, 42);
        assert_eq!(
            subset,
            all.iter()
                .filter(|j| j.b == "b")
                .cloned()
                .collect::<Vec<_>>()
        );
        assert_eq!(subset.iter().filter(|j| j.seat == 0).count(), 4);
        assert_eq!(
            all.iter()
                .map(|j| (&j.b, j.seed, j.seat))
                .collect::<BTreeSet<_>>()
                .len(),
            16
        );
        assert_ne!(subset, generic_jobs(&["b".into()], 8, 43));
    }

    #[test]
    fn generic_rates_count_draws_as_zero_and_never_qualify_errors_or_caps() {
        let win = json!({"seat": 0, "game": {"winner": 0, "limit": false}});
        let loss = json!({"seat": 0, "game": {"winner": 1, "limit": false}});
        let draw = json!({"seat": 0, "game": {"winner": 2, "limit": false}});
        let error = json!({"seat": 0, "game": {"error": "rejected response"}});
        let cap = json!({"seat": 0, "game": {"winner": null, "limit": true}});
        let stats = generic_stats(&[&win, &loss, &draw, &error, &cap], 5);
        assert_eq!(stats["win_rate"], json!(1.0 / 3.0));
        assert_eq!(stats["completed"], 3);
        assert_eq!(stats["failures"], 1);
        assert_eq!(stats["caps"], 1);
        assert_eq!(stats["meets_observed_target"], false);
        let failed = generic_stats(&[&win, &error], 2);
        assert_eq!(failed["win_rate"], 1.0);
        assert_eq!(failed["meets_observed_target"], false);
        let capped = generic_stats(&[&win, &cap], 2);
        assert_eq!(capped["meets_observed_target"], false);
        let incomplete = generic_stats(&[&win], 2);
        assert_eq!(incomplete["meets_observed_target"], false);
    }

    #[test]
    fn generic_search_failures_disqualify_and_coverage_counts_are_preserved() {
        let failed = json!({"seat": 0, "game": {"winner": 0, "limit": false,
            "failed_playouts": 0, "failed_searches": 1, "searched": 3,
            "budget_exhausted": 2, "incomplete_worlds": 1, "truncated_playouts": 4,
            "inner_playouts": 5, "inner_searches": 2, "unresolved_playouts": 6, "unsafe_selections": 7,
            "soft_budget_overruns": 1, "decision_ms": [0.01, 0.01, 0.01], "search_decision_ms": [5.0, 30.0]}});
        let stats = generic_stats(&[&failed], 1);
        assert_eq!(stats["win_rate"], 1.0);
        assert_eq!(stats["complete"], false);
        assert_eq!(stats["meets_observed_target"], false);
        assert_eq!(stats["lower_95_meets_target"], false);
        assert_eq!(stats["failed_searches"], 1);
        assert_eq!(stats["searched"], 3);
        assert_eq!(stats["budget_exhausted"], 2);
        assert_eq!(stats["incomplete_worlds"], 1);
        assert_eq!(stats["truncated_playouts"], 4);
        assert_eq!(stats["inner_playouts"], 5);
        assert_eq!(stats["inner_searches"], 2);
        assert_eq!(stats["unresolved_playouts"], 6);
        assert_eq!(stats["unsafe_selections"], 7);
        assert_eq!(stats["decision_latency_ms"]["p95"], 0.01);
        assert_eq!(stats["search_latency_ms"]["p95"], 30.0);
        assert_eq!(stats["soft_budget_overruns"], 1);
    }

    #[test]
    fn generic_wilson_distinguishes_observation_from_confident_threshold() {
        assert_eq!(generic_wilson(0, 0), None);
        let interval = generic_wilson(55, 100).unwrap();
        assert!(interval[0] < 0.55 && interval[1] > 0.55);
        let all_wins = generic_wilson(100, 100).unwrap();
        assert!(all_wins[0] > 0.96 && all_wins[1] <= 1.0);
        let losses = generic_wilson(0, 100).unwrap();
        assert!(losses[0] >= 0.0 && losses[1] < 0.04);
    }

    #[test]
    fn generic_summary_preserves_exception_results_and_checks_each_deck() {
        let jobs = generic_jobs(&["a".into(), "b".into()], 1, 42);
        let rows: Vec<_> = jobs.iter().map(|j| json!({"opponent": j.b, "seed": j.seed, "seat": j.seat,
            "game": {"winner": if j.b == "a" { 0 } else { 1 }, "limit": false}})).collect();
        let summary = generic_summary(
            &rows,
            &["a".into(), "b".into()],
            1,
            42,
            &["b".to_owned()].into_iter().collect(),
            1.0,
        );
        assert_eq!(summary["all_required_meet_observed_target"], true);
        assert_eq!(summary["matchups"].as_array().unwrap().len(), 2);
        let strict = generic_summary(&rows, &["a".into(), "b".into()], 1, 42, &BTreeSet::new(), 1.0);
        assert_eq!(strict["all_required_meet_observed_target"], false);
        assert_eq!(strict["below_target_or_incomplete"], json!(["b"]));
    }

    #[test]
    fn generic_summary_rejects_duplicated_or_wrong_seed_records() {
        let pilots = vec!["a".into()];
        let jobs = generic_jobs(&pilots, 2, 42);
        let row = |j: &Job| json!({"opponent": j.b, "seed": j.seed, "seat": j.seat,
            "game": {"winner": j.seat, "limit": false}});
        let valid = vec![row(&jobs[0]), row(&jobs[1])];
        assert_eq!(generic_summary(&valid, &pilots, 2, 42, &BTreeSet::new(), 1.0)["overall"]["complete"], true);
        let duplicate = vec![row(&jobs[0]), row(&jobs[0])];
        let rejected = generic_summary(&duplicate, &pilots, 2, 42, &BTreeSet::new(), 1.0);
        assert_eq!(rejected["overall"]["complete"], false);
        assert_eq!(rejected["overall"]["schedule"]["duplicates"], 1);
        assert_eq!(rejected["overall"]["schedule"]["missing"], 1);
        assert_eq!(rejected["all_required_meet_observed_target"], false);
        let wrong_seed = generic_summary(&valid, &pilots, 2, 43, &BTreeSet::new(), 1.0);
        assert_eq!(wrong_seed["overall"]["complete"], false);
    }

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
}
