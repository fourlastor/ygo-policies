//! Seats played in this process: the policies this build is made of, with
//! no library to load and no C ABI on the way.
use std::cell::{Ref, RefCell};
use std::sync::Arc;

use serde_json::json;
use ygo_policies::cards::CardDatabase;
use ygo_policies::registry;
use ygo_policies_ocgcore::Seat;

use crate::seat::{PilotSeat, Pilots};
use crate::Result;

/// The last decision a seat answered, as the C ABI's
/// `ygo_policy_last_answer_json` writes it: what it observed, the choices it
/// had, their responses and the one it took.  `null` before any.
pub fn answer_json(seat: &Seat) -> serde_json::Value {
    match seat.last_answer() {
        None => serde_json::Value::Null,
        Some(answer) => json!({
            "observation": answer.observation,
            "decision": answer.decision,
            "choice": answer.choice,
            "responses": answer.responses,
            "picks": answer.picks,
        }),
    }
}

/// One player, played by a policy of this build.  The seat sits in a
/// `RefCell` because a [`PilotSeat`] is fed through a shared reference, as
/// a library's handle is.
pub struct LocalSeat(RefCell<Seat>);

impl LocalSeat {
    pub fn new(seat: Seat) -> Self {
        Self(RefCell::new(seat))
    }

    /// The seat itself, to read.
    pub fn seat(&self) -> Ref<'_, Seat> {
        self.0.borrow()
    }
}

impl PilotSeat for LocalSeat {
    fn feed(&self, message: &[u8]) -> Result<Option<Vec<u8>>> {
        self.0.borrow_mut().feed(message).map_err(|e| e.to_string())
    }
    fn feed_update_pair(&self, other: &Self, message: &[u8]) -> Result<()> {
        let (mut first, mut second) = (self.0.try_borrow_mut(), other.0.try_borrow_mut());
        let (Ok(first), Ok(second)) = (first.as_deref_mut(), second.as_deref_mut()) else {
            return Err("update pair requires distinct seats".into());
        };
        Seat::feed_update_pair(first, second, message).map_err(|e| e.to_string())
    }
    fn feed_buffer(&self, buffer: &[u8]) -> Result<()> {
        self.0.borrow_mut().feed_buffer(buffer).map(|_| ()).map_err(|e| e.to_string())
    }
    fn fork(&self) -> Result<Self> {
        self.0.borrow().fork().map(Self::new).ok_or_else(|| "policy does not support copying".into())
    }
    fn enable_response_overrides(&self) -> Result<()> {
        self.0.borrow_mut().enable_response_overrides().map_err(|e| e.to_string())
    }
    fn override_response(&self, response: &[u8]) -> Result<()> {
        self.0.borrow_mut().override_response(response).map_err(|e| e.to_string())
    }
    fn alternative_responses(&self, maximum: usize) -> Result<serde_json::Value> {
        let mut seat = self.0.borrow_mut();
        seat.alternative_responses(maximum).map_err(|e| e.to_string())?;
        Ok(answer_json(&seat))
    }
    fn last_answer(&self) -> Result<serde_json::Value> {
        Ok(answer_json(&self.0.borrow()))
    }
    fn observation(&self) -> Result<serde_json::Value> {
        serde_json::to_value(self.0.borrow().observation()).map_err(|e| e.to_string())
    }
    fn set_effect_profiles(&self, profiles: &serde_json::Value) -> Result<()> {
        let profiles = serde_json::from_value(profiles.clone()).map_err(|e| format!("effect profiles: {e}"))?;
        self.0.borrow_mut().set_effect_profiles(profiles).map_err(|e| e.to_string())
    }
}

/// The policies of this build, reading the card database `cards`.
#[derive(Clone)]
pub struct LocalPilots {
    pub cards: Arc<dyn CardDatabase>,
}

impl Pilots for LocalPilots {
    type Seat = LocalSeat;
    fn seat(&self, name: &str, player: u8, seed: u64) -> Result<LocalSeat> {
        let policy =
            registry::create_seeded(name, self.cards.clone(), seed).ok_or_else(|| format!("unknown policy {name:?}"))?;
        Ok(LocalSeat::new(Seat::new(policy, self.cards.clone(), Some(player))))
    }
}
