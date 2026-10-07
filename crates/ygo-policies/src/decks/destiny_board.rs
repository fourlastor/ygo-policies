//! The Final Sentence: survive four opposing End Phases while reserving
//! backrow space for Destiny Board's messages. Hand shields keep slots free.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
pub const DECK: &str = "The Final Sentence";
const BOARD: u32 = 94212438;
const LETTERS: [u32; 4] = [31893528, 67287533, 94772232, 30170981];
const UPSTART: u32 = 70368879;
const DUALITY: u32 = 98645731;
const GOLD: u32 = 75500286;
const PEACE: u32 = 44656491;
const CAGE: u32 = 58775978;
const SWORDS: u32 = 72302403;
const RECKLESS: u32 = 37576645;
const VALLEY: u32 = 3657444;
const MARSH: u32 = 31305911;
const REAPER: u32 = 23205979;
#[derive(Clone, Default)]
pub struct DestinyBoard;
impl DestinyBoard {
    fn letters(ctx: &Ctx) -> usize {
        ctx.spell_traps(ctx.me)
            .iter()
            .filter(|c| {
                c.position.face_up
                    && c.code
                        .map_or(false, |k| LETTERS.contains(&ctx.canonical(k)))
            })
            .count()
    }
    fn board(ctx: &Ctx) -> bool {
        ctx.face_up_on_field(ctx.me, BOARD)
    }
}
impl Strategy for DestinyBoard {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            BOARD => {
                if Self::board(ctx) {
                    500
                } else {
                    6000
                }
            }
            PEACE | CAGE | SWORDS => 2500,
            VALLEY | MARSH | REAPER => 1900,
            _ if LETTERS.contains(&code) => 1500,
            _ => return None,
        })
    }
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if ctx.deck_size(ctx.me) > 4 {
            for code in [UPSTART, DUALITY] {
                if let Some(i) = t.activate(code) {
                    return t.pick(i);
                }
            }
        }
        if !Self::board(&ctx) && !ctx.in_hand(BOARD) {
            if let Some(i) = t.activate(GOLD) {
                return t.pick(i);
            }
        }
        if !Self::board(&ctx) {
            if let Some(i) = t.activate(BOARD) {
                return t.pick(i);
            }
        }
        let n = Self::letters(&ctx);
        if n < 3 && !ctx.face_up_on_field(ctx.me, PEACE) {
            if let Some(i) = t.activate(PEACE) {
                return t.pick(i);
            }
        }
        if n < 2 && !ctx.face_up_on_field(ctx.me, CAGE) && !ctx.face_up_on_field(ctx.me, SWORDS) {
            if let Some(i) = t.activate(CAGE) {
                return t.pick(i);
            }
        }
        None
    }
    fn summon_score(&self, _: &Turn, c: &Choice) -> Option<Option<f64>> {
        Some(match (c.kind, c.code()?) {
            (ChoiceKind::NormalSummon, VALLEY) => Some(2200.0),
            (ChoiceKind::SetMonster, MARSH | REAPER) => Some(2400.0),
            _ => None,
        })
    }
    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        let c = t.choice(i);
        Some(match ctx.canonical(c.code()?) {
            BOARD if !Self::board(&ctx) || c.description == ((BOARD as u64) << 20) => {
                Response::new(130.0)
            }
            RECKLESS if ctx.deck_size(ctx.me) > 3 => Response::new(120.0),
            BOARD | RECKLESS | UPSTART | DUALITY | GOLD | PEACE | CAGE => Response::no(),
            _ => return support::stall_chain(t, i),
        })
    }
    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        if m.at.controller != t.ctx.me {
            return None;
        }
        if t.memory.last_activated == Some(GOLD) && t.decision.hint == Hint::Banish {
            return Some(if m.code == Some(BOARD) { 6000.0 } else { 0.0 });
        }
        None
    }
    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let ctx = t.ctx;
        if LETTERS.contains(&code) {
            return Some(false);
        }
        if code == BOARD {
            return Some(
                !Self::board(&ctx) && ctx.count_in(ctx.me, Location::SpellTrapZone, BOARD) == 0,
            );
        }
        // Keep a slot for the next message; spent draw traps free theirs again.
        Some(
            ctx.data(code).is_trap()
                && ctx.spell_traps(ctx.me).len() < if Self::board(&ctx) { 4 } else { 3 },
        )
    }
    fn allow_staple(&self, t: &Turn, code: u32) -> bool {
        code != SWORDS || Self::letters(&t.ctx) < 2
    }
    fn yes_no(&self, t: &Turn) -> Option<bool> {
        (t.decision.subject == Some(PEACE)
            || t.choices()
                .any(|(_, c)| c.description == (PEACE as u64) << 20))
        .then(|| Self::letters(&t.ctx) < 3)
    }
    fn wants_battle(&self, _: &Turn) -> Option<bool> {
        Some(false)
    }
}
