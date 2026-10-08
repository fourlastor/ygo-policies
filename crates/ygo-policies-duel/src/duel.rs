//! One duel on the engine, stepped by its caller.
use std::{cell::RefCell, ffi::CString, ptr};

use ygo_policies_ocgcore::{
    message::{self, Message},
    wire::{msg, query},
};

use crate::core::{self, Core, Handle, NewCard, Options, Player, Query, Resources};
use crate::deck::{shuffle, Deck};
use crate::Result;

/// How a duel starts.  [`DuelOptions::seeded`] is what `policy-bench` plays:
/// change a field from there rather than copying its values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DuelOptions {
    /// The engine's random seed.
    pub seed: [u64; 4],
    /// OCGCore's duel flags.
    pub flags: u64,
    /// Starting Life Points of seat 0, who goes first, and of seat 1.
    pub life_points: [u32; 2],
    /// Cards in the opening hand.
    pub hand: u32,
    /// Cards drawn each turn.
    pub per_turn: u32,
    /// The seed [`Core::deal`] shuffles both Decks with, seat 0's first;
    /// `None` leaves each Deck in the order of its list.
    pub shuffle: Option<u64>,
}

impl DuelOptions {
    /// Master Rule 1, the bench's default rules.
    pub const MASTER_RULE_1: u64 = 0xD0700;
    /// `DUEL_MODE_MR5`: modern zones and summon rules, without a first-turn draw.
    pub const MASTER_RULE_5: u64 = 0x2E800;
    /// `DUEL_ATTACK_FIRST_TURN`: the player who goes first may attack.
    pub const ATTACK_FIRST_TURN: u64 = 0x02;
    /// Each player's Life Points at the start.
    pub const LIFE_POINTS: u32 = 8000;

    /// A duel from one number: the engine's seed and the Deck shuffle both
    /// come from `seed`.  Master Rule 1, 8000 Life Points, a five-card
    /// opening hand and one draw a turn.
    pub fn seeded(seed: u64) -> Self {
        Self {
            seed: [seed, (seed.wrapping_mul(1103515245) + 12345) & 0xffffffff, 3, 4],
            flags: Self::MASTER_RULE_1,
            life_points: [Self::LIFE_POINTS; 2],
            hand: 5,
            per_turn: 1,
            shuffle: Some(seed),
        }
    }
}

/// A card put somewhere before a duel starts (see [`Duel::add`]).
/// `location`, `sequence` and `position` are OCGCore's own values.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    /// The player it belongs to: 0 or 1.
    pub controller: u8,
    /// The card.
    pub code: u32,
    /// Where: 0x01 Deck, 0x02 hand, 0x04 Monster Zone, 0x08 Spell & Trap
    /// Zone, 0x10 Graveyard, 0x20 banished, 0x40 Extra Deck.
    pub location: u32,
    /// Its place in that location.
    pub sequence: u32,
    /// 0x1 face-up Attack, 0x2 face-down Attack, 0x4 face-up Defense, 0x8
    /// face-down Defense (as cards in a Deck are).
    pub position: u32,
}

/// A monster on the field, as the engine reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Monster {
    /// The player who controls it.
    pub controller: u8,
    /// Its zone.
    pub sequence: u32,
    /// The card.
    pub code: u32,
    /// Its position, as in [`Placed::position`].
    pub position: u32,
    /// Its ATK now.
    pub attack: i32,
    /// Its DEF now.
    pub defense: i32,
}

/// One message of a [`Step`], id first, as a seat is fed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sent {
    /// The message.
    pub bytes: Vec<u8>,
    /// Not the engine's own message: part of the field refresh the duel adds
    /// before a selection (see [`Duel::field`]).
    pub refresh: bool,
}

/// How a duel ended: the winner (2: a draw) and the engine's reason
/// (1: Life Points, 2: deck-out, 0x10 and up: a card's own win condition).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// 0 or 1, or 2 for a draw.
    pub winner: u8,
    /// The engine's reason.
    pub reason: u8,
}

/// Where a duel stands after a [`Step`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// The engine goes on by itself: step again.
    Running,
    /// The engine waits for the response to the selection it just sent
    /// ([`Duel::respond`]).
    Awaiting,
    /// The duel is over.  `None` when the engine stopped without naming a
    /// winner.
    Over(Option<Outcome>),
}

/// What one [`Duel::step`] produced.
#[derive(Clone, Debug)]
pub struct Step {
    /// The engine's messages, in order.  Before each selection come the
    /// messages of the field refresh, so feeding a seat every message in
    /// order shows it current ATK, DEF and positions when it decides.
    pub messages: Vec<Sent>,
    /// What the engine does next.
    pub state: State,
}

/// A duel's whole state at a decision, to go back to ([`Duel::snapshot`]).
pub struct Snapshot {
    handle: Handle,
    discard: unsafe extern "C" fn(Handle),
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        unsafe { (self.discard)(self.handle) }
    }
}

/// One duel.  Put its cards in ([`Core::deal`] does, for two Decks), then
/// step it: each [`Duel::step`] returns the engine's messages and says
/// whether it now waits for a response.  The seats are the caller's.
pub struct Duel<'a> {
    core: &'a Core,
    handle: Handle,
    /// Script errors the engine logged since they were last looked at.
    errors: Box<RefCell<Vec<String>>>,
    life_points: [u32; 2],
    /// Cards put in each player's Deck and Extra Deck, for [`Duel::start_message`].
    piles: [[u16; 2]; 2],
}

impl Drop for Duel<'_> {
    fn drop(&mut self) {
        unsafe { (self.core.destroy)(self.handle) }
    }
}

const DECK: u32 = 0x01;
const EXTRA: u32 = 0x40;
const FACE_DOWN_DEFENSE: u32 = 0x8;

impl Core {
    /// A duel with no card in it yet: [`Duel::add`] them, then
    /// [`Duel::start`] it.  `options.shuffle` is not used here.
    pub fn duel(&self, options: &DuelOptions) -> Result<Duel<'_>> {
        let errors = Box::new(RefCell::new(Vec::new()));
        let payload = (&*self.resources) as *const Resources as Handle;
        let player = |lp| Player { lp, draw: options.hand, per_turn: options.per_turn };
        let raw = Options {
            seed: options.seed,
            flags: options.flags,
            p0: player(options.life_points[0]),
            p1: player(options.life_points[1]),
            reader: core::reader,
            payload1: payload,
            scripts: core::scripts,
            payload2: payload,
            log: core::log,
            payload3: (&*errors) as *const RefCell<Vec<String>> as Handle,
            done: core::done,
            payload4: payload,
            unsafe_libraries: 0,
        };
        unsafe {
            let mut handle = ptr::null_mut();
            let status = (self.create)(&mut handle, &raw);
            if status != 0 {
                return Err(format!("OCG_CreateDuel: {status}"));
            }
            let duel = Duel { core: self, handle, errors, life_points: options.life_points, piles: [[0; 2]; 2] };
            for name in ["constant.lua", "utility.lua"] {
                let name = CString::new(name).unwrap();
                if core::scripts(payload, handle, name.as_ptr()) == 0 {
                    return Err(format!("Failed to load {name:?}"));
                }
            }
            Ok(duel)
        }
    }

    /// A duel between two Decks, started: seat 0 goes first.  Each Main Deck
    /// is shuffled with `options.shuffle` (seat 0's first, seat 1's from
    /// where that stream stopped) and goes in from its first card to its
    /// last, then the Extra Deck in its own order.
    pub fn deal(&self, options: &DuelOptions, decks: &[Deck; 2]) -> Result<Duel<'_>> {
        if let Some(code) = self.missing(decks.iter().flat_map(|d| d.main.iter().chain(&d.extra))) {
            return Err(format!("Deck card {code} is absent from the database"));
        }
        let mut duel = self.duel(options)?;
        let mut rng = options.shuffle;
        for (p, deck) in decks.iter().enumerate() {
            let mut main = deck.main.clone();
            if let Some(state) = &mut rng {
                shuffle(&mut main, state);
            }
            for (location, pile) in [(DECK, &main), (EXTRA, &deck.extra)] {
                for code in pile {
                    duel.add(Placed { controller: p as u8, code: *code, location, sequence: 0, position: FACE_DOWN_DEFENSE });
                }
            }
        }
        duel.start();
        Ok(duel)
    }
}

impl Duel<'_> {
    /// Put a card in the duel.  Only before [`Duel::start`].
    pub fn add(&mut self, card: Placed) {
        let pile = match card.location {
            DECK => Some(0),
            EXTRA => Some(1),
            _ => None,
        };
        if let (Some(pile), Some(count)) = (pile, self.piles.get_mut(card.controller as usize)) {
            count[pile] += 1;
        }
        unsafe {
            (self.core.new_card)(
                self.handle,
                &NewCard {
                    team: card.controller,
                    duelist: 0,
                    code: card.code,
                    controller: card.controller,
                    location: card.location,
                    sequence: card.sequence,
                    position: card.position,
                },
            );
        }
    }

    /// Start the duel: the opening hands are drawn at the first step.
    pub fn start(&mut self) {
        unsafe { (self.core.start)(self.handle) }
    }

    /// `MSG_START` for the seat of `player`, the first message to feed it:
    /// both players' Life Points and how many cards went into each Deck and
    /// Extra Deck.
    pub fn start_message(&self, player: u8) -> Vec<u8> {
        message::start_message(player, self.life_points, [self.piles[0][0], self.piles[1][0]], [self.piles[0][1], self.piles[1][1]])
    }

    /// Let the engine run until it asks something or the duel ends.
    /// A script error the engine logged on the way is an `Err`.
    pub fn step(&mut self) -> Result<Step> {
        let (status, buffer) = unsafe {
            let status = (self.core.process)(self.handle);
            let mut length = 0;
            let bytes = (self.core.messages)(self.handle, &mut length);
            let buffer = if length == 0 { vec![] } else { std::slice::from_raw_parts(bytes, length as usize).to_vec() };
            (status, buffer)
        };
        let mut messages = Vec::new();
        let mut outcome = None;
        let mut offset = 0;
        while offset < buffer.len() {
            if offset + 4 > buffer.len() {
                return Err("Truncated message length".into());
            }
            let n = u32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap()) as usize;
            let message = buffer.get(offset + 4..offset + 4 + n).ok_or("Truncated message")?;
            offset += 4 + n;
            let id = *message.first().ok_or("Empty message")?;
            if id == msg::WIN {
                outcome = Some(Outcome {
                    winner: *message.get(1).ok_or("Truncated message")?,
                    reason: *message.get(2).ok_or("Truncated message")?,
                });
            }
            if message::is_selection(id) {
                messages.extend(self.field().into_iter().map(|bytes| Sent { bytes, refresh: true }));
            }
            messages.push(Sent { bytes: message.to_vec(), refresh: false });
        }
        let errors = self.errors.borrow();
        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }
        let state = match (outcome, status) {
            (Some(outcome), _) => State::Over(Some(outcome)),
            (None, 0) => State::Over(None),
            (None, 1) => State::Awaiting,
            (None, _) => State::Running,
        };
        Ok(Step { messages, state })
    }

    /// Answer the selection the engine waits on.
    pub fn respond(&mut self, response: &[u8]) {
        unsafe { (self.core.respond)(self.handle, response.as_ptr(), response.len() as u32) }
    }

    /// The field refresh: both players' hands, zones, Graveyards, banished
    /// cards and Extra Decks as `MSG_UPDATE_DATA` messages, with current ATK,
    /// DEF and positions.  [`Duel::step`] puts it before every selection.
    pub fn field(&self) -> Vec<Vec<u8>> {
        let mut updates = Vec::new();
        for p in 0..2 {
            for loc in [2, 4, 8, 16, 32, 64] {
                let mut update = vec![msg::UPDATE_DATA, p, loc as u8];
                update.extend_from_slice(&self.location(p, loc));
                updates.push(update);
            }
        }
        updates
    }

    /// `OCG_DuelQueryLocation` for one player's pile or zone.
    fn location(&self, controller: u8, location: u32) -> Vec<u8> {
        self.query_location(query::RECOMMENDED, controller, location)
    }

    /// Query a complete pile or zone with the caller's requested fields.
    /// This contains hidden information; callers must apply their seat's visibility rules.
    pub fn query_location(&self, flags: u32, controller: u8, location: u32) -> Vec<u8> {
        let q = Query { flags, controller, location, sequence: 0, overlay: 0 };
        let mut length = 0;
        unsafe {
            let bytes = (self.core.query)(self.handle, &mut length, &q);
            if length == 0 {
                vec![]
            } else {
                std::slice::from_raw_parts(bytes, length as usize).to_vec()
            }
        }
    }

    /// Both Monster Zones as they stand.
    pub fn monsters(&self) -> Vec<Monster> {
        let mut out = Vec::new();
        for controller in 0..2u8 {
            let mut update = vec![msg::UPDATE_DATA, controller, 4];
            update.extend_from_slice(&self.location(controller, 4));
            if let Ok(Message::UpdateData { cards, .. }) = message::parse(&update) {
                for (sequence, card) in cards.iter().enumerate() {
                    if let Some(card) = card {
                        out.push(Monster {
                            controller,
                            sequence: sequence as u32,
                            code: card.code.unwrap_or(0),
                            position: card.position.unwrap_or(0),
                            attack: card.attack.unwrap_or(0),
                            defense: card.defense.unwrap_or(0),
                        });
                    }
                }
            }
        }
        out
    }

    /// Printed data of a card, from the duel's [`Core`].
    pub fn printed(&self, code: u32) -> Option<crate::Printed> {
        self.core.printed(code)
    }

    /// Printed Link arrows, including those of a card used as Xyz Material.
    pub fn link_markers(&self, code: u32) -> Option<u32> {
        self.core.link_markers(code)
    }

    fn extensions(&self) -> Result<&core::Extensions> {
        self.core.extensions.as_ref().ok_or_else(|| "this engine has no arena snapshots or no hidden-card swap".to_owned())
    }

    /// How many cards `player` has in `location` (`OCG_DuelQueryCount`).
    pub fn count(&self, player: u8, location: u32) -> Result<u32> {
        let count = self.extensions()?.count;
        Ok(unsafe { count(self.handle, player, location) })
    }

    /// `OCG_DuelQuery` for one card: the fields `flags` ask for, as the
    /// engine writes them; empty where there is no card.
    pub fn card(&self, flags: u32, controller: u8, location: u32, sequence: u32) -> Result<Vec<u8>> {
        let card = self.extensions()?.card;
        let q = Query { flags, controller, location, sequence, overlay: 0 };
        let mut length = 0;
        unsafe {
            let bytes = card(self.handle, &mut length, &q);
            Ok(if length == 0 { vec![] } else { std::slice::from_raw_parts(bytes, length as usize).to_vec() })
        }
    }

    /// Copy the duel's state.  Only while the engine waits for a response
    /// ([`State::Awaiting`]) and before one is given.
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        let extensions = self.extensions()?;
        let mut handle = ptr::null_mut();
        let status = unsafe { (extensions.snapshot)(self.handle, &mut handle) };
        if status != 0 {
            return Err(format!("OCG_DuelCreateSnapshot: {status}"));
        }
        Ok(Snapshot { handle, discard: extensions.discard })
    }

    /// Go back to a snapshot of this duel: the engine waits for the same
    /// response again.  Script errors logged since are forgotten.
    pub fn restore(&mut self, snapshot: &Snapshot) -> Result<()> {
        let restore = self.extensions()?.restore;
        let status = unsafe { restore(self.handle, snapshot.handle) };
        if status != 0 {
            return Err(format!("OCG_DuelRestoreSnapshot: {status}"));
        }
        self.errors.borrow_mut().clear();
        Ok(())
    }

    /// Exchange two hidden cards of `player`: each is a (location, sequence)
    /// in the Deck, the hand or the Spell & Trap Zone.  `false` when the
    /// engine refuses: an empty slot, the same slot twice, a card a link of
    /// the current chain refers to, a Spell & Trap Zone card that is not
    /// face-down, or a card that is neither Spell nor Trap landing in that
    /// zone.  Face-down monsters cannot be exchanged.
    pub fn swap(&mut self, player: u8, first: (u32, u32), second: (u32, u32)) -> Result<bool> {
        let swap = self.extensions()?.swap;
        Ok(unsafe { swap(self.handle, player, first.0, first.1, second.0, second.1) } != 0)
    }
}
