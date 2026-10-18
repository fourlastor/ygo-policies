//! Real-script checks of the static metadata bridge, independent of deck IDs.
use std::path::PathBuf;

use serde_json::Value;
use ygo_policies_duel::{Core, Deck, DuelOptions, Printed};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn engine() -> Core {
    let root = root();
    Core::open(None, &root.join("vendor/BabelCdb/cards.cdb"), &root.join("vendor/CardScripts")).unwrap()
}

fn effects(profiles: &Value, code: u32) -> &[Value] {
    profiles[code.to_string()].as_array().unwrap()
}

fn field(effect: &Value, name: &str) -> u64 {
    effect[name].as_u64().unwrap()
}

#[test]
fn real_scripts_expose_setup_grave_activation_cost_and_target_declarations() {
    let core = engine();
    assert!(core.has_effect_profiles());
    let profiles = core.effect_profiles(&[91351370, 67441435, 86197239, 44095762]).unwrap();
    // Black Whirlwind: activation itself has no category, but the permanent
    // card also registers a summon-triggered search from its Spell/Trap Zone.
    let whirlwind = effects(&profiles, 91351370);
    assert!(whirlwind.iter().any(|e| field(e, "effect_type") & 0x10 != 0 && field(e, "category") == 0));
    assert!(whirlwind.iter().any(|e| {
        field(e, "event") == 1100 && field(e, "range") == 0x08
            && field(e, "effect_type") & 0x80 != 0 && field(e, "category") & 0x08 != 0
    }));

    // Bulb can activate from the Graveyard. Mirage has a passive restriction
    // there, while its actual revive activation is on the field and costs it.
    let bulb = effects(&profiles, 67441435);
    assert!(bulb.iter().any(|e| {
        field(e, "range") == 0x10 && field(e, "effect_type") & 0x40 != 0
            && field(e, "category") & 0x200 != 0
    }));
    let mirage = effects(&profiles, 86197239);
    assert!(mirage.iter().any(|e| field(e, "range") == 0x10 && field(e, "effect_type") & 0x01 != 0));
    assert!(!mirage.iter().any(|e| field(e, "range") & 0x10 != 0 && field(e, "effect_type") & 0x40 != 0));
    assert!(mirage.iter().any(|e| {
        field(e, "range") == 0x04 && field(e, "category") & 0x200 != 0
            && e["has_cost"] == true && e["card_target"] == true
    }));

    // Mirror Force destroys without selecting card targets or paying a cost.
    assert!(effects(&profiles, 44095762).iter().any(|e| {
        field(e, "category") & 1 != 0 && e["card_target"] == false && e["has_cost"] == false
    }));
}

#[test]
fn profiles_are_order_independent_cached_and_do_not_change_a_live_duel() {
    let mut core = engine();
    let first = core.effect_profiles(&[91351370, 67441435, 91351370]).unwrap();
    let other = engine();
    let second = other.effect_profiles(&[67441435, 91351370]).unwrap();
    assert_eq!(first, second);
    assert_eq!(first, core.effect_profiles(&[67441435, 91351370]).unwrap());

    let deck = Deck::load(&root().join("decks/Blackwing Assassin.ydk")).unwrap();
    let options = DuelOptions::seeded(153);
    let mut live = core.deal(&options, &[deck.clone(), deck.clone()]).unwrap();
    let mut control = other.deal(&options, &[deck.clone(), deck]).unwrap();
    let before = live.field();
    core.effect_profiles(&[86197239, 44095762]).unwrap();
    assert_eq!(live.field(), before);
    assert_eq!(live.step().unwrap().messages, control.step().unwrap().messages);
    drop(live);

    // Arbitrary unfamiliar normal monsters need no script or catalog entry.
    const UNSEEN: u32 = 990000001;
    core.define(UNSEEN, Printed { kind: 0x11, level: 4, attack: 1700, ..Printed::default() });
    assert_eq!(core.effect_profiles(&[UNSEEN]).unwrap()[UNSEEN.to_string()], serde_json::json!([]));
    assert!(core.effect_profiles(&[0]).is_err());
}
