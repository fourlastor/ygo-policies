//! Destination-specific material value, after the policy has actually chosen
//! the Fusion monster. No deck identifier or inferred archetype selects this.
use crate::agent::Turn;
use crate::model::{Hint, Location, Member};

const WORM_ZERO: u32 = 74506079;
const FUTURE_FUSION: u32 = 77565204;

/// Partial facts only: Worm Zero's distinct-name thresholds and Future Fusion's
/// deck-to-grave route. Existing engine legality still determines every choice.
pub const SUPPORTED_CODES: &[u32] = &[WORM_ZERO, FUTURE_FUSION];

pub(super) fn continue_optional_selection(t: &Turn) -> bool {
    let Some(fusion) = &t.memory.fusion_selection else {
        return false;
    };
    fusion.destination == WORM_ZERO
        && fusion.source == Some(FUTURE_FUSION)
        && t.choices().any(|(_, choice)| {
            choice.kind == crate::model::ChoiceKind::Toggle
                && choice.card.is_some_and(|member| {
                    member.at.location == Location::Deck
                        && material_score(t, &member).is_some_and(|score| score > 0.0)
                })
        })
}

pub(super) fn material_score(t: &Turn, member: &Member) -> Option<f64> {
    if t.decision.hint != Hint::FusionMaterial || member.at.controller != t.ctx.me {
        return None;
    }
    let fusion = t.memory.fusion_selection.as_ref()?;
    if fusion.destination != WORM_ZERO {
        return None;
    }
    // Native SelectUnselect no longer offers selected cards. Their identities
    // must come from the actual earlier choices, never an inferred deck list.
    let selected: Option<std::collections::BTreeSet<u32>> = t
        .decision
        .selected
        .iter()
        .map(|at| {
            fusion
                .selected
                .iter()
                .find(|selected| selected.at == *at)
                .and_then(|selected| selected.code)
                .map(|code| t.ctx.canonical(code))
        })
        .collect();
    let selected = selected?;
    let code = t.ctx.canonical(member.code?);
    let ordinary = super::conversion::material_score(t, member)?;
    if selected.contains(&code) {
        // c74506079.matcheck counts distinct names, not material count. A
        // second copy does not increase ATK or unlock another ability.
        return Some(ordinary.min(0.0) - 6000.0);
    }
    if fusion.source == Some(FUTURE_FUSION) && member.at.location == Location::Deck {
        // Deck materials do not consume a current attacker or hand option.
        // At 2/4/6 different names Zero gains revival/removal/draw respectively;
        // all those effects are created dynamically after material selection.
        return Some(if selected.len() < 6 {
            7000.0 + ordinary.min(2000.0)
        } else {
            -6000.0
        });
    }
    // For live resources the minimum two distinct names unlock revival. More
    // names still cost their ordinary hand/field value; do not automatically
    // sacrifice four additional live cards merely to reach the sixth name.
    Some(ordinary + if selected.len() < 2 { 4000.0 } else { 0.0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, FusionSelection, Memory};
    use crate::cards::{types, CardData, MemoryCards};
    use crate::ctx::Ctx;
    use crate::model::{CardRef, Choice, ChoiceKind, Decision, DecisionKind, Observation, Phase};
    use crate::Policy;
    use std::sync::Arc;

    fn member(code: u32, sequence: u32, location: Location) -> Member {
        Member {
            at: CardRef {
                controller: 0,
                location,
                sequence,
            },
            code: Some(code),
            value: 0,
            required: false,
        }
    }
    fn observation() -> Observation {
        Observation {
            me: 0,
            turn: 1,
            turn_player: Some(0),
            phase: Some(Phase::Standby),
            life_points: [8000; 2],
            cards: vec![],
            pile_sizes: vec![],
            chain: vec![],
            battle_attacker: None,
            battle_target: None,
            event_cards: vec![],
            summon_used: false,
            chain_known: true,
            can_attack_known: true,
            coin_toss: None,
        }
    }
    fn decision() -> Decision {
        Decision {
            kind: DecisionKind::SelectToggle,
            hint: Hint::FusionMaterial,
            minimum: 1,
            maximum: 1,
            selected: vec![],
            subject: None,
            choices: vec![],
        }
    }
    fn database() -> MemoryCards {
        MemoryCards(
            (900_600_001..=900_600_008)
                .map(|code| {
                    (
                        code,
                        CardData {
                            code,
                            kind: types::MONSTER | types::EFFECT,
                            attack: 1500,
                            defense: 1000,
                            ..CardData::default()
                        },
                    )
                })
                .collect(),
        )
    }
    #[test]
    fn future_fusion_prefers_six_distinct_names_and_stops_after_the_sixth() {
        let obs = observation();
        let db = database();
        let mut prompt = decision();
        let mut memory = Memory::default();
        memory.last_activated = Some(FUTURE_FUSION);
        memory.fusion_selection = Some(FusionSelection {
            destination: WORM_ZERO,
            source: Some(FUTURE_FUSION),
            selected: vec![],
        });
        for i in 0..6 {
            let candidate = member(900_600_001 + i, i, Location::Deck);
            let turn = Turn {
                ctx: Ctx::new(&obs, &db),
                decision: &prompt,
                memory: &mut memory,
            };
            assert!(material_score(&turn, &candidate).unwrap() > 0.0);
            prompt.selected.push(candidate.at);
            memory
                .fusion_selection
                .as_mut()
                .unwrap()
                .selected
                .push(candidate);
        }
        let turn = Turn {
            ctx: Ctx::new(&obs, &db),
            decision: &prompt,
            memory: &mut memory,
        };
        assert!(material_score(&turn, &member(900_600_007, 7, Location::Deck)).unwrap() < 0.0);
        assert!(material_score(&turn, &member(900_600_001, 8, Location::Deck)).unwrap() < 0.0);
    }
    #[test]
    fn no_zero_prior_without_exact_destination_and_known_selected_identities() {
        let obs = observation();
        let db = database();
        let mut prompt = decision();
        let mut memory = Memory::default();
        memory.last_activated = Some(FUTURE_FUSION);
        let candidate = member(900_600_001, 0, Location::Deck);
        {
            let turn = Turn {
                ctx: Ctx::new(&obs, &db),
                decision: &prompt,
                memory: &mut memory,
            };
            assert_eq!(material_score(&turn, &candidate), None);
        }
        memory.fusion_selection = Some(FusionSelection {
            destination: 900_600_099,
            source: Some(FUTURE_FUSION),
            selected: vec![],
        });
        {
            let turn = Turn {
                ctx: Ctx::new(&obs, &db),
                decision: &prompt,
                memory: &mut memory,
            };
            assert_eq!(material_score(&turn, &candidate), None);
        }
        memory.fusion_selection.as_mut().unwrap().destination = WORM_ZERO;
        prompt
            .selected
            .push(member(900_600_002, 1, Location::Deck).at);
        let turn = Turn {
            ctx: Ctx::new(&obs, &db),
            decision: &prompt,
            memory: &mut memory,
        };
        assert_eq!(material_score(&turn, &candidate), None);
    }
    #[test]
    fn sixth_name_does_not_receive_free_deck_material_credit_when_from_hand() {
        let obs = observation();
        let db = database();
        let mut prompt = decision();
        let selected: Vec<_> = (0..5)
            .map(|i| member(900_600_001 + i, i, Location::Hand))
            .collect();
        prompt.selected = selected.iter().map(|m| m.at).collect();
        let mut memory = Memory::default();
        memory.last_activated = Some(24094653);
        memory.fusion_selection = Some(FusionSelection {
            destination: WORM_ZERO,
            source: Some(24094653),
            selected,
        });
        let turn = Turn {
            ctx: Ctx::new(&obs, &db),
            decision: &prompt,
            memory: &mut memory,
        };
        assert!(material_score(&turn, &member(900_600_006, 5, Location::Hand)).unwrap() < 0.0);
    }

    fn choice(kind: ChoiceKind, card: Option<Member>) -> Choice {
        Choice {
            kind,
            card,
            members: vec![],
            description: 0,
            place: None,
        }
    }

    fn select_fusion_materials(destination: u32, source: Option<u32>, distinct: u32) -> Vec<u32> {
        let obs = observation();
        let mut db = database();
        db.0.insert(
            destination,
            CardData {
                code: destination,
                kind: types::MONSTER | types::FUSION,
                ..CardData::default()
            },
        );
        let mut agent = Agent::new(crate::generic::Generic, Arc::new(db));
        if let Some(source) = source {
            let mut activate = decision();
            activate.kind = DecisionKind::Idle;
            activate.hint = Hint::None;
            activate.choices = vec![choice(
                ChoiceKind::Activate,
                Some(member(source, 0, Location::Hand)),
            )];
            assert!(agent.commit_choice(&obs, &activate, 0));
        }
        let mut target = decision();
        target.kind = DecisionKind::SelectCards;
        target.hint = Hint::Confirm;
        target.choices = vec![choice(
            ChoiceKind::Toggle,
            Some(member(destination, 0, Location::Extra)),
        )];
        assert_eq!(agent.choose(&obs, &target), 0);

        // Match native SelectUnselect: selected references are authoritative,
        // while choices contain only remaining cards and no selected members.
        // These unknown card names need no Worm-specific material whitelist.
        let mut remaining: Vec<_> = (0..distinct)
            .map(|i| member(900_600_001 + i, i, Location::Deck))
            .collect();
        remaining.push(member(900_600_001, distinct, Location::Deck));
        let mut selected = Vec::<Member>::new();
        for _ in 0..10 {
            let mut prompt = decision();
            prompt.selected = selected.iter().map(|member| member.at).collect();
            prompt.choices = remaining
                .iter()
                .copied()
                .map(|member| choice(ChoiceKind::Toggle, Some(member)))
                .collect();
            if selected.len() >= 2 {
                prompt.choices.push(choice(ChoiceKind::Finish, None));
            }
            let picked = &prompt.choices[agent.choose(&obs, &prompt)];
            if picked.kind == ChoiceKind::Finish {
                return selected
                    .into_iter()
                    .map(|member| member.code.unwrap())
                    .collect();
            }
            let picked = picked.card.unwrap();
            assert!(picked.code.is_some());
            remaining.retain(|member| member.at != picked.at);
            selected.push(picked);
        }
        panic!("material selection did not terminate");
    }

    #[test]
    fn native_future_fusion_selects_six_distinct_names_only_for_chosen_zero() {
        let zero = select_fusion_materials(WORM_ZERO, Some(FUTURE_FUSION), 8);
        assert_eq!(zero.len(), 6);
        assert_eq!(
            zero.iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            6
        );
        // The same visible materials and source must not infer Worm Zero when
        // the actual selected Extra Deck destination was a different Fusion.
        assert_eq!(
            select_fusion_materials(900_600_099, Some(FUTURE_FUSION), 8).len(),
            2
        );
        assert_eq!(select_fusion_materials(WORM_ZERO, None, 8).len(), 2);
        assert_eq!(
            select_fusion_materials(WORM_ZERO, Some(FUTURE_FUSION), 3).len(),
            3
        );
    }
}
