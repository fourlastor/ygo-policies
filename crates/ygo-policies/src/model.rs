//! Engine-agnostic view of a duel from one player's seat.
//!
//! An engine front end (see `ygo-policies-ocgcore`) builds these types from
//! the engine's event stream.  It is responsible for stripping everything the
//! seat cannot legally see: an [`Observation`] and a [`Decision`] never carry
//! the identity of an opponent's hidden card.

/// Where a card is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Location {
    Deck,
    Hand,
    MonsterZone,
    SpellTrapZone,
    Graveyard,
    Banished,
    Extra,
    Overlay,
    Other,
}

impl Location {
    /// Face-up piles whose contents both players always know.
    pub fn is_public_pile(self) -> bool {
        matches!(self, Location::Graveyard | Location::MonsterZone | Location::SpellTrapZone)
    }

    pub fn is_field(self) -> bool {
        matches!(self, Location::MonsterZone | Location::SpellTrapZone)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Position {
    pub face_up: bool,
    pub attack: bool,
}

impl Position {
    pub const FACE_UP_ATTACK: Position = Position { face_up: true, attack: true };
    pub const FACE_DOWN_ATTACK: Position = Position { face_up: false, attack: true };
    pub const FACE_UP_DEFENSE: Position = Position { face_up: true, attack: false };
    pub const FACE_DOWN_DEFENSE: Position = Position { face_up: false, attack: false };
}

/// A card slot: controller + location + index.  Stable for the duration of a
/// prompt, which is all a policy needs to name a card in its answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CardRef {
    pub controller: u8,
    pub location: Location,
    pub sequence: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CardView {
    pub at: CardRef,
    /// `None` when the viewer cannot see the card.
    pub code: Option<u32>,
    pub position: Position,
    pub attack: i32,
    pub defense: i32,
    pub level: u32,
    pub can_attack: bool,
    pub counters: u32,
}

impl CardView {
    pub fn known(&self) -> bool {
        self.code.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Phase {
    Draw,
    Standby,
    Main1,
    BattleStart,
    BattleStep,
    Damage,
    DamageCalculation,
    Battle,
    Main2,
    End,
}

impl Phase {
    pub fn is_battle(self) -> bool {
        matches!(
            self,
            Phase::BattleStart | Phase::BattleStep | Phase::Damage | Phase::DamageCalculation | Phase::Battle
        )
    }

    pub fn is_damage_step(self) -> bool {
        matches!(self, Phase::Damage | Phase::DamageCalculation)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ChainLink {
    pub code: u32,
    pub controller: u8,
    pub source: CardRef,
    pub targets: Vec<CardRef>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Observation {
    pub me: u8,
    pub turn: u32,
    pub turn_player: Option<u8>,
    pub phase: Option<Phase>,
    pub life_points: [i32; 2],
    pub cards: Vec<CardView>,
    /// Pile sizes (deck/hand/...): public even when the contents are not.
    pub pile_sizes: Vec<(u8, Location, u32)>,
    pub chain: Vec<ChainLink>,
    pub battle_attacker: Option<CardRef>,
    pub battle_target: Option<CardRef>,
    /// Card(s) of the event being responded to, e.g. the monster being
    /// Summoned.  Codes follow the same visibility rule as everything else.
    pub event_cards: Vec<(CardRef, Option<u32>)>,
    /// Whether `chain` is tracked by the front end.  When `false`, an empty
    /// `chain` means "unknown", not "no chain".
    pub chain_known: bool,
    /// Whether `CardView::can_attack` is real.  When `false` it must be
    /// approximated (see `Ctx::can_attack`).
    pub can_attack_known: bool,
}

impl Observation {
    pub fn opponent(&self) -> u8 {
        1 - self.me
    }

    pub fn card(&self, at: CardRef) -> Option<&CardView> {
        self.cards.iter().find(|c| c.at == at)
    }

    pub fn pile(&self, controller: u8, location: Location) -> impl Iterator<Item = &CardView> {
        self.cards
            .iter()
            .filter(move |c| c.at.controller == controller && c.at.location == location)
    }

    pub fn pile_size(&self, controller: u8, location: Location) -> u32 {
        self.pile_sizes
            .iter()
            .find(|(c, l, _)| *c == controller && *l == location)
            .map(|(_, _, n)| *n)
            .unwrap_or_else(|| self.pile(controller, location).count() as u32)
    }
}

/// Why the engine is asking for cards (drives the "good for me?" sign).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Hint {
    None,
    Release,
    Discard,
    Destroy,
    Banish,
    ToGraveyard,
    ReturnToHand,
    AddToHand,
    ToDeck,
    Summon,
    SpecialSummon,
    Set,
    FusionMaterial,
    SynchroMaterial,
    XyzMaterial,
    Tribute,
    FaceUp,
    FaceDown,
    Attack,
    Defense,
    Equip,
    Control,
    AttackTarget,
    Target,
    Effect,
    Confirm,
    Other(u64),
}

impl Hint {
    /// Selecting our own card for this reason costs us the card.
    pub fn is_cost(self) -> bool {
        matches!(
            self,
            Hint::Release
                | Hint::Discard
                | Hint::Banish
                | Hint::ToGraveyard
                | Hint::ReturnToHand
                | Hint::ToDeck
                | Hint::FusionMaterial
                | Hint::SynchroMaterial
                | Hint::XyzMaterial
                | Hint::Tribute
        )
    }

    /// Selecting a card for this reason gives it to us.
    pub fn is_gain(self) -> bool {
        matches!(self, Hint::AddToHand | Hint::SpecialSummon | Hint::Summon | Hint::Set | Hint::Equip)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum DecisionKind {
    /// Main Phase command.
    Idle,
    /// Battle Phase command.
    Battle,
    /// Chain window; `forced` means passing is not allowed.
    Chain { forced: bool },
    YesNo,
    Option,
    Position,
    /// Pick cards one at a time (Toggle / Finish / Cancel); `selected` holds
    /// the cards picked so far.
    SelectCards,
    /// Incremental select: toggle one card or finish.
    SelectToggle,
    /// Pick materials one at a time until they satisfy an engine-defined sum
    /// (Toggle / Finish); `selected` holds the cards picked so far.
    SelectSum,
    Place,
    Sort,
    Announce,
    Counter,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum ChoiceKind {
    NormalSummon,
    SpecialSummon,
    ChangePosition,
    SetMonster,
    SetSpellTrap,
    Activate,
    Attack,
    EnterBattle,
    EnterMain2,
    EndTurn,
    ShuffleHand,
    Pass,
    Yes,
    No,
    Option,
    Position(Position),
    Cards,
    Toggle,
    Finish,
    Cancel,
    Place,
    Sort,
    Announce,
    Counter,
    Other,
}

/// A card named by a choice.  `code` is `None` whenever the viewer cannot see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Member {
    pub at: CardRef,
    pub code: Option<u32>,
    pub value: i64,
    /// Engine-required members (always part of the answer) are marked.
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Choice {
    pub kind: ChoiceKind,
    /// The acting / subject card, if any.
    pub card: Option<Member>,
    /// The chosen cards for selection answers.
    pub members: Vec<Member>,
    /// Engine effect/option description id (0 when not applicable).
    pub description: u64,
    /// Zone chosen by a `Place` answer.
    pub place: Option<CardRef>,
}

impl Choice {
    pub fn code(&self) -> Option<u32> {
        self.card.and_then(|c| c.code)
    }

    pub fn at(&self) -> Option<CardRef> {
        self.card.map(|c| c.at)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Decision {
    pub kind: DecisionKind,
    pub hint: Hint,
    pub minimum: u32,
    pub maximum: u32,
    pub selected: Vec<CardRef>,
    /// Card the prompt is about (Position prompt, YesNo subject...).
    pub subject: Option<u32>,
    pub choices: Vec<Choice>,
}
