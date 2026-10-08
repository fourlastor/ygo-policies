//! The engine: OCGCore's C API, the card database and the card scripts.
//! Callbacks and their backing data outlive every duel.
use libloading::Library;
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    path::Path,
};

use crate::Result;

pub(crate) type Handle = *mut c_void;

/// Printed data of a card, in the card database's own values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Printed {
    /// The card type bits (monster, Spell, Trap, Tuner, Synchro...).
    pub kind: u32,
    /// Level or Rank.
    pub level: u32,
    /// The Attribute bits.
    pub attribute: u32,
    /// The monster Type bits.
    pub race: u64,
    /// ATK; -2 for "?".
    pub attack: i32,
    /// DEF; -2 for "?".
    pub defense: i32,
    /// The code of the card this one is another print of, or 0.
    pub alias: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub(crate) struct CardData {
    code: u32,
    alias: u32,
    sets: *const u16,
    kind: u32,
    level: u32,
    attribute: u32,
    race: u64,
    attack: i32,
    defense: i32,
    lscale: u32,
    rscale: u32,
    link: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Player {
    pub(crate) lp: u32,
    pub(crate) draw: u32,
    pub(crate) per_turn: u32,
}

#[repr(C)]
pub(crate) struct Options {
    pub(crate) seed: [u64; 4],
    pub(crate) flags: u64,
    pub(crate) p0: Player,
    pub(crate) p1: Player,
    pub(crate) reader: unsafe extern "C" fn(Handle, u32, *mut CardData),
    pub(crate) payload1: Handle,
    pub(crate) scripts: unsafe extern "C" fn(Handle, Handle, *const c_char) -> i32,
    pub(crate) payload2: Handle,
    pub(crate) log: unsafe extern "C" fn(Handle, *const c_char, i32),
    pub(crate) payload3: Handle,
    pub(crate) done: unsafe extern "C" fn(Handle, *mut CardData),
    pub(crate) payload4: Handle,
    pub(crate) unsafe_libraries: u8,
}

#[repr(C)]
pub(crate) struct NewCard {
    pub(crate) team: u8,
    pub(crate) duelist: u8,
    pub(crate) code: u32,
    pub(crate) controller: u8,
    pub(crate) location: u32,
    pub(crate) sequence: u32,
    pub(crate) position: u32,
}

#[repr(C)]
pub(crate) struct Query {
    pub(crate) flags: u32,
    pub(crate) controller: u8,
    pub(crate) location: u32,
    pub(crate) sequence: u32,
    pub(crate) overlay: u32,
}

type LoadScript = unsafe extern "C" fn(Handle, *const u8, u32, *const c_char) -> i32;

// Linked from the pinned vendor tree by build.rs.  Another build of the
// engine can be loaded in its place (see [`Core::open`]).
extern "C" {
    fn OCG_CreateDuel(out: *mut Handle, options: *const Options) -> i32;
    fn OCG_DestroyDuel(duel: Handle);
    fn OCG_DuelNewCard(duel: Handle, card: *const NewCard);
    fn OCG_StartDuel(duel: Handle);
    fn OCG_DuelProcess(duel: Handle) -> i32;
    fn OCG_DuelGetMessage(duel: Handle, length: *mut u32) -> *const u8;
    fn OCG_DuelSetResponse(duel: Handle, response: *const u8, length: u32);
    fn OCG_DuelQueryLocation(duel: Handle, length: *mut u32, query: *const Query) -> *const u8;
    fn OCG_LoadScript(duel: Handle, bytes: *const u8, length: u32, name: *const c_char) -> i32;
    fn OCG_DuelQueryCount(duel: Handle, team: u8, location: u32) -> u32;
    fn OCG_DuelQuery(duel: Handle, length: *mut u32, query: *const Query) -> *const u8;
    // The fork's additions: arena snapshots and the hidden-card swap.
    fn OCG_DuelCreateSnapshot(duel: Handle, out: *mut Handle) -> i32;
    fn OCG_DuelRestoreSnapshot(duel: Handle, snapshot: Handle) -> i32;
    fn OCG_DuelDestroySnapshot(snapshot: Handle);
    fn OCG_DuelSwapHiddenCards(duel: Handle, team: u8, loc1: u32, seq1: u32, loc2: u32, seq2: u32) -> i32;
}

/// What a search needs beyond the public OCGCore API: the fork's snapshots
/// and hidden-card swap, and the two single-card queries.
#[derive(Clone, Copy)]
pub(crate) struct Extensions {
    pub(crate) snapshot: unsafe extern "C" fn(Handle, *mut Handle) -> i32,
    pub(crate) restore: unsafe extern "C" fn(Handle, Handle) -> i32,
    pub(crate) discard: unsafe extern "C" fn(Handle),
    pub(crate) swap: unsafe extern "C" fn(Handle, u8, u32, u32, u32, u32) -> i32,
    pub(crate) count: unsafe extern "C" fn(Handle, u8, u32) -> u32,
    pub(crate) card: unsafe extern "C" fn(Handle, *mut u32, *const Query) -> *const u8,
}

impl Extensions {
    /// Those of the engine this crate is built with.
    fn built_in() -> Self {
        Self {
            snapshot: OCG_DuelCreateSnapshot,
            restore: OCG_DuelRestoreSnapshot,
            discard: OCG_DuelDestroySnapshot,
            swap: OCG_DuelSwapHiddenCards,
            count: OCG_DuelQueryCount,
            card: OCG_DuelQuery,
        }
    }

    /// Those of an engine loaded from a file, if it has them.  The swap was
    /// `YGO_DuelSwapHiddenCards` before it moved into the engine's fork.
    unsafe fn load(library: &Library) -> Option<Self> {
        Some(Self {
            snapshot: symbol(library, b"OCG_DuelCreateSnapshot").ok()?,
            restore: symbol(library, b"OCG_DuelRestoreSnapshot").ok()?,
            discard: symbol(library, b"OCG_DuelDestroySnapshot").ok()?,
            swap: symbol(library, b"OCG_DuelSwapHiddenCards").or_else(|_| symbol(library, b"YGO_DuelSwapHiddenCards")).ok()?,
            count: symbol(library, b"OCG_DuelQueryCount").ok()?,
            card: symbol(library, b"OCG_DuelQuery").ok()?,
        })
    }
}

pub(crate) struct Resources {
    cards: HashMap<u32, (CardData, Box<[u16]>)>,
    scripts: HashMap<String, Vec<u8>>,
    load: LoadScript,
}

pub(crate) unsafe extern "C" fn reader(payload: Handle, code: u32, out: *mut CardData) {
    let resources = &*(payload as *const Resources);
    *out = resources
        .cards
        .get(&code)
        .map(|(data, _)| *data)
        .unwrap_or_default();
}

pub(crate) unsafe extern "C" fn scripts(payload: Handle, duel: Handle, name: *const c_char) -> i32 {
    let resources = &*(payload as *const Resources);
    let name_str = CStr::from_ptr(name).to_string_lossy();
    match resources.scripts.get(name_str.as_ref()) {
        Some(bytes) => (resources.load)(duel, bytes.as_ptr(), bytes.len() as u32, name),
        None => 0,
    }
}

/// The payload is the duel's own list of script errors.
pub(crate) unsafe extern "C" fn log(payload: Handle, text: *const c_char, kind: i32) {
    if kind == 0 {
        (*(payload as *const RefCell<Vec<String>>))
            .borrow_mut()
            .push(CStr::from_ptr(text).to_string_lossy().into_owned());
    }
}

pub(crate) unsafe extern "C" fn done(_: Handle, _: *mut CardData) {}

pub(crate) unsafe fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T> {
    library
        .get::<T>(name)
        .map(|s| *s)
        .map_err(|e| e.to_string())
}

/// The engine with its card database and its scripts.  One per thread: a
/// worker opens its own and plays its duels on it.
pub struct Core {
    _library: Option<Library>,
    /// Snapshots and the hidden-card swap: the built-in engine has them, an
    /// engine loaded from a file may not.
    pub(crate) extensions: Option<Extensions>,
    pub(crate) resources: Box<Resources>,
    pub(crate) create: unsafe extern "C" fn(*mut Handle, *const Options) -> i32,
    pub(crate) destroy: unsafe extern "C" fn(Handle),
    pub(crate) new_card: unsafe extern "C" fn(Handle, *const NewCard),
    pub(crate) start: unsafe extern "C" fn(Handle),
    pub(crate) process: unsafe extern "C" fn(Handle) -> i32,
    pub(crate) messages: unsafe extern "C" fn(Handle, *mut u32) -> *const u8,
    pub(crate) respond: unsafe extern "C" fn(Handle, *const u8, u32),
    pub(crate) query: unsafe extern "C" fn(Handle, *mut u32, *const Query) -> *const u8,
}

impl Core {
    /// The engine, the card database `cards` (EDOPro's `cards.cdb` layout,
    /// set codes included) and the scripts under `script_dir`: its root
    /// helpers, then `official/`, then `pre-errata/` for names still missing.
    ///
    /// `path` is another build of OCGCore to load instead of the one this
    /// crate compiles (its C API must match).  [`Core::has_snapshots`] says
    /// whether that build can do what [`Duel::snapshot`](crate::Duel::snapshot)
    /// and [`Duel::swap`](crate::Duel::swap) need.
    pub fn open(path: Option<&Path>, cards: &Path, script_dir: &Path) -> Result<Self> {
        let mut data = HashMap::new();
        let db = rusqlite::Connection::open_with_flags(
            cards,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| e.to_string())?;
        let mut stmt = db
            .prepare("select id,alias,setcode,type,atk,def,level,race,attribute from datas")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let mut values = [0i64; 9];
                for (i, v) in values.iter_mut().enumerate() {
                    *v = row.get(i)?;
                }
                Ok(values)
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let [code, alias, sets, kind, attack, defense, level, race, attribute] =
                row.map_err(|e| e.to_string())?;
            let sets: Box<[u16]> = (0..4)
                .map(|i| ((sets as u64 >> (i * 16)) & 65535) as u16)
                .filter(|s| *s != 0)
                .chain([0])
                .collect();
            let link = kind & 0x4000000 != 0;
            data.insert(
                code as u32,
                (
                    CardData {
                        code: code as u32,
                        alias: alias as u32,
                        sets: sets.as_ptr(),
                        kind: kind as u32,
                        level: level as u32 & 255,
                        attribute: attribute as u32,
                        race: race as u64,
                        attack: attack as i32,
                        defense: if link { 0 } else { defense as i32 },
                        lscale: (level as u32 >> 24) & 255,
                        rscale: (level as u32 >> 16) & 255,
                        link: if link { defense as u32 } else { 0 },
                    },
                    sets,
                ),
            );
        }
        let mut script_data = HashMap::new();
        // Root helpers first, then official cards, then missing pre-errata scripts.
        for folder in ["", "official", "pre-errata"] {
            let dir = script_dir.join(folder);
            if !dir.is_dir() {
                continue;
            }
            for file in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
                let file = file.map_err(|e| e.to_string())?;
                if file.path().extension().and_then(|e| e.to_str()) == Some("lua") {
                    script_data
                        .entry(file.file_name().to_string_lossy().into_owned())
                        .or_insert(std::fs::read(file.path()).map_err(|e| e.to_string())?);
                }
            }
        }
        unsafe {
            let library = path
                .map(|p| Library::new(p).map_err(|e| e.to_string()))
                .transpose()?;
            macro_rules! api {
                ($name:ident) => {
                    match &library {
                        Some(library) => symbol(library, stringify!($name).as_bytes())?,
                        None => $name,
                    }
                };
            }
            Ok(Self {
                resources: Box::new(Resources {
                    cards: data,
                    scripts: script_data,
                    load: api!(OCG_LoadScript),
                }),
                create: api!(OCG_CreateDuel),
                destroy: api!(OCG_DestroyDuel),
                new_card: api!(OCG_DuelNewCard),
                start: api!(OCG_StartDuel),
                process: api!(OCG_DuelProcess),
                messages: api!(OCG_DuelGetMessage),
                respond: api!(OCG_DuelSetResponse),
                query: api!(OCG_DuelQueryLocation),
                extensions: match &library {
                    Some(library) => Extensions::load(library),
                    None => Some(Extensions::built_in()),
                },
                _library: library,
            })
        }
    }

    /// Whether this engine has arena snapshots and the hidden-card swap.
    /// The built-in one does.
    pub fn has_snapshots(&self) -> bool {
        self.extensions.is_some()
    }

    /// Printed data of a card, if the database (or [`Core::define`]) has it.
    pub fn printed(&self, code: u32) -> Option<Printed> {
        self.resources.cards.get(&code).map(|(c, _)| Printed {
            kind: c.kind,
            level: c.level,
            attribute: c.attribute,
            race: c.race,
            attack: c.attack,
            defense: c.defense,
            alias: c.alias,
        })
    }

    /// Printed Link arrows, stored in the database's DEF column.
    pub fn link_markers(&self, code: u32) -> Option<u32> {
        self.resources.cards.get(&code).map(|(card, _)| card.link)
    }

    /// Every code in the database, in order.
    pub fn codes(&self) -> Vec<u32> {
        let mut codes: Vec<u32> = self.resources.cards.keys().copied().collect();
        codes.sort_unstable();
        codes
    }

    /// Add or replace a card of our own: staged duels use plain monsters
    /// that no script touches.  Duels read card data when they start, so a
    /// card can be redefined between duels.
    pub fn define(&mut self, code: u32, card: Printed) {
        let sets: Box<[u16]> = Box::new([0]);
        let data = CardData {
            code,
            alias: card.alias,
            sets: sets.as_ptr(),
            kind: card.kind,
            level: card.level,
            attribute: card.attribute,
            race: card.race,
            attack: card.attack,
            defense: card.defense,
            lscale: 0,
            rscale: 0,
            link: 0,
        };
        self.resources.cards.insert(code, (data, sets));
    }

    /// The first of `codes` the database does not have.
    pub(crate) fn missing<'a>(&self, codes: impl IntoIterator<Item = &'a u32>) -> Option<u32> {
        codes.into_iter().copied().find(|code| !self.resources.cards.contains_key(code))
    }
}
