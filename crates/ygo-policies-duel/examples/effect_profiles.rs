//! Dump initial static card-effect metadata for arbitrary codes or .ydk files,
//! or, with `--pool LFLIST`, for every card an EDOPro lflist allows, in the
//! stored layout of `data/wc2011-effect-profiles.txt`:
//!
//! ```text
//! cargo run --release -p ygo-policies-duel --example effect_profiles -- \
//!     --pool data/wc2011.lflist.conf > data/wc2011-effect-profiles.txt
//! ```
use std::collections::BTreeSet;
use std::path::Path;
use ygo_policies_duel::{Core, Deck};

const CARDS: &str = "vendor/BabelCdb/cards.cdb";

fn main() -> Result<(), String> {
    let core = || Core::open(None, Path::new(CARDS), Path::new("vendor/CardScripts"));
    let mut arguments = std::env::args().skip(1);
    let mut codes = Vec::new();
    while let Some(argument) = arguments.next() {
        if argument == "--pool" {
            let pool = arguments.next().ok_or("--pool takes an lflist")?;
            return stored(&core()?, &pool);
        }
        if let Ok(code) = argument.parse::<u32>() {
            codes.push(code);
        } else {
            let deck = Deck::load(Path::new(&argument))?;
            codes.extend(deck.main);
            codes.extend(deck.extra);
        }
    }
    if codes.is_empty() {
        return Err("Usage: effect_profiles <card-code | path.ydk> [...] | --pool <lflist>".into());
    }
    println!("{}", serde_json::to_string_pretty(&core()?.effect_profiles(&codes)?).map_err(|error| error.to_string())?);
    Ok(())
}

/// Every card `pool` lists, forbidden ones too, and every other print of one
/// (a card whose alias it lists, which a Deck may hold in its place), one
/// line per effect; a card whose script fails is left out, and said so on
/// stderr.
fn stored(core: &Core, pool: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(pool).map_err(|error| format!("{pool}: {error}"))?;
    let mut codes: BTreeSet<u32> = text.lines()
        .filter_map(|line| line.split_whitespace().next()?.parse().ok())
        .collect();
    let db = rusqlite::Connection::open_with_flags(CARDS, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("{CARDS}: {error}"))?;
    let mut prints = db.prepare("select id, alias from datas where alias != 0").map_err(|error| error.to_string())?;
    let prints: Vec<(u32, u32)> = prints.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .and_then(Iterator::collect)
        .map_err(|error| error.to_string())?;
    codes.extend(prints.into_iter().filter(|(_, alias)| codes.contains(alias)).map(|(code, _)| code).collect::<Vec<_>>());
    println!("# Effect profiles of every card {pool} lists and of every other print of one: what each");
    println!("# card's initial_effect registers, read as Core::effect_profiles reads it (an unstarted duel of its own).");
    println!("# A search seat describes its own cards with these, with no script run.");
    println!("# Regenerate when vendor/CardScripts or vendor/BabelCdb moves (the search crate's tests say so):");
    println!("#   cargo run --release -p ygo-policies-duel --example effect_profiles -- --pool {pool} > data/wc2011-effect-profiles.txt");
    println!("# One line per effect: code category effect_type range event description flags has_cost card_target,");
    println!("# bit sets in hex. A card that registers no effect has a line of its code alone.");
    let (mut written, mut left_out) = (0, 0);
    for code in codes {
        let profiles = match core.effect_profiles(&[code]) {
            Ok(profiles) => profiles,
            Err(error) => {
                eprintln!("{code} left out: {error}");
                left_out += 1;
                continue;
            }
        };
        let effects = profiles[code.to_string()].as_array().ok_or(format!("{code}: no profile"))?;
        if effects.is_empty() {
            println!("{code}");
        }
        for effect in effects {
            let number = |field: &str| effect[field].as_u64().ok_or(format!("{code}: no {field}"));
            let flag = |field: &str| effect[field].as_bool().map(u8::from).ok_or(format!("{code}: no {field}"));
            println!(
                "{code} {:#x} {:#x} {:#x} {} {} {:#x} {} {}",
                number("category")?, number("effect_type")?, number("range")?, number("event")?,
                number("description")?, number("flags")?, flag("has_cost")?, flag("card_target")?,
            );
        }
        written += 1;
    }
    eprintln!("{written} cards written, {left_out} left out");
    Ok(())
}
