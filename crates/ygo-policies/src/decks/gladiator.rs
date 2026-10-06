//! "Fight, Gladiators!": Gladiator Beast tag-out tempo.
//!
//! A Gladiator Beast that battles shuffles itself back at the end of the
//! Battle Phase and brings the right Beast for the board from the deck
//! (Bestiari vs Spells/Traps, Murmillo vs monsters, Darius/Equeste to
//! recycle, Laquari to hit, Hoplomus to wall).  So a Beast attacks whenever it
//! survives the battle, not only when it wins it.  Contact Fusions (Gyzarus:
//! destroy 2, Heraklinos: 3000 ATK Spell/Trap negation) and War Chariot keep
//! control; Test Tiger re-buys a Beast's summon effect.

use crate::agent::{value, Outcome, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Position};
use crate::tactics::default_outcome;

pub const DECK: &str = "Fight, Gladiators!";

const AIRBELLUM: u32 = 90508760;
const PRISMA: u32 = 89312388;
const EQUESTE: u32 = 57731460;
const SECUTOR: u32 = 77642288;
const DARIUS: u32 = 25924653;
const BESTIARI: u32 = 41470137;
const HOPLOMUS: u32 = 4253484;
const MURMILLO: u32 = 5975022;
const LAQUARI: u32 = 78868776;
const TEST_TIGER: u32 = 92373006;
const ENEMY_CONTROLLER: u32 = 98045062;
const PROVING_GROUND: u32 = 35224440;
const INDOMITABLE: u32 = 55136228;
const SHRINK: u32 = 55713623;
const REINFORCEMENT: u32 = 32807846;
const WAR_CHARIOT: u32 = 96216229;
const GYZARUS: u32 = 48156348;
const HERAKLINOS: u32 = 27346636;
const BLACK_ROSE: u32 = 73580471;
const RED_DRAGON_ARCHFIEND: u32 = 70902743;
const SET_GLADIATOR_BEAST: u16 = 0x1019;

#[derive(Clone, Default)]
pub struct Gladiator;

impl Gladiator {
    fn is_beast(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.in_set(SET_GLADIATOR_BEAST)
    }

    fn control_beast(ctx: &Ctx) -> bool {
        ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && Self::is_beast(ctx, c))
    }

    /// Gyzarus destroys up to 2 cards on either field: only worth it when
    /// some are theirs, or it destroys our own.
    fn gyzarus_has_targets(ctx: &Ctx) -> bool {
        !ctx.monsters(ctx.opp).is_empty() || !ctx.spell_traps(ctx.opp).is_empty()
    }

    /// Which Beast to bring from the deck for this board.
    fn tag_in_score(ctx: &Ctx, code: u32) -> f64 {
        let opp_backrow = ctx.spell_traps(ctx.opp).len();
        let opp_face_up = ctx.monsters(ctx.opp).iter().filter(|c| c.position.face_up).count();
        let graveyard_beasts = ctx.graveyard(ctx.me).iter().filter(|c| Self::is_beast(ctx, c)).count();
        let under_pressure = ctx.opp_best_attack() > 2100;
        match code {
            // Darius revives the named Gyzarus material; Equeste recycles our negate.
            DARIUS if ctx.graveyard(ctx.me).iter().any(|c| ctx.is(c, BESTIARI)) && ctx.free_monster_zones(ctx.me) >= 2
                && ctx.pile(ctx.me, Location::Extra).iter().any(|c| ctx.is(c, GYZARUS)) => 3400.0,
            EQUESTE if ctx.graveyard(ctx.me).iter().any(|c| ctx.is(c, WAR_CHARIOT))
                && !ctx.in_hand(WAR_CHARIOT) => 3200.0,
            BESTIARI if opp_backrow > 0 => 3000.0,
            MURMILLO if opp_face_up > 0 => 2900.0 + ctx.opp_best_attack() as f64 / 10.0,
            HOPLOMUS if under_pressure => 2600.0,
            DARIUS if graveyard_beasts > 0 => 2300.0,
            LAQUARI => 2200.0,
            EQUESTE if graveyard_beasts > 0 => 2000.0,
            SECUTOR => 1500.0,
            BESTIARI => 1900.0, // Gyzarus material
            _ => 1000.0,
        }
    }

    /// Shrink the monster ours is battling (at declaration or in the Damage Step).
    fn shrink(ctx: &Ctx) -> f64 {
        if ctx.phase().map_or(false, |p| !p.is_battle()) {
            return 0.0;
        }
        let (Some(attacker), Some(target)) = (ctx.battle_attacker(), ctx.battle_target()) else { return 0.0 };
        let (ours, theirs) = if attacker.at.controller == ctx.me { (attacker, target) } else { (target, attacker) };
        if !ours.position.attack || !theirs.position.attack || !theirs.position.face_up {
            return 0.0;
        }
        let halved = theirs.attack - ctx.view_data(theirs).attack / 2;
        if ours.attack <= theirs.attack && ours.attack > halved { 65.0 } else { 0.0 }
    }
}

impl Strategy for Gladiator {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            HERAKLINOS => 3200,
            GYZARUS => 2900,
            WAR_CHARIOT => 2000,
            LAQUARI | DARIUS => 1900,
            BESTIARI | MURMILLO => 1800,
            EQUESTE | PROVING_GROUND | REINFORCEMENT => 1700,
            HOPLOMUS => 1600,
            ENEMY_CONTROLLER | TEST_TIGER => 1500,
            AIRBELLUM | PRISMA => 1400,
            SHRINK | INDOMITABLE => 1200,
            SECUTOR => 900,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        for code in [PROVING_GROUND, REINFORCEMENT] {
            if let Some(i) = t.activate(code) {
                return t.pick(i);
            }
        }
        // Copy Bestiari when the name enables contact Fusion or Test Tiger.
        if Self::control_beast(&ctx) || ctx.in_hand(TEST_TIGER) {
            if let Some(i) = t.activate_from(PRISMA, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        // Resolve Gyzarus's removal before committing three bodies to Heraklinos.
        let opp_cards = ctx.monsters(ctx.opp).len() + ctx.spell_traps(ctx.opp).len();
        if opp_cards >= 1 {
            if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(GYZARUS), Some(Location::Extra)) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(HERAKLINOS), Some(Location::Extra)) {
            return t.pick(i);
        }
        // Test Tiger: re-summon a Beast "by a Gladiator Beast effect".
        if let Some(i) = t.activate_from(TEST_TIGER, Location::MonsterZone) {
            let target = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| c.position.face_up && Self::is_beast(&ctx, c))
                .min_by_key(|c| c.attack);
            if let Some(target) = target {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        Some(match (choice.kind, code) {
            // Even a small Beast is useful when Test Tiger can immediately tag it out.
            (ChoiceKind::NormalSummon, _) if ctx.in_hand(TEST_TIGER) && !Self::control_beast(&ctx)
                && ctx.data(code).in_set(SET_GLADIATOR_BEAST) => Some(2400.0 + ctx.data(code).attack as f64 / 10.0),
            (ChoiceKind::NormalSummon, LAQUARI) => Some(2100.0),
            (ChoiceKind::NormalSummon, DARIUS) => Some(2000.0),
            (ChoiceKind::NormalSummon, EQUESTE | BESTIARI) => Some(1900.0),
            (ChoiceKind::NormalSummon, AIRBELLUM | PRISMA) => Some(1600.0),
            (ChoiceKind::SetMonster, HOPLOMUS) => Some(1500.0),
            (ChoiceKind::NormalSummon, HOPLOMUS | MURMILLO | SECUTOR | TEST_TIGER) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code().unwrap_or(0));
        Some(match code {
            BLACK_ROSE | RED_DRAGON_ARCHFIEND => false,
            TEST_TIGER => Self::control_beast(&ctx),
            _ => true,
        })
    }

    fn attack_trick(&self, ctx: &Ctx, attacker: &CardView) -> i32 {
        if Self::is_beast(ctx, attacker) && ctx.in_hand(INDOMITABLE) { 500 } else { 0 }
    }

    fn attack_outcome(&self, ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
        if !Self::is_beast(ctx, attacker) || ctx.is(attacker, HOPLOMUS) {
            return None;
        }
        let trick = self.attack_trick(ctx, attacker);
        if !target.position.face_up && !target.known() {
            // A surviving Beast still tags out: risk mid-sized face-downs.
            return Some(if attacker.attack >= 1600 { Outcome::Win { trick: false } } else { Outcome::Lose });
        }
        Some(match default_outcome(ctx, attacker, target, trick) {
            Outcome::Bounce if ctx.battle_stat(target) - attacker.attack <= 1000 => Outcome::Win { trick: false },
            other => other,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let hostile = t.hostile_top();
        let incoming = ctx.incoming_attack();
        Some(match code {
            WAR_CHARIOT if hostile.matches(|link| ctx.data(link.code).is_monster()) => Response::new(90.0),
            HERAKLINOS if ctx.hand_size(ctx.me) >= 1 && hostile.matches(|link| !ctx.data(link.code).is_monster()) => {
                Response::new(88.0)
            }
            WAR_CHARIOT | HERAKLINOS => Response::no(),
            SHRINK => Response::new(Self::shrink(&ctx)),
            INDOMITABLE => {
                if ctx.phase().map_or(false, |p| !p.is_battle()) {
                    return Some(Response::no());
                }
                match (ctx.battle_attacker(), ctx.battle_target()) {
                    (Some(a), Some(d)) => {
                        let (ours, theirs) = if a.at.controller == ctx.me { (a, d) } else { (d, a) };
                        let stat = ctx.battle_stat(theirs);
                        if Self::is_beast(&ctx, ours) && ours.position.attack && ours.attack <= stat && ours.attack + 500 > stat {
                            Response::targeting(60.0, vec![ours.at])
                        } else {
                            Response::no()
                        }
                    }
                    _ => Response::no(),
                }
            }
            ENEMY_CONTROLLER => match incoming {
                Some((attacker, target)) if attacker.position.face_up && ctx.attack_hurts(attacker, target) => {
                    Response::targeting(60.0, vec![attacker.at])
                }
                _ => Response::no(),
            },
            TEST_TIGER | PRISMA => Response::no(),
            GYZARUS if choice.description & 0xf == 0 => {
                if Self::gyzarus_has_targets(&ctx) { Response::new(60.0) } else { Response::no() }
            }
            _ => return None,
        })
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        let ctx = t.ctx;
        let destroy = t.decision.choices.iter().any(|c| c.kind == ChoiceKind::Yes && c.description & 0xf == 0);
        (t.decision.subject.map(|c| ctx.canonical(c)) == Some(GYZARUS) && destroy).then(|| Self::gyzarus_has_targets(&ctx))
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if member.at.controller != ctx.me {
            return None;
        }
        let code = member.code.map(|c| ctx.canonical(c))?;
        // Prisma reveals Gyzarus to send Bestiari and copy its name.
        if member.at.location == Location::Extra && t.decision.hint == Hint::Confirm
            && t.memory.last_activated.map(|c| ctx.canonical(c)) == Some(PRISMA) {
            return Some(if code == GYZARUS { 4000.0 } else { 0.0 });
        }
        if member.at.location == Location::Graveyard && t.decision.hint == Hint::SpecialSummon && code == BESTIARI
            && t.memory.last_activated.map(|c| ctx.canonical(c)) == Some(DARIUS)
            && ctx.pile(ctx.me, Location::Extra).iter().any(|c| ctx.is(c, GYZARUS)) {
            return Some(4000.0);
        }
        let beast = ctx.data(code).in_set(SET_GLADIATOR_BEAST) && ctx.data(code).is_monster();
        // A hand search needs a playable Normal Summon, not a tag-only effect.
        if beast && member.at.location == Location::Deck && t.decision.hint == Hint::AddToHand {
            return Some(match code {
                BESTIARI if Self::control_beast(&ctx) && !ctx.face_up_on_field(ctx.me, BESTIARI) => 3500.0,
                LAQUARI => 2200.0,
                DARIUS => 2000.0,
                EQUESTE | BESTIARI => 1900.0,
                HOPLOMUS => 1500.0,
                _ => 900.0,
            });
        }
        if beast && member.at.location == Location::Deck && t.decision.hint == Hint::SpecialSummon {
            return Some(Self::tag_in_score(&ctx, code));
        }
        if t.decision.hint == Hint::FusionMaterial {
            return Some(-(value(self, &ctx, Some(code), None) as f64));
        }
        None
    }

    fn option(&self, _t: &Turn) -> Option<usize> {
        // Enemy Controller: "change battle position" is listed first.
        Some(0)
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        let ctx = t.ctx;
        (ctx.canonical(code) == HOPLOMUS).then_some(Position::FACE_UP_DEFENSE)
    }
}
