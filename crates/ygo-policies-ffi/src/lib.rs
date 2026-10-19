//! C ABI: one handle per seat, fed OCGCore messages, answering with OCGCore
//! responses.  See `include/ygo_policies.h` for the contract.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::json;
use ygo_policies::cards::CardDatabase;
use ygo_policies::model::{DecisionKind, Location};
use ygo_policies::registry;
use ygo_policies_duel::{Core, Duel};
use ygo_policies_ocgcore::{wire, Seat, SqliteCards};
use ygo_policies_search::local::answer_json;
use ygo_policies_search::{LocalPilots, PilotSeat, SearchOptions, SearchSeat};

pub struct YgoPolicy {
    player: Player,
    response: Vec<u8>,
    json: CString,
}

/// What answers for a handle: a policy's seat, or the staged search over the
/// generic policy's (`ygo_policy_create_search`).
enum Player {
    Plain(Seat),
    Search(Box<SearchSeat<'static, LocalPilots>>),
}

impl Player {
    /// The seat whose answers these are, to read: the policy's own, or the
    /// search's pilot.
    fn read<T>(&self, read: impl FnOnce(&Seat) -> T) -> T {
        match self {
            Player::Plain(seat) => read(seat),
            Player::Search(search) => read(&search.pilot().seat()),
        }
    }

    fn feed(&mut self, message: &[u8]) -> Result<Option<Vec<u8>>, String> {
        match self {
            Player::Plain(seat) => seat.feed(message).map_err(|e| e.to_string()),
            Player::Search(search) => search.feed(message),
        }
    }
}

impl YgoPolicy {
    fn handle(player: Player) -> *mut YgoPolicy {
        Box::into_raw(Box::new(YgoPolicy { player, response: Vec::new(), json: CString::default() }))
    }

    /// Plain seats only: the search keeps its pilot's to itself.
    fn plain(&mut self) -> Result<&mut Seat, String> {
        match &mut self.player {
            Player::Plain(seat) => Ok(seat),
            Player::Search(_) => Err("not for a search seat".into()),
        }
    }
}

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_error(message: impl Into<String>) {
    let message = CString::new(message.into().replace('\0', " ")).unwrap_or_default();
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(message));
}

/// Engines are loaded once per path, card database and thread, and kept: a
/// search seat plays on its duel's engine for as long as it lives, reading
/// printed cards from the same database as its policies.
fn engine(path: &str, cards: &str) -> Result<&'static Core, String> {
    thread_local! {
        static ENGINES: RefCell<HashMap<(PathBuf, PathBuf), &'static Core>> = RefCell::new(HashMap::new());
    }
    let canonical = |path: &str| std::fs::canonicalize(path).map_err(|e| format!("{path}: {e}"));
    let key = (canonical(path)?, canonical(cards)?);
    ENGINES.with(|engines| {
        if let Some(core) = engines.borrow().get(&key) {
            return Ok(*core);
        }
        let core = Core::load(&key.0, &key.1).map_err(|e| format!("{path}: {e}"))?;
        let core: &'static Core = Box::leak(Box::new(core));
        engines.borrow_mut().insert(key, core);
        Ok(core)
    })
}

/// Card databases are large and immutable: load each path once per process.
fn cards(path: &str) -> Result<Arc<dyn CardDatabase>, String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<SqliteCards>>>> = OnceLock::new();
    let key = std::fs::canonicalize(path).map_err(|e| format!("{path}: {e}"))?;
    let mut cache = CACHE.get_or_init(Default::default).lock().map_err(|_| "card cache poisoned")?;
    if let Some(db) = cache.get(&key) {
        return Ok(db.clone());
    }
    let db = Arc::new(SqliteCards::open(&key).map_err(|e| format!("{path}: {e}"))?);
    cache.insert(key, db.clone());
    Ok(db)
}

unsafe fn text<'a>(ptr: *const c_char, name: &str) -> Result<&'a str, String> {
    if ptr.is_null() {
        return Err(format!("{name} is NULL"));
    }
    CStr::from_ptr(ptr).to_str().map_err(|e| format!("{name}: {e}"))
}

unsafe fn bytes<'a>(ptr: *const u8, length: usize) -> Result<&'a [u8], String> {
    if ptr.is_null() {
        return if length == 0 { Ok(&[]) } else { Err("message is NULL".into()) };
    }
    Ok(std::slice::from_raw_parts(ptr, length))
}

unsafe fn handle<'a>(policy: *mut YgoPolicy) -> Result<&'a mut YgoPolicy, String> {
    policy.as_mut().ok_or_else(|| "policy is NULL".to_string())
}

fn guarded<T>(fallback: T, body: impl FnOnce() -> Result<T, String>) -> T {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(value)) => value,
        Ok(Err(message)) => {
            set_error(message);
            fallback
        }
        Err(_) => {
            set_error("policy panicked");
            fallback
        }
    }
}

fn store_json(policy: &mut YgoPolicy, value: serde_json::Value) -> *const c_char {
    policy.json = CString::new(value.to_string()).unwrap_or_default();
    policy.json.as_ptr()
}

/// # Safety
/// `policy_id` and `cards_cdb_path` must be NUL-terminated strings.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_create(
    policy_id: *const c_char,
    cards_cdb_path: *const c_char,
    seat: i32,
) -> *mut YgoPolicy {
    ygo_policy_create_seeded(policy_id, cards_cdb_path, seat, 0)
}

/// Like `ygo_policy_create`; `seed` seeds the policy's tie-breaking, so equally
/// good choices (which face-down card to destroy, which zone to use) are random
/// but reproducible: the same seed and message stream give the same answers.
///
/// # Safety
/// `policy_id` and `cards_cdb_path` must be NUL-terminated strings.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_create_seeded(
    policy_id: *const c_char,
    cards_cdb_path: *const c_char,
    seat: i32,
    seed: u64,
) -> *mut YgoPolicy {
    guarded(std::ptr::null_mut(), || {
        let id = text(policy_id, "policy_id")?;
        let seat = match seat {
            -1 => None,
            0 | 1 => Some(seat as u8),
            other => return Err(format!("seat must be 0, 1 or -1, got {other}")),
        };
        let db = cards(text(cards_cdb_path, "cards_cdb_path")?)?;
        let policy = registry::create_seeded(id, db.clone(), seed).ok_or_else(|| format!("unknown policy {id:?}"))?;
        Ok(YgoPolicy::handle(Player::Plain(Seat::new(policy, db, seat))))
    })
}

/// The generic policy with the staged search, seated as `seat` of the host's
/// `duel`, on which it plays its worlds while it answers.  It describes its
/// own cards' effects to its pilot with the stored profiles.
///
/// # Safety
/// `cards_cdb_path` and `engine_path` are NUL-terminated strings, and so is
/// `options_json` unless it is NULL.  `duel` is a live `OCG_Duel` made by the
/// engine library at `engine_path`; it outlives the handle, and the host
/// leaves it alone while the handle is fed.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_create_search(
    cards_cdb_path: *const c_char,
    seat: i32,
    seed: u64,
    engine_path: *const c_char,
    duel: *mut c_void,
    options_json: *const c_char,
) -> *mut YgoPolicy {
    guarded(std::ptr::null_mut(), || {
        let seat = u8::try_from(seat).ok().filter(|seat| *seat < 2).ok_or(format!("seat must be 0 or 1, got {seat}"))?;
        if duel.is_null() {
            return Err("duel is NULL".into());
        }
        let cards_cdb_path = text(cards_cdb_path, "cards_cdb_path")?;
        let db = cards(cards_cdb_path)?;
        let core = engine(text(engine_path, "engine_path")?, cards_cdb_path)?;
        let options = match options_json.is_null() {
            true => SearchOptions::MEASURED,
            false => SearchOptions::from_json(
                &serde_json::from_str(text(options_json, "options_json")?).map_err(|e| format!("search options: {e}"))?,
            )?,
        };
        let duel = Rc::new(RefCell::new(Duel::borrow(core, duel)));
        let search = SearchSeat::new(duel, LocalPilots { cards: db }, seat, seed, options)?;
        Ok(YgoPolicy::handle(Player::Search(Box::new(search))))
    })
}

/// Independently copy a seat, preserving policy memory and random state.
/// # Safety
/// `policy` is a live handle and is not mutated concurrently.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_clone(policy: *const YgoPolicy) -> *mut YgoPolicy {
    guarded(std::ptr::null_mut(), || {
        let policy = policy.as_ref().ok_or("policy is NULL")?;
        let Player::Plain(seat) = &policy.player else { return Err("a search seat cannot be copied".into()) };
        let seat = seat.fork().ok_or("policy does not support copying")?;
        Ok(Box::into_raw(Box::new(YgoPolicy {
            // Getter output is disposable scratch, regenerated from the seat
            // on every JSON request; search forks need not copy that buffer.
            player: Player::Plain(seat), response: policy.response.clone(), json: CString::default(),
        })))
    })
}

/// Enable state-safe replacement of future responses. Call before the first
/// selection prompt. Ordinary seats do not pay for pre-choice snapshots.
/// # Safety
/// `policy` is a live handle and is not used concurrently.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_enable_response_overrides(policy: *mut YgoPolicy) -> i32 {
    guarded(-1, || {
        // A search seat's pilot takes them from the start.
        if let Player::Plain(seat) = &mut handle(policy)?.player {
            seat.enable_response_overrides().map_err(|e| e.to_string())?;
        }
        Ok(0)
    })
}

/// Stop future override checkpoints after the final searched response has been
/// committed. Keeps the committed policy, RNG and pending response unchanged.
/// Search tails using this API must reject MSG_RETRY rather than feed it.
/// # Safety
/// `policy` is a live handle and is not used concurrently.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_disable_response_overrides(policy: *mut YgoPolicy) -> i32 {
    guarded(-1, || {
        handle(policy)?.plain()?.disable_response_overrides();
        Ok(0)
    })
}

/// Install public effect-definition metadata before the first decision.
/// # Safety
/// `policy` is live; `profiles_json` is a NUL-terminated JSON object keyed by code.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_set_effect_profiles_json(policy: *mut YgoPolicy, profiles_json: *const c_char) -> i32 {
    guarded(-1, || {
        let profiles = text(profiles_json, "profiles_json")?;
        match &mut handle(policy)?.player {
            Player::Plain(seat) => {
                let profiles = serde_json::from_str(profiles).map_err(|e| format!("effect profiles: {e}"))?;
                seat.set_effect_profiles(profiles).map_err(|e| e.to_string())?;
            }
            Player::Search(search) => {
                search.set_effect_profiles(serde_json::from_str(profiles).map_err(|e| format!("effect profiles: {e}"))?)?;
            }
        }
        Ok(0)
    })
}

/// Replace the pending answer and commit its action to the policy's memory.
/// Returns 0 on success and -1 on error; errors preserve the prior response.
/// # Safety
/// `policy` is live; `response` points to `length` initialized bytes.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_override_response(policy: *mut YgoPolicy, response: *const u8, length: usize) -> i32 {
    guarded(-1, || {
        let policy = handle(policy)?;
        let response = bytes(response, length)?;
        match &mut policy.player {
            Player::Plain(seat) => seat.override_response(response).map_err(|e| e.to_string())?,
            // The answer played in the search's place, for its pilot to remember.
            Player::Search(search) => search.pilot().override_response(response)?,
        }
        policy.response = response.to_vec();
        Ok(0)
    })
}

/// Expand legal sequential alternatives and return the updated answer JSON.
/// At most twelve initial candidates are attempted. No engine state is read.
/// # Safety
/// `policy` is a live handle and is not used concurrently.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_alternative_responses_json(policy: *mut YgoPolicy, maximum: usize) -> *const c_char {
    guarded(std::ptr::null(), || {
        let policy = handle(policy)?;
        let value = match &mut policy.player {
            Player::Plain(seat) => {
                seat.alternative_responses(maximum).map_err(|e| e.to_string())?;
                answer_json(seat)
            }
            Player::Search(search) => search.pilot().alternative_responses(maximum)?,
        };
        Ok(store_json(policy, value))
    })
}

/// # Safety
/// `policy` comes from `ygo_policy_create`; `message` points to `length` bytes.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_feed(policy: *mut YgoPolicy, message: *const u8, length: usize) -> i32 {
    guarded(-1, || {
        let policy = handle(policy)?;
        let answer = policy.player.feed(bytes(message, length)?)?;
        Ok(answer.map_or(0, |response| {
            policy.response = response;
            1
        }))
    })
}

/// Decode a card update once and apply it through each seat's visibility boundary.
/// # Safety
/// Both handles are live, distinct, and from this library. `message` holds `length` bytes.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_feed_update_pair(first: *mut YgoPolicy, second: *mut YgoPolicy, message: *const u8, length: usize) -> i32 {
    guarded(-1, || {
        if first == second { return Err("update pair requires distinct handles".into()); }
        let (first, second) = (handle(first)?, handle(second)?);
        let message = bytes(message, length)?;
        match (&mut first.player, &mut second.player) {
            (Player::Plain(first), Player::Plain(second)) => {
                Seat::feed_update_pair(first, second, message).map_err(|e| e.to_string())?;
            }
            (first, second) => {
                first.feed(message)?;
                second.feed(message)?;
            }
        }
        Ok(0)
    })
}

/// # Safety
/// `policy` comes from `ygo_policy_create`; `buffer` points to `length` bytes.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_feed_buffer(policy: *mut YgoPolicy, buffer: *const u8, length: usize) -> i32 {
    guarded(-1, || {
        let policy = handle(policy)?;
        let buffer = bytes(buffer, length)?;
        let answer = match &mut policy.player {
            Player::Plain(seat) => seat.feed_buffer(buffer).map_err(|e| e.to_string())?,
            Player::Search(search) => {
                let mut answer = None;
                for message in wire::split_messages(buffer).map_err(|e| e.to_string())? {
                    answer = search.feed(message)?.or(answer);
                }
                answer
            }
        };
        Ok(answer.map_or(0, |response| {
            policy.response = response;
            1
        }))
    })
}

/// # Safety
/// `policy` comes from `ygo_policy_create`; `length` may be NULL.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_response(policy: *const YgoPolicy, length: *mut usize) -> *const u8 {
    let Some(policy) = policy.as_ref() else { return std::ptr::null() };
    if let Some(length) = length.as_mut() {
        *length = policy.response.len();
    }
    policy.response.as_ptr()
}

/// # Safety
/// `policy` comes from `ygo_policy_create`.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_seat(policy: *const YgoPolicy) -> i32 {
    policy.as_ref().and_then(|p| p.player.read(Seat::seat)).map_or(-1, i32::from)
}

/// # Safety
/// `policy` comes from `ygo_policy_create`.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_observation_json(policy: *mut YgoPolicy) -> *const c_char {
    guarded(std::ptr::null(), || {
        let policy = handle(policy)?;
        let value = policy.player.read(|seat| serde_json::to_value(seat.observation())).map_err(|e| e.to_string())?;
        Ok(store_json(policy, value))
    })
}

/// # Safety
/// `policy` comes from `ygo_policy_create`.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_last_answer_json(policy: *mut YgoPolicy) -> *const c_char {
    guarded(std::ptr::null(), || {
        let policy = handle(policy)?;
        let value = policy.player.read(answer_json);
        Ok(store_json(policy, value))
    })
}

/// Compact alternatives and situation for search, without serializing the full
/// observation. Unsupported decision kinds return JSON null.
/// # Safety
/// `policy` comes from `ygo_policy_create`.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_search_view_json(policy: *mut YgoPolicy) -> *const c_char {
    guarded(std::ptr::null(), || {
        let policy = handle(policy)?;
        let value = policy.player.read(|seat| match seat.last_answer() {
            Some(answer) if matches!(answer.decision.kind,
                DecisionKind::Idle | DecisionKind::Battle | DecisionKind::YesNo
                | DecisionKind::Position | DecisionKind::Option | DecisionKind::SelectCards
                | DecisionKind::SelectSum | DecisionKind::SelectToggle | DecisionKind::Chain { forced: false, .. }) => {
                let observation = &answer.observation;
                let me = observation.me;
                let count = |mine: bool, location: Location, face_down: bool| {
                    observation.cards.iter().filter(|card| {
                        (card.at.controller == me) == mine && card.at.location == location
                            && (!face_down || !card.position.face_up)
                    }).count()
                };
                let life = |player: usize| u64::try_from(observation.life_points[player]).unwrap_or(0);
                json!({
                    "decision": answer.decision,
                    "choice": answer.choice,
                    "responses": answer.responses,
                    "situation": {
                        "phase": observation.phase,
                        "own_turn": observation.turn_player == Some(me),
                        "life": [life(me as usize), life(1 - me as usize)],
                        "monsters": [count(true, Location::MonsterZone, false), count(false, Location::MonsterZone, false)],
                        "backrow": [count(true, Location::SpellTrapZone, false), count(false, Location::SpellTrapZone, false)],
                        "set_backrow": [count(true, Location::SpellTrapZone, true), count(false, Location::SpellTrapZone, true)],
                        "hand": [count(true, Location::Hand, false), count(false, Location::Hand, false)],
                        "attack": observation.battle_attacker.is_some(),
                    },
                })
            }
            _ => serde_json::Value::Null,
        });
        Ok(store_json(policy, value))
    })
}

/// What a search seat has done so far: its questions, searches, deviations
/// from its pilot and how long each search took.
/// # Safety
/// `policy` comes from `ygo_policy_create_search`.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_search_stats_json(policy: *mut YgoPolicy) -> *const c_char {
    guarded(std::ptr::null(), || {
        let policy = handle(policy)?;
        let Player::Search(search) = &policy.player else { return Err("not a search seat".into()) };
        let value = search.stats().to_json();
        Ok(store_json(policy, value))
    })
}

/// # Safety
/// `policy` comes from `ygo_policy_create` and is not used afterwards.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_destroy(policy: *mut YgoPolicy) {
    if !policy.is_null() {
        drop(Box::from_raw(policy));
    }
}

#[no_mangle]
pub extern "C" fn ygo_policy_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ref().map_or(std::ptr::null(), |m| m.as_ptr()))
}

#[no_mangle]
pub extern "C" fn ygo_policy_catalog() -> *const c_char {
    static CATALOG: OnceLock<CString> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let entries: Vec<_> = registry::POLICIES.iter().map(|e| json!({ "id": e.id, "deck": e.deck })).collect();
            CString::new(serde_json::Value::from(entries).to_string()).unwrap_or_default()
        })
        .as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ygo_policies::cards::{effect_categories, effect_locations, effect_types, types, CardData, EffectDescriptor, EffectProfiles, MemoryCards};
    use ygo_policies_ocgcore::wire::{location, msg, Writer};

    // Invented identity: the behavior below follows the supplied public effect
    // declaration, not a recognized staple, deck family or benchmark card.
    const DRAW_TRAP: u32 = 900_005_011;
    const DESCRIPTION: u64 = 144_000_801_761;

    struct TestHandle(*mut YgoPolicy);

    impl TestHandle {
        fn generic() -> Self {
            let db: Arc<dyn CardDatabase> = Arc::new(MemoryCards([(DRAW_TRAP, CardData {
                code: DRAW_TRAP, kind: types::TRAP, ..CardData::default()
            })].into()));
            let policy = registry::create_seeded("generic", db.clone(), 1).unwrap();
            Self(YgoPolicy::handle(Player::Plain(Seat::new(policy, db, Some(0)))))
        }

        fn install(&self, profiles: &str) -> i32 {
            let profiles = CString::new(profiles).unwrap();
            unsafe { ygo_policy_set_effect_profiles_json(self.0, profiles.as_ptr()) }
        }

        fn feed(&self, message: &[u8]) -> i32 {
            unsafe { ygo_policy_feed(self.0, message.as_ptr(), message.len()) }
        }

        fn response(&self) -> Vec<u8> {
            let mut length = 0;
            unsafe {
                let bytes = ygo_policy_response(self.0, &mut length);
                std::slice::from_raw_parts(bytes, length).to_vec()
            }
        }

        fn answer(&self) -> serde_json::Value {
            unsafe { serde_json::from_slice(CStr::from_ptr(ygo_policy_last_answer_json(self.0)).to_bytes()).unwrap() }
        }

        fn fork(&self) -> Self {
            let clone = unsafe { ygo_policy_clone(self.0) };
            assert!(!clone.is_null(), "{}", last_error());
            Self(clone)
        }
    }

    impl Drop for TestHandle {
        fn drop(&mut self) { unsafe { ygo_policy_destroy(self.0) } }
    }

    fn last_error() -> String {
        let error = ygo_policy_last_error();
        if error.is_null() { String::new() } else { unsafe { CStr::from_ptr(error).to_string_lossy().into_owned() } }
    }

    fn profiles(enabled: bool) -> String {
        let effects = if enabled { vec![EffectDescriptor {
            category: effect_categories::DRAW,
            effect_type: effect_types::ACTIVATE,
            range: effect_locations::SPELL_TRAP,
            description: DESCRIPTION,
            ..EffectDescriptor::default()
        }] } else { Vec::new() };
        let profiles: EffectProfiles = [(DRAW_TRAP, effects)].into();
        serde_json::to_string(&profiles).unwrap()
    }

    fn idle_prompt() -> Vec<u8> {
        let mut message = Writer::default().u8(msg::SELECT_IDLECMD).u8(0);
        for _ in 0..5 { message = message.u32(0); }
        message.u32(1).u32(DRAW_TRAP).u8(0).u8(location::SZONE).u32(0)
            .u64(DESCRIPTION).u8(0).u8(0).u8(1).u8(0).0
    }

    #[test]
    fn effect_profiles_before_first_prompt_change_real_generic_choice() {
        let plain = TestHandle::generic();
        let annotated = TestHandle::generic();
        // Public nonselection updates are allowed before installing profiles.
        assert_eq!(annotated.feed(&[msg::NEW_TURN, 0]), 0);
        assert_eq!(annotated.install(&profiles(true)), 0, "{}", last_error());
        assert_eq!(plain.feed(&idle_prompt()), 1);
        assert_eq!(annotated.feed(&idle_prompt()), 1);
        assert_eq!(plain.response(), 7u32.to_le_bytes());
        assert_eq!(annotated.response(), 5u32.to_le_bytes());
    }

    #[test]
    fn invalid_effect_profiles_preserve_previously_installed_metadata() {
        // Cover syntax errors, nonobject input and a partly decoded object
        // whose later entry has the wrong descriptor shape.
        for invalid in ["{", "[]", "null", r#"{"900005011":[],"900005012":false}"#] {
            let seat = TestHandle::generic();
            assert_eq!(seat.install(&profiles(true)), 0);
            assert_eq!(seat.install(invalid), -1, "{invalid}");
            assert!(last_error().contains("effect profiles"), "{}", last_error());
            assert_eq!(seat.feed(&idle_prompt()), 1);
            assert_eq!(seat.response(), 5u32.to_le_bytes(), "{invalid}");
        }
    }

    #[test]
    fn late_effect_profiles_are_rejected_without_changing_pending_response() {
        let seat = TestHandle::generic();
        assert_eq!(seat.install(&profiles(true)), 0);
        assert_eq!(seat.feed(&idle_prompt()), 1);
        let before = seat.answer();
        assert_eq!(seat.install(&profiles(false)), -1);
        assert!(last_error().contains("before feeding a selection prompt"));
        assert_eq!(seat.response(), 5u32.to_le_bytes());
        assert_eq!(seat.answer(), before);
        // Clearing the awaiting flag with a public update must not reopen the
        // initialization window after a policy has already made decisions.
        assert_eq!(seat.feed(&[msg::NEW_TURN, 0]), 0);
        assert_eq!(seat.install(&profiles(false)), -1);
        assert_eq!(seat.feed(&idle_prompt()), 1);
        assert_eq!(seat.response(), 5u32.to_le_bytes());
    }

    #[test]
    fn fork_retains_effect_profiles_independently_of_parent_replacement() {
        let parent = TestHandle::generic();
        assert_eq!(parent.install(&profiles(true)), 0);
        let child = parent.fork();
        // An explicit empty descriptor list shadows that card's earlier
        // metadata. The child must still retain its original immutable copy.
        assert_eq!(parent.install(&profiles(false)), 0);
        assert_eq!(parent.feed(&idle_prompt()), 1);
        assert_eq!(child.feed(&idle_prompt()), 1);
        assert_eq!(parent.response(), 7u32.to_le_bytes());
        assert_eq!(child.response(), 5u32.to_le_bytes());
    }

    #[test]
    fn fork_rebuilds_json_and_preserves_pending_response_independently() {
        let parent = TestHandle::generic();
        assert_eq!(parent.install(&profiles(true)), 0);
        assert_eq!(unsafe { ygo_policy_enable_response_overrides(parent.0) }, 0);
        assert_eq!(parent.feed(&idle_prompt()), 1);
        let expected = parent.answer();
        let parent_json = unsafe { (*parent.0).json.as_ptr() };
        let parent_bytes = unsafe { CStr::from_ptr(parent_json).to_bytes().to_vec() };
        let child = parent.fork();
        assert_eq!(child.response(), parent.response());
        assert_eq!(child.answer(), expected);

        // A different getter overwrites only the child's scratch buffer. Its
        // next answer getter must regenerate the same complete pending answer.
        let child_observation = unsafe {
            serde_json::from_slice::<serde_json::Value>(CStr::from_ptr(ygo_policy_observation_json(child.0)).to_bytes()).unwrap()
        };
        assert_eq!(child_observation, expected["observation"]);
        assert_eq!(child.answer(), expected);
        assert_eq!(unsafe { CStr::from_ptr(parent_json).to_bytes() }, parent_bytes);

        // The actual pending state, unlike JSON scratch, is copied: changing a
        // child's selected action must retain a valid independent parent.
        let end_turn = 7u32.to_le_bytes();
        assert_eq!(unsafe { ygo_policy_override_response(child.0, end_turn.as_ptr(), end_turn.len()) }, 0);
        assert_eq!(child.response(), end_turn);
        assert_eq!(child.answer()["choice"], 1);
        assert_eq!(parent.response(), 5u32.to_le_bytes());
        assert_eq!(parent.answer(), expected);
    }

    #[test]
    fn disabling_checkpoints_keeps_pending_response_and_future_generic_choices() {
        let plain = TestHandle::generic();
        assert_eq!(plain.install(&profiles(true)), 0);
        assert_eq!(unsafe { ygo_policy_enable_response_overrides(plain.0) }, 0);
        assert_eq!(plain.feed(&idle_prompt()), 1);
        let end_turn = 7u32.to_le_bytes();
        assert_eq!(unsafe { ygo_policy_override_response(plain.0, end_turn.as_ptr(), end_turn.len()) }, 0);
        let reference = plain.fork();
        let before = plain.answer();
        assert_eq!(unsafe { ygo_policy_disable_response_overrides(plain.0) }, 0);
        assert_eq!(plain.response(), end_turn);
        assert_eq!(plain.answer(), before);
        for message in [vec![msg::NEW_TURN, 0], idle_prompt(), vec![msg::CHAIN_END], idle_prompt()] {
            assert_eq!(plain.feed(&message), reference.feed(&message));
            assert_eq!(plain.response(), reference.response());
            assert_eq!(plain.answer(), reference.answer());
        }
        assert_eq!(unsafe { ygo_policy_disable_response_overrides(std::ptr::null_mut()) }, -1);
        assert!(last_error().contains("NULL"));
    }
}
