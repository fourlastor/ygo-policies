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
    if !ctx.phase().map_or(false, |p| p.is_damage_step()) {
        return Response::no();
    }
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
        HONEST if c.at().map(|a| a.location) == Some(Location::Hand) => honest(t),
        HONEST => Response::no(),
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
