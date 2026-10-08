//! Reference players: uniform random play and the YGOPro first-option order.
use crate::agent::{Policy, TieBreak};
use crate::model::{ChoiceKind, Decision, Observation};

#[derive(Clone)]
pub struct Random(pub TieBreak);

impl Policy for Random {
    fn choose(&mut self, _: &Observation, decision: &Decision) -> usize {
        let choices: Vec<_> = decision
            .choices
            .iter()
            .enumerate()
            .filter(|(_, c)| c.kind != ChoiceKind::ShuffleHand)
            .map(|(i, _)| i)
            .collect();
        choices
            .get(self.0.index(choices.len().max(1)))
            .copied()
            .unwrap_or(0)
    }
    fn fork(&self) -> Option<Box<dyn Policy>> {
        Some(Box::new(self.clone()))
    }
}

#[derive(Clone)]
pub struct First;

/// Commands and selections already follow engine order; declining comes last.
/// Hand shuffling is absent from the reference environment's action list.
pub fn first(decision: &Decision) -> usize {
    decision
        .choices
        .iter()
        .enumerate()
        .min_by_key(|(i, c)| {
            (
                match c.kind {
                    ChoiceKind::ShuffleHand => 2,
                    ChoiceKind::Pass | ChoiceKind::No | ChoiceKind::Cancel => 1,
                    _ => 0,
                },
                *i,
            )
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

impl Policy for First {
    fn choose(&mut self, _: &Observation, decision: &Decision) -> usize {
        first(decision)
    }
    fn fork(&self) -> Option<Box<dyn Policy>> {
        Some(Box::new(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Choice, DecisionKind, Hint};
    fn decision(kinds: &[ChoiceKind]) -> Decision {
        Decision {
            kind: DecisionKind::Idle,
            hint: Hint::None,
            minimum: 0,
            maximum: 0,
            selected: vec![],
            subject: None,
            choices: kinds
                .iter()
                .map(|&kind| Choice {
                    kind,
                    card: None,
                    members: vec![],
                    description: 0,
                    place: None,
                })
                .collect(),
        }
    }
    #[test]
    fn first_plays_before_declining() {
        assert_eq!(
            first(&decision(&[ChoiceKind::Pass, ChoiceKind::Activate])),
            1
        );
        assert_eq!(first(&decision(&[ChoiceKind::No, ChoiceKind::Yes])), 1);
        assert_eq!(
            first(&decision(&[
                ChoiceKind::NormalSummon,
                ChoiceKind::Activate,
                ChoiceKind::EndTurn
            ])),
            0
        );
        assert_eq!(
            first(&decision(&[ChoiceKind::ShuffleHand, ChoiceKind::EndTurn])),
            1
        );
    }
    #[test]
    fn random_is_reproducible_and_visits_all_options() {
        let d = decision(&[
            ChoiceKind::NormalSummon,
            ChoiceKind::Activate,
            ChoiceKind::EndTurn,
            ChoiceKind::Pass,
        ]);
        let a = Random(TieBreak::new(7));
        let b = a.clone();
        let mut counts = [0; 4];
        for _ in 0..10000 {
            let i = a.0.index(d.choices.len());
            assert_eq!(i, b.0.index(d.choices.len()));
            counts[i] += 1;
        }
        assert!(counts.iter().all(|&n| (2300..2700).contains(&n)));
    }
}
