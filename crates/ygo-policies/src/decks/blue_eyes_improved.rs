//! Search-guided pilot for the same Blue-Eyes list; `blue_eyes` stays the baseline.
use super::support;
use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};

pub const DECK: &str = "BlueEyes";
const BLUE: u32 = 89631139;
const ALTERNATIVE: u32 = 38517737;
const WHITE: u32 = 45467446;
const ANCIENTS: u32 = 71039903;
const LEGEND: u32 = 79814787;
const SAGE: u32 = 8240199;
const MASTER: u32 = 45644898;
const MAXX: u32 = 23434538;
const VEILER: u32 = 97268402;
const RETURN: u32 = 6853254;
const TRADE: u32 = 38120068;
const CONSONANCE: u32 = 39701395;
const SHRINE: u32 = 41620959;
const MELODY: u32 = 48800175;
const TWIN: u32 = 43898403;
const WIND: u32 = 63356631;
const SPIRIT: u32 = 59822133;
const AZURE: u32 = 40908371;
const HARBINGER: u32 = 63767246;
const PRIME: u32 = 31801517;
const ARMOR: u32 = 39030163;
const LINKURIBOH: u32 = 41999284;

#[derive(Clone, Default)]
pub struct BlueEyes;

impl Strategy for BlueEyes {
    fn value(&self, _: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            ANCIENTS | LEGEND => 400,
            SAGE => 1800,
            MASTER => 600,
            BLUE | ALTERNATIVE => 3000,
            WHITE => 2400,
            SPIRIT | AZURE | HARBINGER => 4000,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Recover the missing half of Alternative's hand summon before revival
        // spends it. Ancients can also recover Alternative with Blue already held.
        let recover = if ctx.in_hand(ALTERNATIVE) && !ctx.in_hand(BLUE) {
            Some(BLUE)
        } else if ctx.in_hand(BLUE) && !ctx.in_hand(ALTERNATIVE) {
            Some(ALTERNATIVE)
        } else {
            None
        };
        if let Some(target) =
            recover.and_then(|code| ctx.graveyard(ctx.me).into_iter().find(|c| ctx.is(c, code)))
        {
            if let Some(i) = t.find(
                ChoiceKind::Activate,
                Some(ANCIENTS),
                Some(Location::Graveyard),
            ) {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        // Use the revealed Blue-Eyes before a draw spell can discard it.
        if let Some(i) = t.find(
            ChoiceKind::SpecialSummon,
            Some(ALTERNATIVE),
            Some(Location::Hand),
        ) {
            return t.pick(i);
        }
        // Draw/search next, then deploy the remaining dragons.
        for code in [CONSONANCE, TRADE, SHRINE, MELODY, RETURN, 2295440] {
            if let Some(i) = t.activate(code) {
                return t.pick(i);
            }
        }
        for (i, c) in t.choices() {
            if c.kind != ChoiceKind::Activate {
                continue;
            }
            match c.code()? {
                ALTERNATIVE
                    if ctx
                        .monsters(ctx.opp)
                        .iter()
                        .any(|c| c.attack >= 3000 || !c.position.attack) =>
                {
                    if let Some(target) = support::target(&ctx, ALTERNATIVE, true) {
                        return t.pick_targeting(i, vec![target.at]);
                    }
                }
                ANCIENTS if c.at()?.location == Location::Graveyard && !ctx.in_hand(BLUE) => {
                    return t.pick(i)
                }
                SAGE if c.at()?.location == Location::Hand
                    && ctx.monsters(ctx.me).iter().any(|c| c.attack < 1000) =>
                {
                    return t.pick(i)
                }
                MASTER
                    if c.at()?.location == Location::Graveyard
                        && ctx.monsters(ctx.me).iter().any(|c| c.attack < 1000) =>
                {
                    return t.pick(i)
                }
                ARMOR
                    if !ctx.monsters(ctx.opp).is_empty()
                        || !ctx.spell_traps(ctx.opp).is_empty() =>
                {
                    return t.pick(i)
                }
                _ => {}
            }
        }
        if !ctx.spell_traps(ctx.opp).is_empty() {
            if let Some(i) = t.activate(TWIN) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        Some(match (c.kind, c.code()?) {
            (ChoiceKind::NormalSummon, SAGE) => Some(3500.0),
            (ChoiceKind::NormalSummon, ANCIENTS | LEGEND | MASTER) => Some(2200.0),
            (ChoiceKind::NormalSummon, BLUE | WHITE) => None,
            (ChoiceKind::SetMonster, MAXX | VEILER) if ctx.monsters(ctx.me).is_empty() => {
                Some(300.0)
            }
            (_, MAXX | VEILER) => None,
            _ => return None,
        })
    }

    fn main_phase_late(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let best = t
            .choices()
            .filter(|(_, c)| {
                c.kind == ChoiceKind::SpecialSummon
                    && c.at().map(|at| at.location) == Some(Location::Extra)
            })
            .filter_map(|(i, c)| {
                let rank = match c.code()? {
                    SPIRIT => 6000,
                    AZURE => 5500,
                    ARMOR => 5000,
                    // A 3000-DEF wall also stops Blue-Eyes. Prime can beat it
                    // and supplies the material for Full Armor's follow-up.
                    PRIME
                        if ctx
                            .monsters(ctx.opp)
                            .iter()
                            .any(|c| ctx.battle_stat(c) >= 3000) =>
                    {
                        4800
                    }
                    HARBINGER if ctx.obs.turn == 1 || !ctx.main1() => 4500,
                    LINKURIBOH
                        if ctx
                            .monsters(ctx.me)
                            .iter()
                            .any(|c| ctx.is(c, ANCIENTS) || ctx.is(c, LEGEND)) =>
                    {
                        2000
                    }
                    _ => return None,
                };
                Some((rank, i))
            })
            .max_by_key(|(rank, _)| *rank)
            .map(|(_, i)| i);
        best.and_then(|i| t.pick(i))
    }

    fn special_summon(&self, _: &Turn, c: &Choice) -> Option<bool> {
        (c.at()?.location == Location::Extra).then_some(false)
    }

    fn chain(&mut self, t: &Turn, i: usize) -> Option<Response> {
        let ctx = t.ctx;
        let c = t.choice(i);
        Some(match c.code()? {
            ANCIENTS | LEGEND | SAGE | MASTER | AZURE | WHITE
                if c.at()?.location != Location::Hand =>
            {
                Response::new(120.0)
            }
            RETURN => Response::new(120.0),
            MAXX if !ctx.my_turn() => Response::new(90.0),
            VEILER if t.hostile_top().matches(|l| ctx.data(l.code).is_monster()) => {
                Response::new(90.0)
            }
            HARBINGER if t.hostile_top().matches(|l| ctx.data(l.code).is_spell()) => {
                Response::new(90.0)
            }
            SPIRIT
                if t.hostile_top()
                    .matches(|l| l.source.location == Location::Graveyard) =>
            {
                Response::new(90.0)
            }
            SPIRIT if !ctx.my_turn() && !ctx.face_up_on_field(ctx.me, AZURE) => Response::new(60.0),
            PRIME if ctx.phase().map_or(false, |p| p.is_battle()) => Response::new(90.0),
            LINKURIBOH if ctx.incoming_attack().is_some() => Response::new(90.0),
            WIND => support::target(&ctx, WIND, false)
                .map(|c| Response::targeting(90.0, vec![c.at]))
                .unwrap_or_else(Response::no),
            TWIN if !ctx.spell_traps(ctx.opp).is_empty() => Response::new(70.0),
            _ => Response::no(),
        })
    }

    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let code = m.code?;
        if m.at.controller != ctx.me {
            return None;
        }
        if t.decision.hint == Hint::ToGraveyard && m.at.location == Location::Deck {
            return Some(match code {
                BLUE if ctx.graveyard(ctx.me).iter().all(|c| !ctx.is(c, BLUE))
                    && ctx.in_hand(RETURN) =>
                {
                    6500.0
                }
                ANCIENTS => 6000.0,
                LEGEND if !ctx.in_hand(BLUE) => 5000.0,
                _ => 1000.0,
            });
        }
        if t.decision.hint == Hint::AddToHand {
            return Some(match code {
                ALTERNATIVE if !ctx.in_hand(ALTERNATIVE) => 6500.0,
                BLUE if !ctx.in_hand(BLUE) => 6000.0,
                ANCIENTS => 4000.0,
                _ => ctx.data(code).attack as f64,
            });
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(match code {
                AZURE => 6500.0,
                BLUE => 6000.0,
                WHITE => 5500.0,
                ANCIENTS => 4000.0,
                _ => ctx.data(code).attack as f64,
            });
        }
        if t.decision.hint.is_cost()
            && m.at.location == Location::Hand
            && matches!(code, ANCIENTS | LEGEND)
        {
            return Some(6500.0);
        }
        None
    }

    fn set_spell_trap(&self, _: &Turn, code: u32) -> Option<bool> {
        Some(code == WIND || code == TWIN)
    }
}
