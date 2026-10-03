//! The public OCGCore C API. Callbacks and their backing data outlive every duel.
use libloading::Library;
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
    ptr,
};
use ygo_policies_ocgcore::{
    message::{self, Message},
    wire::{msg, query},
};

pub type Result<T> = std::result::Result<T, String>;
type Handle = *mut c_void;

#[derive(Clone, Copy)]
pub struct PlayOptions {
    pub seed: u64,
    pub limit: usize,
    pub trace: bool,
    /// Starting Life Points of seat 0 (who goes first) and seat 1.
    pub life_points: [u32; 2],
}

/// Life Points of both players in a staged duel.
pub const STAGED_LIFE_POINTS: u32 = 30000;

/// A card put somewhere before a staged duel starts (see [`Core::stage`]).
/// `location`, `sequence` and `position` are OCGCore's own values.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub controller: u8,
    pub code: u32,
    pub location: u32,
    pub sequence: u32,
    pub position: u32,
}

/// A monster on the field, as the engine reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Monster {
    pub controller: u8,
    pub sequence: u32,
    pub code: u32,
    pub position: u32,
    pub attack: i32,
    pub defense: i32,
}

/// Printed data of a card.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Printed {
    pub kind: u32,
    pub level: u32,
    pub attribute: u32,
    pub race: u64,
    pub attack: i32,
    pub defense: i32,
    pub alias: u32,
}

/// What a staged duel shows its watcher.
pub enum Step<'a> {
    /// Every engine message, in order, with its bytes (id first).
    Message(&'a Message, &'a [u8]),
    /// A selection about to be answered, with both monster zones as they stand.
    Prompt(&'a Message, &'a [Monster]),
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct CardData {
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
struct Player {
    lp: u32,
    draw: u32,
    per_turn: u32,
}

#[repr(C)]
struct Options {
    seed: [u64; 4],
    flags: u64,
    p0: Player,
    p1: Player,
    reader: unsafe extern "C" fn(Handle, u32, *mut CardData),
    payload1: Handle,
    scripts: unsafe extern "C" fn(Handle, Handle, *const c_char) -> i32,
    payload2: Handle,
    log: unsafe extern "C" fn(Handle, *const c_char, i32),
    payload3: Handle,
    done: unsafe extern "C" fn(Handle, *mut CardData),
    payload4: Handle,
    unsafe_libraries: u8,
}

#[repr(C)]
struct NewCard {
    team: u8,
    duelist: u8,
    code: u32,
    controller: u8,
    location: u32,
    sequence: u32,
    position: u32,
}

#[repr(C)]
struct Query {
    flags: u32,
    controller: u8,
    location: u32,
    sequence: u32,
    overlay: u32,
}

type LoadScript = unsafe extern "C" fn(Handle, *const u8, u32, *const c_char) -> i32;

// Linked from the pinned vendor tree by build.rs. --core can override it for
// comparisons against another host's engine without changing policy code.
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
}

struct Resources {
    cards: HashMap<u32, (CardData, Box<[u16]>)>,
    scripts: HashMap<String, Vec<u8>>,
    load: LoadScript,
    errors: Vec<String>,
}

unsafe extern "C" fn reader(payload: Handle, code: u32, out: *mut CardData) {
    let resources = &*(payload as *const Resources);
    *out = resources
        .cards
        .get(&code)
        .map(|(data, _)| *data)
        .unwrap_or_default();
}

unsafe extern "C" fn scripts(payload: Handle, duel: Handle, name: *const c_char) -> i32 {
    let resources = &*(payload as *const Resources);
    let name_str = CStr::from_ptr(name).to_string_lossy();
    match resources.scripts.get(name_str.as_ref()) {
        Some(bytes) => (resources.load)(duel, bytes.as_ptr(), bytes.len() as u32, name),
        None => 0,
    }
}

unsafe extern "C" fn log(payload: Handle, text: *const c_char, kind: i32) {
    if kind == 0 {
        (*(payload as *mut Resources))
            .errors
            .push(CStr::from_ptr(text).to_string_lossy().into_owned());
    }
}

unsafe extern "C" fn done(_: Handle, _: *mut CardData) {}

unsafe fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T> {
    library
        .get::<T>(name)
        .map(|s| *s)
        .map_err(|e| e.to_string())
}

pub struct Core {
    _library: Option<Library>,
    resources: Box<Resources>,
    create: unsafe extern "C" fn(*mut Handle, *const Options) -> i32,
    destroy: unsafe extern "C" fn(Handle),
    new_card: unsafe extern "C" fn(Handle, *const NewCard),
    start: unsafe extern "C" fn(Handle),
    process: unsafe extern "C" fn(Handle) -> i32,
    messages: unsafe extern "C" fn(Handle, *mut u32) -> *const u8,
    respond: unsafe extern "C" fn(Handle, *const u8, u32),
    query: unsafe extern "C" fn(Handle, *mut u32, *const Query) -> *const u8,
}

impl Core {
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
                    errors: vec![],
                }),
                create: api!(OCG_CreateDuel),
                destroy: api!(OCG_DestroyDuel),
                new_card: api!(OCG_DuelNewCard),
                start: api!(OCG_StartDuel),
                process: api!(OCG_DuelProcess),
                messages: api!(OCG_DuelGetMessage),
                respond: api!(OCG_DuelSetResponse),
                query: api!(OCG_DuelQueryLocation),
                _library: library,
            })
        }
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

    fn monsters(&self, handle: Handle) -> Vec<Monster> {
        let mut out = Vec::new();
        for controller in 0..2u8 {
            let q = Query { flags: query::RECOMMENDED, controller, location: 4, sequence: 0, overlay: 0 };
            let mut length = 0;
            let bytes = unsafe { (self.query)(handle, &mut length, &q) };
            let mut update = vec![msg::UPDATE_DATA, controller, 4];
            if length > 0 {
                update.extend_from_slice(unsafe { std::slice::from_raw_parts(bytes, length as usize) });
            }
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

    /// A duel that starts from a staged board instead of two shuffled decks:
    /// `cards` are put in place, nobody draws an opening hand, and seat 0
    /// takes the first turn with attacks allowed (Master Rule 1 otherwise).
    /// Both players start at [`STAGED_LIFE_POINTS`], so that a few attacks
    /// in a row do not end the duel.
    /// The seats answer in process.  `watch` sees every message, and every
    /// selection with the monster zones before it is answered; it returns
    /// `false` to end the duel there.  Returns the monster zones at the end.
    pub fn stage(
        &mut self,
        cards: &[Placed],
        seed: u64,
        seats: &mut [ygo_policies_ocgcore::Seat; 2],
        limit: usize,
        watch: &mut dyn FnMut(Step) -> bool,
    ) -> Result<Vec<Monster>> {
        self.resources.errors.clear();
        let payload = (&mut *self.resources) as *mut Resources as Handle;
        let player = Player { lp: STAGED_LIFE_POINTS, draw: 0, per_turn: 1 };
        let options = Options {
            seed: [seed, 2, 3, 4],
            // Master Rule 1 plus DUEL_ATTACK_FIRST_TURN.
            flags: 0xD0700 | 0x02,
            p0: player,
            p1: player,
            reader,
            payload1: payload,
            scripts,
            payload2: payload,
            log,
            payload3: payload,
            done,
            payload4: payload,
            unsafe_libraries: 0,
        };
        let failed = |e: ygo_policies_ocgcore::ProtocolError| e.to_string();
        unsafe {
            let mut handle = ptr::null_mut();
            let status = (self.create)(&mut handle, &options);
            if status != 0 {
                return Err(format!("OCG_CreateDuel: {status}"));
            }
            let _duel = DuelGuard { handle, destroy: self.destroy };
            for name in ["constant.lua", "utility.lua"] {
                let name = CString::new(name).unwrap();
                if scripts(payload, handle, name.as_ptr()) == 0 {
                    return Err(format!("Failed to load {name:?}"));
                }
            }
            let count = |p: u8, l: u32| cards.iter().filter(|c| c.controller == p && c.location == l).count() as u16;
            for (p, seat) in seats.iter_mut().enumerate() {
                let start = message::start_message(
                    p as u8,
                    [STAGED_LIFE_POINTS; 2],
                    [count(0, 1), count(1, 1)],
                    [count(0, 0x40), count(1, 0x40)],
                );
                seat.feed(&start).map_err(failed)?;
            }
            for card in cards {
                (self.new_card)(
                    handle,
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
            (self.start)(handle);
            let mut decisions = 0;
            loop {
                let status = (self.process)(handle);
                let mut length = 0;
                let bytes = (self.messages)(handle, &mut length);
                let buffer = if length == 0 { vec![] } else { std::slice::from_raw_parts(bytes, length as usize).to_vec() };
                let mut offset = 0;
                let mut response = None;
                let mut running = true;
                while running && offset < buffer.len() {
                    let n = u32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap()) as usize;
                    let raw = buffer.get(offset + 4..offset + 4 + n).ok_or("Truncated message")?;
                    offset += 4 + n;
                    let id = *raw.first().ok_or("Empty message")?;
                    let parsed = message::parse(raw).map_err(failed)?;
                    if matches!(parsed, Message::Retry) {
                        return Err("Engine rejected response (MSG_RETRY)".into());
                    }
                    running = watch(Step::Message(&parsed, raw)) && !matches!(parsed, Message::Win { .. });
                    if running && message::is_selection(id) {
                        running = watch(Step::Prompt(&parsed, &self.monsters(handle)));
                        if running {
                            for p in 0..2 {
                                for loc in [2, 4, 8, 16, 32, 64] {
                                    let q = Query { flags: query::RECOMMENDED, controller: p, location: loc, sequence: 0, overlay: 0 };
                                    let bytes = (self.query)(handle, &mut length, &q);
                                    let mut update = vec![msg::UPDATE_DATA, p, loc as u8];
                                    if length > 0 {
                                        update.extend_from_slice(std::slice::from_raw_parts(bytes, length as usize));
                                    }
                                    for seat in seats.iter_mut() {
                                        seat.feed(&update).map_err(failed)?;
                                    }
                                }
                            }
                        }
                    }
                    if running {
                        for seat in seats.iter_mut() {
                            if let Some(answer) = seat.feed(raw).map_err(failed)? {
                                response = Some(answer);
                            }
                        }
                    }
                }
                if !self.resources.errors.is_empty() {
                    return Err(self.resources.errors.join("\n"));
                }
                if !running || status == 0 || decisions >= limit {
                    return Ok(self.monsters(handle));
                }
                match response {
                    Some(answer) => {
                        (self.respond)(handle, answer.as_ptr(), answer.len() as u32);
                        decisions += 1;
                    }
                    None if status == 1 => return Err("Engine awaiting with no answer".into()),
                    None => {}
                }
            }
        }
    }

    pub fn play(
        &mut self,
        decks: &[Deck; 2],
        policies: [&PolicyLibrary; 2],
        names: [&str; 2],
        cards: &Path,
        run: PlayOptions,
    ) -> Result<serde_json::Value> {
        let PlayOptions { seed, limit, trace, life_points } = run;
        self.resources.errors.clear();
        for code in decks.iter().flat_map(|d| d.main.iter().chain(&d.extra)) {
            if !self.resources.cards.contains_key(code) {
                return Err(format!("Deck card {code} is absent from the database"));
            }
        }
        let payload = (&mut *self.resources) as *mut Resources as Handle;
        let player = |lp| Player {
            lp,
            draw: 5,
            per_turn: 1,
        };
        let options = Options {
            seed: [
                seed,
                (seed.wrapping_mul(1103515245) + 12345) & 0xffffffff,
                3,
                4,
            ],
            flags: 0xD0700,
            p0: player(life_points[0]),
            p1: player(life_points[1]),
            reader,
            payload1: payload,
            scripts,
            payload2: payload,
            log,
            payload3: payload,
            done,
            payload4: payload,
            unsafe_libraries: 0,
        };
        unsafe {
            let mut handle = ptr::null_mut();
            let status = (self.create)(&mut handle, &options);
            if status != 0 {
                return Err(format!("OCG_CreateDuel: {status}"));
            }
            let _duel = DuelGuard {
                handle,
                destroy: self.destroy,
            };
            for name in ["constant.lua", "utility.lua"] {
                let name = CString::new(name).unwrap();
                if scripts(payload, handle, name.as_ptr()) == 0 {
                    return Err(format!("Failed to load {name:?}"));
                }
            }
            let mut seats = Vec::new();
            let mut rng = seed;
            for p in 0..2 {
                seats.push(policies[p].seat(names[p], cards, p as i32, seed + p as u64)?);
                seats[p].feed(&message::start_message(
                    p as u8,
                    life_points,
                    [decks[0].main.len() as u16, decks[1].main.len() as u16],
                    [decks[0].extra.len() as u16, decks[1].extra.len() as u16],
                ))?;
                let mut main = decks[p].main.clone();
                shuffle(&mut main, &mut rng);
                for (location, pile) in [(1, &main), (0x40, &decks[p].extra)] {
                    for code in pile {
                        (self.new_card)(
                            handle,
                            &NewCard {
                                team: p as u8,
                                duelist: 0,
                                code: *code,
                                controller: p as u8,
                                location,
                                sequence: 0,
                                position: 8,
                            },
                        );
                    }
                }
            }
            (self.start)(handle);
            let mut decisions = 0;
            let mut winner = None;
            let (mut reason, mut turns) = (None, 0u32);
            let mut digest = 0xcbf29ce484222325u64;
            let mut traces = Vec::new();
            let mut activations = HashMap::<u32, u32>::new();
            loop {
                let status = (self.process)(handle);
                let mut length = 0;
                let bytes = (self.messages)(handle, &mut length);
                let buffer = if length == 0 {
                    vec![]
                } else {
                    std::slice::from_raw_parts(bytes, length as usize).to_vec()
                };
                let mut offset = 0;
                let mut response = None;
                while offset < buffer.len() {
                    if offset + 4 > buffer.len() {
                        return Err("Truncated message length".into());
                    }
                    let n =
                        u32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap()) as usize;
                    let message = buffer
                        .get(offset + 4..offset + 4 + n)
                        .ok_or("Truncated message")?;
                    offset += 4 + n;
                    let id = *message.first().ok_or("Empty message")?;
                    if id == msg::RETRY {
                        return Err("Engine rejected response (MSG_RETRY)".into());
                    }
                    if id == msg::WIN {
                        winner = message.get(1).copied();
                        // 1: Life Points, 2: deck-out, 0x10 and up: a card's own win condition.
                        reason = message.get(2).copied();
                    }
                    if id == msg::NEW_TURN {
                        turns += 1;
                    }
                    if id == msg::CHAINING && message.len() >= 5 {
                        *activations
                            .entry(u32::from_le_bytes(message[1..5].try_into().unwrap()))
                            .or_default() += 1;
                    }
                    if message::is_selection(id) {
                        for p in 0..2 {
                            for loc in [2, 4, 8, 16, 32, 64] {
                                let q = Query {
                                    flags: query::RECOMMENDED,
                                    controller: p,
                                    location: loc,
                                    sequence: 0,
                                    overlay: 0,
                                };
                                let bytes = (self.query)(handle, &mut length, &q);
                                let mut update = vec![msg::UPDATE_DATA, p, loc as u8];
                                if length > 0 {
                                    update.extend_from_slice(std::slice::from_raw_parts(
                                        bytes,
                                        length as usize,
                                    ));
                                }
                                for seat in &seats {
                                    seat.feed(&update)?;
                                }
                            }
                        }
                    }
                    for seat in &seats {
                        if let Some(answer) = seat.feed(message)? {
                            if response.is_some() {
                                return Err("Multiple answers in one engine batch".into());
                            }
                            for byte in message.iter().chain(&answer) {
                                digest = (digest ^ *byte as u64).wrapping_mul(0x100000001b3);
                            }
                            if trace {
                                traces.push(seat.last_answer()?);
                            }
                            response = Some(answer);
                        }
                    }
                }
                if !self.resources.errors.is_empty() {
                    return Err(self.resources.errors.join("\n"));
                }
                if winner.is_some() || status == 0 || decisions >= limit {
                    if status == 0 && winner.is_none() {
                        return Err("Engine ended without MSG_WIN".into());
                    }
                    return Ok(serde_json::json!({"winner": winner, "reason": reason, "turns": turns, "decisions": decisions,
                        "limit": winner.is_none(), "digest": format!("{digest:016x}"), "trace": traces, "activations": activations}));
                }
                if let Some(answer) = response {
                    (self.respond)(handle, answer.as_ptr(), answer.len() as u32);
                    decisions += 1;
                } else if status == 1 {
                    return Err("Engine awaiting with no policy answer".into());
                }
            }
        }
    }
}

struct DuelGuard {
    handle: Handle,
    destroy: unsafe extern "C" fn(Handle),
}
impl Drop for DuelGuard {
    fn drop(&mut self) {
        unsafe { (self.destroy)(self.handle) }
    }
}

pub struct PolicyLibrary {
    _library: Library,
    create: unsafe extern "C" fn(*const c_char, *const c_char, i32, u64) -> Handle,
    destroy: unsafe extern "C" fn(Handle),
    feed: unsafe extern "C" fn(Handle, *const u8, usize) -> i32,
    response: unsafe extern "C" fn(Handle, *mut usize) -> *const u8,
    error: unsafe extern "C" fn() -> *const c_char,
    answer: unsafe extern "C" fn(Handle) -> *const c_char,
    pub catalog: HashMap<String, String>,
}

impl PolicyLibrary {
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
                response: symbol(&library, b"ygo_policy_response")?,
                error: symbol(&library, b"ygo_policy_last_error")?,
                answer: symbol(&library, b"ygo_policy_last_answer_json")?,
                catalog,
                _library: library,
            })
        }
    }

    fn seat(&self, name: &str, cards: &Path, player: i32, seed: u64) -> Result<Seat<'_>> {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let cards = CString::new(cards.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
        let handle = unsafe { (self.create)(name.as_ptr(), cards.as_ptr(), player, seed) };
        if handle.is_null() {
            return Err(self.last_error());
        }
        Ok(Seat {
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

struct Seat<'a> {
    library: &'a PolicyLibrary,
    handle: Handle,
}
impl Seat<'_> {
    fn feed(&self, message: &[u8]) -> Result<Option<Vec<u8>>> {
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
    fn last_answer(&self) -> Result<serde_json::Value> {
        unsafe {
            let answer = (self.library.answer)(self.handle);
            if answer.is_null() {
                return Err(self.library.last_error());
            }
            serde_json::from_slice(CStr::from_ptr(answer).to_bytes()).map_err(|e| e.to_string())
        }
    }
}
impl Drop for Seat<'_> {
    fn drop(&mut self) {
        unsafe { (self.library.destroy)(self.handle) }
    }
}

#[derive(Clone)]
pub struct Deck {
    pub main: Vec<u32>,
    pub extra: Vec<u32>,
}
impl Deck {
    pub fn load(path: &Path) -> Result<Self> {
        let mut deck = Deck {
            main: vec![],
            extra: vec![],
        };
        let mut section = "";
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        for line in text.lines().map(str::trim) {
            match line {
                "#main" | "#extra" | "!side" => section = line,
                _ => {
                    if let Ok(code) = line.parse() {
                        match section {
                            "#main" => deck.main.push(code),
                            "#extra" => deck.extra.push(code),
                            _ => {}
                        }
                    }
                }
            }
        }
        if deck.main.is_empty() {
            return Err("Empty main deck".into());
        }
        Ok(deck)
    }
}

fn shuffle(cards: &mut [u32], state: &mut u64) {
    for i in (1..cards.len()).rev() {
        *state = state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        cards.swap(i, z as usize % (i + 1));
    }
}
