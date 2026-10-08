//! Fabled Encore: discard into bodies, make Ragin with a small hand, then
//! turn the refill into another Synchro. No rollout/search or hidden-zone reads.
use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::knowledge::{FABLED_UNICORE, SET_FABLED};
use crate::model::{Choice, ChoiceKind, Hint, Location, Member};
use crate::{staples, tactics};

pub const DECK: &str = "Fabled Encore";
const GRIMRO: u32 = 24040093;
const KRUS: u32 = 19439119;
const CERBURREL: u32 = 82888408;
const CHAWA: u32 = 29905795;
const NOZOOCHEE: u32 = 55277252;
const GANASHIA: u32 = 18282103;
const LURRIE: u32 = 97651498;
const KUSHANO: u32 = 97439806;
const RAVEN: u32 = 47217354;
const CATSITH: u32 = 56399890;
const RAGIN: u32 = 47395382;
const VALKYRUS: u32 = 54048462;
const LEVIATHAN: u32 = 39477584;
const DEALINGS: u32 = 74117290;
const CARD_DESTRUCTION: u32 = 72892473;
const RAIGEKI_BREAK: u32 = 4178474;
const WIND_BLAST: u32 = 63356631;
const BLACK_ROSE: u32 = 73580471;
const STARDUST: u32 = 44508094;

#[derive(Clone, Default)]
pub struct Fabled;

impl Fabled {
    fn discard_bonus(ctx: &Ctx, code: u32) -> i32 {
        if ctx.monsters_banished() {
            return 0;
        }
        match code {
            CERBURREL | GANASHIA | LURRIE if ctx.free_monster_zones(ctx.me) > 0 => 3000,
            KRUS if ctx.free_monster_zones(ctx.me) > 0
                && ctx.graveyard(ctx.me).iter().any(|c| {
                    let d = ctx.view_data(c);
                    d.in_set(SET_FABLED) && d.is_monster() && d.level <= 4 && !ctx.is(c, KRUS)
                }) =>
            {
                3200
            }
            CATSITH
                if ctx
                    .monsters(ctx.opp)
                    .into_iter()
                    .chain(ctx.spell_traps(ctx.opp))
                    .any(|c| c.position.face_up && ctx.reaches(c, CATSITH, true, true)) =>
            {
                3500
            }
            _ => 0,
        }
    }

    fn good_discard(ctx: &Ctx, except: u32) -> bool {
        ctx.hand_codes().iter().any(|&code| {
            ctx.canonical(code) != except && Self::discard_bonus(ctx, ctx.canonical(code)) > 0
        })
    }

    fn body_score(&self, ctx: &Ctx, code: u32) -> f64 {
        let d = ctx.data(code);
        let partner = ctx.monsters(ctx.me).iter().any(|c| {
            c.position.face_up
                && ctx.view_data(c).is_tuner() != d.is_tuner()
                && tactics::synchro_worth(self, ctx, c.level + d.level).is_some()
        });
        d.attack as f64 + if partner { 2500.0 } else { 0.0 }
    }

    fn removal(t: &Turn, code: u32) -> Option<Response> {
        let ctx = t.ctx;
        let cheap = ctx
            .hand_codes()
            .iter()
            .any(|&k| Self::discard_bonus(&ctx, ctx.canonical(k)) > 0)
            || ctx.hand_size(ctx.me) >= 3;
        let best = ctx
            .monsters(ctx.opp)
            .into_iter()
            .chain(ctx.spell_traps(ctx.opp))
            .filter(|c| ctx.reaches(c, code, true, code == RAIGEKI_BREAK))
            .max_by_key(|c| ctx.threat(c))?;
        let threatened = ctx.obs.chain.iter().any(|link| {
            link.controller == ctx.opp && link.targets.iter().any(|at| at.controller == ctx.me)
        });
        (ctx.threat(best) >= if cheap { 1200 } else { 2300 } || threatened)
            .then(|| Response::targeting(65.0, vec![best.at]))
    }
}

impl Strategy for Fabled {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            RAGIN if ctx.hand_size(ctx.me) <= 1 && !ctx.effects_drained() => 3900,
            FABLED_UNICORE if ctx.hand_size(ctx.me) == ctx.hand_size(ctx.opp) => 3400,
            LEVIATHAN | 52687916 => 3200,
            VALKYRUS | STARDUST | staples::SCRAP_DRAGON => 2800,
            RAGIN | FABLED_UNICORE | staples::BRIONAC => 2400,
            GRIMRO => 2200,
            CHAWA | NOZOOCHEE | RAVEN => 1800,
            KRUS | CERBURREL | GANASHIA => 1500,
            KUSHANO | LURRIE | CATSITH => 900,
            DEALINGS | CARD_DESTRUCTION => 1300,
            RAIGEKI_BREAK | WIND_BLAST => 1700,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if ctx.hand_size(ctx.me) <= 1 && !ctx.effects_drained() {
            if let Some(i) = t.find(
                ChoiceKind::SpecialSummon,
                Some(RAGIN),
                Some(Location::Extra),
            ) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate_from(GRIMRO, Location::Hand) {
            return t.pick(i);
        }
        for code in [NOZOOCHEE, CHAWA] {
            if let Some(i) = t.activate_from(code, Location::Hand) {
                let has_partner = ctx.monsters(ctx.me).iter().any(|c| {
                    c.position.face_up && ctx.view_data(c).is_tuner() != ctx.data(code).is_tuner()
                });
                if Self::good_discard(&ctx, code) || has_partner {
                    return t.pick(i);
                }
            }
        }
        for (code, loc) in [
            (KUSHANO, Location::Graveyard),
            (RAVEN, Location::MonsterZone),
        ] {
            if Self::good_discard(&ctx, code) {
                if let Some(i) = t.activate_from(code, loc) {
                    return t.pick(i);
                }
            }
        }
        if ctx
            .hand()
            .iter()
            .any(|c| ctx.view_data(c).race & races::FIEND != 0)
        {
            if let Some(i) = t.activate(VALKYRUS) {
                return t.pick(i);
            }
        }
        // Set usable backrow before Ragin counts our hand. Never bury combo monsters.
        if t.find(
            ChoiceKind::SpecialSummon,
            Some(RAGIN),
            Some(Location::Extra),
        )
        .is_some()
            && ctx.hand_size(ctx.me) >= 2
            && !ctx.effects_drained()
        {
            if let Some(i) = t.find_where(|c| c.kind == ChoiceKind::SetSpellTrap) {
                return t.pick(i);
            }
        }
        let payoffs = ctx
            .hand_codes()
            .iter()
            .filter(|&&k| Self::discard_bonus(&ctx, ctx.canonical(k)) > 0)
            .count();
        if payoffs >= 2 && ctx.deck_size(ctx.me) > ctx.hand_size(ctx.me) + 2 {
            if let Some(i) = t.activate(CARD_DESTRUCTION) {
                return t.pick(i);
            }
        }
        if ctx.deck_size(ctx.me) > 3 && (payoffs > 0 || ctx.monsters(ctx.me).is_empty()) {
            if let Some(i) = t.activate(DEALINGS) {
                return t.pick(i);
            }
        }
        for code in [RAIGEKI_BREAK, WIND_BLAST] {
            if let (Some(i), Some(r)) = (t.activate(code), Self::removal(t, code)) {
                return t.pick_targeting(i, r.intent);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        if !ctx.data(code).in_set(SET_FABLED) {
            return None;
        }
        Some(match choice.kind {
            ChoiceKind::NormalSummon => Some(
                self.body_score(&ctx, code)
                    + if code == RAVEN && Self::good_discard(&ctx, RAVEN) {
                        2400.0
                    } else {
                        0.0
                    }
                    + if code == CHAWA && ctx.in_hand(GRIMRO) {
                        4000.0
                    } else {
                        0.0
                    },
            ),
            ChoiceKind::SetMonster
                if ctx.monsters(ctx.me).is_empty()
                    && !ctx.in_hand(GRIMRO)
                    && !ctx.in_hand(CHAWA)
                    && !ctx.in_hand(NOZOOCHEE)
                    && !Self::good_discard(&ctx, RAVEN) =>
            {
                Some(ctx.data(code).defense as f64)
            }
            _ => None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        match t.ctx.canonical(choice.code()?) {
            RAGIN => Some(t.ctx.hand_size(t.ctx.me) <= 1 || t.ctx.my_best_attack() < 2300),
            BLACK_ROSE => {
                Some(t.ctx.field_strength(t.ctx.opp) > t.ctx.field_strength(t.ctx.me) + 2000)
            }
            _ => None,
        }
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let c = t.choice(index);
        let code = t.ctx.canonical(c.code()?);
        Some(match code {
            CERBURREL | KRUS | GANASHIA | LURRIE | CATSITH | RAGIN | LEVIATHAN | 52687916
            | 27315304 => Response::new(120.0),
            NOZOOCHEE if c.at().map(|a| a.location) == Some(Location::MonsterZone) => {
                Response::new(120.0)
            }
            RAIGEKI_BREAK | WIND_BLAST => Self::removal(t, code).unwrap_or_else(Response::no),
            STARDUST if c.at().map(|a| a.location) == Some(Location::Graveyard) => {
                Response::new(120.0)
            }
            STARDUST if t.hostile_top().matches(|_| true) => Response::new(90.0),
            GRIMRO | CHAWA | NOZOOCHEE | RAVEN | KUSHANO | VALKYRUS | DEALINGS
            | CARD_DESTRUCTION | STARDUST => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let source = t.memory.last_activated.map(|c| ctx.canonical(c));
        let worth = value(self, &ctx, Some(code), None) as f64;
        if t.decision.hint == Hint::Discard && m.at.location == Location::Hand {
            // Sending a card (Grimro) is not discarding it. Only this hint
            // earns discard-trigger credit, and Raven keeps non-payoffs.
            let bonus = Self::discard_bonus(&ctx, code) as f64;
            if code == CATSITH
                && !ctx
                    .monsters(ctx.opp)
                    .into_iter()
                    .chain(ctx.spell_traps(ctx.opp))
                    .any(|c| {
                        c.position.face_up
                            && ctx.reaches(c, CATSITH, true, true)
                            && !t.memory.intent.iter().any(|(at, _)| *at == c.at)
                    })
            {
                // The paid-for removal resolves before Catsith's mandatory
                // trigger. Do not leave that trigger only our own cards.
                return Some(-10000.0);
            }
            if source == Some(RAVEN) && !t.decision.selected.is_empty() {
                return Some(-worth);
            }
            return Some(bonus - worth);
        }
        if source == Some(GRIMRO)
            && m.at.location == Location::Deck
            && t.decision.hint == Hint::AddToHand
        {
            let extender = !ctx.in_hand(code) && matches!(code, CHAWA | NOZOOCHEE | RAVEN);
            let needs_fuel =
                ctx.in_hand(CHAWA) || ctx.in_hand(NOZOOCHEE) || ctx.face_up_on_field(ctx.me, RAVEN);
            return Some(
                worth
                    + if extender { 1600.0 } else { 0.0 }
                    + if needs_fuel {
                        Self::discard_bonus(&ctx, code) as f64
                    } else {
                        0.0
                    },
            );
        }
        if t.decision.hint == Hint::SpecialSummon {
            return Some(self.body_score(&ctx, code));
        }
        if t.decision.hint == Hint::SynchroMaterial {
            return Some(
                -worth
                    - if ctx.data(code).is_extra() {
                        2000.0
                    } else {
                        0.0
                    },
            );
        }
        None
    }
}
