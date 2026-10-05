//! "Arcana Force Fortune": Fairies that toss a coin on every summon.
//!
//! Each Arcana Force monster gains a good or a bad effect from its coin:
//! The Chariot steals what it destroys or defects, The Emperor raises or
//! lowers every Arcana's ATK, The Lovers counts twice or blocks Tribute
//! Summons...  The deck's steady parts are around the coins: The Fool cannot
//! be destroyed by battle (a permanent wall when Set), the two EX Rulers
//! (4000/4000) come out of the hand by sending three of our monsters to the
//! Graveyard, and since every monster in the deck is a Fairy, Solidarity
//! gives them all 800 ATK.  Temperance discards itself to stop a battle's
//! damage; Nightmare's Steelcage stalls a stronger board.

use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{CardRef, CardView, Choice, ChoiceKind, Coin, Hint, Location, Member, Position};

pub const DECK: &str = "Arcana Force Fortune";

const THE_FOOL: u32 = 62892347;
const THE_MAGICIAN: u32 = 8396952;
const THE_EMPRESS: u32 = 35781051;
const THE_EMPEROR: u32 = 61175706;
const THE_LOVERS: u32 = 97574404;
const THE_CHARIOT: u32 = 34568403;
const TEMPERANCE: u32 = 60953118;
const THE_MOON: u32 = 97452817;
const LIGHT_RULER: u32 = 5861892;
const DARK_RULER: u32 = 69831560;
const SHINING_ANGEL: u32 = 95956346;
const DUNAMES_DARK_WITCH: u32 = 12493482;
const SECOND_COIN_TOSS: u32 = 36562627;
const GRACEFUL_DICE: u32 = 74137509;
const SOLIDARITY: u32 = 86780027;
const NIGHTMARES_STEELCAGE: u32 = 58775978;
const FISSURE: u32 = 66788016;
const REVERSAL_OF_FATE: u32 = 36690018;
const ARCANA_CALL: u32 = 99189322;
const SKULL_DICE: u32 = 126218;

#[derive(Default)]
pub struct Arcana;

impl Arcana {
    /// Value of the granted effect, separate from the monster's body. The
    /// Fool wants tails; the Rulers have useful effects on both sides.
    fn effect_value(ctx: &Ctx, code: u32, coin: Coin) -> Option<i32> {
        let heads = coin == Coin::Heads;
        Some(match ctx.canonical(code) {
            THE_FOOL => if heads { -200 } else { 1200 },
            THE_MAGICIAN => if heads { 900 } else { -300 },
            THE_EMPRESS => if heads { 1000 } else { -1000 },
            THE_EMPEROR => {
                let count = ctx.monsters(ctx.me).iter().filter(|c| ctx.view_data(c).in_set(0x5)).count().max(1) as i32;
                if heads { 500 * count } else { -500 * count }
            }
            THE_LOVERS => if heads { 350 } else { -350 },
            THE_CHARIOT => if heads { 1200 } else { -2500 },
            TEMPERANCE => if heads { 500 } else { -500 },
            THE_MOON => if heads { 1200 } else { -2500 },
            LIGHT_RULER => if heads { 900 } else { 500 },
            DARK_RULER => if heads { 1800 } else { 600 },
            _ => return None,
        })
    }

    fn reversal_gain(ctx: &Ctx, card: &CardView) -> Option<i32> {
        let effect = card.coin_effect?;
        // Heads Fool negates our own targeting effects, including Reversal.
        if effect.code == THE_FOOL && effect.result == Coin::Heads { return None; }
        let opposite = if effect.result == Coin::Heads { Coin::Tails } else { Coin::Heads };
        Some(Self::effect_value(ctx, effect.code, opposite)? - Self::effect_value(ctx, effect.code, effect.result)?)
    }

    fn call_gain(ctx: &Ctx, target: &CardView, donor: &CardView) -> Option<i32> {
        let effect = target.coin_effect?;
        if target.at == donor.at || donor.code.is_none() || !ctx.view_data(donor).in_set(0x5) { return None; }
        if effect.code == THE_FOOL && effect.result == Coin::Heads { return None; }
        if donor.at.location != Location::Graveyard
            && !(donor.at.controller == ctx.opp && donor.at.location == Location::MonsterZone && donor.position.face_up) { return None; }
        let new = Self::effect_value(ctx, donor.code?, effect.result)?;
        // Copying tails Chariot hands our monster to the opponent immediately.
        if donor.code == Some(THE_CHARIOT) && effect.result == Coin::Tails { return None; }
        let old = Self::effect_value(ctx, effect.code, effect.result)?;
        let removal = if donor.at.location == Location::MonsterZone { ctx.threat(donor) } else { 0 };
        // Arcana Call lasts this turn only: require a substantial improvement
        // or a free removal of an opposing Arcana monster.
        Some(new - old + removal - 400)
    }

    fn call_plan(ctx: &Ctx) -> Option<(i32, CardRef, CardRef)> {
        ctx.monsters(ctx.me).into_iter().filter(|c| c.position.face_up).flat_map(|target| {
            ctx.obs.cards.iter().filter_map(move |donor| Self::call_gain(ctx, target, donor).map(|gain| (gain, target.at, donor.at)))
        }).max_by_key(|p| p.0).filter(|p| p.0 > 0)
    }

    /// Our battle, and by how much our monster falls short of winning it.
    fn shortfall(ctx: &Ctx) -> Option<i32> {
        if ctx.phase().map_or(false, |p| !p.is_battle()) {
            return None;
        }
        let (attacker, target) = (ctx.battle_attacker()?, ctx.battle_target()?);
        let (ours, theirs) = if attacker.at.controller == ctx.me { (attacker, target) } else { (target, attacker) };
        let our_stat = if ours.position.attack { ours.attack } else { ours.defense };
        let their_stat = ctx.battle_stat(theirs);
        (ours.at.controller == ctx.me && theirs.position.face_up).then(|| their_stat - our_stat)
    }
}

impl Strategy for Arcana {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            LIGHT_RULER | DARK_RULER => 4000,
            THE_MOON => 2800,
            TEMPERANCE => 2400,
            SOLIDARITY => 2000,
            DUNAMES_DARK_WITCH => 1800,
            THE_CHARIOT | NIGHTMARES_STEELCAGE | FISSURE => 1700,
            THE_LOVERS => 1600,
            THE_EMPEROR | SHINING_ANGEL => 1400,
            THE_EMPRESS | SKULL_DICE | GRACEFUL_DICE => 1300,
            THE_MAGICIAN => 1100,
            SECOND_COIN_TOSS | REVERSAL_OF_FATE | ARCANA_CALL => 600,
            THE_FOOL => 500,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Every monster in the deck is a Fairy: Solidarity is +800 for all.
        if !ctx.face_up_on_field(ctx.me, SOLIDARITY) {
            let fairies = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).race & races::FAIRY != 0);
            if fairies {
                if let Some(i) = t.activate_from(SOLIDARITY, Location::Hand) {
                    return t.pick(i);
                }
            }
        }
        if !ctx.face_up_on_field(ctx.me, SECOND_COIN_TOSS) {
            if let Some(i) = t.activate_from(SECOND_COIN_TOSS, Location::Hand) {
                return t.pick(i);
            }
        }
        if ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up) {
            if let Some(i) = t.activate(FISSURE) {
                return t.pick(i);
            }
        }
        // A Set Arcana flips (and tosses its coin) once it can win a battle:
        // with ATK equal to DEF, the generic rule never flips it.
        if ctx.main1() && t.has(ChoiceKind::EnterBattle) {
            let threat = ctx.opp_best_attack();
            let opp_empty = ctx.monsters(ctx.opp).is_empty();
            let flip = t.find_where(|c| {
                c.kind == ChoiceKind::ChangePosition
                    && c.at().and_then(|at| ctx.card(at)).map_or(false, |card| {
                        let data = ctx.view_data(card);
                        !card.position.face_up && !ctx.is(card, THE_FOOL) && self.allow_reposition(t, card)
                            && data.attack > 0 && (opp_empty || data.attack > threat)
                    })
            });
            if let Some(i) = flip {
                return t.pick(i);
            }
        }
        // A stronger board than ours: nobody attacks for two of their turns.
        let outclassed = !ctx.monsters(ctx.opp).is_empty() && ctx.opp_best_attack() > ctx.my_best_attack() + 300;
        if outclassed && ctx.own_attack_locks().is_empty() {
            if let Some(i) = t.activate(NIGHTMARES_STEELCAGE) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        Some(match (choice.kind, code) {
            // Set, The Fool is a wall nothing destroys in battle; face-up it
            // is stuck in Attack Position with 0 ATK.
            (ChoiceKind::SetMonster, THE_FOOL) => Some(1400.0),
            (ChoiceKind::NormalSummon, THE_FOOL) => None,
            // Tails Chariot defects immediately. Setting it avoids the summon
            // trigger when it is flipped by an attack; retry support makes a
            // face-up summon a tolerable risk (75% heads instead of 50%).
            (ChoiceKind::NormalSummon, THE_CHARIOT) if !ctx.face_up_on_field(ctx.me, SECOND_COIN_TOSS) => None,
            (ChoiceKind::SetMonster, THE_CHARIOT) => Some(1200.0),
            _ => return None,
        })
    }

    fn allow_reposition(&self, t: &Turn, card: &CardView) -> bool {
        card.position.face_up || !t.ctx.is(card, THE_CHARIOT) || t.ctx.face_up_on_field(t.ctx.me, SECOND_COIN_TOSS)
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // A Ruler costs three of our monsters: worth it for small ones.
            LIGHT_RULER | DARK_RULER => {
                let mut ours: Vec<i32> = ctx.monsters(ctx.me).iter().map(|c| value(self, &ctx, None, Some(c))).collect();
                ours.sort_unstable();
                ours.len() >= 3 && ours.iter().take(3).sum::<i32>() <= 4500
            }
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        Some(match code {
            // Temperance from the hand: no damage from a big hit.
            TEMPERANCE if choice.at().map_or(false, |a| a.location == Location::Hand) => match ctx.incoming_attack() {
                Some((attacker, target)) => {
                    let damage = match target {
                        None => attacker.attack,
                        Some(t) if t.position.attack => attacker.attack - t.attack,
                        Some(_) => 0,
                    };
                    if damage >= 2000 || damage >= ctx.my_lp() { Response::new(50.0) } else { Response::no() }
                }
                None => Response::no(),
            },
            // Dice: +/-100..600 swings a battle we lose by a little.
            GRACEFUL_DICE | SKULL_DICE => match Self::shortfall(&ctx) {
                Some(gap) if (0..300).contains(&gap) => Response::new(40.0),
                _ => Response::no(),
            },
            REVERSAL_OF_FATE => ctx.monsters(ctx.me).iter()
                .filter_map(|c| Self::reversal_gain(&ctx, c).map(|gain| (gain, c.at)))
                .filter(|(gain, _)| *gain > 0).max_by_key(|p| p.0)
                .map_or_else(Response::no, |(gain, at)| Response::targeting(30.0 + gain as f64 / 100.0, vec![at])),
            ARCANA_CALL => Self::call_plan(&ctx).map_or_else(Response::no, |(gain, _, _)| Response::new(25.0 + gain as f64 / 100.0)),
            _ => return None,
        })
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        if t.decision.subject.map(|c| t.ctx.canonical(c)) != Some(SECOND_COIN_TOSS) { return None; }
        let toss = t.ctx.obs.coin_toss.as_ref();
        Some(toss.and_then(|toss| {
            let source = toss.source.as_ref()?;
            if toss.player != t.ctx.me || source.controller != t.ctx.me || toss.results.len() != 1 { return None; }
            let result = toss.results[0];
            let other = if result == Coin::Heads { Coin::Tails } else { Coin::Heads };
            Some(Self::effect_value(&t.ctx, source.code, other)? > Self::effect_value(&t.ctx, source.code, result)?)
        }).unwrap_or(false))
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        match t.memory.last_activated.map(|code| ctx.canonical(code)) {
            Some(ARCANA_CALL) if t.decision.hint == Hint::FaceUp => {
                let target = ctx.card(member.at)?;
                Some(ctx.obs.cards.iter().filter_map(|donor| Self::call_gain(&ctx, target, donor)).max().unwrap_or(-10000) as f64)
            }
            Some(ARCANA_CALL) if t.decision.hint == Hint::Banish => {
                let at = ctx.obs.chain.last()?.targets.first()?;
                Some(Self::call_gain(&ctx, ctx.card(*at)?, ctx.card(member.at)?).unwrap_or(-10000) as f64)
            }
            // Moon's forced handover should cost our least valuable monster.
            Some(THE_MOON) if t.decision.hint == Hint::Control => Some(-value(self, &ctx, member.code, ctx.card(member.at)) as f64),
            _ => None,
        }
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        matches!(t.ctx.canonical(code), THE_FOOL | 97452818).then_some(Position::FACE_UP_DEFENSE)
    }
}
