//! Audit partial card coverage in the catalog's active Main/Extra Deck pool.
//! No match is played, and no private duel state or specialist policy is used.
//!
//! cargo run --release -p ygo-policies-bench --example generic_coverage -- \
//!   --output benchmarks/generic-policy-v2/card-coverage.json
//!
//! Optional: --root DIR --decks DIR --cards FILE --scripts DIR.
//! Repeated --deck FILE replaces the catalog scope with explicit deck files.
//! Existing output files are never overwritten. Without --output, prints JSON.

use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use ygo_policies::cards::effect_types;
use ygo_policies::{generic, knowledge, registry, staples};
use ygo_policies_bench::engine::{Core, Deck};

type Result<T> = std::result::Result<T, String>;

fn fingerprint(bytes: &[u8]) -> String {
    format!(
        "{:016x}",
        bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ *b as u64).wrapping_mul(0x100000001b3)
        })
    )
}

fn identity(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(json!({"path": path, "bytes": bytes.len(), "fnv1a64": fingerprint(&bytes)}))
}

fn revision(path: &Path) -> Option<String> {
    let result = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

fn script_bundle_identity(path: &Path) -> Result<Value> {
    // Match Core::open's filename precedence and include all helper scripts,
    // not merely the card's top-level file (initial_effect can call helpers).
    let mut files = BTreeMap::new();
    for folder in ["", "official", "pre-errata"] {
        let directory = path.join(folder);
        if !directory.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.path().extension().is_some_and(|ext| ext == "lua") {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !files.contains_key(&name) {
                    let bytes = std::fs::read(entry.path()).map_err(|e| e.to_string())?;
                    files.insert(name, (bytes.len(), fingerprint(&bytes)));
                }
            }
        }
    }
    let manifest = serde_json::to_vec(&files).map_err(|e| e.to_string())?;
    Ok(
        json!({"loaded_file_count": files.len(), "sorted_filename_size_hash_manifest_fnv1a64": fingerprint(&manifest)}),
    )
}

fn side_codes(text: &str) -> BTreeSet<u32> {
    let mut side = false;
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            match line {
                "!side" => {
                    side = true;
                    None
                }
                "#main" | "#extra" => {
                    side = false;
                    None
                }
                _ if side => line.parse().ok(),
                _ => None,
            }
        })
        .collect()
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Metadata {
    descriptors: usize,
    actions: usize,
    categorized_actions: usize,
    opaque_actions: usize,
    actions_with_cost_callback: usize,
    category_union: u64,
}

fn metadata(value: &Value) -> Result<Metadata> {
    let descriptors = value
        .as_array()
        .ok_or("initial descriptors are not an array")?;
    let mut stats = Metadata {
        descriptors: descriptors.len(),
        ..Metadata::default()
    };
    for effect in descriptors {
        let kind = effect["effect_type"]
            .as_u64()
            .ok_or("descriptor has no effect_type")?;
        let category = effect["category"]
            .as_u64()
            .ok_or("descriptor has no category")?;
        if kind & effect_types::ACTION as u64 == 0 {
            continue;
        }
        stats.actions += 1;
        stats.category_union |= category;
        stats.categorized_actions += usize::from(category != 0);
        stats.opaque_actions += usize::from(category == 0);
        stats.actions_with_cost_callback +=
            usize::from(effect["has_cost"].as_bool().unwrap_or(false));
    }
    Ok(stats)
}

fn canonical(code: u32, alias: u32) -> u32 {
    // Same alternate-art boundary as Ctx::canonical. Distant name aliases do
    // not acquire another card's rules (e.g. Harpie Lady versus its variants).
    if alias != 0 && code.abs_diff(alias) < 20 {
        alias
    } else {
        code
    }
}

fn compiled_sources() -> Value {
    // Compile-time bytes fingerprint the actual Rust facts linked into this
    // executable, even if a different agent edits the workspace afterwards.
    macro_rules! sources {
        ($($name:literal),* $(,)?) => {{
            let mut result = BTreeMap::new();
            $(result.insert($name, fingerprint(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $name))));)*
            result
        }};
    }
    json!(sources![
        "examples/generic_coverage.rs",
        "../ygo-policies/src/lib.rs",
        "../ygo-policies/src/generic.rs",
        "../ygo-policies/src/agent.rs",
        "../ygo-policies/src/cards.rs",
        "../ygo-policies/src/generic/knowledge.rs",
        "../ygo-policies/src/generic/alternate.rs",
        "../ygo-policies/src/generic/conversion.rs",
        "../ygo-policies/src/generic/requirements.rs",
        "../ygo-policies/src/generic/transactions.rs",
        "../ygo-policies/src/generic/materials.rs",
        "../ygo-policies/src/knowledge.rs",
        "../ygo-policies/src/knowledge/pool.rs",
        "../ygo-policies/src/staples.rs",
        "../ygo-policies-duel/src/core.rs",
        "../ygo-policies-duel/src/duel.rs",
        "../ygo-policies-duel/src/search_bridge.cpp",
        "../ygo-policies-duel/build.rs",
    ])
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut flags = BTreeMap::new();
    let mut explicit_decks = Vec::new();
    while let Some(flag) = args.next() {
        if !matches!(
            flag.as_str(),
            "--root" | "--decks" | "--cards" | "--scripts" | "--deck" | "--output"
        ) {
            return Err(format!("Unknown option {flag}; expected --root, --decks, --cards, --scripts, --deck or --output"));
        }
        let value = args
            .next()
            .ok_or_else(|| format!("Missing value for {flag}"))?;
        if flag == "--deck" {
            explicit_decks.push(PathBuf::from(value));
        } else if flags.insert(flag.clone(), PathBuf::from(value)).is_some() {
            return Err(format!("Repeated {flag}"));
        }
    }
    let root = flags
        .get("--root")
        .cloned()
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let decks_dir = flags
        .get("--decks")
        .cloned()
        .unwrap_or_else(|| root.join("decks"));
    let cards_path = flags
        .get("--cards")
        .cloned()
        .unwrap_or_else(|| root.join("vendor/BabelCdb/cards.cdb"));
    let scripts_path = flags
        .get("--scripts")
        .cloned()
        .unwrap_or_else(|| root.join("vendor/CardScripts"));
    if flags.get("--output").is_some_and(|path| path.exists()) {
        return Err("Output already exists; use a fresh report path".into());
    }
    let catalog_scope = explicit_decks.is_empty();
    let mut decks: Vec<_> = if catalog_scope {
        registry::POLICIES
            .iter()
            .map(|entry| {
                (
                    entry.id.to_owned(),
                    decks_dir.join(format!("{}.ydk", entry.deck)),
                )
            })
            .collect()
    } else {
        explicit_decks
            .into_iter()
            .map(|path| (path.to_string_lossy().into_owned(), path))
            .collect()
    };
    decks.sort();
    let mut active = BTreeSet::new();
    let mut side = BTreeSet::new();
    let mut main = BTreeSet::new();
    let mut extra = BTreeSet::new();
    let mut deck_identities = BTreeMap::new();
    for (name, path) in &decks {
        let deck = Deck::load(path)?;
        main.extend(deck.main.iter().copied());
        extra.extend(deck.extra.iter().copied());
        active.extend(deck.main.iter().chain(&deck.extra).copied());
        side.extend(side_codes(
            &std::fs::read_to_string(path).map_err(|e| e.to_string())?,
        ));
        deck_identities.insert(name, identity(path)?);
    }
    let codes: Vec<_> = active.iter().copied().collect();
    let core = Core::open(None, &cards_path, &scripts_path)?;
    let profiles = core.effect_profiles(&codes)?;
    let db = Connection::open_with_flags(&cards_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    let mut name_query = db
        .prepare("SELECT name FROM texts WHERE id = ?1")
        .map_err(|e| e.to_string())?;
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut overlap_patterns: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows = Vec::with_capacity(codes.len());
    for code in codes {
        let printed = core
            .printed(code)
            .ok_or_else(|| format!("Missing printed card {code}"))?;
        let canonical = canonical(code, printed.alias);
        let name: String = name_query
            .query_row([code], |row| row.get(0))
            .map_err(|e| format!("Card {code} name: {e}"))?;
        let stats = metadata(&profiles[code.to_string()])?;
        let battle = knowledge::facts(canonical) != knowledge::Facts::NONE;
        let probed = knowledge::probed(canonical).is_some();
        let owner_value = knowledge::owner_worth(canonical);
        let staple_value = staples::value(canonical);
        let attack_lock = staples::ATTACK_LOCKS.contains(&canonical);
        let shared_staple_route = staple_value.is_some()
            || matches!(
                canonical,
                staples::SWORDS_OF_REVEALING_LIGHT | staples::BRIONAC | staples::SCRAP_DRAGON
            );
        let generic_facts = generic::knowledge::SUPPORTED_CODES.contains(&canonical);
        let alternate = generic::alternate::SUPPORTED_CODES.contains(&canonical);
        let conversion = generic::CONVERSION_SUPPORTED_CODES.contains(&canonical);
        let requirements = generic::requirements::SUPPORTED_CODES.contains(&canonical);
        let transactions = generic::TRANSACTION_SUPPORTED_CODES.contains(&canonical);
        let materials = generic::MATERIAL_SUPPORTED_CODES.contains(&canonical);
        let explicit =
            generic_facts || alternate || conversion || requirements || transactions || materials;
        let existing = battle
            || owner_value != 0
            || staple_value.is_some()
            || attack_lock
            || shared_staple_route;
        let flags = BTreeMap::from([
            ("initial_descriptors", stats.descriptors > 0),
            ("initial_actions", stats.actions > 0),
            ("initial_categorized_actions", stats.categorized_actions > 0),
            ("initial_zero_category_actions", stats.opaque_actions > 0),
            (
                "initial_only_zero_category_actions",
                stats.actions > 0 && stats.categorized_actions == 0,
            ),
            (
                "initial_mixed_category_actions",
                stats.categorized_actions > 0 && stats.opaque_actions > 0,
            ),
            ("initial_empty_profile", stats.descriptors == 0),
            ("existing_probed_facts", probed),
            ("existing_battle_or_protection_facts", battle),
            ("existing_owner_value", owner_value != 0),
            ("existing_staple_value", staple_value.is_some()),
            ("existing_attack_lock", attack_lock),
            ("existing_generic_staple_route", shared_staple_route),
            ("existing_shared_any", existing),
            ("explicit_generic_facts", generic_facts),
            ("explicit_alternate_win_facts", alternate),
            ("explicit_conversion_facts", conversion),
            ("explicit_requirement_facts", requirements),
            ("explicit_transaction_facts", transactions),
            ("explicit_material_facts", materials),
            ("explicit_new_any", explicit),
            (
                "any_initial_or_shared_fact",
                stats.descriptors > 0 || existing || explicit,
            ),
        ]);
        for (flag, enabled) in &flags {
            *totals.entry((*flag).to_owned()).or_default() += usize::from(*enabled);
        }
        // These disjoint patterns make overlap visible; marginal flag totals
        // above must never be summed as though they represented separate cards.
        let pattern = format!(
            "metadata_action={},existing_shared={},explicit_new={}",
            stats.actions > 0,
            existing,
            explicit
        );
        *overlap_patterns.entry(pattern).or_default() += 1;
        rows.push(json!({
            "code": code, "name": name, "canonical_code": canonical,
            "in_main": main.contains(&code), "in_extra": extra.contains(&code),
            "coverage": flags,
            "initial_metadata": {
                "descriptors": stats.descriptors, "action_descriptors": stats.actions,
                "categorized_action_descriptors": stats.categorized_actions,
                "zero_category_action_descriptors": stats.opaque_actions,
                "actions_with_cost_callback": stats.actions_with_cost_callback,
                "action_category_union_hex": format!("0x{:x}", stats.category_union),
            },
            "existing_owner_value": owner_value, "existing_staple_value": staple_value,
        }));
    }
    let explicit_registry_union: BTreeSet<u32> = [
        generic::knowledge::SUPPORTED_CODES,
        generic::alternate::SUPPORTED_CODES,
        generic::CONVERSION_SUPPORTED_CODES,
        generic::requirements::SUPPORTED_CODES,
        generic::TRANSACTION_SUPPORTED_CODES,
        generic::MATERIAL_SUPPORTED_CODES,
    ]
    .into_iter()
    .flatten()
    .copied()
    .collect();
    let report = json!({
        "schema": 1,
        "scope": {
            "source": if catalog_scope { "compiled policy catalog deck files" } else { "explicit deck files" },
            "deck_count": decks.len(), "active_unique_main_extra": active.len(),
            "main_unique": main.len(), "extra_unique": extra.len(),
            "side_only_unique_excluded": side.difference(&active).count(),
            "side_only_codes_excluded": side.difference(&active).collect::<Vec<_>>(),
        },
        "interpretation": [
            "Coverage flags overlap and mean at least one partial fact, not complete card understanding or measured playing strength.",
            "Explicit requirements, transaction guards and material facts are contextual partial rules; registry membership does not mean that a card's effect is currently legal, active or profitable.",
            "Initial metadata comes only from isolated unstarted card definitions; dynamic conditions, costs, targets, operations and later-created effects are not executed or inferred.",
            "An empty initial profile may describe a normal monster or an opaque/missing script; it is not proof that the card has no useful rules.",
            "Category zero is opaque metadata, not a useless action. Nonzero categories do not establish who benefits or whether a cost is profitable.",
            "Existing shared coverage counts the named public lookups only; it is not an exhaustive census of every conditional branch in tactics or staples.",
            "Unknown cards have no explicit fact entry and retain the generic printed-data, initial-metadata and legal-action fallback.",
            "Passcode rows remain distinct; explicit/shared lookups use the same near-alias canonicalization as the policy.",
        ],
        "provenance": {
            "root": root, "repository_commit": revision(&root),
            "card_database": identity(&cards_path)?,
            "scripts_path": scripts_path, "scripts_commit": revision(&scripts_path),
            "scripts_loaded_bundle": script_bundle_identity(&scripts_path)?,
            "engine_commit": revision(&root.join("vendor/ocgcore")),
            "executable": identity(&std::env::current_exe().map_err(|e| e.to_string())?)?,
            "compiled_source_fnv1a64": compiled_sources(), "decks": deck_identities,
        },
        "coverage_totals_overlapping": totals,
        "coverage_overlap_disjoint_patterns": overlap_patterns,
        "explicit_registries": {
            "generic_knowledge": generic::knowledge::SUPPORTED_CODES,
            "generic_fact_kinds": generic::knowledge::FACT_KINDS,
            "alternate": generic::alternate::SUPPORTED_CODES,
            "conversion": generic::CONVERSION_SUPPORTED_CODES,
            "requirements": generic::requirements::SUPPORTED_CODES,
            "transactions": generic::TRANSACTION_SUPPORTED_CODES,
            "materials": generic::MATERIAL_SUPPORTED_CODES,
        },
        "explicit_registry_codes_outside_active_pool": {
            "generic_knowledge": generic::knowledge::SUPPORTED_CODES.iter().filter(|code| !active.contains(code)).collect::<Vec<_>>(),
            "alternate": generic::alternate::SUPPORTED_CODES.iter().filter(|code| !active.contains(code)).collect::<Vec<_>>(),
            "conversion": generic::CONVERSION_SUPPORTED_CODES.iter().filter(|code| !active.contains(code)).collect::<Vec<_>>(),
            "requirements": generic::requirements::SUPPORTED_CODES.iter().filter(|code| !active.contains(code)).collect::<Vec<_>>(),
            "transactions": generic::TRANSACTION_SUPPORTED_CODES.iter().filter(|code| !active.contains(code)).collect::<Vec<_>>(),
            "materials": generic::MATERIAL_SUPPORTED_CODES.iter().filter(|code| !active.contains(code)).collect::<Vec<_>>(),
        },
        "explicit_registry_union": {
            "unique_codes": explicit_registry_union.len(),
            "codes": explicit_registry_union,
            "codes_outside_active_pool": explicit_registry_union.difference(&active).collect::<Vec<_>>(),
        },
        "cards": rows,
    });
    let mut rendered = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    rendered.push(b'\n');
    if let Some(path) = flags.get("--output") {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        file.write_all(&rendered).map_err(|e| e.to_string())?;
        eprintln!(
            "Wrote {} active cards from {} decks to {}",
            active.len(),
            decks.len(),
            path.display()
        );
    } else {
        std::io::stdout()
            .lock()
            .write_all(&rendered)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_zero_is_distinct_from_no_action_and_passive_categories() {
        let stats = metadata(&json!([
            {"effect_type": effect_types::IGNITION, "category": 0, "has_cost": true},
            {"effect_type": effect_types::ACTIVATE, "category": 0x20},
            {"effect_type": 1, "category": 0x10000},
        ]))
        .unwrap();
        assert_eq!(
            stats,
            Metadata {
                descriptors: 3,
                actions: 2,
                categorized_actions: 1,
                opaque_actions: 1,
                actions_with_cost_callback: 1,
                category_union: 0x20
            }
        );
        assert_eq!(metadata(&json!([])).unwrap(), Metadata::default());
        assert!(metadata(&json!([{"effect_type": effect_types::IGNITION}])).is_err());
    }

    #[test]
    fn side_cards_and_distant_aliases_do_not_expand_active_fact_coverage() {
        assert_eq!(
            side_codes("#main\n1\n#extra\n2\n!side\n3\n#comment\n4\n"),
            BTreeSet::from([3, 4])
        );
        assert_eq!(canonical(83764719, 83764718), 83764718);
        assert_eq!(canonical(76812113, 76812113 + 1000), 76812113);
        assert_eq!(canonical(1, 0), 1);
    }
}
