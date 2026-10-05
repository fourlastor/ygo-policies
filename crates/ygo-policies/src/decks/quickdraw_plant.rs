//! "Quickdraw Plants": Plant Synchro engine.
//!
//! Small Plants that come back from the Graveyard become Synchro Summons:
//! Glow-Up Bulb (mill 1) and Spore (banish a Plant, add its Level) revive
//! themselves as Tuners once per Duel, Level Eater revives by taking a
//! Level from a big monster, Dandylion leaves two Fluff Tokens wherever it
//! goes.  Lonefire Blossom Tributes a Plant (Dandylion first) for another
//! from the Deck, up to Tytannial.  Foolish Burial, One for One and
//! Quickdraw Synchron's summon cost put the right Plant in the Graveyard.
//! The Tuners on the field turn it all into Synchros: Debris Dragon revives a
//! 500-ATK monster for a Dragon (Black Rose, Stardust, Scrap), Junk Synchron
//! a Level 2 or lower one, Quickdraw Synchron stands in for Junk Synchron in
//! Junk Destroyer.  Each Tuner is only brought out when it completes a
//! Synchro the Extra Deck holds.

use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
use crate::tactics;

pub const DECK: &str = "Quickdraw Plants";

const LONEFIRE_BLOSSOM: u32 = 48686504;
const DANDYLION: u32 = 15341821;
const SPORE: u32 = 11747708;
const GLOW_UP_BULB: u32 = 67441435;
const TYTANNIAL: u32 = 11819616;
const DEBRIS_DRAGON: u32 = 14943837;
const QUICKDRAW_SYNCHRON: u32 = 20932152;
const LEVEL_EATER: u32 = 57421866;
const JUNK_SYNCHRON: u32 = 63977008;
const DOPPELWARRIOR: u32 = 53855409;
const CARD_TROOPER: u32 = 85087012;
const SANGAN: u32 = 26202165;
const CAIUS: u32 = 9748752;
const PLAGUESPREADER: u32 = 33420078;
const ONE_FOR_ONE: u32 = 2295440;
const FOOLISH_BURIAL: u32 = 81439173;
const FLUFF_TOKEN: u32 = 15341822;
const STARDUST_DRAGON: u32 = 44508094;
const BLACK_ROSE_DRAGON: u32 = 73580471;
const SCRAP_DRAGON: u32 = 76774528;
const BRIONAC: u32 = 50321796;
const TRISHULA: u32 = 52687916;
const FORMULA_SYNCHRON: u32 = 50091196;
const SHOOTING_STAR_DRAGON: u32 = 24696097;
const JUNK_DESTROYER: u32 = 74860293;
const JUNK_WARRIOR: u32 = 60800381;
const ARMORY_ARM: u32 = 29071332;
const RED_DRAGON_ARCHFIEND: u32 = 70902743;
const MAGICAL_ANDROID: u32 = 43385557;
const COLOSSAL_FIGHTER: u32 = 23693634;

#[derive(Default)]
pub struct QuickdrawPlant;

impl QuickdrawPlant {
    fn is_plant(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.race & races::PLANT != 0
    }

    fn in_graveyard(ctx: &Ctx, code: u32) -> bool {
        ctx.graveyard(ctx.me).iter().any(|c| ctx.is(c, code))
    }

    /// What the Graveyard should get next (Foolish Burial, discards): the
    /// Plants that act from there.
    fn graveyard_rank(ctx: &Ctx, code: u32) -> i32 {
        match code {
            DANDYLION => 400,
            GLOW_UP_BULB if !Self::in_graveyard(ctx, GLOW_UP_BULB) => 350,
            SPORE if !Self::in_graveyard(ctx, SPORE) => 300,
            LEVEL_EATER => 250,
            PLAGUESPREADER => 200,
            _ => 0,
        }
    }

    /// A Tuner at `level` would complete a Synchro worth having.
    fn tuner_pays(&self, ctx: &Ctx, level: u32) -> bool {
        tactics::synchro_with_tuner(self, ctx, level).map_or(false, |worth| worth >= 2300)
    }

    /// Spore's Level once it banishes the best other Plant from the Graveyard.
    fn spore_levels(ctx: &Ctx) -> Vec<u32> {
        ctx.graveyard(ctx.me)
            .iter()
            .filter(|c| Self::is_plant(ctx, c) && !ctx.is(c, SPORE))
            .map(|c| 1 + ctx.view_data(c).level)
            .collect()
    }
}

impl Strategy for QuickdrawPlant {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        let their_cards = (ctx.monsters(ctx.opp).len() + ctx.spell_traps(ctx.opp).len()) as i32;
        Some(match code {
            SHOOTING_STAR_DRAGON | TRISHULA => 3300,
            JUNK_DESTROYER => 2600 + 200 * their_cards.min(3),
            STARDUST_DRAGON | SCRAP_DRAGON | COLOSSAL_FIGHTER | TYTANNIAL => 2800,
            BRIONAC => 2300 + 100 * their_cards.min(3),
            MAGICAL_ANDROID | CAIUS => 2400,
            JUNK_WARRIOR => 2300,
            BLACK_ROSE_DRAGON | RED_DRAGON_ARCHFIEND => 2000,
            LONEFIRE_BLOSSOM | DEBRIS_DRAGON => 1900,
            ARMORY_ARM | QUICKDRAW_SYNCHRON => 1800,
            JUNK_SYNCHRON | FORMULA_SYNCHRON => 1700,
            DANDYLION | SANGAN | ONE_FOR_ONE | FOOLISH_BURIAL => 1500,
            GLOW_UP_BULB | SPORE | CARD_TROOPER => 1400,
            LEVEL_EATER | DOPPELWARRIOR | PLAGUESPREADER => 1200,
            FLUFF_TOKEN => 200,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let zones = ctx.free_monster_zones(ctx.me);
        // Stock the Graveyard: Dandylion's tokens, the once-per-Duel Tuners.
        if let Some(i) = t.activate(FOOLISH_BURIAL) {
            return t.pick(i);
        }
        if zones >= 1 && ctx.hand().iter().any(|c| c.code.map_or(false, |k| Self::graveyard_rank(&ctx, ctx.canonical(k)) > 0)) {
            if let Some(i) = t.activate(ONE_FOR_ONE) {
                return t.pick(i);
            }
        }
        // Lonefire Blossom: a Plant (Dandylion, a token) for one from the Deck.
        let fodder = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && Self::is_plant(&ctx, c));
        if fodder && zones >= 1 {
            if let Some(i) = t.activate_from(LONEFIRE_BLOSSOM, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        // Graveyard Tuners and Level Eater, when they complete a Synchro.
        if zones >= 1 {
            if self.tuner_pays(&ctx, 1) {
                if let Some(i) = t.activate_from(GLOW_UP_BULB, Location::Graveyard) {
                    return t.pick(i);
                }
            }
            if Self::spore_levels(&ctx).into_iter().any(|level| self.tuner_pays(&ctx, level)) {
                if let Some(i) = t.activate_from(SPORE, Location::Graveyard) {
                    return t.pick(i);
                }
            }
            if self.tuner_pays(&ctx, 2) && ctx.hand_size(ctx.me) >= 1 {
                if let Some(i) = t.activate_from(PLAGUESPREADER, Location::Graveyard) {
                    return t.pick(i);
                }
            }
            let tuner_up = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).is_tuner());
            if tuner_up {
                if let Some(i) = t.activate_from(LEVEL_EATER, Location::Graveyard) {
                    return t.pick(i);
                }
            }
        }
        // Quickdraw Synchron: a Graveyard Plant as its cost, Junk Destroyer next.
        let destroyer = tactics::synchro_worth(self, &ctx, 8).is_some()
            && ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && !ctx.view_data(c).is_tuner() && c.level == 3);
        if destroyer {
            if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(QUICKDRAW_SYNCHRON), Some(Location::Hand)) {
                return t.pick(i);
            }
        }
        // Card Trooper mills Plants for the Graveyard Tuners.
        if let Some(i) = t.activate_from(CARD_TROOPER, Location::MonsterZone) {
            return t.pick(i);
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let small_in_graveyard = ctx.graveyard(ctx.me).iter().any(|c| ctx.view_data(c).is_monster() && ctx.view_data(c).attack <= 500);
        let level2_in_graveyard = ctx.graveyard(ctx.me).iter().any(|c| ctx.view_data(c).is_monster() && ctx.view_data(c).level <= 2);
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, LONEFIRE_BLOSSOM) => Some(2300.0),
            (ChoiceKind::NormalSummon, DEBRIS_DRAGON) if small_in_graveyard => Some(2250.0),
            (ChoiceKind::NormalSummon, JUNK_SYNCHRON) if level2_in_graveyard => Some(2150.0),
            (ChoiceKind::NormalSummon, CARD_TROOPER) => Some(1700.0),
            (ChoiceKind::SetMonster, DANDYLION | SANGAN) => Some(1500.0),
            // Better in the Graveyard than on the field.
            (_, GLOW_UP_BULB | SPORE | LEVEL_EATER | PLAGUESPREADER) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // Quickdraw's cost and payoff are planned in the Main Phase.
            QUICKDRAW_SYNCHRON => false,
            BLACK_ROSE_DRAGON => ctx.field_strength(ctx.opp) > ctx.field_strength(ctx.me) + 2500,
            RED_DRAGON_ARCHFIEND => ctx.monsters(ctx.me).len() <= 2,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let hostile = t.hostile_top();
        Some(match code {
            // Negates anything that targets, for a Plant.
            TYTANNIAL if hostile.matches(|_| true) => Response::new(85.0),
            TYTANNIAL => Response::no(),
            // From the hand when a monster comes back from our Graveyard.
            DOPPELWARRIOR if choice.at().map_or(false, |a| a.location == Location::Hand) => Response::new(35.0),
            SHOOTING_STAR_DRAGON if hostile.matches(|_| true) => Response::new(88.0),
            SHOOTING_STAR_DRAGON => match ctx.incoming_attack() {
                Some((attacker, target)) if ctx.attack_hurts(attacker, target) => Response::new(60.0),
                _ => Response::no(),
            },
            // Its quick Synchro in their Main Phase is not planned.
            FORMULA_SYNCHRON if !ctx.my_turn() => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if member.at.controller != ctx.me {
            return None;
        }
        let code = member.code.map(|c| ctx.canonical(c))?;
        let worth = value(self, &ctx, Some(code), None) as f64;
        match t.decision.hint {
            // Foolish Burial / costs sent from the hand: the Graveyard Plants.
            Hint::ToGraveyard | Hint::Discard if matches!(member.at.location, Location::Deck | Location::Hand) => {
                Some(Self::graveyard_rank(&ctx, code) as f64 - worth / 10.0)
            }
            // Lonefire's Tribute: Dandylion leaves tokens, a token costs nothing.
            Hint::Release if member.at.location == Location::MonsterZone => Some(match code {
                DANDYLION => 1000.0,
                FLUFF_TOKEN => 800.0,
                _ => -worth,
            }),
            // Lonefire from the Deck: Tytannial first, else Dandylion's tokens.
            Hint::SpecialSummon if member.at.location == Location::Deck => Some(match code {
                TYTANNIAL if !ctx.face_up_on_field(ctx.me, TYTANNIAL) => 3000.0,
                DANDYLION => 2500.0,
                _ => worth,
            }),
            _ => None,
        }
    }
}
