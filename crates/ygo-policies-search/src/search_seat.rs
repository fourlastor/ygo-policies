//! The staged search as one more seat of a duel its host steps.
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use serde_json::json;
use ygo_policies_duel::Duel;
use ygo_policies_ocgcore::message;
use ygo_policies_ocgcore::wire::msg;

use crate::memory::PublicIdentityMemory;
use crate::seat::{PilotSeat, Pilots};
use crate::staged::{candidates, choose_staged, unknown_set_monsters, unsafe_selection, Context, StagedOptions, Stats};
use crate::Result;

/// How a [`SearchSeat`] searches.
#[derive(Clone, Copy, Debug)]
pub struct SearchOptions {
    pub staged: StagedOptions,
    /// Alternatives tried at a decision, the pilot's answer included.
    pub candidates: usize,
    /// Answers given in the duel, by both players, after which a world is a draw.
    pub limit: usize,
}

impl SearchOptions {
    /// What `policy-bench generic --method staged` played in the measured
    /// runs (benchmarks/generic-policy-v3, `--limit 8192`).
    pub const MEASURED: Self = Self { staged: StagedOptions::MEASURED, candidates: 8, limit: 8192 };

    /// [`SearchOptions::MEASURED`], with what `options` (a JSON object) sets
    /// of `stages` (three counts), `z`, `kept`, `chains`, `set_monsters`,
    /// `candidates` and `limit`.
    pub fn from_json(options: &serde_json::Value) -> Result<Self> {
        let mut out = Self::MEASURED;
        let count = |key: &str, value: &serde_json::Value| {
            value.as_u64().map(|count| count as usize).ok_or_else(|| format!("Search option {key} is a count"))
        };
        let flag = |key: &str, value: &serde_json::Value| {
            value.as_bool().ok_or_else(|| format!("Search option {key} is true or false"))
        };
        for (key, value) in options.as_object().ok_or("Search options are a JSON object")? {
            match key.as_str() {
                "stages" => {
                    out.staged.stages = serde_json::from_value(value.clone())
                        .map_err(|_| "Search option stages is three counts")?;
                }
                "z" => out.staged.z = value.as_f64().ok_or("Search option z is a number")?,
                "kept" => out.staged.kept = count(key, value)?,
                "chains" => out.staged.chains = flag(key, value)?,
                "set_monsters" => out.staged.set_monsters = flag(key, value)?,
                "candidates" => out.candidates = count(key, value)?,
                "limit" => out.limit = count(key, value)?,
                _ => return Err(format!("Unknown search option {key:?}")),
            }
        }
        Ok(out)
    }
}

/// What a [`SearchSeat`] has done so far.
#[derive(Default)]
pub struct SeatStats {
    /// Questions its pilot answered, and those searched.
    pub asked: u64,
    pub searched: u64,
    /// Decisions with alternatives left to the pilot: inside a chain
    /// (without `chains`), facing an unknown Set monster (without
    /// `set_monsters`), facing a face-down banished card, or a selection
    /// whose cards the worlds could move.
    pub in_chain: u64,
    pub facing_set: u64,
    pub facedown_banished: u64,
    pub unsafe_selections: u64,
    /// Searches that dealt the other player's Set monsters again.
    pub resampled_set_monsters: u64,
    /// Each answer that is not the pilot's: the decision and turn, what it
    /// replaced with what, on how many worlds and by how much.
    pub deviations: Vec<serde_json::Value>,
    /// What each search took, in milliseconds.
    pub search_ms: Vec<f64>,
    pub search: Stats,
}

impl SeatStats {
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "asked": self.asked, "searched": self.searched, "in_chain": self.in_chain, "facing_set": self.facing_set,
            "search_skipped_facedown_banished": self.facedown_banished, "unsafe_selections": self.unsafe_selections,
            "resampled_set_monsters": self.resampled_set_monsters, "deviations": self.deviations,
            "search_decision_ms": self.search_ms, "playouts": self.search.playouts,
            "failed_playouts": self.search.failed_playouts, "unresolved_playouts": self.search.unresolved_playouts,
            "failed_searches": self.search.failed_searches, "undealt_roots": self.search.undealt_roots,
            "search_errors": self.search.errors,
        })
    }
}

/// The generic policy with the staged search, in one seat of the host's
/// duel.  It is fed every message of the duel, its own `MSG_START` first,
/// like any other seat, and answers each of its seat's questions as
/// `policy-bench generic --method staged` does in that seat: the same
/// pilot, the same guards, the same worlds.
///
/// It plays its worlds on the host's duel itself: while it answers it
/// snapshots the duel, deals and plays every world, and restores it before
/// the answer comes back.  The host must not hold the duel meanwhile (take
/// each [`Duel::step`] before feeding its messages), and dropping the seat
/// leaves the duel as it is.
pub struct SearchSeat<'a, P: Pilots> {
    duel: Rc<RefCell<Duel<'a>>>,
    context: Context<P>,
    options: SearchOptions,
    pilot: P::Seat,
    /// Everything the other player's seat would have been fed, from its own
    /// `MSG_START`: the public history its model is built from.
    history: Vec<u8>,
    memory: PublicIdentityMemory,
    chain: bool,
    /// Answers given in the duel so far, by both players.
    decisions: usize,
    turns: u32,
    stats: SeatStats,
}

impl<'a, P: Pilots> SearchSeat<'a, P> {
    /// Seat `viewer` of `duel`, the generic pilot from `pilots` seeded with
    /// `seed + viewer` and the worlds dealt from `seed`, as `play_fast` seats
    /// a searcher in a duel of that seed.  `profiles` describes the effects
    /// of the seat's own cards (`Core::effect_profiles`) to its pilot and to
    /// the other player's model.
    pub fn new(
        duel: Rc<RefCell<Duel<'a>>>,
        pilots: P,
        viewer: u8,
        seed: u64,
        profiles: Option<serde_json::Value>,
        options: SearchOptions,
    ) -> Result<Self> {
        let staged = options.staged;
        if viewer > 1 {
            return Err("A search seat is seat 0 or 1".into());
        }
        if staged.stages[0] == 0 || staged.stages[0] > staged.stages[1] || staged.stages[1] > staged.stages[2]
            || staged.kept == 0 || !staged.z.is_finite() {
            return Err("Staged search needs 0 < first stage <= middle stage <= last stage, kept > 0 and a finite z".into());
        }
        if options.candidates < 2 || options.limit == 0 {
            return Err("A search seat needs at least two candidates and a positive answer limit".into());
        }
        if !duel.try_borrow().map_err(|_| "The duel is in use")?.can_search() {
            return Err("A search seat needs an engine with snapshots, the hidden-card swap and random reseeding, \
                which lends its log handler for a duel it did not make".into());
        }
        let pilot = pilots.seat("generic", viewer, seed + viewer as u64)?;
        if let Some(profiles) = &profiles {
            pilot.set_effect_profiles(profiles)?;
        }
        pilot.enable_response_overrides()?;
        let context = Context {
            pilots, model_name: "generic".into(), viewer, seed, limit: options.limit, horizon: 0, profiles,
            specialist_opponent: false, adversarial_opponent: false,
        };
        Ok(Self {
            duel, context, options, pilot, history: Vec::new(), memory: PublicIdentityMemory::default(),
            chain: false, decisions: 0, turns: 0, stats: SeatStats::default(),
        })
    }

    /// Feed one message of the duel (id first).  When it asks this seat to
    /// decide, the response for the engine comes back, searched.
    pub fn feed(&mut self, message: &[u8]) -> Result<Option<Vec<u8>>> {
        let id = *message.first().ok_or("Empty message")?;
        self.memory.observe(message)?;
        let start = self.history.len();
        self.history.extend_from_slice(&(message.len() as u32).to_le_bytes());
        self.history.extend_from_slice(message);
        if id == msg::START {
            // The other seat's MSG_START is the same, from its own side.
            if let Some(seat) = self.history.get_mut(start + 5) {
                *seat = 1 - self.context.viewer;
            }
        }
        match id {
            msg::NEW_TURN => self.turns += 1,
            msg::CHAINING => self.chain = true,
            msg::CHAIN_END => self.chain = false,
            _ => {}
        }
        let answer = self.pilot.feed(message)?;
        // One answer follows each question, and each refused answer.
        if !message::is_selection(id) && id != msg::RETRY {
            return Ok(answer);
        }
        let decisions = self.decisions;
        self.decisions += 1;
        match answer {
            Some(answer) if id != msg::RETRY => {
                self.stats.asked += 1;
                self.decide(answer, decisions).map(Some)
            }
            answer => Ok(answer),
        }
    }

    /// Describe the effects of the seat's own cards (`Core::effect_profiles`)
    /// to its pilot and to the other player's model, before its first question.
    pub fn set_effect_profiles(&mut self, profiles: serde_json::Value) -> Result<()> {
        self.pilot.set_effect_profiles(&profiles)?;
        self.context.profiles = Some(profiles);
        Ok(())
    }

    /// What the seat has done so far.
    pub fn stats(&self) -> &SeatStats {
        &self.stats
    }

    /// The pilot, waiting on the seat's last question: what it saw, the
    /// choices it had and the answer the seat gave (`last_answer`).
    pub fn pilot(&self) -> &P::Seat {
        &self.pilot
    }

    /// `play_fast`'s decision with the staged search: the pilot's answer,
    /// unless an alternative wins clearly more often over the same worlds.
    fn decide(&mut self, answer: Vec<u8>, decisions: usize) -> Result<Vec<u8>> {
        let started = Instant::now();
        let (viewer, staged) = (self.context.viewer, self.options.staged);
        let original = self.pilot.last_answer()?;
        let described = self.pilot.alternative_responses(self.options.candidates)?;
        let Some(listed) = candidates(&described, &answer, self.options.candidates) else { return Ok(answer) };
        let observation = &original["observation"];
        let unknown =
            if staged.set_monsters { unknown_set_monsters(&self.memory, observation, viewer) } else { Vec::new() };
        let stats = &mut self.stats;
        if self.chain && !staged.chains {
            stats.in_chain += 1;
        } else if !staged.set_monsters && self.memory.has_unknown_monsters(observation, viewer) {
            stats.facing_set += 1;
        } else if self.memory.has_facedown_banished(observation, viewer) {
            stats.facedown_banished += 1;
        } else if unsafe_selection(&described, viewer) {
            stats.unsafe_selections += 1;
        } else {
            stats.searched += 1;
            stats.resampled_set_monsters += u64::from(!unknown.is_empty());
            let mut duel = self.duel.try_borrow_mut().map_err(|_| "The host holds the duel while its search seat answers")?;
            let (context, pilot, history) = (&self.context, &self.pilot, &self.history);
            let (chosen, worlds, gain) = duel.with_logs(|duel| {
                choose_staged(duel, context, pilot, history, &listed, decisions, &mut stats.search, staged, unknown)
            })??;
            stats.search_ms.push(started.elapsed().as_secs_f64() * 1000.0);
            if chosen != 0 {
                self.pilot.override_response(&listed.responses[chosen])?;
                stats.deviations.push(json!({"decision": decisions, "turn": self.turns, "kind": listed.kind,
                    "from": listed.labels[0], "to": listed.labels[chosen], "worlds": worlds, "gain": gain}));
                return Ok(listed.responses[chosen].clone());
            }
        }
        Ok(answer)
    }
}
