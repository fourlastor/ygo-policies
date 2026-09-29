//! "Fortune Ladies": Carly's Spellcasters that grow every turn.
//!
//! A Fortune Lady's ATK/DEF are its Level times 200 (Light, Fire), 300
//! (Wind, Water) or 400 (Dark, Earth), and it gains a Level in each of our
//! Standby Phases: weak on arrival, strong if left alone.  Their effects
//! chain into each other: Fortune Lady Light, leaving the field by an
//! effect, Special Summons another from the Deck; Fire, Special Summoned
//! that way in Attack Position, destroys a monster and burns its ATK; Water
//! draws 2 when Special Summoned beside another Fortune Lady; Wind's Normal
//! Summon destroys a Spell/Trap per Fortune Lady; Dark revives one when they
//! win a battle; Earth burns 400 per Level.  Summoner Monk turns a Spell into
//! Fortune Lady Water from the Deck, Inherited Fortune brings two from the
//! hand after one is destroyed, Slip of Fortune banishes our own attacked
//! monster until the next Standby Phase (Light floats on the way out).

use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Phase, Position};

pub const DECK: &str = "Fortune Ladies";

const LIGHT: u32 = 34471458;
const FIRE: u32 = 71870152;
const WIND: u32 = 82693917;
const WATER: u32 = 29088922;
const DARK: u32 = 55586621;
const EARTH: u32 = 82971335;
const SUMMONER_MONK: u32 = 423585;
const MAGICIAN_OF_FAITH: u32 = 31560081;
const FORTUNES_FUTURE: u32 = 68663748;
const MAGICAL_STONE_EXCAVATION: u32 = 98494543;
const INHERITED_FORTUNE: u32 = 20057949;
const OMINOUS_FORTUNETELLING: u32 = 56995655;
const SLIP_OF_FORTUNE: u32 = 72885174;
const SET_FORTUNE_LADY: u16 = 0x31;
/// System string of the "Monster" answer when a card type is called.
const DECLARE_MONSTER: u64 = 70;

#[derive(Default)]
pub struct FortuneLady;

impl FortuneLady {
    fn is_lady(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.in_set(SET_FORTUNE_LADY)
    }

    fn ladies_up(ctx: &Ctx) -> usize {
        ctx.monsters(ctx.me).iter().filter(|c| c.position.face_up && Self::is_lady(ctx, c)).count()
    }

    /// ATK of a Fortune Lady at its printed Level (the database says "?").
    fn arrival_attack(ctx: &Ctx, code: u32) -> i32 {
        let level = ctx.data(code).level as i32;
        level
            * match code {
                LIGHT | FIRE => 200,
                WIND | WATER => 300,
                DARK | EARTH => 400,
                _ => return ctx.data(code).attack,
            }
    }

    /// Which Fortune Lady a Deck summon should bring: Fire burns their best
    /// monster, Water draws beside another Lady, else the biggest.
    fn deck_pick(ctx: &Ctx, code: u32) -> f64 {
        let their_best = ctx.monsters(ctx.opp).iter().filter(|c| c.position.face_up).map(|c| c.attack).max().unwrap_or(0);
        match code {
            FIRE if their_best >= 1000 => 3000.0 + their_best as f64 / 10.0,
            WATER if Self::ladies_up(ctx) >= 1 => 2800.0,
            _ => Self::arrival_attack(ctx, code) as f64,
        }
    }
}

impl Strategy for FortuneLady {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            EARTH => 2100,
            DARK => 2000,
            WIND => 1800,
            WATER => 1700,
            FIRE => 1600,
            LIGHT | SUMMONER_MONK => 1500,
            MAGICIAN_OF_FAITH | SLIP_OF_FORTUNE => 1300,
            INHERITED_FORTUNE => 1200,
            OMINOUS_FORTUNETELLING => 1100,
            MAGICAL_STONE_EXCAVATION => 900,
            FORTUNES_FUTURE => 800,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let banished = ctx.banished(ctx.me).iter().any(|c| Self::is_lady(&ctx, c));
        if banished {
            if let Some(i) = t.activate(FORTUNES_FUTURE) {
                return t.pick(i);
            }
        }
        // Summoner Monk: a Spell for Fortune Lady Water (2 cards beside another Lady).
        let spell_in_hand = ctx.hand().iter().any(|c| ctx.view_data(c).is_spell());
        if spell_in_hand && Self::ladies_up(&ctx) >= 1 && ctx.free_monster_zones(ctx.me) > 0 {
            if let Some(i) = t.activate_from(SUMMONER_MONK, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        // Magical Stone Excavation: two dead cards for a Spell back.
        let dead = ctx.hand().iter().filter(|c| ctx.is(c, FORTUNES_FUTURE)).count();
        let prize = ctx.graveyard(ctx.me).iter().any(|c| ctx.view_data(c).is_spell() && value(self, &ctx, c.code, None) >= 2000);
        if prize && (dead >= 1 && ctx.hand_size(ctx.me) >= 4) {
            if let Some(i) = t.activate(MAGICAL_STONE_EXCAVATION) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let their_backrow = ctx.spell_traps(ctx.opp).len();
        let threat = ctx.opp_best_attack();
        Some(match (choice.kind, code) {
            // Wind destroys a Spell/Trap per Fortune Lady.
            (ChoiceKind::NormalSummon, WIND) if their_backrow > 0 => {
                Some(1900.0 + 300.0 * their_backrow.min(Self::ladies_up(&ctx) + 1) as f64)
            }
            (ChoiceKind::NormalSummon, DARK | EARTH) => {
                let attack = Self::arrival_attack(&ctx, code);
                (attack >= threat).then_some(attack as f64 + 500.0)
            }
            (ChoiceKind::NormalSummon, WIND | WATER) if Self::arrival_attack(&ctx, code) >= threat => Some(1300.0),
            // Monk goes to Defense anyway; its effect makes Water from the Deck.
            (ChoiceKind::NormalSummon, SUMMONER_MONK) if ctx.hand().iter().any(|c| ctx.view_data(c).is_spell()) => Some(1600.0),
            (ChoiceKind::SetMonster, MAGICIAN_OF_FAITH) if ctx.graveyard(ctx.me).iter().any(|c| ctx.view_data(c).is_spell()) => Some(1500.0),
            // A face-down Fortune Lady does not grow; set only as a last wall.
            (ChoiceKind::SetMonster, LIGHT | FIRE | WIND | WATER) if !ctx.monsters(ctx.me).is_empty() => None,
            (ChoiceKind::NormalSummon, LIGHT | FIRE) => None,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let incoming = ctx.incoming_attack();
        Some(match code {
            // Only offered the turn a face-up Fortune Lady was destroyed.
            INHERITED_FORTUNE => {
                let in_hand = ctx.hand().iter().filter(|c| Self::is_lady(&ctx, c)).count();
                if in_hand >= 1 { Response::new(30.0) } else { Response::no() }
            }
            // Our attacked monster steps out until the next Standby Phase.
            SLIP_OF_FORTUNE => match incoming {
                Some((attacker, Some(target))) if ctx.attack_hurts(attacker, Some(target)) => {
                    Response::new(if ctx.is(target, LIGHT) { 65.0 } else { 55.0 })
                }
                _ => Response::no(),
            },
            OMINOUS_FORTUNETELLING if t.view(choice).map_or(false, |v| !v.position.face_up) => {
                let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
                if end_of_their_turn || (ctx.my_turn() && ctx.main1()) { Response::new(10.0) } else { Response::no() }
            }
            _ => return None,
        })
    }

    fn option(&self, t: &Turn) -> Option<usize> {
        // Ominous Fortunetelling calls a card type (70 Monster, 71 Spell,
        // 72 Trap): most hands hold a monster.
        t.choices().find(|(_, c)| c.description == DECLARE_MONSTER).map(|(i, _)| i)
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if member.at.controller != ctx.me {
            return None;
        }
        let code = member.code.map(|c| ctx.canonical(c))?;
        match t.decision.hint {
            Hint::SpecialSummon if member.at.location == Location::Deck && ctx.data(code).in_set(SET_FORTUNE_LADY) => {
                Some(Self::deck_pick(&ctx, code))
            }
            // Costs: Fortune's Future is the card we can spare.
            Hint::Discard | Hint::ToGraveyard if member.at.location == Location::Hand => {
                Some(-(if code == FORTUNES_FUTURE { 0 } else { value(self, &ctx, Some(code), None) }) as f64)
            }
            _ => None,
        }
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        let ctx = t.ctx;
        match ctx.canonical(code) {
            // Fire's effect needs an Attack Position Special Summon.
            FIRE => Some(Position::FACE_UP_ATTACK),
            code if ctx.data(code).in_set(SET_FORTUNE_LADY) => Some(if Self::arrival_attack(&ctx, code) >= ctx.opp_best_attack() {
                Position::FACE_UP_ATTACK
            } else {
                Position::FACE_UP_DEFENSE
            }),
            _ => None,
        }
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let code = t.ctx.canonical(code);
        Some(t.ctx.data(code).is_trap())
    }
}
