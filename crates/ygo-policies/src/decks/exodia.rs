//! Normal Monsters defend while multiple Hearts draw through the deck.
//! Reload restarts a stopped Draw Phase sequence. Keep the five pieces,
//! one lasting attack lock, and room for both the draw engine and Skill Drain.
use super::countdown::Countdown;
use crate::agent::{Response, Strategy, Turn};
use crate::cards::types;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Location, Member, Phase, Position};
pub const DECK: &str = "Astra's Exodia";
const PIECES: &[u32] = &[33396948, 70903634, 7902349, 8124921, 44519536];
const HEART: u32 = 35762283;
const RELOAD: u32 = 22589918;
const DUALITY: u32 = 98645731;
const DRAIN: u32 = 82732705;
const PEACE: u32 = 44656491;
const LEVEL: u32 = 3136426;
const GRAVITY: u32 = 85742772;
#[derive(Clone, Default)]
pub struct Exodia {
    protection: Countdown,
}
impl Exodia {
    fn locked(ctx: &Ctx) -> bool {
        [PEACE, LEVEL, GRAVITY]
            .iter()
            .any(|&c| ctx.face_up_on_field(ctx.me, c))
    }
    fn hearts(ctx: &Ctx) -> usize {
        ctx.spell_traps(ctx.me)
            .iter()
            .filter(|c| ctx.is(c, HEART) && c.position.face_up)
            .count()
    }
    fn pieces(ctx: &Ctx) -> usize {
        ctx.hand_codes()
            .iter()
            .filter(|c| PIECES.contains(c))
            .count()
    }
}
impl Strategy for Exodia {
    fn value(&self, ctx: &Ctx, c: u32) -> Option<i32> {
        Some(match c {
            c if PIECES.contains(&c) => 100_000,
            HEART => {
                if Self::hearts(ctx) < 2 {
                    9000
                } else {
                    6000
                }
            }
            DUALITY => 5000,
            RELOAD => 4000,
            c if ctx.data(c).kind & (types::MONSTER | types::NORMAL)
                == (types::MONSTER | types::NORMAL) =>
            {
                ctx.data(c).defense
            }
            _ => return self.protection.value(ctx, c),
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if Self::hearts(&ctx) == 0 {
            if let Some(i) = t.activate(HEART) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(DUALITY) {
            return t.pick(i);
        }
        // One lasting attack lock leaves room for multiple Hearts and Skill Drain.
        if !Self::locked(&ctx) {
            for c in [GRAVITY, PEACE, LEVEL] {
                if let Some(i) = t.activate(c) {
                    return t.pick(i);
                }
            }
        }
        let drain_waiting =
            ctx.in_hand(DRAIN) && !ctx.spell_traps(ctx.me).iter().any(|c| ctx.is(c, DRAIN));
        if ctx.spell_traps(ctx.me).len() + usize::from(drain_waiting) < 4 {
            if let Some(i) = t.activate(HEART) {
                return t.pick(i);
            }
        }
        if Self::hearts(&ctx) == 0 && !ctx.in_hand(HEART) && ctx.hand_size(ctx.me) >= 3 {
            if let Some(i) = t.activate(RELOAD) {
                return t.pick(i);
            }
        }
        None
    }
    fn allow_staple(&self, t: &Turn, c: u32) -> bool {
        c != RELOAD && self.protection.allow_staple(t, c)
    }
    fn wants_battle(&self, _t: &Turn) -> Option<bool> {
        Some(false)
    }
    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let code = c.code()?;
        if PIECES.contains(&code) {
            return Some(None);
        }
        if t.ctx.data(code).kind & (types::MONSTER | types::NORMAL)
            == (types::MONSTER | types::NORMAL)
        {
            return Some(
                (c.kind == ChoiceKind::SetMonster).then_some(t.ctx.data(code).defense as f64),
            );
        }
        self.protection.summon_score(t, c)
    }
    fn allow_reposition(&self, _t: &Turn, _c: &CardView) -> bool {
        false
    }
    fn position(&self, _t: &Turn, _c: u32) -> Option<Position> {
        Some(Position::FACE_UP_DEFENSE)
    }
    fn set_spell_trap(&self, t: &Turn, c: u32) -> Option<bool> {
        let ctx = &t.ctx;
        let used = ctx.spell_traps(ctx.me).len();
        if c == DRAIN {
            return Some(used < 4 && !ctx.spell_traps(ctx.me).iter().any(|v| ctx.is(v, DRAIN)));
        }
        if c == GRAVITY {
            return Some(
                used < 4
                    && !Self::locked(ctx)
                    && !ctx.spell_traps(ctx.me).iter().any(|v| ctx.is(v, GRAVITY)),
            );
        }
        self.protection.set_spell_trap(t, c)
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let c = t.choice(i).code()?;
        let ctx = &t.ctx;
        if c == HEART {
            let pending = ctx
                .obs
                .chain
                .iter()
                .filter(|l| l.controller == ctx.me && l.code == HEART)
                .count();
            return Some(Response::new(if ctx.deck_size(ctx.me) > pending as u32 {
                150.0
            } else {
                0.0
            }));
        }
        if c == RELOAD {
            let go = ctx.my_turn()
                && ctx.phase() == Some(Phase::Draw)
                && Self::hearts(ctx) > 0
                && ctx.obs.chain.is_empty()
                && ctx.hand_size(ctx.me) >= 3;
            return Some(Response::new(if go { 90.0 } else { 0.0 }));
        }
        self.protection.chain(t, i)
    }
    fn yes_no(&self, t: &Turn) -> Option<bool> {
        if t.decision.subject == Some(HEART) {
            let pending = t
                .ctx
                .obs
                .chain
                .iter()
                .filter(|l| l.controller == t.ctx.me && l.code == HEART)
                .count();
            return Some(t.ctx.deck_size(t.ctx.me) > pending as u32);
        }
        None
    }
    fn allow_repeated_chain(&self, t: &Turn, i: usize) -> bool {
        t.choice(i).code() == Some(HEART)
            && t.ctx.my_turn()
            && t.ctx.phase() == Some(Phase::Draw)
            && matches!(
                t.decision.kind,
                crate::model::DecisionKind::Chain { triggers: true, .. }
            )
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let c = m.code?;
        let ctx = &t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        if m.at.location == Location::Hand {
            return self.value(ctx, c).map(|v| -(v as f64));
        }
        if m.at.location == Location::Deck && t.memory.last_activated == Some(DUALITY) {
            return Some(if PIECES.contains(&c) {
                if Self::pieces(ctx) >= 4 {
                    100_000.0
                } else {
                    100.0
                }
            } else {
                self.value(ctx, c).unwrap_or(500) as f64
            });
        }
        self.protection.select(t, m)
    }
}
