//! C ABI: one handle per seat, fed OCGCore messages, answering with OCGCore
//! responses.  See `include/ygo_policies.h` for the contract.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::json;
use ygo_policies::cards::CardDatabase;
use ygo_policies::registry;
use ygo_policies_ocgcore::{Seat, SqliteCards};

pub struct YgoPolicy {
    seat: Seat,
    response: Vec<u8>,
    json: CString,
}

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_error(message: impl Into<String>) {
    let message = CString::new(message.into().replace('\0', " ")).unwrap_or_default();
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(message));
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
    guarded(std::ptr::null_mut(), || {
        let id = text(policy_id, "policy_id")?;
        let seat = match seat {
            -1 => None,
            0 | 1 => Some(seat as u8),
            other => return Err(format!("seat must be 0, 1 or -1, got {other}")),
        };
        let db = cards(text(cards_cdb_path, "cards_cdb_path")?)?;
        let policy = registry::create(id, db.clone()).ok_or_else(|| format!("unknown policy {id:?}"))?;
        let seat = Seat::new(policy, db, seat);
        Ok(Box::into_raw(Box::new(YgoPolicy { seat, response: Vec::new(), json: CString::default() })))
    })
}

/// # Safety
/// `policy` comes from `ygo_policy_create`; `message` points to `length` bytes.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_feed(policy: *mut YgoPolicy, message: *const u8, length: usize) -> i32 {
    guarded(-1, || {
        let policy = handle(policy)?;
        let answer = policy.seat.feed(bytes(message, length)?).map_err(|e| e.to_string())?;
        Ok(answer.map_or(0, |response| {
            policy.response = response;
            1
        }))
    })
}

/// # Safety
/// `policy` comes from `ygo_policy_create`; `buffer` points to `length` bytes.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_feed_buffer(policy: *mut YgoPolicy, buffer: *const u8, length: usize) -> i32 {
    guarded(-1, || {
        let policy = handle(policy)?;
        let answer = policy.seat.feed_buffer(bytes(buffer, length)?).map_err(|e| e.to_string())?;
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
    policy.as_ref().and_then(|p| p.seat.seat()).map_or(-1, i32::from)
}

/// # Safety
/// `policy` comes from `ygo_policy_create`.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_observation_json(policy: *mut YgoPolicy) -> *const c_char {
    guarded(std::ptr::null(), || {
        let policy = handle(policy)?;
        let value = serde_json::to_value(policy.seat.observation()).map_err(|e| e.to_string())?;
        Ok(store_json(policy, value))
    })
}

/// # Safety
/// `policy` comes from `ygo_policy_create`.
#[no_mangle]
pub unsafe extern "C" fn ygo_policy_last_answer_json(policy: *mut YgoPolicy) -> *const c_char {
    guarded(std::ptr::null(), || {
        let policy = handle(policy)?;
        let value = match policy.seat.last_answer() {
            None => serde_json::Value::Null,
            Some(answer) => json!({
                "observation": answer.observation,
                "decision": answer.decision,
                "choice": answer.choice,
                "responses": answer.responses,
                "picks": answer.picks,
            }),
        };
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
