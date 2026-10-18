//! One duel on the engine, stepped by its caller.
use std::{
    cell::RefCell,
    ffi::{c_void, CString},
    ptr,
};

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
    /// Master Rule 1, as every duel of the bench is played.
    pub const MASTER_RULE_1: u64 = 0xD0700;
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
    byte_len: Option<u64>,
}

impl Snapshot {
    /// Bytes in the saved native arena image, cached when it was created.
    /// This excludes allocation capacity and snapshot metadata. `None` means
    /// the loaded core has no snapshot-size diagnostic.
    pub fn byte_len(&self) -> Option<u64> { self.byte_len }
}

/// Successful native arena copies observed while a duel's optional meter is
/// enabled. These are logical payload bytes, not measured memory bandwidth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotMetrics {
    /// Successful calls to [`Duel::snapshot`].
    pub snapshots: u64,
    /// Successful calls to [`Duel::restore`].
    pub restores: u64,
    /// Largest image copied in either direction during this period.
    pub max_snapshot_bytes: Option<u64>,
    /// Total bytes copied from the arena into newly created snapshots.
    pub snapshotted_bytes: Option<u64>,
    /// Total bytes copied from snapshots back into the arena.
    pub restored_bytes: Option<u64>,
}

impl SnapshotMetrics {
    fn new(sizes_available: bool) -> Self {
        let zero = sizes_available.then_some(0);
        Self { snapshots: 0, restores: 0, max_snapshot_bytes: zero,
            snapshotted_bytes: zero, restored_bytes: zero }
    }

    fn record(&mut self, bytes: Option<u64>, restoring: bool) {
        self.max_snapshot_bytes = self.max_snapshot_bytes.zip(bytes).map(|(old, bytes)| old.max(bytes));
        let total = if restoring {
            self.restores = self.restores.saturating_add(1);
            &mut self.restored_bytes
        } else {
            self.snapshots = self.snapshots.saturating_add(1);
            &mut self.snapshotted_bytes
        };
        *total = total.zip(bytes).and_then(|(old, bytes)| old.checked_add(bytes));
    }
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
    /// Made by this crate, and destroyed with it; not [`Duel::borrow`]ed.
    owned: bool,
    /// Script errors the engine logged since they were last looked at.
    errors: Box<RefCell<Vec<String>>>,
    life_points: [u32; 2],
    /// Cards put in each player's Deck and Extra Deck, for [`Duel::start_message`].
    piles: [[u16; 2]; 2],
    /// Host-side counters are deliberately outside the native snapshot.
    snapshot_metrics: Option<SnapshotMetrics>,
}

impl Drop for Duel<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { (self.core.destroy)(self.handle) }
        }
    }
}

const DECK: u32 = 0x01;
const EXTRA: u32 = 0x40;
const FACE_DOWN_DEFENSE: u32 = 0x8;

impl Core {
    /// Initial card-script declarations, keyed by decimal card code.
    ///
    /// Each value is an array of objects containing `category` (u64),
    /// `effect_type` (u16), `range` (u16), `event` (u32), `description` (u64),
    /// `flags` (u64), `has_cost` and `card_target`. Numeric layouts follow
    /// OCGCore's constants. `event` is the effect code, including a passive
    /// effect identifier for non-action effects.
    ///
    /// A separate, unstarted Master Rule 1 duel initializes each definition. No
    /// live duel, hidden card, activation condition or Lua effect operation is
    /// inspected. Results are sorted, deduplicated and cached by this core.
    /// These are static hints, not a claim that an effect is legal or useful;
    /// effects created later by an operation are intentionally absent.
    /// `has_cost` means a cost function was registered, not that every use of
    /// the effect consumes a card or Life Points.
    pub fn effect_profiles(&self, codes: &[u32]) -> Result<serde_json::Value> {
        if self.initial_effects.is_none() {
            return Err("This core has no initial-effect metadata bridge".into());
        }
        let mut codes = codes.to_vec();
        codes.sort_unstable();
        codes.dedup();
        let mut profiles = serde_json::Map::new();
        for code in codes {
            if self.printed(code).is_none() {
                return Err(format!("Profile card {code} is absent from the database"));
            }
            let cached = self.effect_profile_cache.borrow().get(&code).cloned();
            let value = match cached {
                Some(value) => value,
                None => {
                    let mut duel = self.duel(&DuelOptions::seeded(0))?;
                    let value = duel.initial_effect_profile(code)?;
                    self.effect_profile_cache.borrow_mut().insert(code, value.clone());
                    value
                }
            };
            profiles.insert(code.to_string(), value);
        }
        Ok(serde_json::Value::Object(profiles))
    }

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
            let duel = Duel { core: self, handle, owned: true, errors, life_points: options.life_points,
                piles: [[0; 2]; 2], snapshot_metrics: None };
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

impl<'a> Duel<'a> {
    /// A duel a host made on `core`'s engine, played through this one too:
    /// a search snapshots it, plays its worlds on it and restores it.  It is
    /// still the host's: dropping this leaves it be.  Its script errors go
    /// to the host's log handler, but in [`Duel::with_logs`]; and since the
    /// host started it, [`Duel::start_message`] knows nothing of it.
    ///
    /// # Safety
    /// `handle` is a live `OCG_Duel` of `core`'s engine.  It outlives this
    /// duel, and the host leaves it alone while this one steps it.
    pub unsafe fn borrow(core: &'a Core, handle: *mut c_void) -> Self {
        Duel { core, handle, owned: false, errors: Box::default(), life_points: [0; 2], piles: [[0; 2]; 2],
            snapshot_metrics: None }
    }

    /// The engine's `OCG_Duel`.
    pub fn handle(&self) -> *mut c_void {
        self.handle
    }

    /// Whether a search can play worlds on this duel: its engine has arena
    /// snapshots, the hidden-card swap and random reseeding, and, for a
    /// borrowed duel, can lend its log handler ([`Duel::with_logs`]).
    pub fn can_search(&self) -> bool {
        self.core.has_reseed()
            && (self.owned || self.core.extensions.as_ref().is_some_and(|extensions| extensions.swap_log.is_some()))
    }

    /// Run `f` with the engine logging to this duel, so that [`Duel::step`]
    /// returns what a script got wrong in `f` (and [`Duel::restore`] forgets
    /// it): a duel this crate made always does; a borrowed one only here,
    /// and logs to the host's handler again once `f` returns.
    pub fn with_logs<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> Result<T> {
        if self.owned {
            return Ok(f(self));
        }
        let swap = self.extensions()?.swap_log.ok_or("This engine cannot lend its log handler")?;
        /// Gives the host's handler back, even when `f` panics.
        struct Lent {
            swap: unsafe extern "C" fn(Handle, *mut core::LogHandler, *mut Handle) -> i32,
            duel: Handle,
            handler: core::LogHandler,
            payload: Handle,
        }
        impl Drop for Lent {
            fn drop(&mut self) {
                unsafe { (self.swap)(self.duel, &mut self.handler, &mut self.payload) };
            }
        }
        let mut lent = Lent {
            swap,
            duel: self.handle,
            handler: core::log,
            payload: (&*self.errors) as *const RefCell<Vec<String>> as Handle,
        };
        if unsafe { swap(self.handle, &mut lent.handler, &mut lent.payload) } != 1 {
            std::mem::forget(lent);
            return Err("The engine would not lend its log handler".into());
        }
        Ok(f(self))
    }

    // Private: exposing this on a live duel would mutate it and would also
    // allow an accidental dependency on the live field during initialization.
    fn initial_effect_profile(&mut self, code: u32) -> Result<serde_json::Value> {
        unsafe extern "C" fn append(output: Handle, effect: *const core::InitialEffect) {
            (*(output as *mut Vec<core::InitialEffect>)).push(*effect);
        }
        let inspect = self.core.initial_effects.ok_or("No initial-effect metadata bridge")?;
        let mut effects = Vec::<core::InitialEffect>::new();
        let status = unsafe {
            inspect(self.handle, code, &mut effects as *mut _ as Handle, append)
        };
        let errors = std::mem::take(&mut *self.errors.borrow_mut());
        if !errors.is_empty() {
            return Err(format!("Initializing profile {code}: {}", errors.join("; ")));
        }
        if status != 1 {
            return Err(format!("Initializing profile {code} failed"));
        }
        effects.sort_unstable();
        effects.dedup();
        Ok(serde_json::Value::Array(effects.into_iter().map(|effect| {
            serde_json::json!({
                "category": effect.category,
                "effect_type": effect.effect_type,
                "range": effect.range,
                "event": effect.event,
                "description": effect.description,
                "flags": effect.flags,
                "has_cost": effect.has_cost != 0,
                "card_target": effect.card_target != 0,
            })
        }).collect()))
    }

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
        let mut updates = Vec::with_capacity(12);
        for p in 0..2 {
            for loc in [2, 4, 8, 16, 32, 64] {
                updates.push(self.location_message(p, loc));
            }
        }
        updates
    }

    /// Copy the engine-owned query buffer straight into its final message.
    fn location_message(&self, controller: u8, location: u32) -> Vec<u8> {
        let q = Query { flags: query::RECOMMENDED, controller, location, sequence: 0, overlay: 0 };
        let mut length = 0;
        let bytes = unsafe { (self.core.query)(self.handle, &mut length, &q) };
        let mut message = Vec::with_capacity(3 + length as usize);
        message.extend_from_slice(&[msg::UPDATE_DATA, controller, location as u8]);
        if length != 0 {
            // OCG_DuelQueryLocation owns these bytes until the next query or
            // engine mutation. Copy now, without another native call, and do
            // not expose a borrow of that buffer. Zero length may be null.
            unsafe { message.extend_from_slice(std::slice::from_raw_parts(bytes, length as usize)) };
        }
        message
    }

    /// `OCG_DuelQueryLocation` for one player's pile or zone.
    fn location(&self, controller: u8, location: u32) -> Vec<u8> {
        let q = Query { flags: query::RECOMMENDED, controller, location, sequence: 0, overlay: 0 };
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

    /// The engine this duel is played on.
    pub fn core(&self) -> &Core {
        self.core
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

    /// Current native arena high-water usage, including space retained from
    /// freed allocations. This is not the arena's reserved capacity or a
    /// count of live objects. `None` means this diagnostic is unavailable.
    pub fn arena_used_bytes(&self) -> Option<u64> {
        self.core.extensions.as_ref()?.arena_used.map(|used| unsafe { used(self.handle) })
    }

    fn empty_snapshot_metrics(&self) -> SnapshotMetrics {
        SnapshotMetrics::new(self.core.extensions.as_ref().is_some_and(|extensions| extensions.snapshot_size.is_some()))
    }

    /// Enable the host-side arena-copy meter, initially disabled. Enabling an
    /// already enabled meter preserves its counters; disabling discards them.
    /// The meter performs no allocation, timing or native queries on restore.
    pub fn enable_snapshot_metrics(&mut self, enabled: bool) {
        if !enabled { self.snapshot_metrics = None; }
        else if self.snapshot_metrics.is_none() { self.snapshot_metrics = Some(self.empty_snapshot_metrics()); }
    }

    /// Start a fresh counter period if the meter is enabled. Missing native
    /// size diagnostics remain `None`, rather than being reported as zero.
    pub fn reset_snapshot_metrics(&mut self) {
        if self.snapshot_metrics.is_some() { self.snapshot_metrics = Some(self.empty_snapshot_metrics()); }
    }

    /// Read and reset this period, leaving the meter enabled. `None` means the
    /// meter is disabled. Individual byte fields are `None` when unavailable;
    /// totals also become `None` on overflow. Failed operations are excluded.
    pub fn take_snapshot_metrics(&mut self) -> Option<SnapshotMetrics> {
        let empty = self.empty_snapshot_metrics();
        self.snapshot_metrics.as_mut().map(|metrics| std::mem::replace(metrics, empty))
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
        let snapshot = Snapshot { handle, discard: extensions.discard,
            byte_len: extensions.snapshot_size.map(|size| unsafe { size(handle) }) };
        if let Some(metrics) = &mut self.snapshot_metrics { metrics.record(snapshot.byte_len, false); }
        Ok(snapshot)
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
        if let Some(metrics) = &mut self.snapshot_metrics { metrics.record(snapshot.byte_len, true); }
        Ok(())
    }

    /// Replace future engine randomness on a simulation branch. Take a
    /// snapshot first and restore it before continuing the live game.
    /// Does not reorder existing decks; hidden-card sampling is separate.
    pub fn reseed(&mut self, seed: u64) -> Result<()> {
        let reseed = self.extensions()?.reseed.ok_or("this engine cannot reseed simulated chance events")?;
        if unsafe { reseed(self.handle, seed) } != 1 {
            return Err("engine refused to reseed simulated chance events".into());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};
    use ygo_policies::{cards::CardDatabase, model::ChoiceKind, registry};
    use ygo_policies_ocgcore::{redact::redact, wire::position, Seat, SqliteCards};

    fn core() -> Core {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        Core::open(None, &root.join("vendor/BabelCdb/cards.cdb"), &root.join("vendor/CardScripts")).unwrap()
    }

    fn snapshot_boundary(core: &Core) -> Duel<'_> {
        let options = DuelOptions { hand: 0, per_turn: 0, ..DuelOptions::seeded(2493302) };
        let mut duel = core.duel(&options).unwrap();
        duel.add(Placed { controller: 0, code: 5053103, location: 2, sequence: 0,
            position: position::FACEDOWN_DEFENSE });
        duel.start();
        for _ in 0..16 {
            let step = duel.step().unwrap();
            if step.state == State::Awaiting {
                assert!(step.messages.iter().any(|sent| matches!(message::parse(&sent.bytes),
                    Ok(Message::SelectIdle(idle)) if !idle.summon.is_empty())));
                return duel;
            }
            assert_eq!(step.state, State::Running);
        }
        panic!("snapshot fixture never reached its first summon selection");
    }

    #[test]
    fn snapshot_metrics_count_exact_native_images_without_changing_continuation() {
        let core = core();
        let mut duel = snapshot_boundary(&core);
        let root = duel.snapshot().unwrap();
        let bytes = root.byte_len().unwrap();
        assert!(bytes > 0);
        assert_eq!(duel.arena_used_bytes(), Some(bytes));
        assert_eq!(duel.take_snapshot_metrics(), None, "the meter is disabled by default");
        let summon = 0i32.to_le_bytes();
        duel.respond(&summon);
        let expected = duel.step().unwrap();
        assert!(duel.arena_used_bytes().unwrap() >= bytes);
        duel.restore(&root).unwrap();
        assert_eq!(duel.arena_used_bytes(), Some(bytes));
        assert_eq!(duel.take_snapshot_metrics(), None);

        duel.enable_snapshot_metrics(true);
        let copy = duel.snapshot().unwrap();
        assert_eq!(copy.byte_len(), Some(bytes));
        // Owner errors and unsafe-boundary failures perform no successful copy.
        let mut other = snapshot_boundary(&core);
        let foreign = other.snapshot().unwrap();
        assert!(duel.restore(&foreign).is_err());
        duel.respond(&summon);
        assert!(duel.snapshot().is_err());
        let actual = duel.step().unwrap();
        assert_eq!(actual.messages, expected.messages);
        assert_eq!(actual.state, expected.state);
        duel.restore(&copy).unwrap();
        duel.restore(&root).unwrap();
        // The root predates enabling the meter; its cached size is still valid.
        assert_eq!(duel.take_snapshot_metrics(), Some(SnapshotMetrics {
            snapshots: 1, restores: 2, max_snapshot_bytes: Some(bytes),
            snapshotted_bytes: Some(bytes), restored_bytes: Some(2 * bytes),
        }));
        assert_eq!(duel.take_snapshot_metrics(), Some(SnapshotMetrics::new(true)));
        duel.restore(&root).unwrap();
        duel.enable_snapshot_metrics(true);
        assert_eq!(duel.take_snapshot_metrics().unwrap().restores, 1,
            "enabling twice preserves the current period");
        duel.restore(&root).unwrap();
        duel.reset_snapshot_metrics();
        assert_eq!(duel.take_snapshot_metrics(), Some(SnapshotMetrics::new(true)));
        duel.enable_snapshot_metrics(false);
        duel.restore(&root).unwrap();
        let _ = duel.snapshot().unwrap();
        duel.reset_snapshot_metrics();
        assert_eq!(duel.take_snapshot_metrics(), None);
        duel.respond(&summon);
        let unmetered = duel.step().unwrap();
        assert_eq!(unmetered.messages, expected.messages);
        assert_eq!(unmetered.state, expected.state);
    }

    #[test]
    fn snapshot_metrics_optional_getters_are_independent_and_restore_uses_cached_size() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SIZE_CALLS: AtomicU64 = AtomicU64::new(0);
        unsafe extern "C" fn counted_size(_: Handle) -> u64 {
            SIZE_CALLS.fetch_add(1, Ordering::Relaxed);
            1234
        }
        let mut core = core();
        let native = core.extensions.unwrap();
        for arena in [false, true] {
            for size in [false, true] {
                let extensions = core.extensions.as_mut().unwrap();
                extensions.arena_used = if arena { native.arena_used } else { None };
                extensions.snapshot_size = size.then_some(counted_size);
                assert!(core.has_snapshots(), "missing diagnostics must not disable search");
                let mut duel = snapshot_boundary(&core);
                assert_eq!(duel.arena_used_bytes().is_some(), arena);
                duel.enable_snapshot_metrics(true);
                SIZE_CALLS.store(0, Ordering::Relaxed);
                let snapshot = duel.snapshot().unwrap();
                assert_eq!(snapshot.byte_len(), size.then_some(1234));
                for _ in 0..3 { duel.restore(&snapshot).unwrap(); }
                assert_eq!(SIZE_CALLS.load(Ordering::Relaxed), u64::from(size),
                    "the optional native size getter is called once per image, never per restore");
                assert_eq!(duel.take_snapshot_metrics(), Some(SnapshotMetrics {
                    snapshots: 1, restores: 3, max_snapshot_bytes: size.then_some(1234),
                    snapshotted_bytes: size.then_some(1234), restored_bytes: size.then_some(3702),
                }));
            }
        }
    }

    #[test]
    fn snapshot_metrics_unavailable_or_overflowed_bytes_never_become_zero() {
        let mut metrics = SnapshotMetrics::new(true);
        metrics.record(Some(u64::MAX), false);
        metrics.record(Some(1), false);
        assert_eq!(metrics.snapshotted_bytes, None);
        assert_eq!(metrics.max_snapshot_bytes, Some(u64::MAX));
        metrics.record(None, true);
        metrics.record(Some(42), true);
        assert_eq!(metrics.max_snapshot_bytes, None);
        assert_eq!(metrics.restored_bytes, None);
        assert_eq!((metrics.snapshots, metrics.restores), (2, 2));
    }

    /// The original two-copy field transport, kept independent of the new
    /// message helper so the native fixture compares both implementations.
    fn legacy_field(duel: &Duel<'_>) -> Vec<Vec<u8>> {
        let mut updates = Vec::new();
        for player in 0..2 {
            for location in [2, 4, 8, 16, 32, 64] {
                let mut update = vec![msg::UPDATE_DATA, player, location as u8];
                update.extend_from_slice(&duel.location(player, location));
                updates.push(update);
            }
        }
        updates
    }

    #[test]
    fn field_transport_preserves_empty_and_malformed_native_payloads() {
        unsafe extern "C" fn query_payload(_: Handle, length: *mut u32, query: *const Query) -> *const u8 {
            let payload: &'static [u8] = match (*query).location {
                2 => &[],
                4 => &[1],
                8 => &[8, 0, 0, 0, 4],
                16 => &[0, 0, 0, 0],
                32 => &[2, 0, 0, 0, 0, 0],
                _ => &[4, 0, 0, 0, 1, 2, 3, 4],
            };
            *length = payload.len() as u32;
            if payload.is_empty() { ptr::null() } else { payload.as_ptr() }
        }
        let mut core = core();
        core.query = query_payload;
        let duel = core.duel(&DuelOptions::seeded(2493300)).unwrap();
        duel.errors.borrow_mut().push("existing script error".to_owned());
        let expected = legacy_field(&duel);
        let actual = duel.field();
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 12);
        assert_eq!(actual[0], [msg::UPDATE_DATA, 0, 2]);
        assert!(actual.iter().filter(|bytes| message::parse(bytes).is_err()).count() >= 6);
        assert_eq!(&*duel.errors.borrow(), &["existing script error"]);
    }

    #[test]
    fn native_field_messages_match_legacy_with_hidden_cards_and_counters() {
        const BREAKER: u32 = 71413901;
        const MONSTER: u32 = 5053103;
        const SPELL: u32 = 5318639;
        const BARRIER: u32 = 53239672;
        const FUSION: u32 = 23995346;
        let core = core();
        let options = DuelOptions { hand: 0, per_turn: 0, ..DuelOptions::seeded(2493301) };
        let mut duel = core.duel(&options).unwrap();
        for controller in 0..2 {
            for (code, location, sequence, position) in [
                (MONSTER, 1, 0, position::FACEDOWN_DEFENSE),
                (SPELL, 2, 0, position::FACEDOWN_DEFENSE),
                (MONSTER, 4, 0, position::FACEUP_ATTACK),
                (MONSTER, 4, 1, position::FACEDOWN_DEFENSE),
                (BARRIER, 8, 0, position::FACEUP_ATTACK),
                (SPELL, 8, 1, position::FACEDOWN_DEFENSE),
                (MONSTER, 16, 0, position::FACEUP_ATTACK),
                (MONSTER, 32, 0, position::FACEUP_ATTACK),
                (MONSTER, 32, 1, position::FACEDOWN_DEFENSE),
                (FUSION, 64, 0, position::FACEDOWN_DEFENSE),
            ] {
                duel.add(Placed { controller, code, location, sequence, position });
            }
        }
        duel.add(Placed { controller: 0, code: BREAKER, location: 2, sequence: 1, position: position::FACEDOWN_DEFENSE });
        duel.start();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let db: Arc<dyn CardDatabase> = Arc::new(SqliteCards::open(root.join("vendor/BabelCdb/cards.cdb")).unwrap());
        let mut seats = [0, 1].map(|player| {
            let mut seat = Seat::new(registry::create_seeded("generic", db.clone(), 2493301).unwrap(), db.clone(), None);
            seat.enable_response_overrides().unwrap();
            seat.feed(&duel.start_message(player)).unwrap();
            seat
        });
        let mut summoned = false;
        for _ in 0..32 {
            let step = duel.step().unwrap();
            let mut pending = None;
            for sent in &step.messages {
                assert_ne!(sent.bytes.first(), Some(&msg::RETRY));
                for (player, seat) in seats.iter_mut().enumerate() {
                    if let Some(response) = seat.feed(&sent.bytes).unwrap() {
                        assert!(pending.replace((player, response)).is_none());
                    }
                }
            }
            assert!(!matches!(step.state, State::Over(_)));
            let Some((player, mut response)) = pending else {
                assert_ne!(step.state, State::Awaiting);
                continue;
            };
            let snapshot = duel.snapshot().unwrap();
            let expected = legacy_field(&duel);
            duel.restore(&snapshot).unwrap();
            let actual = duel.field();
            assert_eq!(actual, expected);
            assert_eq!(actual.len(), 12);
            for (index, bytes) in actual.iter().enumerate() {
                assert_eq!(&bytes[..3], &[msg::UPDATE_DATA, (index / 6) as u8, [2, 4, 8, 16, 32, 64][index % 6]]);
            }
            let cards = actual.iter().flat_map(|bytes| match message::parse(bytes).unwrap() {
                Message::UpdateData { player, location, cards } => cards.into_iter().flatten()
                    .map(move |card| (player, location, card)).collect::<Vec<_>>(),
                _ => unreachable!(),
            }).collect::<Vec<_>>();
            if cards.iter().any(|(_, _, card)| card.code == Some(BREAKER) && card.counters == Some(1)) {
                assert!(summoned);
                let breaker = &cards.iter().find(|(_, _, card)| card.code == Some(BREAKER)).unwrap().2;
                assert_eq!(breaker.attack, Some(1900));
                assert_eq!(breaker.is_public, Some(true));
                assert!(cards.iter().any(|(_, _, card)| card.is_public == Some(false)));
                for owner in 0..2 {
                    for location in [2, 4, 8, 16, 32, 64] {
                        assert!(cards.iter().any(|(player, loc, _)| *player == owner && *loc == location));
                    }
                }
                for viewer in [Some(0), Some(1), None] {
                    for (before, after) in expected.iter().zip(&actual) {
                        assert_eq!(redact(before, viewer).unwrap(), redact(after, viewer).unwrap());
                        let redacted = redact(after, viewer).unwrap().unwrap();
                        if let Message::UpdateData { player, location: 2, cards } = message::parse(&redacted).unwrap() {
                            assert!(!cards.is_empty());
                            for card in cards.into_iter().flatten() {
                                assert_eq!(card.code.filter(|code| *code != 0).is_some(), viewer == Some(player));
                            }
                        }
                    }
                }
                // The same query calls also leave the same native continuation:
                // no reference to its shared query buffer escaped either path.
                duel.respond(&response);
                let optimized_next = duel.step().unwrap();
                duel.restore(&snapshot).unwrap();
                assert_eq!(legacy_field(&duel), expected);
                duel.respond(&response);
                let legacy_next = duel.step().unwrap();
                assert_eq!(optimized_next.messages, legacy_next.messages);
                assert_eq!(optimized_next.state, legacy_next.state);
                return;
            }
            let answer = seats[player].last_answer().unwrap();
            let choice = if !summoned && player == 0 {
                answer.decision.choices.iter().position(|choice| choice.kind == ChoiceKind::NormalSummon && choice.code() == Some(BREAKER))
            } else {
                answer.decision.choices.iter().position(|choice| choice.kind == ChoiceKind::Pass)
            };
            if let Some(index) = choice {
                response = answer.responses[index].clone();
                if answer.decision.choices[index].kind == ChoiceKind::NormalSummon {
                    summoned = true;
                }
                seats[player].override_response(&response).unwrap();
            }
            duel.respond(&response);
        }
        panic!("Breaker never gained its native Spell Counter");
    }

    /// A script the engine refuses: it logs why, as an error.
    fn break_a_script(duel: &Duel) {
        let source = b"this is not Lua";
        let loaded = unsafe {
            (duel.core.resources.load)(duel.handle, source.as_ptr(), source.len() as u32, c"broken.lua".as_ptr())
        };
        assert_eq!(loaded, 0);
    }

    #[test]
    fn a_borrowed_duel_keeps_script_errors_to_itself_only_while_it_lends_the_log() {
        let core = core();
        let mut host = snapshot_boundary(&core);
        let mut borrowed = unsafe { Duel::borrow(&core, host.handle()) };
        assert!(borrowed.can_search());
        borrowed.with_logs(|duel| break_a_script(duel)).unwrap();
        assert_eq!(borrowed.errors.borrow().len(), 1, "the error goes to the duel that lent the log");
        assert!(host.errors.borrow().is_empty());
        break_a_script(&borrowed);
        assert_eq!(borrowed.errors.borrow().len(), 1, "and only while it does");
        assert_eq!(host.errors.borrow().len(), 1);
        // A panic gives the host's handler back too.
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            borrowed.with_logs(|_| panic!("a search failed")).unwrap();
        }));
        assert!(unwound.is_err());
        break_a_script(&borrowed);
        assert_eq!(host.errors.borrow().len(), 2);
        // Dropping the borrowed duel leaves the host's to play on.
        drop(borrowed);
        assert!(host.snapshot().is_ok());
    }
}
