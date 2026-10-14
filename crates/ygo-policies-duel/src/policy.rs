//! Policies behind the C ABI of a policy library (`libygo_policies`), seated.
use libloading::Library;
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
};

use crate::core::symbol;
use crate::Result;

type Handle = *mut c_void;

/// A policy library loaded from a file: the policies of one build, each to
/// be seated by its id.  Two libraries can face each other in one duel.
pub struct PolicyLibrary {
    _library: Library,
    create: unsafe extern "C" fn(*const c_char, *const c_char, i32, u64) -> Handle,
    destroy: unsafe extern "C" fn(Handle),
    fork: Option<unsafe extern "C" fn(Handle) -> Handle>,
    feed: unsafe extern "C" fn(Handle, *const u8, usize) -> i32,
    feed_buffer: unsafe extern "C" fn(Handle, *const u8, usize) -> i32,
    update_pair: Option<unsafe extern "C" fn(Handle, Handle, *const u8, usize) -> i32>,
    response: unsafe extern "C" fn(Handle, *mut usize) -> *const u8,
    error: unsafe extern "C" fn() -> *const c_char,
    answer: unsafe extern "C" fn(Handle) -> *const c_char,
    search_view: Option<unsafe extern "C" fn(Handle) -> *const c_char>,
    observation: Option<unsafe extern "C" fn(Handle) -> *const c_char>,
    enable_response_overrides: Option<unsafe extern "C" fn(Handle) -> i32>,
    disable_response_overrides: Option<unsafe extern "C" fn(Handle) -> i32>,
    override_response: Option<unsafe extern "C" fn(Handle, *const u8, usize) -> i32>,
    alternative_responses: Option<unsafe extern "C" fn(Handle, usize) -> *const c_char>,
    set_effect_profiles: Option<unsafe extern "C" fn(Handle, *const c_char) -> i32>,
    /// Policy id to the name of its deck list.
    pub catalog: HashMap<String, String>,
}

impl PolicyLibrary {
    /// Load a policy library (`libygo_policies.so` or another build of it).
    pub fn open(path: &Path) -> Result<Self> {
        unsafe {
            let path =
                std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let library = Library::new(path).map_err(|e| e.to_string())?;
            let catalog: unsafe extern "C" fn() -> *const c_char =
                symbol(&library, b"ygo_policy_catalog")?;
            let entries: serde_json::Value =
                serde_json::from_slice(CStr::from_ptr(catalog()).to_bytes())
                    .map_err(|e| e.to_string())?;
            let catalog = entries
                .as_array()
                .ok_or("Invalid catalog")?
                .iter()
                .map(|v| {
                    (
                        v["id"].as_str().unwrap().to_owned(),
                        v["deck"].as_str().unwrap().to_owned(),
                    )
                })
                .collect();
            Ok(Self {
                create: symbol(&library, b"ygo_policy_create_seeded")?,
                destroy: symbol(&library, b"ygo_policy_destroy")?,
                fork: symbol(&library, b"ygo_policy_clone").ok(),
                feed: symbol(&library, b"ygo_policy_feed")?,
                feed_buffer: symbol(&library, b"ygo_policy_feed_buffer")?,
                update_pair: symbol(&library, b"ygo_policy_feed_update_pair").ok(),
                response: symbol(&library, b"ygo_policy_response")?,
                error: symbol(&library, b"ygo_policy_last_error")?,
                answer: symbol(&library, b"ygo_policy_last_answer_json")?,
                search_view: symbol(&library, b"ygo_policy_search_view_json").ok(),
                observation: symbol(&library, b"ygo_policy_observation_json").ok(),
                enable_response_overrides: symbol(&library, b"ygo_policy_enable_response_overrides").ok(),
                disable_response_overrides: symbol(&library, b"ygo_policy_disable_response_overrides").ok(),
                override_response: symbol(&library, b"ygo_policy_override_response").ok(),
                alternative_responses: symbol(&library, b"ygo_policy_alternative_responses_json").ok(),
                set_effect_profiles: symbol(&library, b"ygo_policy_set_effect_profiles_json").ok(),
                catalog,
                _library: library,
            })
        }
    }

    /// Seat the policy `name` as `player`.  `cards` is the card database it
    /// reads; `seed` is what it breaks ties with.
    pub fn seat(&self, name: &str, cards: &Path, player: i32, seed: u64) -> Result<LibrarySeat<'_>> {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let cards = CString::new(cards.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        let handle = unsafe { (self.create)(name.as_ptr(), cards.as_ptr(), player, seed) };
        if handle.is_null() {
            return Err(self.last_error());
        }
        Ok(LibrarySeat {
            library: self,
            handle,
        })
    }

    /// Older policy libraries are still usable through history replay.
    pub fn can_fork(&self) -> bool {
        self.fork.is_some()
    }

    fn last_error(&self) -> String {
        unsafe {
            let error = (self.error)();
            if error.is_null() {
                "Unspecified policy error".into()
            } else {
                CStr::from_ptr(error).to_string_lossy().into_owned()
            }
        }
    }
}

/// One player, played by a policy of a [`PolicyLibrary`].  Feed it every
/// message of the duel in order, as a [`ygo_policies_ocgcore::Seat`].
pub struct LibrarySeat<'a> {
    library: &'a PolicyLibrary,
    handle: Handle,
}

impl LibrarySeat<'_> {
    /// Public card-definition metadata only, installed before play.
    pub fn set_effect_profiles(&self, profiles: &serde_json::Value) -> Result<()> {
        let install = self.library.set_effect_profiles
            .ok_or("policy library has no effect profile API")?;
        let encoded = CString::new(profiles.to_string()).map_err(|e| e.to_string())?;
        match unsafe { install(self.handle, encoded.as_ptr()) } {
            0 => Ok(()),
            _ => Err(self.library.last_error()),
        }
    }

    /// An independent copy with the same library lifetime as this seat.
    pub fn fork(&self) -> Result<Self> {
        let fork = self.library.fork.ok_or("policy library has no copy API")?;
        let handle = unsafe { fork(self.handle) };
        if handle.is_null() {
            return Err(self.library.last_error());
        }
        Ok(Self { library: self.library, handle })
    }

    /// Enable policy snapshots before feeding this search seat's first prompt.
    /// Forked seats retain the setting and the pending pre-choice snapshot.
    pub fn enable_response_overrides(&self) -> Result<()> {
        let enable = self.library.enable_response_overrides
            .ok_or("policy library has no response override API")?;
        match unsafe { enable(self.handle) } {
            0 => Ok(()),
            _ => Err(self.library.last_error()),
        }
    }

    /// Stop checkpointing after the final searched response is committed.
    /// The committed policy and RNG remain unchanged. Only for plain rollout
    /// tails which reject MSG_RETRY before feeding it. Returns false for older
    /// libraries, which safely retain their ordinary checkpoint behavior.
    pub fn disable_response_overrides(&self) -> Result<bool> {
        let Some(disable) = self.library.disable_response_overrides else { return Ok(false); };
        match unsafe { disable(self.handle) } {
            0 => Ok(true),
            _ => Err(self.library.last_error()),
        }
    }

    /// Commit the actual legal response chosen by search, including the
    /// corresponding policy memory, before sending it to the engine.
    pub fn override_response(&self, response: &[u8]) -> Result<()> {
        let commit = self.library.override_response
            .ok_or("policy library has no response override API")?;
        match unsafe { commit(self.handle, response.as_ptr(), response.len()) } {
            0 => Ok(()),
            _ => Err(self.library.last_error()),
        }
    }

    /// Expand a sequential target/material prompt into bounded legal whole
    /// responses, and return the updated last-answer JSON. Its choices and
    /// responses stay aligned; overrides restore the matching cached policy.
    /// At most twelve initial candidates are attempted. Other prompts return
    /// their existing answer without changing the available responses.
    pub fn alternative_responses(&self, maximum: usize) -> Result<serde_json::Value> {
        let expand = self.library.alternative_responses
            .ok_or("policy library has no sequential alternative API")?;
        unsafe {
            let answer = expand(self.handle, maximum);
            if answer.is_null() { return Err(self.library.last_error()); }
            serde_json::from_slice(CStr::from_ptr(answer).to_bytes()).map_err(|e| e.to_string())
        }
    }

    /// Feed one message (id first).  When it asks this seat to decide, the
    /// response bytes for the engine come back.
    pub fn feed(&self, message: &[u8]) -> Result<Option<Vec<u8>>> {
        unsafe {
            match (self.library.feed)(self.handle, message.as_ptr(), message.len()) {
                0 => Ok(None),
                1 => {
                    let mut length = 0;
                    let bytes = (self.library.response)(self.handle, &mut length);
                    Ok(Some(std::slice::from_raw_parts(bytes, length).to_vec()))
                }
                _ => Err(self.library.last_error()),
            }
        }
    }

    /// Apply a field update to both seats; older or different libraries use
    /// their ordinary feed API, preserving paired-library comparisons.
    pub fn feed_update_pair(&self, other: &Self, message: &[u8]) -> Result<()> {
        if std::ptr::eq(self.library, other.library) {
            if let Some(feed) = self.library.update_pair {
                return match unsafe { feed(self.handle, other.handle, message.as_ptr(), message.len()) } {
                    0 => Ok(()),
                    _ => Err(self.library.last_error()),
                };
            }
        }
        self.feed(message)?;
        other.feed(message)?;
        Ok(())
    }

    /// Feed several messages at once, each behind its length as OCGCore
    /// writes them; answers along the way are dropped.
    pub fn feed_buffer(&self, buffer: &[u8]) -> Result<()> {
        unsafe {
            match (self.library.feed_buffer)(self.handle, buffer.as_ptr(), buffer.len()) {
                0 | 1 => Ok(()),
                _ => Err(self.library.last_error()),
            }
        }
    }

    /// The last decision this seat answered, as `policy-bench --trace`
    /// writes one: what it observed, the choices it had and the one it took.
    pub fn last_answer(&self) -> Result<serde_json::Value> {
        self.read_json(self.library.answer)
    }

    /// The seat's current observation, after the same redaction used by its
    /// policy. This never queries hidden engine state.
    pub fn observation(&self) -> Result<serde_json::Value> {
        self.read_json(self.library.observation.ok_or("policy library has no observation API")?)
    }

    /// Alternatives and a small situation summary, or JSON null for a decision
    /// search does not support. Older libraries supply the full answer instead.
    pub fn search_view(&self) -> Result<serde_json::Value> {
        self.read_json(self.library.search_view.unwrap_or(self.library.answer))
    }

    fn read_json(&self, read: unsafe extern "C" fn(Handle) -> *const c_char) -> Result<serde_json::Value> {
        unsafe {
            let answer = read(self.handle);
            if answer.is_null() {
                return Err(self.library.last_error());
            }
            serde_json::from_slice(CStr::from_ptr(answer).to_bytes()).map_err(|e| e.to_string())
        }
    }
}

impl Drop for LibrarySeat<'_> {
    fn drop(&mut self) {
        unsafe { (self.library.destroy)(self.handle) }
    }
}
