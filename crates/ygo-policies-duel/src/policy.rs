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
    feed: unsafe extern "C" fn(Handle, *const u8, usize) -> i32,
    feed_buffer: unsafe extern "C" fn(Handle, *const u8, usize) -> i32,
    response: unsafe extern "C" fn(Handle, *mut usize) -> *const u8,
    error: unsafe extern "C" fn() -> *const c_char,
    answer: unsafe extern "C" fn(Handle) -> *const c_char,
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
                feed: symbol(&library, b"ygo_policy_feed")?,
                feed_buffer: symbol(&library, b"ygo_policy_feed_buffer")?,
                response: symbol(&library, b"ygo_policy_response")?,
                error: symbol(&library, b"ygo_policy_last_error")?,
                answer: symbol(&library, b"ygo_policy_last_answer_json")?,
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
        unsafe {
            let answer = (self.library.answer)(self.handle);
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
