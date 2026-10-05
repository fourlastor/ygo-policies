//! "Toon Kingdom": Pegasus's Toons, which attack around the opponent's monsters.
//!
//! Toon World (1000 LP) is the whole deck: Toon Table of Contents finds it,
//! and every Toon is destroyed if it leaves.  While it is up, the Toons
//! attack directly unless the opponent has a Toon of their own.  The small
//! ones are Normal Summoned (Toon Gemini Elf discards, Toon Masked Sorcerer
//! draws); the big ones come out of the hand by Tributing (Toon Summoned
//! Skull and Toon Dark Magician Girl one monster, Blue-Eyes Toon Dragon and
//! Manga Ryu-Ran two), and Scapegoat's sheep are the fodder.  No Toon
//! attacks the turn it arrives.  Relinquished steals a monster's stats,
//! Toon Cannon Soldier Tributes for 500 damage, Toon Defense turns an attack
//! on a small Toon into a direct one.

use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Location, Phase, Position};
use crate::tactics;

pub const DECK: &str = "Toon Kingdom";

const TOON_GEMINI_ELF: u32 = 42386471;
const TOON_MASKED_SORCERER: u32 = 16392422;
const TOON_GOBLIN_ATTACK_FORCE: u32 = 15270885;
const TOON_MERMAID: u32 = 65458948;
const TOON_ALLIGATOR: u32 = 59383041;
const TOON_CANNON_SOLDIER: u32 = 79875176;
const TOON_SUMMONED_SKULL: u32 = 91842653;
const TOON_DARK_MAGICIAN_GIRL: u32 = 90960358;
const BLUE_EYES_TOON_DRAGON: u32 = 53183600;
const MANGA_RYU_RAN: u32 = 38369349;
const RELINQUISHED: u32 = 64631466;
const MYSTIC_TOMATO: u32 = 83011277;
const TOON_WORLD: u32 = 15259703;
const TOON_TABLE_OF_CONTENTS: u32 = 89997728;
const BLACK_ILLUSION_RITUAL: u32 = 41426869;
const SCAPEGOAT: u32 = 73915051;
const TOON_DEFENSE: u32 = 43509019;
const SHEEP_TOKEN: u32 = 73915052;
const SET_TOON: u16 = 0x62;

#[derive(Default)]
pub struct Toon;

impl Toon {
    fn world_up(ctx: &Ctx) -> bool {
        ctx.face_up_on_field(ctx.me, TOON_WORLD)
    }

    fn is_toon(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.in_set(SET_TOON)
    }

    /// Tributes a Toon's Special Summon from the hand needs.
    fn tributes(code: u32) -> Option<usize> {
        match code {
            TOON_MERMAID => Some(0),
            TOON_SUMMONED_SKULL | TOON_DARK_MAGICIAN_GIRL => Some(1),
            BLUE_EYES_TOON_DRAGON | MANGA_RYU_RAN => Some(2),
            _ => None,
        }
    }

    /// Our cheapest `n` monsters' worth, as Tribute fodder.
    fn fodder_cost(&self, ctx: &Ctx, n: usize) -> Option<i32> {
        let mut ours: Vec<i32> = ctx.monsters(ctx.me).iter().map(|c| value(self, ctx, None, Some(c))).collect();
        ours.sort_unstable();
        (ours.len() >= n).then(|| ours.iter().take(n).sum())
    }
}

impl Strategy for Toon {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            TOON_WORLD | BLUE_EYES_TOON_DRAGON => 3000,
            TOON_SUMMONED_SKULL => 2500,
            TOON_GOBLIN_ATTACK_FORCE => 2300,
            TOON_DARK_MAGICIAN_GIRL | MANGA_RYU_RAN => 2200,
            RELINQUISHED => 2000,
            TOON_GEMINI_ELF => 1900,
            TOON_TABLE_OF_CONTENTS => 1700,
            SCAPEGOAT | BLACK_ILLUSION_RITUAL => 1500,
            TOON_MERMAID | TOON_CANNON_SOLDIER => 1400,
            MYSTIC_TOMATO => 1300,
            TOON_MASKED_SORCERER => 1200,
            TOON_DEFENSE => 1100,
            TOON_ALLIGATOR => 900,
            SHEEP_TOKEN => 100,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !Self::world_up(&ctx) && ctx.my_lp() > 1500 {
            if let Some(i) = t.activate_from(TOON_WORLD, Location::Hand) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(TOON_TABLE_OF_CONTENTS) {
            return t.pick(i);
        }
        // Relinquished takes their best monster (and its stats).
        if let Some(i) = t.activate_from(RELINQUISHED, Location::MonsterZone) {
            if let Some(best) = ctx.monsters(ctx.opp).into_iter().filter(|c| c.position.face_up).max_by_key(|c| ctx.threat(c)) {
                return t.pick_targeting(i, vec![best.at]);
            }
        }
        if Self::world_up(&ctx) {
            // The biggest Toon the Tributes on the field can pay for.
            let mut best: Option<(i32, usize)> = None;
            for (i, c) in t.choices() {
                if c.kind != ChoiceKind::SpecialSummon || !t.fresh(i) || c.at().map(|a| a.location) != Some(Location::Hand) {
                    continue;
                }
                let Some(code) = c.code().map(|k| ctx.canonical(k)) else { continue };
                let Some(n) = Self::tributes(code) else { continue };
                let cost = self.fodder_cost(&ctx, n).unwrap_or(i32::MAX);
                let worth = ctx.data(code).attack;
                if cost < worth - 500 && best.map_or(true, |b| worth > b.0) {
                    best = Some((worth, i));
                }
            }
            if let Some((_, i)) = best {
                return t.pick(i);
            }
        }
        // Black Illusion Ritual: a spare Level 1+ monster for Relinquished.
        if ctx.in_hand(RELINQUISHED) && !ctx.monsters(ctx.opp).is_empty() {
            if let Some(i) = t.activate(BLACK_ILLUSION_RITUAL) {
                return t.pick(i);
            }
        }
        // Toon Cannon Soldier: a Tribute for the last points.
        if ctx.opp_lp() <= 500 || (!ctx.main1() && ctx.monsters(ctx.me).iter().any(|c| ctx.is(c, SHEEP_TOKEN))) {
            if let Some(i) = t.activate_from(TOON_CANNON_SOLDIER, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let world = Self::world_up(&ctx) || ctx.in_hand(TOON_WORLD);
        Some(match (choice.kind, code) {
            // Toons with Toon World up (or coming) are attackers.
            (ChoiceKind::NormalSummon, TOON_GOBLIN_ATTACK_FORCE) if world => Some(2300.0),
            (ChoiceKind::NormalSummon, TOON_GEMINI_ELF) if world => Some(2000.0),
            (ChoiceKind::NormalSummon, TOON_CANNON_SOLDIER) if world => Some(1500.0),
            (ChoiceKind::NormalSummon, TOON_MASKED_SORCERER) if world => Some(1450.0),
            (ChoiceKind::SetMonster, TOON_ALLIGATOR | MYSTIC_TOMATO) => Some(1200.0),
            // A face-down Toon is neither an attacker nor much of a wall.
            (ChoiceKind::SetMonster, _) if ctx.data(code).in_set(SET_TOON) => None,
            (ChoiceKind::NormalSummon, RELINQUISHED) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, _t: &Turn, choice: &Choice) -> Option<bool> {
        // Tribute Toons are chosen in the Main Phase plan.
        Some(Self::tributes(choice.code().unwrap_or(0)).is_none())
    }

    fn battle(&mut self, t: &mut Turn) -> Option<usize> {
        // A Toon that can go around their monsters always does.
        if let Some(i) = tactics::direct_attack(t) {
            return t.pick_targeting(i, Vec::new());
        }
        let (i, target) = tactics::plan_attack(self, t)?;
        t.pick_targeting(i, target.into_iter().collect())
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let incoming = ctx.incoming_attack();
        Some(match code {
            // Tokens to block with, or to Tribute for the big Toons.
            SCAPEGOAT => {
                let exposed = ctx.monsters(ctx.me).is_empty();
                let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
                let big_toon_in_hand = ctx.hand().iter().any(|c| c.code.and_then(|k| Self::tributes(ctx.canonical(k))).map_or(false, |n| n > 0));
                match incoming {
                    Some((attacker, target)) if exposed && ctx.attack_hurts(attacker, target) => Response::new(60.0),
                    _ if end_of_their_turn && ctx.free_monster_zones(ctx.me) >= 3 && (big_toon_in_hand || exposed) => Response::new(15.0),
                    _ => Response::no(),
                }
            }
            // Face-down, the Continuous Trap goes up ahead of their attacks;
            // face-up, it turns an attack on a small Toon into a direct one.
            TOON_DEFENSE if t.view(choice).map_or(false, |v| !v.position.face_up) => {
                let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
                if end_of_their_turn || (ctx.my_turn() && ctx.main1()) { Response::new(10.0) } else { Response::no() }
            }
            TOON_DEFENSE => match incoming {
                Some((attacker, Some(target)))
                    if Self::is_toon(&ctx, target) && ctx.attack_hurts(attacker, Some(target)) && attacker.attack + 1000 < ctx.my_lp() =>
                {
                    Response::new(45.0)
                }
                _ => Response::no(),
            },
            _ => return None,
        })
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        tactics::attack_directly(t)
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        let ctx = t.ctx;
        let code = ctx.canonical(code);
        let data = ctx.data(code);
        (data.in_set(SET_TOON) && code != TOON_ALLIGATOR).then_some(Position::FACE_UP_ATTACK)
    }
}
