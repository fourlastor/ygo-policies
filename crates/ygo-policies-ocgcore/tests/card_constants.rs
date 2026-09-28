//! Every card constant in the policies must be a canonical passcode.
//!
//! Policies compare `Ctx::canonical(code)` against their constants, and
//! `canonical` folds alternate artworks onto their original passcode.  A
//! constant spelled with an alternate artwork's passcode therefore never
//! matches, and the rule using it silently never fires.
//!
//! Needs a `cards.cdb`: set `YGO_CARDS_CDB` (skipped otherwise).

use std::path::Path;

use ygo_policies::cards::CardDatabase;
use ygo_policies_ocgcore::SqliteCards;

/// `Ctx::canonical`, on the raw database.
fn canonical(db: &SqliteCards, code: u32) -> u32 {
    match db.card(code) {
        Some(card) if card.alias != 0 && (code as i64 - card.alias as i64).abs() < 20 => card.alias,
        _ => code,
    }
}

#[test]
fn card_constants_are_canonical() {
    let Some(path) = std::env::var_os("YGO_CARDS_CDB") else {
        eprintln!("YGO_CARDS_CDB is not set: skipping");
        return;
    };
    let db = SqliteCards::open(path).unwrap();
    let sources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ygo-policies/src");
    let mut files: Vec<_> = std::fs::read_dir(sources.join("decks")).unwrap().map(|e| e.unwrap().path()).collect();
    files.push(sources.join("staples.rs"));
    let mut wrong = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for line in text.lines() {
            // `const NAME: u32 = 12345;`
            let Some(rest) = line.trim().strip_prefix("const ").or_else(|| line.trim().strip_prefix("pub const ")) else { continue };
            let Some((name, value)) = rest.split_once(": u32 = ") else { continue };
            let Ok(code) = value.trim_end_matches(';').parse::<u32>() else { continue };
            if db.card(code).is_none() {
                continue; // not a passcode (a hint id, a count...)
            }
            let base = canonical(&db, code);
            if base != code {
                wrong.push(format!("{}: {name} = {code} should be {base}", file.file_name().unwrap().to_string_lossy()));
            }
        }
    }
    assert!(wrong.is_empty(), "alternate-artwork passcodes never match:\n{}", wrong.join("\n"));
}
