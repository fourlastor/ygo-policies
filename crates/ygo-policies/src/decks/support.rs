//! Small shared building blocks for the initial archetype policies.
//! Card identities come only from the seat observation and legal choices.
use crate::agent::{Hostile, Response, Strategy, Turn};
use crate::cards::{attributes, types};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, Hint, Location, Member};
use crate::{staples, tactics};

pub const HONEST: u32 = 37742478;
pub const STARDUST: u32 = 44508094;
pub const BLACK_ROSE: u32 = 73580471;

pub fn target<'a>(ctx: &Ctx<'a>, by: u32, destroys: bool) -> Option<&'a CardView> {
    ctx.monsters(ctx.opp)
        .into_iter()
        .chain(ctx.spell_traps(ctx.opp))
        .filter(|c| ctx.reaches(c, by, true, destroys))
        .max_by_key(|c| ctx.threat(c))
}

pub fn extra_value(code: u32) -> Option<i32> {
    Some(match code {
        52687916 => 3300, // Trishula
        70902743 => 2500, // Red Dragon Archfiend can destroy our non-attacking board.
        STARDUST | staples::SCRAP_DRAGON | 23693634 => 2800,
        27315304 => 2900, // Mist Wurm
        staples::BRIONAC => 2700,
        26593852 => 2500, // Catastor
        43385557 => 2400, // Android
        BLACK_ROSE => 2000,
        _ => return None,
    })
}

pub fn extra_allowed(t: &Turn, c: &Choice) -> Option<bool> {
    (t.ctx.canonical(c.code()?) == BLACK_ROSE)
        .then(|| t.ctx.field_strength(t.ctx.opp) > t.ctx.field_strength(t.ctx.me) + 1800)
}

pub fn body_score<S: Strategy>(s: &S, ctx: &Ctx, code: u32) -> f64 {
    let d = ctx.data(code);
    let partner = ctx.monsters(ctx.me).iter().any(|c| {
        c.position.face_up
            && ctx.view_data(c).is_tuner() != d.is_tuner()
            && tactics::synchro_worth(s, ctx, c.level + d.level).is_some()
    });
    d.attack as f64 + if partner { 2200.0 } else { 0.0 }
}

pub fn honest(t: &Turn) -> Response {
    let ctx = t.ctx;
    // A legal Honest choice already identifies its damage-step window.
    // OCGCore NEW_PHASE can still leave the observation at BattleStart.
    let (Some(a), Some(b)) = (ctx.battle_attacker(), ctx.battle_target()) else {
        return Response::no();
    };
    let (ours, theirs) = if a.at.controller == ctx.me {
        (a, b)
    } else {
        (b, a)
    };
    if ctx.view_data(ours).attribute & attributes::LIGHT != 0
        && ours.position.attack
        && (ours.attack <= ctx.battle_stat(theirs)
            || (ctx.my_turn()
                && ours.attack + theirs.attack - ctx.battle_stat(theirs) >= ctx.opp_lp()))
    {
        Response::new(95.0)
    } else {
        Response::no()
    }
}

pub fn honest_trick(ctx: &Ctx, c: &CardView) -> i32 {
    if ctx.in_hand(HONEST) && ctx.view_data(c).attribute & attributes::LIGHT != 0 {
        ctx.opp_best_attack()
    } else {
        0
    }
}

pub fn chain(t: &Turn, i: usize) -> Option<Response> {
    let c = t.choice(i);
    let code = t.ctx.canonical(c.code()?);
    Some(match code {
        50078509 => {
            // Fiendish Chain: negate a live effect or stop its attack.
            let target = ctx_fiendish_target(&t.ctx);
            target
                .map(|c| Response::targeting(90.0, vec![c.at]))
                .unwrap_or_else(Response::no)
        }
        HONEST if c.at().map(|a| a.location) == Some(Location::Hand) => honest(t),
        HONEST => Response::no(),
        52687916 | 27315304 => Response::new(120.0), // independent Synchro Summon triggers
        23693634 if c.at().map(|a| a.location) == Some(Location::Graveyard) => Response::new(120.0),
        STARDUST if c.at().map(|a| a.location) == Some(Location::Graveyard) => Response::new(120.0),
        STARDUST if !matches!(t.hostile_top(), Hostile::No) => Response::new(90.0),
        STARDUST => Response::no(),
        BLACK_ROSE => {
            if t.ctx.field_strength(t.ctx.opp) > t.ctx.field_strength(t.ctx.me) + 1800 {
                Response::new(110.0)
            } else {
                Response::no()
            }
        }
        _ => return None,
    })
}

pub fn material_score(ctx: &Ctx, m: &Member, hint: Hint) -> Option<f64> {
    if m.at.controller != ctx.me || hint != Hint::SynchroMaterial {
        return None;
    }
    let d = ctx.data(m.code?);
    Some(-(d.attack.max(d.defense) as f64) - if d.is(types::SYNCHRO) { 2000.0 } else { 0.0 })
}

/// Basic defense and draw cards used by the initial alternate-win decks.
/// Never spend two resolved turn-long battle shields on the same turn.
pub fn stall_chain(t: &Turn, i: usize) -> Option<Response> {
    let ctx = t.ctx;
    let c = t.choice(i);
    let code = ctx.canonical(c.code()?);
    let covered = t
        .memory
        .activated
        .iter()
        .any(|k| matches!(*k, 36361633 | 12607053 | 18964575 | 19665973));
    Some(match code {
        18964575 | 19665973
            if !covered && ctx.incoming_attack().map_or(false, |(_, b)| b.is_none()) =>
        {
            Response::new(90.0)
        }
        3657444
            if !covered
                && c.description == ((3657444u64) << 20)
                && ctx
                    .incoming_attack()
                    .map_or(false, |(_, b)| b.map_or(false, |b| ctx.is(b, 3657444))) =>
        {
            Response::new(95.0)
        }
        36361633
            if !covered
                && !ctx.my_turn()
                && ctx.phase() == Some(crate::model::Phase::BattleStart)
                && !ctx.monsters(ctx.opp).is_empty() =>
        {
            Response::new(85.0)
        }
        12607053 if !covered && ctx.incoming_attack().is_some() => Response::new(90.0),
        83968380 | 30461781 if ctx.deck_size(ctx.me) > 1 => Response::new(120.0),
        18964575 | 19665973 | 3657444 | 36361633 | 12607053 | 83968380 | 30461781 => Response::no(),
        _ => return None,
    })
}

/// Spend a legal Flip Summon on a useful effect instead of waiting to be attacked.
/// Each caller opts in only its own engines; the generic loop remains conservative.
pub fn flip(t: &mut Turn, codes: &[u32]) -> Option<usize> {
    let ctx = t.ctx;
    if ctx.effects_drained() {
        return None;
    }
    let i = t.find_where(|c| {
        if c.kind != crate::model::ChoiceKind::ChangePosition {
            return false;
        }
        let Some(v) = t.view(c) else {
            return false;
        };
        let Some(code) = v.code.map(|k| ctx.canonical(k)) else {
            return false;
        };
        if v.position.face_up || !codes.contains(&code) {
            return false;
        }
        match code {
            91133740 => ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up),
            21502796 => !ctx.monsters(ctx.opp).is_empty() || !ctx.spell_traps(ctx.opp).is_empty(),
            60694662 => ctx.deck_size(ctx.me) > 1,
            33508719 => ctx.deck_size(ctx.me) >= 5 && ctx.hand_size(ctx.me) <= 3,
            79106360 => {
                ctx.monsters(ctx.me).len() <= 2
                    || ctx.monsters(ctx.opp).len() > ctx.monsters(ctx.me).len()
            }
            56839613 | 5220687 => ctx.free_monster_zones(ctx.me) > 0,
            62437709 => ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up),
            _ => true,
        }
    })?;
    t.pick(i)
}

fn ctx_fiendish_target<'a>(ctx: &Ctx<'a>) -> Option<&'a CardView> {
    let at = ctx
        .obs
        .chain
        .last()
        .filter(|l| l.controller == ctx.opp && l.source.location == Location::MonsterZone)
        .map(|l| l.source)
        .or_else(|| ctx.incoming_attack().map(|(a, _)| a.at))?;
    ctx.card(at).filter(|c| {
        c.position.face_up
            && ctx.view_data(c).is(types::EFFECT)
            && ctx.reaches(c, 50078509, true, false)
    })
}

/// Spore changes Level according to the public GY card paid as its cost.
pub fn spore_cost<S: Strategy>(s: &S, t: &Turn, m: &Member) -> Option<f64> {
    if m.at.controller != t.ctx.me
        || m.at.location != Location::Graveyard
        || t.decision.hint != Hint::Banish
        || t.memory.last_activated != Some(11747708)
    {
        return None;
    }
    let code = m.code?;
    let d = t.ctx.data(code);
    let synchro = tactics::synchro_with_tuner(s, &t.ctx, 1 + d.level).unwrap_or(0);
    Some(3.0 * synchro as f64 - d.attack as f64 / 4.0 - if code == 67441435 { 1000.0 } else { 0.0 })
}

/// Use Copy Plant's legal Level change when it improves the visible Synchro options.
pub fn copy_plant<S: Strategy>(s: &S, t: &mut Turn) -> Option<usize> {
    let ctx = t.ctx;
    let i = t.activate_from(66457407, Location::MonsterZone)?;
    let ours = t.view(t.choice(i))?;
    let current = tactics::synchro_with_tuner(s, &ctx, ours.level).unwrap_or(0);
    let target = ctx
        .monsters(ctx.me)
        .into_iter()
        .filter(|c| {
            c.at != ours.at
                && c.position.face_up
                && ctx.view_data(c).race & crate::cards::races::PLANT != 0
                && c.level != ours.level
        })
        .max_by_key(|c| tactics::synchro_with_tuner(s, &ctx, c.level).unwrap_or(0))?;
    if tactics::synchro_with_tuner(s, &ctx, target.level).unwrap_or(0) > current {
        t.pick_targeting(i, vec![target.at])
    } else {
        None
    }
}
