//! Gusto's Reprisal: recruit defensively, assemble Sphreez with a Gusto
//! non-Tuner, then attack stronger monsters to reflect damage. Recycle the
//! small Gustos with Caam/Contact and preserve the Sphreez already on board.
use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::knowledge::{
    DAIGUSTO_SPHREEZ as SPHREEZ, GUSTO_EGUL as EGUL, GUSTO_GULLDO as GULLDO, GUSTO_WINDA as WINDA,
    SET_GUSTO,
};
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Position};
use crate::staples;

pub const DECK: &str = "Gusto's Reprisal";
const CAAM: u32 = 91449144;
const REEZE: u32 = 36331074;
const GULLDOS: u32 = 84766279;
const EGULS: u32 = 10755984;
const KREBONS: u32 = 59575539;
const SOLDIER: u32 = 22837504;
const SANGAN: u32 = 26202165;
const TELEPORT: u32 = 67723438;
const CONTACT: u32 = 83544697;
const SWAP: u32 = 31036355;
const LIMIT_REVERSE: u32 = 27551;
const BLACK_ROSE: u32 = 73580471;
const STARDUST: u32 = 44508094;

#[derive(Clone, Default)]
pub struct Gusto;

impl Gusto {
    fn sphreez(ctx: &Ctx) -> bool {
        !ctx.effects_drained() && ctx.face_up_on_field(ctx.me, SPHREEZ)
    }

    fn makes_sphreez(ctx: &Ctx, code: u32) -> bool {
        let d = ctx.data(code);
        ctx.monsters(ctx.me).iter().any(|c| {
            let other = ctx.view_data(c);
            c.position.face_up
                && c.level + d.level == 6
                && d.is_tuner() != other.is_tuner()
                && if d.is_tuner() {
                    other.in_set(SET_GUSTO)
                } else {
                    d.in_set(SET_GUSTO)
                }
        })
    }

    fn body_score(&self, ctx: &Ctx, code: u32) -> f64 {
        if code == SPHREEZ {
            return 6000.0;
        }
        if Self::makes_sphreez(ctx, code) && !Self::sphreez(ctx) {
            return 5000.0;
        }
        if Self::sphreez(ctx) {
            return match code {
                GULLDO => 4200.0,
                EGUL => 4100.0,
                WINDA => 3000.0,
                CAAM => 2500.0,
                _ => 1000.0,
            };
        }
        if !ctx.my_turn() {
            return match code {
                GULLDO => 3600.0,
                WINDA => 3500.0,
                EGUL => 3400.0,
                _ => ctx.data(code).defense as f64,
            };
        }
        // Leave a non-Tuner alongside an existing Tuner and vice versa.
        let partner = ctx.monsters(ctx.me).iter().any(|c| {
            c.position.face_up && ctx.view_data(c).is_tuner() != ctx.data(code).is_tuner()
        });
        value(self, ctx, Some(code), None) as f64 + if partner { 1000.0 } else { 0.0 }
    }

    fn removal<'a>(ctx: &Ctx<'a>, by: u32, face_up: bool) -> Option<&'a CardView> {
        ctx.monsters(ctx.opp)
            .into_iter()
            .chain(ctx.spell_traps(ctx.opp))
            .filter(|c| (!face_up || c.position.face_up) && ctx.reaches(c, by, true, true))
            // With Sphreez, a large plain attacker is useful to us. Remove
            // backrow, effect threats, and attack locks before that target.
            .max_by_key(|c| {
                ctx.threat(c)
                    + if Self::sphreez(ctx) && c.at.location == Location::SpellTrapZone {
                        3000
                    } else {
                        0
                    }
            })
    }

    fn revive<'a>(&self, ctx: &Ctx<'a>) -> Option<&'a CardView> {
        ctx.graveyard(ctx.me)
            .into_iter()
            .filter(|c| ctx.view_data(c).is_monster() && ctx.view_data(c).attack <= 1000)
            .max_by(|a, b| {
                self.body_score(ctx, a.code.unwrap_or(0))
                    .total_cmp(&self.body_score(ctx, b.code.unwrap_or(0)))
            })
    }

    fn swap<'a>(&self, ctx: &Ctx<'a>) -> Option<&'a CardView> {
        let ours = ctx
            .monsters(ctx.me)
            .into_iter()
            .filter(|c| !ctx.is(c, SPHREEZ))
            .min_by_key(|c| value(self, ctx, c.code, Some(c)))?;
        let worst = ctx
            .monsters(ctx.opp)
            .into_iter()
            .map(|c| ctx.threat(c))
            .min()?;
        (worst > value(self, ctx, ours.code, Some(ours)) + 700).then_some(ours)
    }
}

impl Strategy for Gusto {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            SPHREEZ => 3400,
            GULLDOS => 2700,
            EGULS | STARDUST | staples::SCRAP_DRAGON => 2800,
            CAAM | REEZE => 1900,
            SOLDIER => 1900,
            GULLDO | WINDA => 1300,
            EGUL => 1100,
            KREBONS | SANGAN => 1500,
            CONTACT | TELEPORT | LIMIT_REVERSE | SWAP => 1800,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !Self::sphreez(&ctx) {
            if let Some(i) = t.find(
                ChoiceKind::SpecialSummon,
                Some(SPHREEZ),
                Some(Location::Extra),
            ) {
                return t.pick(i);
            }
            if Self::makes_sphreez(&ctx, KREBONS) {
                if let Some(i) = t.activate(TELEPORT) {
                    return t.pick(i);
                }
            }
        }
        if Self::sphreez(&ctx) && ctx.main1() && t.has(ChoiceKind::EnterBattle) {
            // Generic positioning avoids losing battles. Here that is our plan.
            if let Some(i) = t.find_where(|c| {
                c.kind == ChoiceKind::ChangePosition
                    && t.view(c).map_or(false, |v| {
                        !v.position.attack && ctx.view_data(v).in_set(SET_GUSTO)
                    })
            }) {
                return t.pick(i);
            }
        }
        for code in [CONTACT, GULLDOS] {
            if let Some(i) = t.activate(code) {
                let target = if code == GULLDOS {
                    ctx.monsters(ctx.opp)
                        .into_iter()
                        .filter(|c| c.position.face_up && ctx.reaches(c, code, true, true))
                        .max_by_key(|c| ctx.threat(c))
                } else {
                    Self::removal(&ctx, code, false)
                };
                if let Some(target) = target {
                    let ram_target = Self::sphreez(&ctx)
                        && ctx.main1()
                        && target.at.location == Location::MonsterZone
                        && target.position.face_up
                        && target.position.attack
                        && target.attack >= 2000
                        && ctx.facts(target).negates_any == 0
                        && !ctx.battle_proof(target);
                    if !ram_target {
                        return t.pick_targeting(i, vec![target.at]);
                    }
                }
            }
        }
        if let Some(i) = t.activate_from(CAAM, Location::MonsterZone) {
            return t.pick(i);
        }
        if let Some(i) = t.activate_from(REEZE, Location::MonsterZone) {
            let ours = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| {
                    c.position.face_up && ctx.view_data(c).in_set(SET_GUSTO) && !ctx.is(c, SPHREEZ)
                })
                .min_by_key(|c| value(self, &ctx, c.code, Some(c)));
            let theirs = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| ctx.reaches(c, REEZE, true, false))
                .max_by_key(|c| ctx.threat(c));
            if let (Some(ours), Some(theirs)) = (ours, theirs) {
                if ctx.threat(theirs) > value(self, &ctx, ours.code, Some(ours)) + 500 {
                    return t.pick_targeting(i, vec![theirs.at, ours.at]);
                }
            }
        }
        if let (Some(i), Some(ours)) = (t.activate(SWAP), self.swap(&ctx)) {
            return t.pick_targeting(i, vec![ours.at]);
        }
        if let (Some(i), Some(target)) = (t.activate(LIMIT_REVERSE), self.revive(&ctx)) {
            if Self::makes_sphreez(&ctx, target.code.unwrap_or(0))
                || Self::sphreez(&ctx)
                || ctx.monsters(ctx.me).is_empty()
            {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, c: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        if c.kind == ChoiceKind::NormalSummon && Self::makes_sphreez(&ctx, code) {
            return Some(Some(6000.0));
        }
        Some(match (c.kind, code) {
            (ChoiceKind::NormalSummon, GULLDO | EGUL | WINDA) if Self::sphreez(&ctx) => {
                Some(self.body_score(&ctx, code))
            }
            (ChoiceKind::NormalSummon, CAAM) => {
                Some(if ctx.in_hand(KREBONS) || ctx.in_hand(TELEPORT) {
                    2800.0
                } else {
                    1700.0
                })
            }
            (ChoiceKind::NormalSummon, SOLDIER) => {
                Some(if ctx.in_hand(WINDA) { 2800.0 } else { 1900.0 })
            }
            (ChoiceKind::SetMonster, WINDA | GULLDO | EGUL | SANGAN) => Some(1600.0),
            (ChoiceKind::SetMonster, KREBONS) => Some(1100.0),
            (ChoiceKind::NormalSummon, REEZE)
                if ctx.face_up_on_field(ctx.me, EGUL) && ctx.monsters(ctx.me).len() >= 2 =>
            {
                Some(2600.0)
            }
            (_, REEZE) => None,
            (ChoiceKind::NormalSummon, KREBONS | WINDA | GULLDO | EGUL | SANGAN) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, c: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(c.code()?);
        if c.at().map(|a| a.location) == Some(Location::Extra) && Self::sphreez(&ctx) {
            return Some(false);
        }
        match code {
            SPHREEZ => Some(true),
            BLACK_ROSE => Some(ctx.field_strength(ctx.opp) > ctx.field_strength(ctx.me) + 2000),
            _ => None,
        }
    }

    fn battle(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if !Self::sphreez(&ctx) {
            return None;
        }
        let mut options = Vec::new();
        for (i, c) in t
            .choices()
            .filter(|(i, c)| c.kind == ChoiceKind::Attack && t.fresh(*i))
        {
            let Some(ours) = t.view(c).filter(|c| ctx.gusto_reflects(c)) else {
                continue;
            };
            for theirs in ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| c.position.face_up && !ctx.gusto_reflects(c))
            {
                let met = ctx.attack_meets(Some(ours), theirs);
                // Armor Master and similar cards prevent their controller's
                // battle damage even when Sphreez would reflect it.
                if met.facts.before_damage || met.facts.no_damage || ctx.attack_negatable(theirs) {
                    continue;
                }
                let damage = met.stat
                    - met
                        .facts
                        .attacker_atk
                        .apply(ours.attack + ctx.facts(ours).striking.bonus);
                if damage <= 0 {
                    continue;
                }
                let safe = !theirs.position.attack || ctx.battle_proof(ours);
                let recruits =
                    !ctx.monsters_banished() && (ctx.is(ours, GULLDO) || ctx.is(ours, EGUL));
                if safe || recruits || damage >= 1600 || damage >= ctx.opp_lp() {
                    let score = damage
                        + if damage >= ctx.opp_lp() { 20000 } else { 0 }
                        + if recruits { 600 } else { 0 };
                    options.push((score as f64, (i, theirs.at)));
                }
            }
        }
        let (_, (i, at)) = t.memory.ties.best(options)?;
        t.pick_targeting(i, vec![at])
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let c = t.choice(index);
        let code = ctx.canonical(c.code()?);
        Some(match code {
            GULLDO | EGUL | WINDA | SPHREEZ | SANGAN => Response::new(120.0),
            EGULS => {
                match ctx
                    .set_backrow(ctx.opp)
                    .into_iter()
                    .max_by_key(|c| ctx.threat(c))
                {
                    Some(c) => Response::targeting(35.0, vec![c.at]),
                    None => Response::no(),
                }
            }
            KREBONS
                if ctx.my_lp() > 1600
                    && ctx
                        .incoming_attack()
                        .map_or(false, |(a, b)| ctx.attack_hurts(a, b)) =>
            {
                Response::new(55.0)
            }
            SOLDIER
                if ctx.battle_target().map_or(false, |c| {
                    c.at.controller == ctx.opp && ctx.battle_stat(c) >= 1900
                }) =>
            {
                Response::new(40.0)
            }
            LIMIT_REVERSE if ctx.incoming_attack().is_some() && ctx.monsters(ctx.me).is_empty() => {
                self.revive(&ctx)
                    .map(|c| Response::targeting(45.0, vec![c.at]))
                    .unwrap_or_else(Response::no)
            }
            TELEPORT if ctx.incoming_attack().is_some() && ctx.monsters(ctx.me).is_empty() => {
                Response::new(40.0)
            }
            STARDUST if c.at().map(|a| a.location) == Some(Location::Graveyard) => {
                Response::new(120.0)
            }
            STARDUST if t.hostile_top().matches(|_| true) => Response::new(90.0),
            CAAM | REEZE | GULLDOS | CONTACT | SWAP | TELEPORT | LIMIT_REVERSE | KREBONS
            | SOLDIER | STARDUST => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, m: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if m.at.controller != ctx.me {
            return None;
        }
        let code = ctx.canonical(m.code?);
        let worth = value(self, &ctx, Some(code), None) as f64;
        if t.decision.hint == Hint::SpecialSummon {
            return Some(self.body_score(&ctx, code));
        }
        if t.decision.hint == Hint::SynchroMaterial {
            return Some(-worth - if code == SPHREEZ { 20000.0 } else { 0.0 });
        }
        if t.decision.hint == Hint::ToDeck && m.at.location == Location::Graveyard {
            // Put spent recruiters back into the Deck; keep a Sphreez for revival.
            return Some(match code {
                GULLDO | EGUL | WINDA => 3000.0,
                SPHREEZ => -2000.0,
                _ => 1000.0,
            });
        }
        if t.decision.hint == Hint::AddToHand {
            return Some(if Self::makes_sphreez(&ctx, code) {
                5000.0
            } else if code == WINDA || code == GULLDO {
                2400.0
            } else {
                worth
            });
        }
        None
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        let ctx = t.ctx;
        let code = ctx.canonical(code);
        // Limit Reverse destroys its revived monster in Defense Position.
        if t.memory.last_activated == Some(LIMIT_REVERSE) {
            return Some(Position::FACE_UP_ATTACK);
        }
        if ctx.data(code).in_set(SET_GUSTO) {
            return Some(
                if code == SPHREEZ || (Self::sphreez(&ctx) && ctx.my_turn()) {
                    Position::FACE_UP_ATTACK
                } else if matches!(code, GULLDO | EGUL | WINDA) {
                    Position::FACE_UP_DEFENSE
                } else {
                    Position::FACE_UP_ATTACK
                },
            );
        }
        None
    }

    fn allow_reposition(&self, t: &Turn, card: &CardView) -> bool {
        // Changing a Limit Reverse target to Defense would destroy it; we
        // cannot identify equip-style links in this projection, so keep the
        // small revived monsters attacking while a face-up copy remains.
        !(card.position.attack
            && card.attack <= 1000
            && t.ctx.face_up_on_field(t.ctx.me, LIMIT_REVERSE))
    }

    fn allow_staple(&self, t: &Turn, code: u32) -> bool {
        match code {
            staples::DARK_HOLE => !Self::sphreez(&t.ctx),
            staples::DOUBLE_SUMMON => {
                t.ctx
                    .hand()
                    .iter()
                    .any(|c| Self::makes_sphreez(&t.ctx, c.code.unwrap_or(0)))
                    || t.ctx.hand().iter().any(|a| {
                        t.ctx.hand().iter().any(|b| {
                            a.at != b.at
                                && t.ctx.view_data(a).is_tuner() != t.ctx.view_data(b).is_tuner()
                                && t.ctx.view_data(a).level + t.ctx.view_data(b).level == 6
                                && (t.ctx.view_data(a).in_set(SET_GUSTO)
                                    || t.ctx.view_data(b).in_set(SET_GUSTO))
                        })
                    })
            }
            _ => true,
        }
    }
}
