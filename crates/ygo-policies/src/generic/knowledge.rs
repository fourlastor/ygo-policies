//! Small, source-backed facts about effects that initial engine categories omit.
//!
//! These describe card rules, never a deck, matchup, strategy or preferred move.
//! A missing fact means unknown, not that a card lacks an effect. Callers still
//! need public context and legal prompts; none of these facts proves an effect
//! is currently active, payable, or beneficial. In particular, a displayed
//! aggregate counter count does not identify that counter's type.
//!
//! Sources are the checked-in `vendor/CardScripts/official/c<code>.lua` files.
//! The initial table is restricted to cards in the supplied main/extra pool.

use crate::cards::{attributes, races};

pub const FOG_COUNTER: u16 = 0x1019;
pub const CLOUDIAN_SET: u16 = 0x18;
pub const FROG_SET: u16 = 0x12;
pub const DRAGUNITY_SET: u16 = 0x29;

/// Card-script descriptions distinguish effects/options, but are not unique
/// across effect types: Swap Frog deliberately uses the same id for two effects.
pub const fn description(code: u32, index: u8) -> u64 {
    ((code as u64) << 20) | index as u64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageShield {
    /// Protects the controller while they control at least one monster.
    ControllerWhileControlsMonster,
    /// Protects either player from battle damage involving their monster of
    /// this race. It does not prevent a direct attack's damage.
    BattleInvolvingRace { race: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterGain {
    /// Normal Summon trigger; count face-up matching monsters on both fields.
    FaceUpSetOnBothFields { setcode: u16 },
    /// Tribute Summon trigger; count matching monsters actually tributed.
    AttributeTributes { attribute: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterPool {
    ThisCard,
    BothFields,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetDomain {
    AnyCard,
    Monster,
    SpellTrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterPayoff {
    SpecialSummonNamed {
        code: u32,
        own_deck: bool,
        own_grave: bool,
        opponent_grave: bool,
    },
    /// Both players' cards are legal targets, including one's own cards.
    DestroyOnEitherField {
        domain: TargetDomain,
    },
    RandomDiscardOpponent {
        count: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CounterSpend {
    pub amount: u8,
    pub pool: CounterPool,
    pub payoff: CounterPayoff,
    pub description: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CounterEngine {
    pub counter: u16,
    pub normal_summon_gain: CounterGain,
    pub spend: Option<CounterSpend>,
    /// Continuous ATK gain for each such counter anywhere on either field.
    /// Live ATK already includes this; do not add it a second time.
    pub attack_per_counter_on_field: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterTiming {
    OwnStandby,
    Ignition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterRecipients {
    EveryFaceUpMonster,
    OneFaceUpMonsterOnEitherField,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CounterSupport {
    pub counter: u16,
    pub amount: u8,
    pub timing: CounterTiming,
    pub recipients: CounterRecipients,
    pub description: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalOperation {
    TopOfDeck,
    BanishFaceUp,
}

/// Mandatory trigger on a successful Tribute Summon. A legal summon can force
/// its controller to remove their own card when no opponent target remains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MandatoryRemoval {
    pub domain: TargetDomain,
    pub operation: RemovalOperation,
    pub can_target_own: bool,
    /// Damage to the opponent when the banished target is a DARK monster.
    /// This matters even when the controller targets their own monster.
    pub dark_monster_banish_damage: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CardProfile {
    pub damage_shield: Option<DamageShield>,
    pub battle_indestructible: bool,
    pub dies_in_face_up_defense: bool,
    pub counter_engine: Option<CounterEngine>,
    pub counter_support: Option<CounterSupport>,
    pub mandatory_removal: Option<MandatoryRemoval>,
    /// A live field effect disables field traps. This does not claim every
    /// trap effect in the hand or graveyard is negated, nor encode effect order.
    pub negates_field_traps: bool,
}

/// Unknown cards return an empty profile. Alias resolution belongs to the card
/// database; unrelated cards which share a name are not interchangeable.
pub fn profile(code: u32) -> CardProfile {
    let mut facts = CardProfile::default();
    match code {
        // c53239672.initial_effect / condition: any controlled MZONE card.
        53239672 => facts.damage_shield = Some(DamageShield::ControllerWhileControlsMonster),
        // c56433456.initial_effect: Fairy battle damage, both players.
        56433456 => {
            facts.damage_shield = Some(DamageShield::BattleInvolvingRace { race: races::FAIRY })
        }
        // Each source registers battle immunity and face-up DEF self-destruction.
        16197610 | 79703905 | 17810268 | 43318266 | 13474291 | 20003527 => {
            facts.battle_indestructible = true;
            facts.dies_in_face_up_defense = true;
            let spend = match code {
                // c16197610.spcost / spfilter / sptg: one counter on self.
                16197610 => Some(CounterSpend {
                    amount: 1,
                    pool: CounterPool::ThisCard,
                    payoff: CounterPayoff::SpecialSummonNamed {
                        code: 80825553,
                        own_deck: true,
                        own_grave: true,
                        opponent_grave: true,
                    },
                    description: description(code, 1),
                }),
                // c79703905.cost / operation: remove three across both fields.
                79703905 => Some(CounterSpend {
                    amount: 3,
                    pool: CounterPool::BothFields,
                    payoff: CounterPayoff::RandomDiscardOpponent { count: 1 },
                    description: description(code, 1),
                }),
                // c17810268/c43318266.cost / target: own targets remain legal.
                17810268 | 43318266 => Some(CounterSpend {
                    amount: 2,
                    pool: CounterPool::ThisCard,
                    payoff: CounterPayoff::DestroyOnEitherField {
                        domain: if code == 17810268 {
                            TargetDomain::SpellTrap
                        } else {
                            TargetDomain::Monster
                        },
                    },
                    description: description(code, 1),
                }),
                _ => None,
            };
            if code == 13474291 {
                // c13474291.cttg / ctop: any face-up monster, not just Cloudians.
                facts.counter_support = Some(CounterSupport {
                    counter: FOG_COUNTER,
                    amount: 1,
                    timing: CounterTiming::Ignition,
                    recipients: CounterRecipients::OneFaceUpMonsterOnEitherField,
                    description: description(code, 0),
                });
            } else {
                facts.counter_engine = Some(CounterEngine {
                    counter: FOG_COUNTER,
                    normal_summon_gain: if code == 20003527 {
                        // c20003527.valcheck / addcon / addc: only WATER tributes.
                        CounterGain::AttributeTributes {
                            attribute: attributes::WATER,
                        }
                    } else {
                        CounterGain::FaceUpSetOnBothFields {
                            setcode: CLOUDIAN_SET,
                        }
                    },
                    spend,
                    attack_per_counter_on_field: if code == 20003527 { 500 } else { 0 },
                });
            }
        }
        // c90135989.ctop: every face-up monster, at controller's Standby Phase.
        90135989 => {
            facts.counter_support = Some(CounterSupport {
                counter: FOG_COUNTER,
                amount: 1,
                timing: CounterTiming::OwnStandby,
                recipients: CounterRecipients::EveryFaceUpMonster,
                description: description(code, 0),
            })
        }
        // c73125233/c9748752: forced Tribute Summon trigger, either field.
        73125233 | 9748752 => {
            facts.mandatory_removal = Some(MandatoryRemoval {
                domain: TargetDomain::AnyCard,
                operation: if code == 73125233 {
                    RemovalOperation::TopOfDeck
                } else {
                    RemovalOperation::BanishFaceUp
                },
                can_target_own: true,
                dark_monster_banish_damage: if code == 9748752 { 1000 } else { 0 },
            })
        }
        // c77585513 / c51452091: disable field traps while this effect is live.
        77585513 | 51452091 => facts.negates_field_traps = true,
        _ => {}
    }
    facts
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Activation,
    Ignition,
    Trigger,
    SpecialSummonProcedure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cost {
    None,
    DiscardHand { count: u8 },
    DiscardOtherMonster { attribute: u32 },
    ReturnOwnMonsterToHand,
    BanishOwnMonsterEquippedWith { equip_setcode: u16 },
    BanishOwnGraveMonster { setcode: u16 },
    SkipBattlePhaseThisTurn,
}

/// Operations are deliberately narrower than a category union. A selection
/// between search and grave setup must be valued as a choice, not both gains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    SearchOwnDeck {
        setcode: u16,
        level_max: u8,
    },
    SendOwnDeckToGrave {
        race: u32,
    },
    ExtraNormalSummon {
        setcode: u16,
        excluded_code: u32,
    },
    /// Sets the limit, rather than adding this many summons to the old limit.
    SetNormalSummonLimit {
        limit: u8,
    },
    /// A targeted opponent monster may be used for a Tribute this turn. This
    /// includes appropriate effect costs; it is not limited to Normal Summons.
    UseTargetAsTributeThisTurn,
    SpecialSummonSelf,
    PlaceCounterOnFaceUpMonster {
        counter: u16,
        amount: u8,
    },
    Choose(&'static [Operation]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionFacts {
    pub cost: Cost,
    pub payoff: Operation,
}

const RAVINE_MODES: &[Operation] = &[
    Operation::SearchOwnDeck {
        setcode: DRAGUNITY_SET,
        level_max: 4,
    },
    Operation::SendOwnDeckToGrave {
        race: races::DRAGON,
    },
];

/// Exact effect-type/description lookup; no whole-card category promotion.
/// Special Summon procedures and unlabelled spell activations use description 0.
pub fn action(code: u32, kind: ActionKind, desc: u64) -> Option<ActionFacts> {
    let (cost, payoff) = match (code, kind) {
        // c62265044.effcost / efftg / effop: discard before selecting one mode.
        (62265044, ActionKind::Ignition) if desc == description(code, 0) => (
            Cost::DiscardHand { count: 1 },
            Operation::Choose(RAVINE_MODES),
        ),
        // c9126351.excost / exop / estg. The summon trigger shares desc1; it
        // must not acquire this cost or payoff through description alone.
        (9126351, ActionKind::Ignition) if desc == description(code, 1) => (
            Cost::ReturnOwnMonsterToHand,
            Operation::ExtraNormalSummon {
                setcode: FROG_SET,
                excluded_code: code,
            },
        ),
        (9126351, ActionKind::SpecialSummonProcedure) if desc == 0 => (
            Cost::DiscardOtherMonster {
                attribute: attributes::WATER,
            },
            Operation::SpecialSummonSelf,
        ),
        // c63487632.spfilter / spop: can consume an existing copy of itself.
        // Its opponent-effect grave trigger does not refund this banish cost.
        (63487632, ActionKind::SpecialSummonProcedure) if desc == 0 => (
            Cost::BanishOwnMonsterEquippedWith {
                equip_setcode: DRAGUNITY_SET,
            },
            Operation::SpecialSummonSelf,
        ),
        // c13474291.spfilter / spop and cttg / ctop.
        (13474291, ActionKind::SpecialSummonProcedure) if desc == 0 => (
            Cost::BanishOwnGraveMonster {
                setcode: CLOUDIAN_SET,
            },
            Operation::SpecialSummonSelf,
        ),
        (13474291, ActionKind::Ignition) if desc == description(code, 0) => (
            Cost::None,
            Operation::PlaceCounterOnFaceUpMonster {
                counter: FOG_COUNTER,
                amount: 1,
            },
        ),
        // c68005187.cost / activate: restriction is paid even if negated.
        (68005187, ActionKind::Activation) if desc == 0 => (
            Cost::SkipBattlePhaseThisTurn,
            Operation::UseTargetAsTributeThisTurn,
        ),
        // c43422537.target / activate: limit two until the end of this turn.
        (43422537, ActionKind::Activation) if desc == 0 => {
            (Cost::None, Operation::SetNormalSummonLimit { limit: 2 })
        }
        _ => return None,
    };
    Some(ActionFacts { cost, payoff })
}

pub fn option(code: u32, desc: u64) -> Option<Operation> {
    if code == 62265044 {
        if desc == description(code, 1) {
            return Some(RAVINE_MODES[0]);
        }
        if desc == description(code, 2) {
            return Some(RAVINE_MODES[1]);
        }
    }
    None
}

/// Auditable coverage, not a claim that all effects of these cards are known.
/// Sources are listed in profile/action above; unknown cards use the fallback.
pub const SUPPORTED_CODES: &[u32] = &[
    9126351, 9748752, 13474291, 16197610, 17810268, 20003527, 43318266, 43422537, 51452091,
    53239672, 56433456, 62265044, 63487632, 68005187, 73125233, 77585513, 79703905, 90135989,
];

pub const FACT_KINDS: &[&str] = &[
    "conditional battle damage protection",
    "battle indestructibility",
    "face-up defense self-destruction",
    "normal summon counter gain",
    "counter costs and destinations",
    "counter support timing",
    "mandatory tribute-summon removal",
    "field trap negation",
    "summon and activation costs",
    "dynamic operation modes",
    "restricted extra summons",
    "normal summon count limit",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_and_unrelated_ids_do_not_acquire_facts() {
        assert_eq!(profile(0), CardProfile::default());
        // Mind Augus is not Jinzo, despite an easy-to-misremember card code.
        assert!(!profile(11877465).negates_field_traps);
        assert_eq!(action(0, ActionKind::Ignition, 0), None);
        assert_eq!(option(0, 0), None);
        assert!(SUPPORTED_CODES.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn shields_describe_distinct_conditions() {
        assert_eq!(
            profile(53239672).damage_shield,
            Some(DamageShield::ControllerWhileControlsMonster)
        );
        assert_eq!(
            profile(56433456).damage_shield,
            Some(DamageShield::BattleInvolvingRace { race: races::FAIRY })
        );
        assert!(profile(77585513).negates_field_traps);
        assert!(profile(51452091).negates_field_traps);
    }

    #[test]
    fn counter_cost_scope_and_summon_requirement_are_explicit() {
        let turbulence = profile(16197610).counter_engine.unwrap();
        assert_eq!(
            turbulence.normal_summon_gain,
            CounterGain::FaceUpSetOnBothFields {
                setcode: CLOUDIAN_SET
            }
        );
        assert_eq!(turbulence.spend.unwrap().pool, CounterPool::ThisCard);
        let altus = profile(79703905).counter_engine.unwrap().spend.unwrap();
        assert_eq!((altus.amount, altus.pool), (3, CounterPool::BothFields));
        let nimbus = profile(20003527).counter_engine.unwrap();
        assert_eq!(
            nimbus.normal_summon_gain,
            CounterGain::AttributeTributes {
                attribute: attributes::WATER
            }
        );
        assert_eq!(nimbus.attack_per_counter_on_field, 500);
        assert!(profile(13474291).counter_engine.is_none());
        assert_eq!(
            profile(90135989).counter_support.unwrap().recipients,
            CounterRecipients::EveryFaceUpMonster
        );
    }

    #[test]
    fn compulsory_removal_keeps_self_targets_and_self_banish_lethal() {
        let raiza = profile(73125233).mandatory_removal.unwrap();
        assert!(raiza.can_target_own);
        assert_eq!(raiza.operation, RemovalOperation::TopOfDeck);
        let caius = profile(9748752).mandatory_removal.unwrap();
        assert!(caius.can_target_own);
        assert_eq!(caius.dark_monster_banish_damage, 1000);
    }

    #[test]
    fn same_description_never_confuses_swaps_trigger_and_ignition() {
        let desc = description(9126351, 1);
        assert_eq!(action(9126351, ActionKind::Trigger, desc), None);
        let extra = action(9126351, ActionKind::Ignition, desc).unwrap();
        assert_eq!(extra.cost, Cost::ReturnOwnMonsterToHand);
        assert_eq!(
            extra.payoff,
            Operation::ExtraNormalSummon {
                setcode: FROG_SET,
                excluded_code: 9126351
            }
        );
        assert_eq!(
            action(9126351, ActionKind::Ignition, description(9126351, 0)),
            None
        );
    }

    #[test]
    fn ravine_modes_remain_alternatives_and_activation_stays_setup() {
        assert_eq!(action(62265044, ActionKind::Activation, 0), None);
        let ignition = action(62265044, ActionKind::Ignition, description(62265044, 0)).unwrap();
        assert_eq!(ignition.cost, Cost::DiscardHand { count: 1 });
        assert_eq!(ignition.payoff, Operation::Choose(RAVINE_MODES));
        assert_eq!(
            option(62265044, description(62265044, 1)),
            Some(RAVINE_MODES[0])
        );
        assert_eq!(
            option(62265044, description(62265044, 2)),
            Some(RAVINE_MODES[1])
        );
        assert_eq!(option(62265044, description(62265044, 3)), None);
    }

    #[test]
    fn summons_and_tribute_access_keep_their_resource_costs() {
        assert_eq!(
            action(63487632, ActionKind::SpecialSummonProcedure, 0)
                .unwrap()
                .cost,
            Cost::BanishOwnMonsterEquippedWith {
                equip_setcode: DRAGUNITY_SET
            }
        );
        assert_eq!(
            action(68005187, ActionKind::Activation, 0).unwrap().cost,
            Cost::SkipBattlePhaseThisTurn
        );
        assert_eq!(
            action(43422537, ActionKind::Activation, 0).unwrap().payoff,
            Operation::SetNormalSummonLimit { limit: 2 }
        );
    }
}
