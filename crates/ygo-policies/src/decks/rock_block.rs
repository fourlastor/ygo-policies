//! "Koa'ki Meiru - Rock Block": Skill Drain control.
//!
//! Skill Drain turns the deck's drawbacks off: Beast King Barbaros keeps 3000
//! ATK without Tributes and the Koa'ki Meiru beaters skip their End Phase
//! upkeep.  Around that, a trap-heavy shell (Solemn Warning, Bottomless Trap
//! Hole, Mirror Force, Torrential Tribute, Magic Jammer, Royal Oppression)
//! and the on-board negations (Sandman vs Traps, Guardian vs monster effects,
//! Rai-Oh vs Special Summons) keep the opponent from ever resolving a threat.

use crate::agent::{value, Outcome, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Phase};

pub const DECK: &str = "06 Koaki Meiru - Rock Block";

const AD_CHANGER: u32 = 96146814;
const BARBAROS: u32 = 78651105;
const BOULDER: u32 = 6320631;
const GUARDIAN: u32 = 45041488;
const SANDMAN: u32 = 49680980;
const MORPHING_JAR: u32 = 33508719;
const NECRO_GARDNA: u32 = 4906301;
const GRAND_MOLE: u32 = 80344569;
const RAI_OH: u32 = 71564252;
const MYSTIC_BOX: u32 = 25774450;
const MAGIC_JAMMER: u32 = 77414722;
const ROYAL_OPPRESSION: u32 = 93016201;
const SKILL_DRAIN: u32 = 82732705;
const SOLEMN_WARNING: u32 = 84749824;

/// Opponent Spells worth a card to stop.
const DANGEROUS_SPELLS: &[u32] = &[
    53129443, // Dark Hole
    12580477, // Raigeki
    19613556, // Heavy Storm
    42703248, // Giant Trunade
    83764718, // Monster Reborn
    72302403, // Swords of Revealing Light
    25774450, // Mystic Box
    5318639,  // Mystical Space Typhoon
    14087893, // Book of Moon
];

#[derive(Default)]
pub struct RockBlock;

impl RockBlock {
    fn drained(ctx: &Ctx) -> bool {
        ctx.face_up_on_field(ctx.me, SKILL_DRAIN) || ctx.face_up_on_field(ctx.opp, SKILL_DRAIN)
    }

    /// Our face-up monsters whose drawback Skill Drain would switch off.
    fn drain_beneficiaries(ctx: &Ctx) -> usize {
        ctx.monsters(ctx.me)
            .iter()
            .filter(|c| c.position.face_up && [BARBAROS, GUARDIAN, SANDMAN].iter().any(|k| ctx.is(c, *k)))
            .count()
    }

    fn opp_effect_monsters(ctx: &Ctx) -> usize {
        ctx.monsters(ctx.opp)
            .iter()
            .filter(|c| c.position.face_up && ctx.view_data(c).is(crate::cards::types::EFFECT))
            .count()
    }

    fn want_skill_drain(ctx: &Ctx) -> bool {
        !Self::drained(ctx)
            && ctx.my_lp() > 2000
            && (Self::drain_beneficiaries(ctx) > 0 || Self::opp_effect_monsters(ctx) > 0)
    }

    /// The monster the engine is currently Summoning, if the viewer can see it.
    fn summoned<'a>(ctx: &Ctx<'a>) -> Option<(&'a CardView, u32)> {
        ctx.obs
            .event_cards
            .iter()
            .filter(|(at, _)| at.controller == ctx.opp)
            .find_map(|(at, code)| Some((ctx.card(*at)?, (*code)?)))
    }

    fn rock_in_hand(ctx: &Ctx) -> bool {
        ctx.hand().iter().any(|c| ctx.view_data(c).race & races::ROCK != 0)
    }
}

impl Strategy for RockBlock {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            SKILL_DRAIN => 2800,
            BARBAROS => 2600,
            SOLEMN_WARNING => 2300,
            GUARDIAN | SANDMAN => 2000,
            RAI_OH => 1950,
            MAGIC_JAMMER | ROYAL_OPPRESSION | MYSTIC_BOX => 1700,
            BOULDER => 1300,
            GRAND_MOLE | MORPHING_JAR => 1100,
            NECRO_GARDNA => 1000,
            AD_CHANGER => 600,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        if Self::want_skill_drain(&ctx) {
            if let Some(i) = t.activate_from(SKILL_DRAIN, Location::SpellTrapZone) {
                return t.pick(i);
            }
        }
        // Mystic Box: trade our weakest body for their best monster.
        if let Some(i) = t.activate(MYSTIC_BOX) {
            let theirs = ctx.monsters(ctx.opp).into_iter().max_by_key(|c| ctx.threat(c));
            let ours = ctx
                .monsters(ctx.me)
                .into_iter()
                .min_by_key(|c| value(self, &ctx, None, Some(c)));
            if let (Some(theirs), Some(ours)) = (theirs, ours) {
                if ctx.threat(theirs) >= 1900 && value(self, &ctx, None, Some(ours)) <= 1300 && ours.attack < 1500 {
                    return t.pick_targeting(i, vec![theirs.at, ours.at]);
                }
            }
        }
        // Barbaros' Tribute 3 summon clears the opponent's whole side.
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let data = ctx.data(code);
        let drained = Self::drained(&ctx);
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, BARBAROS) => {
                let opp_cards = ctx.monsters(ctx.opp).len() + ctx.spell_traps(ctx.opp).len();
                let fodder = ctx.monsters(ctx.me).len();
                if fodder >= 3 && opp_cards >= 3 {
                    Some(4000.0)
                } else if fodder == 0 {
                    Some(if drained { 3200.0 } else { 2400.0 })
                } else {
                    // The engine would ask to Tribute; never feed it our board.
                    None
                }
            }
            (ChoiceKind::NormalSummon, GUARDIAN | SANDMAN) => {
                // Without Skill Drain they need a Rock to reveal every End Phase.
                let rocks = ctx.hand().iter().filter(|c| ctx.view_data(c).race & races::ROCK != 0).count();
                if drained || rocks >= 2 { Some(2000.0) } else { Some(1300.0) }
            }
            (ChoiceKind::NormalSummon, RAI_OH) => Some(1950.0),
            (ChoiceKind::SetMonster, MORPHING_JAR) if ctx.hand_size(ctx.me) <= 2 => Some(1500.0),
            (ChoiceKind::SetMonster, MORPHING_JAR) => None,
            (ChoiceKind::NormalSummon, MORPHING_JAR | AD_CHANGER | NECRO_GARDNA) => None,
            (ChoiceKind::SetMonster, BARBAROS) => None,
            (ChoiceKind::NormalSummon, _) if data.attack >= ctx.opp_best_attack() || ctx.monsters(ctx.opp).is_empty() => {
                Some(data.attack as f64)
            }
            _ => return None,
        })
    }

    fn attack_outcome(&self, ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
        // Grand Mole bounces whatever it battles (unless we drained it).
        (ctx.is(attacker, GRAND_MOLE) && !Self::drained(ctx) && ctx.threat(target) >= 1500)
            .then_some(Outcome::Win { trick: false })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let from_field = choice.at().map_or(false, |a| a.location.is_field());
        let hostile = t.hostile_top();
        let incoming = ctx.incoming_attack();
        Some(match code {
            SKILL_DRAIN if Self::want_skill_drain(&ctx) => Response::new(35.0),
            SKILL_DRAIN => Response::no(),
            SANDMAN if hostile.matches(|link| ctx.data(link.code).is_trap()) => Response::new(95.0),
            GUARDIAN if hostile.matches(|link| ctx.data(link.code).is_monster()) => Response::new(90.0),
            SANDMAN | GUARDIAN => Response::no(),
            RAI_OH if from_field => Response::new(60.0),
            MAGIC_JAMMER
                if hostile.matches(|link| {
                    ctx.data(link.code).is_spell()
                        && (DANGEROUS_SPELLS.contains(&ctx.canonical(link.code))
                            || link.targets.iter().any(|at| at.controller == ctx.me)
                            || ctx.hand_size(ctx.me) >= 3)
                }) =>
            {
                Response::new(92.0)
            }
            MAGIC_JAMMER => Response::no(),
            SOLEMN_WARNING => match Self::summoned(&ctx) {
                Some((card, code)) if ctx.my_lp() > 3000 => {
                    let data = ctx.data(code);
                    if card.attack.max(data.attack) >= 1900 || data.is_extra() {
                        Response::new(80.0)
                    } else {
                        Response::no()
                    }
                }
                _ => Response::no(),
            },
            ROYAL_OPPRESSION if from_field && choice.at().and_then(|a| ctx.card(a)).map_or(false, |c| c.position.face_up) => {
                // The "pay 800 to negate a Special Summon" effect.
                if ctx.my_lp() > 2400 && !ctx.my_turn() { Response::new(45.0) } else { Response::no() }
            }
            ROYAL_OPPRESSION => {
                let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
                if end_of_their_turn { Response::new(15.0) } else { Response::no() }
            }
            NECRO_GARDNA => match incoming {
                Some((attacker, target)) if ctx.attack_hurts(attacker, target) => Response::new(55.0),
                _ => Response::no(),
            },
            AD_CHANGER => Response::no(),
            _ => return None,
        })
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        // Koa'ki Meiru upkeep: reveal/send the least useful Rock.
        if t.decision.hint == Hint::Confirm && member.at.controller == ctx.me {
            return Some(-(value(self, &ctx, member.code, None) as f64));
        }
        None
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        // Keep Koa'ki beaters alive when we can pay their upkeep.
        let subject = t.decision.subject.map(|c| t.ctx.canonical(c));
        match subject {
            Some(GUARDIAN) | Some(SANDMAN) if t.ctx.phase() == Some(Phase::End) => Some(Self::rock_in_hand(&t.ctx)),
            _ => None,
        }
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let data = t.ctx.data(t.ctx.canonical(code));
        Some(data.is_trap() || data.is(crate::cards::types::QUICKPLAY))
    }
}
