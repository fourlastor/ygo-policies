//! Reproducible matchups, round robins, and paired policy comparisons.
mod engine;
mod report;
use engine::{Core, Deck, PlayOptions, PolicyLibrary, Result};
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

fn jobs(pilots: &[String], opponents: &[String], mode: &str, games: usize, seed: u64) -> Vec<Job> {
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
            (0..games).map(move |i| Job {
                a: a.clone(),
                b: b.clone(),
                seed: seed.wrapping_add(salt).wrapping_add(i as u64) & 0x7fffffff,
                seat: i % 2,
            })
        })
        .collect()
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
        "policy-bench <matchup|round-robin|compare|rank> [OPTIONS]\n\
  matchup --policies arcana --opponents blackwing,monarch\n\
  round-robin --policies all --games 256 --markdown DECK-TIER-LIST.md\n\
  compare --policies arcana --opponents existing --baseline old.so --candidate new.so\n\
  rank --input tournament.jsonl --markdown DECK-TIER-LIST.md\n\
\nOptions: --output results.jsonl --games 64 --seed 730000 --workers 4\n\
  --library PATH (matchup/round-robin; defaults to target/release/libygo_policies.so)\n\
  --core PATH (optional external OCGCore; default builds the pinned vendor engine)\n\
  --cards PATH --scripts DIR --decks DIR (default to this checkout's vendor/decks)\n\
  --reference existing (ranking reference pool; all or comma-separated ids also work)\n\
  --limit 4096 --trace true --markdown PATH\n\
Deck lists and libraries are never modified. Each pair alternates seats.\n\
Compare keeps opponents on the baseline library and runs both versions on identical seeds."
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
    if !["matchup", "round-robin", "compare", "rank"].contains(&mode.as_str()) {
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
    let jobs = jobs(&pilots, &opponents, &mode, games, seed);
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
        "decision_limit": limit, "life_points": 8000, "rules": "MasterRule1 (0xD0700)",
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
    std::thread::scope(|scope| -> Result<()> {
        for _ in 0..workers {
            let tx = tx.clone();
            let (jobs, cursor, cards, scripts, baseline, candidate, core_path, deck_map, mode) = (
                &jobs, &cursor, &cards, &scripts, &baseline, &candidate, &core_path, &deck_map,
                &mode,
            );
            scope.spawn(move || {
                let work = || -> Result<()> {
                    let mut core = Core::open(core_path.as_deref(), cards, scripts)?;
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
                            row[label] = match core.play(
                                &decks,
                                policies,
                                names,
                                cards,
                                PlayOptions { seed, limit, trace },
                            ) {
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
                            };
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
        for result in rx {
            let result = result?;
            writeln!(file, "{result}").map_err(|e| e.to_string())?;
            results.push(result);
            if results.len() % games.max(128) == 0 || results.len() == jobs.len() {
                eprintln!(
                    "{}/{} {}",
                    results.len(),
                    jobs.len(),
                    if mode == "compare" { "pairs" } else { "games" }
                );
            }
        }
        Ok(())
    })?;
    if results.len() != jobs.len() {
        return Err("Incomplete run".into());
    }
    if mode == "compare" {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_robin_is_unique_balanced_and_subset_stable() {
        let names = vec!["a".into(), "b".into(), "c".into()];
        let all = jobs(&names, &names, "round-robin", 4, 7);
        assert_eq!(all.len(), 12);
        let subset = jobs(&names[..1], &names[1..2], "matchup", 4, 7);
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
