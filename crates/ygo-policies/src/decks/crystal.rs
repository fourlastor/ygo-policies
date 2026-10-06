//! "Crystal Beast - Rainbow": monsters that live on as Continuous Spells.
//!
//! A destroyed Crystal Beast may go face-up to the Spell & Trap Zone instead
//! of the Graveyard, and Sapphire Pegasus / Crystal Blessing / Crystal Tree /
//! Crystal Release put more there.  That backrow is the resource: Crystal
//! Beacon and Crystal Promise summon from it, Ruby Carbuncle brings all of it
//! back at once, Ancient City - Rainbow Ruins grows with it (4+: draw, 5: a
//! free summon), Hamon eats three of them, and Crystal Abundance trades four
//! for the opponent's whole field.  Seven different Crystal Beast names on the
//! field / in the Graveyard unlock Rainbow Dragon (and Rainbow Gravity).

use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};

pub const DECK: &str = "12 Crystal Beast - Rainbow";

const SAPPHIRE_PEGASUS: u32 = 7093411;
const AMBER_MAMMOTH: u32 = 69937550;
const TOPAZ_TIGER: u32 = 95600067;
const AMETHYST_CAT: u32 = 32933942;
const RUBY_CARBUNCLE: u32 = 32710364;
const EMERALD_TORTOISE: u32 = 68215963;
const COBALT_EAGLE: u32 = 21698716;
const RAINBOW_DRAGON: u32 = 79856792;
const MALEFIC_RAINBOW_DRAGON: u32 = 598988;
const HAMON: u32 = 32491822;
const CRYSTAL_ABUNDANCE: u32 = 72881007;
const CRYSTAL_BEACON: u32 = 95326659;
const CRYSTAL_BLESSING: u32 = 35486099;
const CRYSTAL_PROMISE: u32 = 8275702;
const RAINBOW_RUINS: u32 = 34487429;
const CRYSTAL_TREE: u32 = 47408488;
const RAINBOW_PATH: u32 = 7617253;
const RAINBOW_GRAVITY: u32 = 63806265;
const CRYSTAL_RELEASE: u32 = 10004783;
const SET_CRYSTAL_BEAST: u16 = 0x1034;
/// `HINTMSG_TOFIELD`: place a card in a zone (here: a Crystal Beast in the Spell & Trap Zone).
const HINT_TO_FIELD: u64 = 527;

#[derive(Clone, Default)]
pub struct Crystal;

impl Crystal {
    fn is_beast(ctx: &Ctx, code: u32) -> bool {
        let d = ctx.data(code);
        d.is_monster() && d.in_set(SET_CRYSTAL_BEAST)
    }

    /// Crystal Beasts sitting in our Spell & Trap Zone.
    fn stored<'a>(ctx: &Ctx<'a>) -> Vec<&'a CardView> {
        ctx.spell_traps(ctx.me)
            .into_iter()
            .filter(|c| c.position.face_up && c.code.map_or(false, |code| Self::is_beast(ctx, code)))
            .collect()
    }

    /// Crystal Beast names already on our field or in our Graveyard (Rainbow Dragon counts 7).
    fn names(ctx: &Ctx) -> Vec<u32> {
        let mut names: Vec<u32> = ctx
            .monsters(ctx.me)
            .into_iter()
            .chain(ctx.spell_traps(ctx.me))
            .chain(ctx.graveyard(ctx.me))
            .filter(|c| c.position.face_up || c.at.location == Location::Graveyard)
            .filter_map(|c| c.code.map(|code| ctx.canonical(code)))
            .filter(|code| Self::is_beast(ctx, *code))
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    fn free_spell_zones(ctx: &Ctx) -> usize {
        5usize.saturating_sub(ctx.spell_traps(ctx.me).iter().filter(|c| c.at.sequence < 5).count())
    }
}

impl Strategy for Crystal {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            RAINBOW_DRAGON | HAMON => 3500,
            MALEFIC_RAINBOW_DRAGON => 2600,
            RAINBOW_RUINS => 2300,
            CRYSTAL_ABUNDANCE | RAINBOW_GRAVITY => 2200,
            SAPPHIRE_PEGASUS => 2100,
            CRYSTAL_BEACON => 1900,
            RUBY_CARBUNCLE => 1800,
            CRYSTAL_BLESSING | CRYSTAL_PROMISE | CRYSTAL_TREE => 1700,
            TOPAZ_TIGER => 1700,
            AMBER_MAMMOTH | RAINBOW_PATH => 1650,
            CRYSTAL_RELEASE => 1300,
            COBALT_EAGLE => 1400,
            EMERALD_TORTOISE => 1300,
            AMETHYST_CAT => 1250,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let stored = Self::stored(&ctx).len();
        // The Field Spell first: it protects and pays off everything else.
        if !ctx.face_up_on_field(ctx.me, RAINBOW_RUINS) {
            if let Some(i) = t.activate_from(RAINBOW_RUINS, Location::Hand) {
                return t.pick(i);
            }
        }
        // Rainbow Ruins' draw (4+) and free summon (5).
        if let Some(i) = t.activate(RAINBOW_RUINS) {
            return t.pick(i);
        }
        // Abundance clears the field and rebuilds ours from the Graveyard:
        // two opposing cards already repay it with two summoned Beasts.
        let their_cards = ctx.monsters(ctx.opp).len() + ctx.spell_traps(ctx.opp).len();
        if their_cards >= 2 {
            if let Some(i) = t.activate(CRYSTAL_ABUNDANCE) {
                return t.pick(i);
            }
        }
        // Fill the Spell & Trap Zone: Tree, Blessing.
        if !ctx.face_up_on_field(ctx.me, CRYSTAL_TREE) && Self::free_spell_zones(&ctx) >= 2 {
            if let Some(i) = t.activate_from(CRYSTAL_TREE, Location::Hand) {
                return t.pick(i);
            }
        }
        let tree_counters = ctx
            .spell_traps(ctx.me)
            .into_iter()
            .find(|c| ctx.is(c, CRYSTAL_TREE))
            .map_or(0, |c| c.counters as usize);
        // Tree sends itself as cost, freeing another zone for a Beast.
        if tree_counters >= 1 && Self::free_spell_zones(&ctx) + 1 >= tree_counters {
            if let Some(i) = t.activate_from(CRYSTAL_TREE, Location::SpellTrapZone) {
                return t.pick(i);
            }
        }
        if Self::free_spell_zones(&ctx) >= 1 {
            if let Some(i) = t.activate(CRYSTAL_BLESSING) {
                return t.pick(i);
            }
        }
        // Bring monsters out of the backrow.
        if ctx.free_monster_zones(ctx.me) > 0 {
            if let Some(i) = t.activate(CRYSTAL_BEACON) {
                return t.pick(i);
            }
            if stored >= 1 {
                if let Some(i) = t.activate(CRYSTAL_PROMISE) {
                    return t.pick(i);
                }
            }
        }
        // Rainbow Dragon: when their board is far ahead, shuffle the whole field away.
        if ctx.field_strength(ctx.opp) > ctx.field_strength(ctx.me) + 3000 {
            let rainbow = t.find_where(|c| {
                c.kind == ChoiceKind::Activate
                    && c.code().map(|code| ctx.canonical(code)) == Some(RAINBOW_DRAGON)
                    && c.description & 0xf == 1
            });
            if let Some(i) = rainbow {
                return t.pick(i);
            }
        }
        // Release also adds damage when the attacker already wins or attacks directly.
        if ctx.main1() {
            let target = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| c.position.face_up && ctx.can_attack(c) && c.code.map_or(false, |code| Self::is_beast(&ctx, code)))
                .find(|c| {
                    let best = ctx.opp_best_attack();
                    c.attack + 800 > best
                });
            if let Some(target) = target {
                if let Some(i) = t.activate(CRYSTAL_RELEASE) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, SAPPHIRE_PEGASUS) => Some(2100.0),
            (ChoiceKind::NormalSummon, TOPAZ_TIGER) => Some(1700.0),
            (ChoiceKind::NormalSummon, AMBER_MAMMOTH) => Some(1650.0),
            (ChoiceKind::NormalSummon, COBALT_EAGLE) => Some(1400.0),
            (ChoiceKind::SetMonster, EMERALD_TORTOISE) => Some(1300.0),
            (ChoiceKind::NormalSummon, AMETHYST_CAT) if ctx.monsters(ctx.opp).iter().any(|c| c.attack > 1200) => Some(1250.0),
            // A spare Ruby still provides a body and can become a stored Beast.
            (ChoiceKind::NormalSummon, RUBY_CARBUNCLE) => Some(900.0),
            (ChoiceKind::SetMonster, RUBY_CARBUNCLE) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // Either player's face-up Field Spell keeps Malefic alive.
            MALEFIC_RAINBOW_DRAGON => ctx.spell_traps(ctx.me).into_iter().chain(ctx.spell_traps(ctx.opp))
                .any(|c| c.at.sequence == 5 && c.position.face_up),
            // Hamon eats three stored Beasts: worth it for a 4000/4000 wall.
            HAMON => Self::stored(&ctx).len() >= 3 || ctx.opp_best_attack() >= 2500,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let hostile = t.hostile_top();
        Some(match code {
            RAINBOW_GRAVITY => Response::new(80.0),
            RAINBOW_PATH => match ctx.incoming_attack() {
                Some((attacker, target)) if ctx.attack_hurts(attacker, target) => Response::new(75.0),
                _ => Response::no(),
            },
            // Rainbow Ruins 3+: negate a Spell/Trap for one of our Crystal Beasts.
            RAINBOW_RUINS if hostile.matches(|link| !ctx.data(link.code).is_monster()) => Response::new(70.0),
            // Amber Mammoth takes an attack aimed at a weaker Crystal Beast.
            AMBER_MAMMOTH => match ctx.incoming_attack() {
                Some((_, Some(target))) if target.attack < 1700 => Response::new(40.0),
                _ => Response::no(),
            },
            RAINBOW_RUINS | EMERALD_TORTOISE | COBALT_EAGLE => Response::no(),
            // Rainbow Dragon's Quick Effect sends our own Beasts away: not on defence.
            RAINBOW_DRAGON => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        let code = member.code.map(|c| ctx.canonical(c))?;
        if member.at.controller != ctx.me || !Self::is_beast(&ctx, code) {
            return None;
        }
        let new_name = !Self::names(&ctx).contains(&code);
        match t.decision.hint {
            // Place a Beast in the backrow: a name we lack (Rainbow Dragon), from the Deck first.
            Hint::Other(HINT_TO_FIELD) => {
                let from = match member.at.location {
                    Location::Deck => 300.0,
                    Location::Graveyard => 200.0,
                    _ => 0.0,
                };
                Some(from + if new_name { 1000.0 } else { 0.0 } + ctx.data(code).attack as f64 / 10.0)
            }
            // Summoned from the Deck / backrow: Ruby Carbuncle empties the backrow onto the field.
            Hint::SpecialSummon => {
                // Promise removes Ruby from the backrow before its effect resolves.
                let companions = Self::stored(&ctx).iter().filter(|c| c.at != member.at).count();
                let carbuncle = if code == RUBY_CARBUNCLE && companions >= 2 && ctx.free_monster_zones(ctx.me) >= 3 { 3000.0 } else { 0.0 };
                Some(ctx.data(code).attack as f64 + carbuncle + if code == SAPPHIRE_PEGASUS { 600.0 } else { 0.0 })
            }
            _ => None,
        }
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let code = t.ctx.canonical(code);
        Some(t.ctx.data(code).is_trap() && Self::free_spell_zones(&t.ctx) >= 2)
    }
}
