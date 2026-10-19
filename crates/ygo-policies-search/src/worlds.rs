//! Worlds of a decision: what a seat cannot see, dealt again.
//!
//! The cards a seat cannot see are listed from the engine (never read for
//! their identity), then exchanged with each other in every world: its own
//! Deck order, the other player's Deck, hand and Set Spells and Traps, and
//! the other player's unknown Set monsters (with monsters it could have Set).
use std::collections::HashMap;
use ygo_policies_duel::{Duel, Snapshot};
use ygo_policies_ocgcore::wire::{position, query};

use crate::Result;

const QUERY_IS_PUBLIC: u32 = 0x100000;
const TYPE_SPELL_OR_TRAP: u32 = 0x2 | 0x4;
const TYPE_FIELD: u32 = 0x80000;
const TYPE_MONSTER: u32 = 0x1;
/// Fusion, Ritual, Synchro, Xyz, Link: never Set from the hand.
const TYPE_NOT_SET: u32 = 0x40 | 0x80 | 0x2000 | 0x800000 | 0x4000000;

/// A card the searching seat cannot see, and where it is.
#[derive(Clone, Copy)]
struct Slot {
    code: u32,
    controller: u8,
    location: u32,
    sequence: u32,
}

struct SplitMix(u64);
impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn shuffle(&mut self, values: &mut [usize]) {
        for i in (1..values.len()).rev() {
            values.swap(i, self.next() as usize % (i + 1));
        }
    }
}


/// The fields of one `OCG_DuelQuery` answer the search reads.
fn card_query(bytes: &[u8]) -> (u32, u32, bool) {
    let (mut code, mut position, mut public) = (0, 0, false);
    let mut offset = 0;
    while offset + 6 <= bytes.len() {
        let size = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        if size < 4 || offset + 2 + size > bytes.len() {
            break;
        }
        let flag = u32::from_le_bytes(bytes[offset + 2..offset + 6].try_into().unwrap());
        let value = &bytes[offset + 6..offset + 2 + size];
        if value.len() >= 4 {
            let number = u32::from_le_bytes(value[..4].try_into().unwrap());
            if flag == query::CODE {
                code = number;
            } else if flag == query::POSITION {
                position = number;
            }
        }
        if flag == QUERY_IS_PUBLIC && !value.is_empty() {
            public = value[0] != 0;
        }
        offset += 2 + size;
    }
    (code, position, public)
}


/// The cards `viewer` cannot see: its own Deck, and the other player's
/// Deck, hand and Set Spells and Traps, and the other player's Set monsters
/// in the `monsters` zones (those whose identity `viewer` does not know).
/// Revealed cards stay known.
fn hidden(duel: &Duel, viewer: u8, all_spell_zones: bool, monsters: &[u32]) -> Result<Vec<Slot>> {
    let mut slots = Vec::new();
    let mut add = |controller: u8, location: u32, sequence: u32, face_down_only: bool| -> Result<()> {
        let bytes = duel.card(query::CODE | query::POSITION | QUERY_IS_PUBLIC, controller, location, sequence)?;
        if bytes.is_empty() {
            return Ok(());
        }
        let (code, position, public) = card_query(&bytes);
        if code != 0 && !public && (!face_down_only || position & position::FACEDOWN != 0) {
            slots.push(Slot { code, controller, location, sequence });
        }
        Ok(())
    };
    let opponent = 1 - viewer;
    for (controller, location) in [(viewer, 1), (opponent, 1), (opponent, 2)] {
        for sequence in 0..duel.count(controller, location)? {
            add(controller, location, sequence, false)?;
        }
    }
    for sequence in 0..if all_spell_zones { 8 } else { 7 } {
        add(opponent, 8, sequence, true)?;
    }
    for &sequence in monsters {
        add(opponent, 4, sequence, true)?;
    }
    Ok(slots)
}

/// The codes of the cards `viewer` has in the duel, in no order: before
/// the duel moves any, its Deck list, which a player knows.  Nothing else of
/// them (where they are, in what order) is kept.
pub(crate) fn own_cards(duel: &Duel, viewer: u8) -> Result<Vec<u32>> {
    let mut codes = Vec::new();
    // Piles of any size, then the Monster and Spell & Trap Zones.
    for (location, zones) in [(1, None), (2, None), (0x10, None), (0x20, None), (0x40, None), (4, Some(7)), (8, Some(8))] {
        for sequence in 0..zones.map_or_else(|| duel.count(viewer, location), Ok)? {
            let (code, _, _) = card_query(&duel.card(query::CODE, viewer, location, sequence)?);
            if code != 0 {
                codes.push(code);
            }
        }
    }
    codes.sort_unstable();
    codes.dedup();
    Ok(codes)
}

/// Exact exchanges for a root inventory and world seed. Planning consumes
/// the same random numbers and keeps the original exchange order.
fn swap_plan(duel: &Duel, slots: &[Slot], viewer: u8, seed: u64, canonical: bool) -> Result<Vec<(Slot, Slot)>> {
    swap_plan_with_types(slots, viewer, seed, canonical, |code| duel.printed(code).map(|card| (card.kind, card.level)))
}

fn swap_plan_with_types(
    slots: &[Slot], viewer: u8, seed: u64, canonical: bool,
    printed: impl Fn(u32) -> Option<(u32, u32)>,
) -> Result<Vec<(Slot, Slot)>> {
    let printed_kind = |code: u32| printed(code).map(|(kind, _)| kind);
    let mut rng = SplitMix(seed);
    let mut swaps = Vec::new();
    let mut permute = |group: &[usize], wanted: &[usize]| -> Result<()> {
        let mut current = group.to_vec();
        for destination in 0..group.len() {
            let source = current.iter().position(|slot| *slot == wanted[destination]).ok_or("Lost a hidden card")?;
            if source == destination {
                continue;
            }
            let (a, b) = (slots[group[destination]], slots[group[source]]);
            swaps.push((a, b));
            current.swap(destination, source);
        }
        Ok(())
    };
    let own: Vec<usize> = (0..slots.len()).filter(|i| slots[*i].controller == viewer).collect();
    let mut wanted = own.clone();
    if canonical { wanted.sort_by_key(|index| slots[*index].code); }
    rng.shuffle(&mut wanted);
    permute(&own, &wanted)?;

    let theirs: Vec<usize> = (0..slots.len()).filter(|i| slots[*i].controller != viewer).collect();
    let zones = theirs.clone();
    // Each unknown Set monster becomes any monster the other player could
    // have Set there: a Main Deck monster of Level 4 or lower, none Ritual.
    let set_monsters: Vec<usize> = zones.iter().copied().filter(|i| slots[*i].location == 4).collect();
    let settable = |index: usize| printed(slots[index].code).is_some_and(|(kind, level)| {
        kind & TYPE_MONSTER != 0 && kind & TYPE_NOT_SET == 0 && level <= 4
    });
    // Without Set monsters to deal, the random stream is the one it was.
    let mut monster_pool: Vec<usize> = if set_monsters.is_empty() { Vec::new() } else {
        theirs.iter().copied().filter(|index| settable(*index)).collect()
    };
    if !set_monsters.is_empty() {
        if canonical { monster_pool.sort_by_key(|index| slots[*index].code); }
        rng.shuffle(&mut monster_pool);
        if monster_pool.len() < set_monsters.len() {
            return Err("Fewer hidden monsters that could be Set than Set monsters".into());
        }
        monster_pool.truncate(set_monsters.len());
    }
    // The cards still to place, in their original order.
    let theirs: Vec<usize> = theirs.iter().copied().filter(|i| !monster_pool.contains(i)).collect();
    let set: Vec<usize> = zones.iter().copied().filter(|i| slots[*i].location == 8).collect();
    let backrow = if canonical {
        // Standard set cards belong in SZONE 0..4; only Field Spells belong
        // in sequence 5. The native swap accepts all S/T in either position,
        // so the sampler must enforce this stronger rule before any swap.
        let field = |index: usize| printed_kind(slots[index].code)
            .is_some_and(|kind| kind & TYPE_SPELL_OR_TRAP != 0 && kind & TYPE_FIELD != 0);
        let regular = |index: usize| printed_kind(slots[index].code)
            .is_some_and(|kind| kind & TYPE_SPELL_OR_TRAP != 0 && kind & TYPE_FIELD == 0);
        let mut fields: Vec<usize> = theirs.iter().copied().filter(|index| field(*index)).collect();
        let mut regulars: Vec<usize> = theirs.iter().copied().filter(|index| regular(*index)).collect();
        fields.sort_by_key(|index| slots[*index].code);
        regulars.sort_by_key(|index| slots[*index].code);
        rng.shuffle(&mut fields);
        rng.shuffle(&mut regulars);
        let (mut fields, mut regulars) = (fields.into_iter(), regulars.into_iter());
        let mut selected = Vec::with_capacity(set.len());
        for &index in &set {
            let source = match slots[index].sequence {
                0..=4 if regular(index) => regulars.next().ok_or("Fewer hidden regular Spells/Traps than regular Set cards")?,
                5 if field(index) => fields.next().ok_or("Fewer hidden Field Spells than Set Field cards")?,
                _ => return Err("Unsupported hidden Spell/Trap-zone placement; refusing to sample an illegal world".into()),
            };
            selected.push(source);
        }
        selected
    } else {
        // Preserve the diagnostic search's original pool and random stream.
        let mut backrow: Vec<usize> = theirs.iter().copied()
            .filter(|index| printed_kind(slots[*index].code).is_some_and(|kind| kind & TYPE_SPELL_OR_TRAP != 0))
            .collect();
        if backrow.len() < set.len() { return Err("Fewer hidden Spells and Traps than Set cards".into()); }
        rng.shuffle(&mut backrow);
        backrow.truncate(set.len());
        backrow
    };
    let mut rest: Vec<usize> = theirs.iter().copied().filter(|i| !backrow.contains(i)).collect();
    if canonical { rest.sort_by_key(|index| slots[*index].code); }
    rng.shuffle(&mut rest);
    // The Set monsters' zones first, then the Set Spells and Traps: no card
    // ever passes through a zone it could not be in.
    let destinations: Vec<usize> = set_monsters.iter().copied().chain(set.iter().copied())
        .chain(zones.iter().copied().filter(|i| slots[*i].location != 8 && slots[*i].location != 4)).collect();
    let sources: Vec<usize> = monster_pool.into_iter().chain(backrow).chain(rest).collect();
    permute(&destinations, &sources)?;
    Ok(swaps)
}

/// Cached only for one root decision. The snapshot after inventory queries
/// preserves their native query-buffer/allocation effects on every branch.
#[derive(Default)]
pub struct HiddenWorlds {
    known: Vec<(u8, u32, u32)>,
    monsters: Vec<u32>,
    canonical: bool,
    inventory: Option<Result<(Vec<Slot>, Snapshot)>>,
    plans: HashMap<u64, Result<Vec<(Slot, Slot)>>>,
}

impl HiddenWorlds {
    /// Preserve identities already remembered by the searching player.
    pub fn with_known(known: Vec<(u8, u32, u32)>) -> Self {
        Self { known, canonical: true, ..Self::default() }
    }

    /// Also deal again the other player's Set monsters in these zones, whose
    /// identity the searching player does not know.
    pub fn with_monsters(mut self, monsters: Vec<u32>) -> Self {
        self.monsters = monsters;
        self
    }

    pub fn restore(&mut self, duel: &mut Duel, root: &Snapshot, viewer: u8, seed: u64) -> Result<()> {
        if self.inventory.is_none() {
            duel.restore(root)?;
            self.inventory = Some(hidden(duel, viewer, self.canonical, &self.monsters).and_then(|mut slots| {
                slots.retain(|slot| !self.known.contains(&(slot.controller, slot.location, slot.sequence)));
                Ok((slots, duel.snapshot()?))
            }));
        }
        let (slots, queried) = self.inventory.as_ref().unwrap().as_ref().map_err(Clone::clone)?;
        duel.restore(queried)?;
        let swaps = self.plans.entry(seed).or_insert_with(|| swap_plan(duel, slots, viewer, seed, self.canonical));
        for &(a, b) in swaps.as_ref().map_err(Clone::clone)? {
            if !duel.swap(a.controller, (a.location, a.sequence), (b.location, b.sequence))? {
                return Err(format!("The engine refused to swap {:x}:{} and {:x}:{}", a.location, a.sequence, b.location, b.sequence));
            }
        }
        Ok(())
    }
}


#[cfg(test)]
mod sampling_tests {
    use super::*;

    /// Type and Level: 100s Field Spells, 200s Spells, 300s Traps, 600 a
    /// Level 7 monster, 700 a Ritual monster, the rest Level 4 monsters.
    fn printed(code: u32) -> Option<(u32, u32)> {
        Some(match code {
            100 | 101 => (0x2 | TYPE_FIELD, 0), 200 | 201 => (0x2, 0), 300 | 301 => (0x4, 0),
            600 => (0x1, 7), 700 => (0x1 | 0x80, 4), _ => (0x1, 4),
        })
    }

    fn printed_kind(code: u32) -> Option<u32> {
        printed(code).map(|(kind, _)| kind)
    }

    fn slot(code: u32, controller: u8, location: u32, sequence: u32) -> Slot {
        Slot { code, controller, location, sequence }
    }

    fn apply(slots: &[Slot], swaps: &[(Slot, Slot)]) -> Vec<Slot> {
        let mut cards = slots.to_vec();
        for &(first, second) in swaps {
            let locate = |wanted: Slot| cards.iter().position(|card| (card.controller,card.location,card.sequence)==(wanted.controller,wanted.location,wanted.sequence)).unwrap();
            let (first,second)=(locate(first),locate(second));
            let code=cards[first].code;
            cards[first].code=cards[second].code;
            cards[second].code=code;
            // Check every intermediate native swap, not only the final deal.
            for card in cards.iter().filter(|card| card.location==8) {
                let kind=printed_kind(card.code).unwrap();
                assert_ne!(kind & TYPE_SPELL_OR_TRAP,0);
                assert_eq!(kind & TYPE_FIELD != 0, card.sequence==5);
            }
            // A Set monster's zone only ever holds a monster that could be Set.
            for card in cards.iter().filter(|card| card.location==4) {
                let (kind, level)=printed(card.code).unwrap();
                assert!(kind & TYPE_MONSTER != 0 && kind & TYPE_NOT_SET == 0 && level <= 4, "{} in a monster zone", card.code);
            }
        }
        cards
    }

    #[test]
    fn fast_worlds_keep_field_and_regular_spells_in_legal_zones_after_every_swap() {
        let cards=vec![slot(400,0,1,0),slot(201,0,1,1),
            slot(300,1,8,0),slot(200,1,8,1),slot(100,1,8,5),
            slot(101,1,2,0),slot(201,1,2,1),slot(301,1,1,0),slot(400,1,1,1)];
        let mut original: Vec<_>=cards.iter().map(|card| (card.controller,card.code)).collect();
        original.sort();
        for seed in 0..128 {
            let plan=swap_plan_with_types(&cards,0,seed,true,printed).unwrap();
            let dealt=apply(&cards,&plan);
            let mut inventory: Vec<_>=dealt.iter().map(|card| (card.controller,card.code)).collect();
            inventory.sort();
            assert_eq!(original,inventory);
        }
    }

    #[test]
    fn sampled_code_assignments_do_not_depend_on_actual_hidden_arrangement() {
        let first=vec![slot(200,0,1,0),slot(400,0,1,1),slot(300,1,8,0),slot(100,1,8,5),
            slot(101,1,2,0),slot(201,1,2,1),slot(301,1,1,0),slot(400,1,1,1)];
        let mut second=first.clone();
        second[0].code=400;second[1].code=200;
        second[2].code=301;second[6].code=300;
        second[3].code=101;second[4].code=100;
        second[5].code=400;second[7].code=201;
        for seed in 0..32 {
            let a=apply(&first,&swap_plan_with_types(&first,0,seed,true,printed).unwrap());
            let b=apply(&second,&swap_plan_with_types(&second,0,seed,true,printed).unwrap());
            assert_eq!(a.iter().map(|card|card.code).collect::<Vec<_>>(),b.iter().map(|card|card.code).collect::<Vec<_>>());
        }
    }

    #[test]
    fn set_monsters_are_dealt_again_only_from_monsters_that_could_be_set() {
        // Two unknown Set monsters, a Set Trap, and the hand and Deck holding
        // Level 4 monsters, a Level 7 one, a Ritual one and Spells.
        let cards=vec![slot(500,1,4,0),slot(501,1,4,2),slot(300,1,8,1),
            slot(600,1,2,0),slot(700,1,2,1),slot(502,1,2,2),slot(201,1,1,0),slot(503,1,1,1),slot(301,1,1,2)];
        let mut original: Vec<_>=cards.iter().map(|card| card.code).collect();
        original.sort();
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..256 {
            let dealt=apply(&cards,&swap_plan_with_types(&cards,0,seed,true,printed).unwrap());
            let mut inventory: Vec<_>=dealt.iter().map(|card| card.code).collect();
            inventory.sort();
            assert_eq!(original,inventory);
            seen.extend(dealt.iter().filter(|card| card.location==4).map(|card| card.code));
        }
        // Every Level 4 monster can end up Set, the others never.
        assert_eq!(seen.into_iter().collect::<Vec<_>>(), vec![500,501,502,503]);
    }

    #[test]
    fn set_monster_deals_do_not_depend_on_which_monster_is_really_set() {
        let first=vec![slot(500,1,4,0),slot(502,1,2,0),slot(503,1,1,0),slot(201,1,1,1)];
        let mut second=first.clone();
        second[0].code=503;second[2].code=500;
        for seed in 0..64 {
            let a=apply(&first,&swap_plan_with_types(&first,0,seed,true,printed).unwrap());
            let b=apply(&second,&swap_plan_with_types(&second,0,seed,true,printed).unwrap());
            assert_eq!(a.iter().map(|card|card.code).collect::<Vec<_>>(),b.iter().map(|card|card.code).collect::<Vec<_>>());
        }
    }

    #[test]
    fn too_few_monsters_to_set_falls_back() {
        // The Set cards themselves count among the candidates; here none could be Set.
        assert!(swap_plan_with_types(&[slot(600,1,4,0),slot(700,1,4,1),slot(201,1,2,0)],0,3,true,printed).is_err());
        assert!(swap_plan_with_types(&[slot(600,1,4,0),slot(500,1,2,0),slot(201,1,2,1)],0,3,true,printed).is_ok());
    }

    #[test]
    fn unsupported_set_placement_falls_back_instead_of_inventing_a_world() {
        for bad in [slot(400,1,8,0),slot(200,1,8,5),slot(100,1,8,0),slot(200,1,8,6),slot(200,1,8,7)] {
            assert!(swap_plan_with_types(&[bad,slot(201,1,1,0)],0,17,true,printed).is_err());
        }
    }
}

