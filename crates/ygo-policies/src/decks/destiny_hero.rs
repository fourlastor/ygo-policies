//! "Destiny HEROes": Aster Phoenix's Destiny Draw engine into Dogma.
//!
//! The deck's finisher is Destiny HERO - Dogma, Special Summoned from the
//! hand by Tributing three monsters (one a Destiny HERO): at the opponent's
//! next Standby Phase their Life Points are halved.  The bodies come cheap:
//! Malicious banishes itself from the Graveyard for another from the Deck,
//! Over Destiny and D - Spirit summon small ones, Destiny Signal answers a
//! battle loss.  Destiny Draw discards a Destiny HERO for two cards, and the
//! discard is chosen for the Graveyard (Malicious, Dasher).
//! Diamond Dude excavates Normal Spells to use from the Graveyard next turn,
//! Plasma steals a monster and negates the others, Doom Lord banishes one
//! for two turns.  D - Counter and D - Shield answer attacks on the HEROes.

use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};

pub const DECK: &str = "Destiny HEROes";

const MALICIOUS: u32 = 9411399;
const DIAMOND_DUDE: u32 = 13093792;
const DASHER: u32 = 81866673;
const DOOM_LORD: u32 = 41613948;
const DUNKER: u32 = 93431862;
const DOUBLE_DUDE: u32 = 28355718;
const DEFENDER: u32 = 54749427;
const CAPTAIN_TENACIOUS: u32 = 77608643;
const BLADE_MASTER: u32 = 55461064;
const FEAR_MONGER: u32 = 80744121;
const DOGMA: u32 = 17132130;
const PLASMA: u32 = 83965310;
const DREAD_SERVANT: u32 = 36625827;
const DESTINY_DRAW: u32 = 45809008;
const OVER_DESTINY: u32 = 72204747;
const D_SPIRIT: u32 = 89899996;
const D_FORMATION: u32 = 74329404;
const CLOCK_TOWER_PRISON: u32 = 75041269;
const D_TIME: u32 = 99075257;
const D_COUNTER: u32 = 8698851;
const D_CHAIN: u32 = 43405287;
const D_SHIELD: u32 = 62868900;
const DESTINY_SIGNAL: u32 = 35464895;
const SET_DESTINY_HERO: u16 = 0xc008;

#[derive(Default)]
pub struct DestinyHero;

impl DestinyHero {
    fn is_dhero(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.in_set(SET_DESTINY_HERO)
    }

    fn finisher_in_hand(ctx: &Ctx) -> bool {
        ctx.in_hand(DOGMA) || ctx.in_hand(PLASMA)
    }

    /// How much a Destiny HERO wants to be in the Graveyard (as a discard).
    fn discard_priority(ctx: &Ctx, code: u32) -> i32 {
        match code {
            MALICIOUS => 300,
            DASHER => 200,
            DOUBLE_DUDE | FEAR_MONGER => 100,
            DOGMA | PLASMA if ctx.monsters(ctx.me).len() + 2 < 3 => 50,
            _ => 0,
        }
    }

    /// Bodies still missing for Dogma's three Tributes.
    fn bodies_missing(ctx: &Ctx) -> usize {
        3usize.saturating_sub(ctx.monsters(ctx.me).len())
    }
}

impl Strategy for DestinyHero {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            DOGMA => 3400,
            PLASMA => 2600,
            DASHER => 2100,
            DIAMOND_DUDE => 1900,
            DOUBLE_DUDE | DESTINY_DRAW => 1800,
            D_COUNTER => 1700,
            DOOM_LORD | MALICIOUS | OVER_DESTINY => 1500,
            DUNKER | CLOCK_TOWER_PRISON => 1400,
            CAPTAIN_TENACIOUS | FEAR_MONGER | DEFENDER | D_SPIRIT | D_SHIELD | DESTINY_SIGNAL => 1300,
            BLADE_MASTER | D_FORMATION | D_CHAIN => 1200,
            DREAD_SERVANT => 800,
            D_TIME => 300,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !ctx.face_up_on_field(ctx.me, CLOCK_TOWER_PRISON) {
            if let Some(i) = t.activate_from(CLOCK_TOWER_PRISON, Location::Hand) {
                return t.pick(i);
            }
        }
        if !ctx.face_up_on_field(ctx.me, D_FORMATION) {
            if let Some(i) = t.activate_from(D_FORMATION, Location::Hand) {
                return t.pick(i);
            }
        }
        // Destiny Draw: a Destiny HERO that wants the Graveyard for two cards.
        let discard = ctx.hand().iter().any(|c| Self::is_dhero(&ctx, c) && c.code.map_or(false, |k| Self::discard_priority(&ctx, ctx.canonical(k)) > 0));
        if discard || ctx.hand().iter().filter(|c| Self::is_dhero(&ctx, c)).count() >= 2 {
            if let Some(i) = t.activate(DESTINY_DRAW) {
                return t.pick(i);
            }
        }
        // Dogma (or Plasma) as soon as three bodies are out.
        for code in [DOGMA, PLASMA] {
            if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(code), Some(Location::Hand)) {
                if self.special_summon(t, t.choice(i)) == Some(true) {
                    return t.pick(i);
                }
            }
        }
        // Diamond Dude: a Normal Spell from the top, to use next turn.
        if let Some(i) = t.activate_from(DIAMOND_DUDE, Location::MonsterZone) {
            return t.pick(i);
        }
        // Bodies for Dogma: Malicious from the Graveyard, Over Destiny, D - Spirit.
        let want_bodies = (Self::finisher_in_hand(&ctx) && Self::bodies_missing(&ctx) > 0) || ctx.monsters(ctx.me).is_empty();
        if want_bodies && ctx.free_monster_zones(ctx.me) > 0 {
            if let Some(i) = t.activate_from(MALICIOUS, Location::Graveyard) {
                return t.pick(i);
            }
            let big_in_graveyard = ctx.graveyard(ctx.me).iter().any(|c| Self::is_dhero(&ctx, c) && ctx.view_data(c).level >= 2);
            if big_in_graveyard {
                if let Some(i) = t.activate(OVER_DESTINY) {
                    return t.pick(i);
                }
            }
            if !ctx.monsters(ctx.me).iter().any(|c| Self::is_dhero(&ctx, c)) {
                if let Some(i) = t.activate(D_SPIRIT) {
                    return t.pick(i);
                }
            }
        }
        // Plasma takes their best monster.
        if let Some(i) = t.activate_from(PLASMA, Location::MonsterZone) {
            if let Some(best) = ctx.monsters(ctx.opp).into_iter().filter(|c| c.position.face_up).max_by_key(|c| ctx.threat(c)) {
                return t.pick_targeting(i, vec![best.at]);
            }
        }
        // Doom Lord banishes a monster we cannot beat (no attacks this turn).
        if let Some(i) = t.activate_from(DOOM_LORD, Location::MonsterZone) {
            let best = ctx.monsters(ctx.opp).into_iter().max_by_key(|c| ctx.threat(c));
            if let Some(best) = best.filter(|c| ctx.threat(c) >= 2000 && ctx.threat(c) > ctx.my_best_attack()) {
                return t.pick_targeting(i, vec![best.at]);
            }
        }
        // Dunker: a Destiny HERO from the hand for the last 500.
        if ctx.opp_lp() <= 500 {
            if let Some(i) = t.activate_from(DUNKER, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, DIAMOND_DUDE) => Some(2000.0),
            // Face-up, Defender hands the opponent a card every turn.
            (ChoiceKind::SetMonster, DEFENDER) if ctx.opp_best_attack() >= 2000 && ctx.monsters(ctx.me).is_empty() => Some(700.0),
            (_, DEFENDER) => None,
            // A third body when Dogma is waiting.
            (ChoiceKind::NormalSummon | ChoiceKind::SetMonster, _) if Self::finisher_in_hand(&ctx) && Self::bodies_missing(&ctx) == 1 && ctx.data(code).level <= 4 => {
                Some(1900.0)
            }
            (ChoiceKind::NormalSummon, BLADE_MASTER | DREAD_SERVANT) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // Three Tributes: cheap bodies for a 3400 that halves their LP.
            DOGMA => {
                let mut ours: Vec<i32> = ctx.monsters(ctx.me).iter().map(|c| value(self, &ctx, None, Some(c))).collect();
                ours.sort_unstable();
                ours.iter().take(3).sum::<i32>() <= 5000
            }
            PLASMA => {
                let mut ours: Vec<i32> = ctx.monsters(ctx.me).iter().map(|c| value(self, &ctx, None, Some(c))).collect();
                ours.sort_unstable();
                !ctx.in_hand(DOGMA) && ours.iter().take(3).sum::<i32>() <= 4500 && ctx.opp_best_attack() >= 1500
            }
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let incoming = ctx.incoming_attack();
        Some(match code {
            // Offered when a face-up Destiny HERO is attacked.
            D_COUNTER => match incoming {
                Some((attacker, _)) => Response::targeting(80.0, vec![attacker.at]),
                None => Response::no(),
            },
            D_SHIELD => match incoming {
                Some((attacker, Some(target))) if ctx.attack_hurts(attacker, Some(target)) => Response::targeting(55.0, vec![target.at]),
                _ => Response::no(),
            },
            // A monster of ours died in battle: a small Destiny HERO replaces it.
            DESTINY_SIGNAL => Response::new(30.0),
            // +500 on an attacker that then wins.
            D_CHAIN => {
                if !(ctx.my_turn() && ctx.main1()) {
                    return Some(Response::no());
                }
                let best = ctx.opp_best_attack();
                let holder = ctx
                    .monsters(ctx.me)
                    .into_iter()
                    .filter(|c| Self::is_dhero(&ctx, c) && ctx.can_attack(c) && c.attack <= best && c.attack + 500 > best)
                    .max_by_key(|c| c.attack);
                match holder {
                    Some(holder) => Response::targeting(25.0, vec![holder.at]),
                    None => Response::no(),
                }
            }
            // From the hand in their Battle Phase: +800 for our HEROes.
            BLADE_MASTER if choice.at().map_or(false, |a| a.location == Location::Hand) => match incoming {
                Some((attacker, Some(target))) if Self::is_dhero(&ctx, target) && target.position.attack && target.attack <= attacker.attack && target.attack + 800 > attacker.attack => {
                    Response::new(50.0)
                }
                _ => Response::no(),
            },
            // Its only target would be one of our own Spells/Traps.
            DREAD_SERVANT => Response::no(),
            D_TIME => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if member.at.controller != ctx.me {
            return None;
        }
        let code = member.code.map(|c| ctx.canonical(c))?;
        match t.decision.hint {
            // Destiny Draw / Dunker: the HERO that works from the Graveyard.
            Hint::Discard | Hint::ToGraveyard if member.at.location == Location::Hand => {
                Some(Self::discard_priority(&ctx, code) as f64 - value(self, &ctx, Some(code), None) as f64 / 10.0)
            }
            // Dogma / Plasma Tributes: the cheapest bodies, Malicious first (it comes back).
            Hint::Release | Hint::Tribute => {
                let worth = value(self, &ctx, Some(code), ctx.card(member.at).filter(|c| c.known())) as f64;
                Some(-worth + if code == MALICIOUS { 500.0 } else { 0.0 })
            }
            _ => None,
        }
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        // D - Time needs an Elemental HERO, and this list has none.
        (t.ctx.canonical(code) == D_TIME).then_some(false)
    }
}
