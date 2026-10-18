//! Conservative remembered identities from spectator-visible events only.
//!
//! This lets search retain an already revealed monster after it turns face
//! down. It never learns from native card queries, hidden hand ordering, or a
//! set-card shuffle's hidden permutation. Unknown monsters still block search.
use std::collections::BTreeMap;

use ygo_policies_ocgcore::{
    message::{self, Message},
    query::Query,
    redact,
    wire::{location, msg, Loc},
};

use crate::Result;

type Slot = (u8, u8, u32);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublicIdentityMemory {
    known: BTreeMap<Slot, u32>,
}

fn slot(at: Loc) -> Slot {
    (at.controller, at.location, at.sequence)
}

fn field_zone(location: u8) -> bool {
    matches!(location, location::MZONE | location::SZONE)
}

fn public_pile(location: u8) -> bool {
    matches!(location, location::GRAVE | location::REMOVED)
}

impl PublicIdentityMemory {
    /// Feed each live engine message once, including refresh messages. The
    /// spectator filter is applied before any card code is inspected.
    pub fn observe(&mut self, raw: &[u8]) -> Result<()> {
        let visible = redact::redact(raw, None).map_err(|error| error.to_string())?;
        let id = *raw.first().ok_or("Empty public-memory message")?;
        // These protocol events can invalidate locations wholesale. Forgetting
        // too much is safe; retaining an obsolete identity is not.
        if matches!(id, msg::REMOVE_CARDS | msg::RELOAD_FIELD | msg::TAG_SWAP) {
            self.known.clear();
            return Ok(());
        }
        if id == msg::SWAP_GRAVE_DECK {
            self.known.retain(|(_, location, _), _| *location != location::GRAVE);
        }
        let Some(visible) = visible else { return Ok(()) };
        let message = message::parse(&visible).map_err(|error| error.to_string())?;
        self.apply(&message);
        Ok(())
    }

    pub fn knows(&self, controller: u8, location: u8, sequence: u32) -> bool {
        self.known.contains_key(&(controller, location, sequence))
    }

    /// Read locations and face-up flags from the policy's redacted observation,
    /// never identities or attributes from the live native monster query.
    pub fn has_unknown_monsters(&self, observation: &serde_json::Value, viewer: u8) -> bool {
        let Some(cards) = observation["cards"].as_array() else { return true };
        cards.iter().any(|card| {
            if card["at"]["location"] != "MonsterZone"
                || card["at"]["controller"].as_u64() == Some(viewer as u64)
                || card["position"]["face_up"] == true
            {
                return false;
            }
            let controller = card["at"]["controller"].as_u64().and_then(|p| u8::try_from(p).ok());
            let sequence = card["at"]["sequence"].as_u64().and_then(|s| u32::try_from(s).ok());
            match (controller, sequence) {
                (Some(controller), Some(sequence)) => !self.knows(controller, location::MZONE, sequence),
                _ => true,
            }
        })
    }

    /// Concealed banished cards cannot be resampled by the native swap API.
    /// Doom Lord and Slip of Fortune can later return them to the field, so
    /// ignoring their current zone does not make a simulated continuation safe.
    /// Deliberately read only public location/controller/position metadata:
    /// even a stale remembered code must not authorize this search root.
    pub fn has_facedown_banished(&self, observation: &serde_json::Value, viewer: u8) -> bool {
        if viewer > 1 { return true; }
        let Some(cards) = observation["cards"].as_array() else { return true };
        cards.iter().any(|card| {
            match card["at"]["location"].as_str() {
                Some("Banished") => {}
                Some("Deck" | "Hand" | "MonsterZone" | "SpellTrapZone" | "Graveyard"
                    | "Extra" | "Overlay" | "Other") => return false,
                _ => return true,
            }
            let controller = card["at"]["controller"].as_u64().filter(|player| *player < 2);
            let face_up = card["position"]["face_up"].as_bool();
            match (controller, face_up) {
                (Some(controller), Some(face_up)) => controller != u64::from(viewer) && !face_up,
                _ => true,
            }
        })
    }

    fn remember(&mut self, at: Loc, code: Option<u32>) {
        if !(field_zone(at.location)
            || at.location == location::GRAVE
            || at.location == location::REMOVED && at.face_up())
        {
            return;
        }
        if let Some(code) = code.filter(|code| *code != 0) {
            self.known.insert(slot(at), code);
        }
    }

    fn take(&mut self, at: Loc) -> Option<u32> {
        let known = self.known.remove(&slot(at));
        if public_pile(at.location) {
            let old = std::mem::take(&mut self.known);
            for ((player, location, sequence), code) in old {
                let sequence = if player == at.controller && location == at.location && sequence > at.sequence {
                    sequence - 1
                } else { sequence };
                self.known.insert((player, location, sequence), code);
            }
        }
        known
    }

    fn insert(&mut self, at: Loc, code: Option<u32>) {
        if public_pile(at.location) {
            let old = std::mem::take(&mut self.known);
            for ((player, location, sequence), known) in old {
                let sequence = if player == at.controller && location == at.location && sequence >= at.sequence {
                    sequence + 1
                } else { sequence };
                self.known.insert((player, location, sequence), known);
            }
        }
        self.known.remove(&slot(at));
        self.remember(at, code);
    }

    fn refresh(&mut self, controller: u8, location: u8, sequence: u32, card: Option<&Query>) {
        let key = (controller, location, sequence);
        match card {
            None => { self.known.remove(&key); }
            Some(card) => {
                let at = Loc { controller, location, sequence, position: card.position.unwrap_or(0) };
                // A masked field update does not undo a prior public reveal.
                // Face-down banished cards are deliberately not tracked.
                if location == location::REMOVED && !at.face_up() {
                    self.known.remove(&key);
                } else {
                    self.remember(at, card.code);
                }
            }
        }
    }

    fn apply(&mut self, message: &Message) {
        match message {
            Message::Start { .. } => self.known.clear(),
            Message::UpdateData { player, location, cards } => {
                self.known.retain(|(p, loc, sequence), _| {
                    *p != *player || *loc != *location || (*sequence as usize) < cards.len()
                });
                for (sequence, card) in cards.iter().enumerate() {
                    self.refresh(*player, *location, sequence as u32, card.as_ref());
                }
            }
            Message::UpdateCard { player, location, sequence, card } => {
                self.refresh(*player, *location, *sequence as u32, card.as_ref());
            }
            Message::Move { code, from, to, .. } => {
                let previous = self.take(*from);
                // Hand/deck identities are intentionally never retained here.
                // A direct move from a known field/GY slot remains trackable.
                let known = (*code != 0).then_some(*code).or(previous);
                if !to.is_none() { self.insert(*to, known); }
            }
            Message::PosChange { code, controller, location, sequence, current, .. } => {
                let at = Loc { controller: *controller, location: *location,
                    sequence: *sequence as u32, position: *current as u32 };
                if *location == location::REMOVED && !at.face_up() {
                    self.known.remove(&slot(at));
                } else {
                    self.remember(at, Some(*code));
                }
            }
            Message::ConfirmCards { cards, .. } => {
                // A confirmation explicitly reveals even a face-down field
                // card. We still ignore hand/deck positions entirely.
                for (code, at) in cards { self.remember(*at, Some(*code)); }
            }
            Message::Summoning { code, loc } | Message::SpecialSummoning { code, loc }
            | Message::FlipSummoning { code, loc } => {
                // Some core notifications contain the code of a face-down
                // Special Summon. Never learn that identity from this message.
                if loc.face_up() { self.remember(*loc, Some(*code)); }
            }
            Message::Swap { first, second } => {
                let a = self.known.remove(&slot(first.1));
                let b = self.known.remove(&slot(second.1));
                self.remember(second.1, (first.0 != 0).then_some(first.0).or(a));
                self.remember(first.1, (second.0 != 0).then_some(second.0).or(b));
            }
            Message::ShuffleSetCard { location, from, to } => {
                // `to` can contain placeholders or reveal only attached-card
                // locations. Do not follow it as a public identity permutation.
                let players: Vec<_> = from.iter().chain(to).map(|at| at.controller).collect();
                self.known.retain(|(player, loc, _), _| {
                    *loc != *location || !players.contains(player)
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ygo_policies_ocgcore::wire::query;

    fn at(player: u8, location: u8, sequence: u32, face_up: bool) -> Loc {
        Loc { controller: player, location, sequence, position: if face_up { 1 } else { 8 } }
    }
    fn loc(bytes: &mut Vec<u8>, at: Loc) {
        bytes.extend([at.controller, at.location]);
        bytes.extend(at.sequence.to_le_bytes());
        bytes.extend(at.position.to_le_bytes());
    }
    fn movement(code: u32, from: Loc, to: Loc) -> Vec<u8> {
        let mut bytes = vec![msg::MOVE];
        bytes.extend(code.to_le_bytes());
        loc(&mut bytes, from);
        loc(&mut bytes, to);
        bytes.extend(0u32.to_le_bytes());
        bytes
    }
    fn change(code: u32, at: Loc, from: u8, to: u8) -> Vec<u8> {
        let mut bytes = vec![msg::POS_CHANGE];
        bytes.extend(code.to_le_bytes());
        bytes.extend([at.controller, at.location, at.sequence as u8, from, to]);
        bytes
    }
    fn update(code: u32, at: Loc) -> Vec<u8> {
        let mut bytes = vec![msg::UPDATE_CARD, at.controller, at.location, at.sequence as u8];
        for (flag, value) in [(query::CODE, code), (query::POSITION, at.position)] {
            bytes.extend(8u16.to_le_bytes());
            bytes.extend(flag.to_le_bytes());
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(4u16.to_le_bytes());
        bytes.extend(query::END.to_le_bytes());
        bytes
    }
    fn observation(sequence: u32) -> serde_json::Value {
        serde_json::json!({"cards":[{"code":null,"at":{"controller":1,"location":"MonsterZone","sequence":sequence},"position":{"face_up":false}}]})
    }

    #[test]
    fn public_monster_remains_known_after_set_and_masked_refresh() {
        let mut memory = PublicIdentityMemory::default();
        let monster = at(1, location::MZONE, 2, true);
        memory.observe(&update(100, monster)).unwrap();
        memory.observe(&change(100, monster, 1, 8)).unwrap();
        memory.observe(&update(100, at(1, location::MZONE, 2, false))).unwrap();
        assert_eq!(memory.known.get(&(1, location::MZONE, 2)), Some(&100));
        assert!(!memory.has_unknown_monsters(&observation(2), 0));
        assert!(memory.has_unknown_monsters(&observation(1), 0));
    }

    #[test]
    fn private_substitution_does_not_change_memory_or_search_eligibility() {
        let hidden = |code| {
            let mut memory = PublicIdentityMemory::default();
            let monster = at(1, location::MZONE, 0, false);
            memory.observe(&movement(code, at(1, location::HAND, 0, false), monster)).unwrap();
            let mut set = vec![msg::SET]; set.extend(code.to_le_bytes()); loc(&mut set, monster);
            memory.observe(&set).unwrap();
            memory.observe(&update(code, monster)).unwrap();
            let mut special = vec![msg::SPSUMMONING]; special.extend(code.to_le_bytes()); loc(&mut special, monster);
            memory.observe(&special).unwrap();
            memory
        };
        let first = hidden(100);
        let second = hidden(999);
        assert_eq!(first, second);
        assert!(first.known.is_empty());
        assert!(first.has_unknown_monsters(&observation(0), 0));
        assert!(second.has_unknown_monsters(&observation(0), 0));
    }

    #[test]
    fn public_grave_movement_tracks_sequence_changes_and_control_changes() {
        let mut memory = PublicIdentityMemory::default();
        for (sequence, code) in [(0, 100), (1, 200), (2, 300)] {
            memory.observe(&update(code, at(1, location::GRAVE, sequence, true))).unwrap();
        }
        memory.observe(&movement(100, at(1, location::GRAVE, 0, true), at(1, location::HAND, 0, false))).unwrap();
        assert_eq!(memory.known.get(&(1, location::GRAVE, 0)), Some(&200));
        assert_eq!(memory.known.get(&(1, location::GRAVE, 1)), Some(&300));
        memory.observe(&movement(200, at(1, location::GRAVE, 0, true), at(1, location::MZONE, 3, false))).unwrap();
        assert_eq!(memory.known.get(&(1, location::MZONE, 3)), Some(&200));
        memory.observe(&movement(200, at(1, location::MZONE, 3, false), at(0, location::MZONE, 1, false))).unwrap();
        assert!(!memory.knows(1, location::MZONE, 3));
        assert_eq!(memory.known.get(&(0, location::MZONE, 1)), Some(&200));
        // Moving through a hidden hand loses the exact-slot identity.
        memory.observe(&movement(100, at(1, location::HAND, 0, false), at(1, location::MZONE, 0, false))).unwrap();
        assert!(!memory.knows(1, location::MZONE, 0));
    }

    #[test]
    fn explicit_reveal_is_remembered_but_set_shuffle_invalidates_it() {
        let mut memory = PublicIdentityMemory::default();
        let mut reveal = vec![msg::CONFIRM_CARDS, 0];
        reveal.extend(1u32.to_le_bytes()); reveal.extend(100u32.to_le_bytes());
        reveal.extend([1, location::MZONE]); reveal.extend(0u32.to_le_bytes());
        memory.observe(&reveal).unwrap();
        assert!(memory.knows(1, location::MZONE, 0));
        memory.observe(&update(100, at(1, location::MZONE, 0, false))).unwrap();
        assert!(memory.knows(1, location::MZONE, 0));
        let mut shuffle = vec![msg::SHUFFLE_SET_CARD, location::MZONE, 1];
        loc(&mut shuffle, at(1, location::MZONE, 0, false));
        loc(&mut shuffle, at(1, location::MZONE, 4, false));
        memory.observe(&shuffle).unwrap();
        assert!(!memory.knows(1, location::MZONE, 0));
        assert!(!memory.knows(1, location::MZONE, 4));
        assert!(memory.has_unknown_monsters(&observation(4), 0));
    }

    #[test]
    fn deletion_empty_queries_and_new_duels_forget_old_slots() {
        let mut memory = PublicIdentityMemory::default();
        let monster = at(1, location::MZONE, 0, true);
        memory.observe(&update(100, monster)).unwrap();
        memory.observe(&movement(100, monster, Loc::default())).unwrap();
        assert!(!memory.knows(1, location::MZONE, 0));
        memory.observe(&update(100, monster)).unwrap();
        memory.observe(&[msg::UPDATE_CARD, 1, location::MZONE, 0, 0, 0]).unwrap();
        assert!(!memory.knows(1, location::MZONE, 0));
        memory.observe(&update(100, monster)).unwrap();
        memory.observe(&[msg::REMOVE_CARDS, 0, 0, 0, 0]).unwrap();
        assert!(memory.known.is_empty());
        memory.observe(&update(100, monster)).unwrap();
        memory.observe(&ygo_policies_ocgcore::message::start_message(0, [8000; 2], [40; 2], [0; 2])).unwrap();
        assert!(memory.known.is_empty());
    }

    #[test]
    fn cross_control_swap_moves_only_the_previously_public_identity() {
        let mut memory = PublicIdentityMemory::default();
        let known = at(1, location::MZONE, 2, false);
        let unknown = at(0, location::MZONE, 4, false);
        memory.observe(&update(100, at(1, location::MZONE, 2, true))).unwrap();
        memory.observe(&change(100, known, 1, 8)).unwrap();
        let swap = |private_code: u32| {
            let mut bytes = vec![msg::SWAP];
            bytes.extend(100u32.to_le_bytes()); loc(&mut bytes, known);
            bytes.extend(private_code.to_le_bytes()); loc(&mut bytes, unknown);
            bytes
        };
        let mut other = memory.clone();
        memory.observe(&swap(200)).unwrap();
        other.observe(&swap(999)).unwrap();
        assert_eq!(memory, other);
        assert!(!memory.knows(1, location::MZONE, 2));
        assert_eq!(memory.known.get(&(0, location::MZONE, 4)), Some(&100));
        assert!(memory.has_unknown_monsters(&observation(2), 0));
        let moved = serde_json::json!({"cards":[{"at":{"controller":0,"location":"MonsterZone","sequence":4},"position":{"face_up":false}}]});
        assert!(!memory.has_unknown_monsters(&moved, 1));
    }

    #[test]
    fn placeholder_shuffle_destinations_forget_every_affected_slot() {
        let mut memory = PublicIdentityMemory::default();
        for sequence in [0, 3] {
            memory.observe(&update(100 + sequence, at(1, location::MZONE, sequence, true))).unwrap();
            memory.observe(&change(100 + sequence, at(1, location::MZONE, sequence, false), 1, 8)).unwrap();
        }
        let mut shuffle = vec![msg::SHUFFLE_SET_CARD, location::MZONE, 2];
        loc(&mut shuffle, at(1, location::MZONE, 0, false));
        loc(&mut shuffle, at(1, location::MZONE, 3, false));
        loc(&mut shuffle, Loc::default());
        loc(&mut shuffle, Loc::default());
        memory.observe(&shuffle).unwrap();
        // Subsequent masked queries and stale codes in a projection cannot
        // reconstruct the permutation that the native core kept private.
        memory.observe(&update(103, at(1, location::MZONE, 0, false))).unwrap();
        memory.observe(&update(100, at(1, location::MZONE, 3, false))).unwrap();
        let mut stale = observation(0);
        stale["cards"][0]["code"] = serde_json::json!(100);
        assert!(memory.has_unknown_monsters(&stale, 0));
        assert!(memory.has_unknown_monsters(&observation(3), 0));
        assert!(memory.known.is_empty());
    }

    #[test]
    fn full_location_refresh_clears_empty_and_missing_slots_before_hidden_reuse() {
        let mut memory = PublicIdentityMemory::default();
        for sequence in 0..3 {
            memory.observe(&update(100 + sequence, at(1, location::MZONE, sequence, true))).unwrap();
        }
        let hidden = update(777, at(1, location::MZONE, 1, false));
        let mut payload = vec![0, 0]; // slot zero is now empty
        payload.extend_from_slice(&hidden[4..]); // slot one is still occupied
        let mut refresh = vec![msg::UPDATE_DATA, 1, location::MZONE];
        refresh.extend((payload.len() as u32).to_le_bytes());
        refresh.extend(payload);
        memory.observe(&refresh).unwrap();
        assert!(!memory.knows(1, location::MZONE, 0));
        assert_eq!(memory.known.get(&(1, location::MZONE, 1)), Some(&101));
        assert!(!memory.knows(1, location::MZONE, 2));
        memory.observe(&movement(999, at(1, location::HAND, 0, false), at(1, location::MZONE, 0, false))).unwrap();
        assert!(memory.has_unknown_monsters(&observation(0), 0));
    }

    #[test]
    fn printed_identity_ignores_alias_and_masked_code_substitutions() {
        let with_alias = |code: u32, alias: u32, face_up: bool| {
            let mut bytes = update(code, at(1, location::MZONE, 0, face_up));
            bytes.truncate(bytes.len() - 6); // replace END with ALIAS, then END
            bytes.extend(8u16.to_le_bytes()); bytes.extend(query::ALIAS.to_le_bytes());
            bytes.extend(alias.to_le_bytes());
            bytes.extend(4u16.to_le_bytes()); bytes.extend(query::END.to_le_bytes());
            bytes
        };
        let mut memory = PublicIdentityMemory::default();
        memory.observe(&with_alias(100, 200, true)).unwrap();
        assert_eq!(memory.known.get(&(1, location::MZONE, 0)), Some(&100));
        memory.observe(&change(100, at(1, location::MZONE, 0, false), 1, 8)).unwrap();
        let mut other = memory.clone();
        memory.observe(&with_alias(777, 888, false)).unwrap();
        other.observe(&with_alias(333, 444, false)).unwrap();
        assert_eq!(memory, other);
        assert_eq!(memory.known.get(&(1, location::MZONE, 0)), Some(&100));
        let mut unknown = PublicIdentityMemory::default();
        unknown.observe(&with_alias(100, 200, false)).unwrap();
        assert!(unknown.has_unknown_monsters(&observation(0), 0));
    }

    #[test]
    fn face_down_banished_cards_do_not_restore_a_later_field_identity() {
        let mut memory = PublicIdentityMemory::default();
        let banished = at(1, location::REMOVED, 0, true);
        memory.observe(&update(100, banished)).unwrap();
        memory.observe(&change(100, banished, 1, 8)).unwrap();
        memory.observe(&movement(100, at(1, location::REMOVED, 0, false), at(1, location::MZONE, 0, false))).unwrap();
        assert!(memory.has_unknown_monsters(&observation(0), 0));
    }

    #[test]
    fn facedown_banished_guard_uses_only_public_metadata_on_either_seat() {
        let memory = PublicIdentityMemory::default();
        for viewer in [0, 1] {
            let mut view = serde_json::json!({"cards":[{
                "at":{"controller":1-viewer,"location":"Banished","sequence":0},
                "position":{"face_up":false,"attack":false},"code":null
            }]});
            for private in [serde_json::Value::Null, serde_json::json!(100), serde_json::json!(999)] {
                view["cards"][0]["code"] = private;
                view["cards"][0]["attack"] = serde_json::json!(9000);
                view["cards"][0]["defense"] = serde_json::json!(12);
                assert!(memory.has_facedown_banished(&view, viewer));
            }
            view["cards"][0]["position"]["face_up"] = serde_json::json!(true);
            assert!(!memory.has_facedown_banished(&view, viewer));
            view["cards"][0]["position"]["face_up"] = serde_json::json!(false);
            view["cards"][0]["at"]["controller"] = serde_json::json!(viewer);
            assert!(!memory.has_facedown_banished(&view, viewer));
            view["cards"][0]["at"]["controller"] = serde_json::json!(1-viewer);
            for location in ["Deck", "Hand", "MonsterZone", "SpellTrapZone", "Graveyard", "Extra", "Overlay", "Other"] {
                view["cards"][0]["at"]["location"] = serde_json::json!(location);
                assert!(!memory.has_facedown_banished(&view, viewer));
            }
        }
        assert!(!memory.has_facedown_banished(&serde_json::json!({"cards":[]}), 0));
    }

    #[test]
    fn facedown_banished_guard_rejects_missing_or_invalid_public_metadata() {
        let memory = PublicIdentityMemory::default();
        for view in [serde_json::Value::Null, serde_json::json!({}),
            serde_json::json!({"cards":null}), serde_json::json!({"cards":[null]}),
            serde_json::json!({"cards":[{"at":{"location":"unknown"}}]})] {
            assert!(memory.has_facedown_banished(&view, 0));
        }
        let view = serde_json::json!({"cards":[{
            "at":{"controller":1,"location":"Banished"},"position":{"face_up":false}
        }]});
        for controller in [serde_json::Value::Null, serde_json::json!(2), serde_json::json!(-1)] {
            let mut malformed = view.clone();
            malformed["cards"][0]["at"]["controller"] = controller;
            assert!(memory.has_facedown_banished(&malformed, 0));
        }
        for face_up in [serde_json::Value::Null, serde_json::json!(0), serde_json::json!("false")] {
            let mut malformed = view.clone();
            malformed["cards"][0]["position"]["face_up"] = face_up;
            assert!(memory.has_facedown_banished(&malformed, 0));
        }
        assert!(memory.has_facedown_banished(&serde_json::json!({"cards":[]}), 2));
    }

    #[test]
    fn hidden_temporary_banish_does_not_make_a_search_root_safe() {
        // Doom Lord (c41613948) preserves the target's facedown position and
        // returns it later. Reproduce that wire transition without consulting
        // the native identity; Slip of Fortune can produce the same boundary.
        for viewer in [0, 1] {
            let opponent = 1-viewer;
            let monster = at(opponent, location::MZONE, 0, false);
            let banished = at(opponent, location::REMOVED, 0, false);
            let mut outcomes = Vec::new();
            for private_code in [100, 999] {
                let mut memory = PublicIdentityMemory::default();
                memory.observe(&movement(private_code, at(opponent, location::HAND, 0, false), monster)).unwrap();
                let before = serde_json::json!({"cards":[{
                    "at":{"controller":opponent,"location":"MonsterZone","sequence":0},
                    "position":{"face_up":false},"code":null
                }]});
                assert!(memory.has_unknown_monsters(&before, viewer));
                memory.observe(&movement(private_code, monster, banished)).unwrap();
                memory.observe(&update(private_code, banished)).unwrap();
                let after = serde_json::json!({"cards":[{
                    "at":{"controller":opponent,"location":"Banished","sequence":0},
                    "position":{"face_up":false},"code":null
                }]});
                assert!(!memory.has_unknown_monsters(&after, viewer));
                assert!(memory.has_facedown_banished(&after, viewer));
                outcomes.push(memory);
            }
            assert_eq!(outcomes[0], outcomes[1]);
        }
    }
}
