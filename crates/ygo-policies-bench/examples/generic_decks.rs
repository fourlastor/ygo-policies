//! Seat the same generic policy on two arbitrary .ydk files; no catalog lookup.
//! Usage: cargo run --release -p ygo-policies-bench --example generic_decks -- A.ydk B.ydk [seed]
use std::path::PathBuf;
use ygo_policies_bench::engine::{play_fast, Core, Deck, FastOptions, PlayOptions, PolicyLibrary};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let first = args.next().ok_or("expected first .ydk path")?;
    let second = args.next().ok_or("expected second .ydk path")?;
    let seed = args.next().map(|s| s.parse::<u64>()).transpose().map_err(|e| e.to_string())?.unwrap_or(1);
    if args.next().is_some() { return Err("expected two deck paths and optional seed".into()); }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cards = root.join("vendor/BabelCdb/cards.cdb");
    let core = Core::open(None, &cards, &root.join("vendor/CardScripts"))?;
    let library = PolicyLibrary::open(&root.join(format!(
        "target/release/{}ygo_policies{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX
    )))?;
    let decks = [Deck::load(&PathBuf::from(first))?, Deck::load(&PathBuf::from(second))?];
    let result = play_fast(
        &core, &decks, [&library, &library], ["generic", "generic"], &cards,
        PlayOptions { seed, limit: 8192, trace: false, life_points: [8000; 2], record: false },
        FastOptions { searcher: 0, worlds: 4, max_candidates: 8, max_playouts: 32, horizon: 32, millis: 200, strict: true, depth: 1, effects: true, planner: None, mcts: None, mcts_worlds: 4, mcts_validate_roots: false },
    )?;
    // Seat 0 searches; seat 1 uses the plain generic pilot. Search currently
    // assumes known deck lists, but resamples their unknown arrangement.
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "winner": result["winner"], "turns": result["turns"], "limit": result["limit"],
        "playouts": result["playouts"], "failed_playouts": result["failed_playouts"],
        "latency_ms": result["latency_ms"], "search_information": result["search_information"]
    })).unwrap());
    Ok(())
}
