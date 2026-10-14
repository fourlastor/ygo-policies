//! Dump initial static card-effect metadata for arbitrary codes or .ydk files.
use std::path::Path;
use ygo_policies_duel::{Core, Deck};

fn main() -> Result<(), String> {
    let mut codes = Vec::new();
    for argument in std::env::args().skip(1) {
        if let Ok(code) = argument.parse::<u32>() {
            codes.push(code);
        } else {
            let deck = Deck::load(Path::new(&argument))?;
            codes.extend(deck.main);
            codes.extend(deck.extra);
        }
    }
    if codes.is_empty() {
        return Err("Usage: effect_profiles <card-code | path.ydk> [...]".into());
    }
    let core = Core::open(None, Path::new("vendor/BabelCdb/cards.cdb"), Path::new("vendor/CardScripts"))?;
    println!("{}", serde_json::to_string_pretty(&core.effect_profiles(&codes)?).map_err(|error| error.to_string())?);
    Ok(())
}
