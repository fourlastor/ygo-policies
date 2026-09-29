//! "Legendary Six Samurai": the 2011 swarm deck.
//!
//! The Samurai Special Summon each other for free: Kageki's Normal Summon
//! brings a Level 4 or lower one from the hand, Kizan and Grandmaster come
//! out beside any other Samurai, Shinai and Mizuho beside each other, Great
//! Shogun Shien beside two.  Gateway of the Six gets 2 Bushido Counters per
//! summon and spends 4 to search a Samurai, Six Samurai United draws.  The
//! board then turns into removal: Mizuho and Hand of the Six Samurai Tribute
//! a spare Samurai to destroy a card (Shinai returns one when Tributed),
//! Enishi, Shien's Chancellor destroys a face-up monster, Legendary Enishi
//! bounces one.  Kagemusha or Shien's Squire (Tuners) with a Level 3 make
//! Legendary Six Samurai - Shi En, which negates a Spell/Trap each turn;
//! Great Shogun Shien limits the opponent to one Spell/Trap per turn and
//! Musakani Magatama counters their destruction.

use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};

pub const DECK: &str = "Legendary Six Samurai";

const KAGEKI: u32 = 2511717;
const KAGEMUSHA: u32 = 1498130;
const KIZAN: u32 = 49721904;
const ENISHI: u32 = 75116619;
const SHINAI: u32 = 48505422;
const MIZUHO: u32 = 74094021;
const GRANDMASTER: u32 = 83039729;
const GREAT_SHOGUN_SHIEN: u32 = 63176202;
const SHIENS_SQUIRE: u32 = 33883834;
const HAND_OF_THE_SIX: u32 = 78792195;
const ENISHI_CHANCELLOR: u32 = 38280762;
const SPIRIT_OF_THE_SIX: u32 = 65685470;
const SMOKE_SIGNAL: u32 = 54031490;
const UNITED: u32 = 72345736;
const GATEWAY: u32 = 27970830;
const ASCETICISM: u32 = 27821104;
const CUNNING: u32 = 27178262;
const DOUBLE_EDGED_SWORD: u32 = 21007444;
const MUSAKANI_MAGATAMA: u32 = 41458579;
const RETURN_OF_THE_SIX: u32 = 46874015;
const SHI_EN: u32 = 29981921;
const BLACK_ROSE_DRAGON: u32 = 73580471;
const RED_DRAGON_ARCHFIEND: u32 = 70902743;
const SET_SIX_SAMURAI: u16 = 0x3d;
/// Union: "equip this card" (the other option Special Summons it back).
const UNION_EQUIP: u64 = 1068;

#[derive(Default)]
pub struct SixSamurai;

impl SixSamurai {
    fn is_samurai(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.in_set(SET_SIX_SAMURAI)
    }

    fn samurai_up(ctx: &Ctx) -> usize {
        ctx.monsters(ctx.me).iter().filter(|c| c.position.face_up && Self::is_samurai(ctx, c)).count()
    }

    fn gateway_counters(ctx: &Ctx) -> u32 {
        ctx.spell_traps(ctx.me).iter().filter(|c| c.position.face_up && ctx.is(c, GATEWAY)).map(|c| c.counters).sum()
    }

    /// Their card most worth a spare Samurai.
    fn removal_target<'a>(ctx: &Ctx<'a>, monsters_only: bool) -> Option<&'a CardView> {
        let backrow = if monsters_only { Vec::new() } else { ctx.spell_traps(ctx.opp) };
        ctx.monsters(ctx.opp)
            .into_iter()
            .chain(backrow)
            .max_by_key(|c| ctx.threat(c))
            .filter(|c| ctx.threat(c) >= 1500)
    }

    /// A spare Samurai to Tribute: Shinai (it returns another), else the cheapest.
    fn spare(&self, ctx: &Ctx, except: u32) -> Option<i32> {
        ctx.monsters(ctx.me)
            .into_iter()
            .filter(|c| c.position.face_up && Self::is_samurai(ctx, c) && !ctx.is(c, except))
            .map(|c| if ctx.is(c, SHINAI) { 0 } else { value(self, ctx, None, Some(c)) })
            .min()
    }

    fn small_samurai_in_hand(ctx: &Ctx) -> bool {
        ctx.hand().iter().any(|c| Self::is_samurai(ctx, c) && ctx.view_data(c).level <= 4 && !ctx.is(c, KAGEKI))
    }
}

impl Strategy for SixSamurai {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            SHI_EN => 3000,
            GREAT_SHOGUN_SHIEN | GATEWAY => 2500,
            ENISHI_CHANCELLOR => 2200,
            GRANDMASTER => 2100,
            KAGEKI | ENISHI | MUSAKANI_MAGATAMA => 1900,
            KIZAN | UNITED => 1800,
            MIZUHO | HAND_OF_THE_SIX | SMOKE_SIGNAL => 1700,
            KAGEMUSHA => 1600,
            SHINAI | CUNNING => 1500,
            SHIENS_SQUIRE | RETURN_OF_THE_SIX => 1300,
            SPIRIT_OF_THE_SIX | ASCETICISM => 1200,
            DOUBLE_EDGED_SWORD => 1100,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Counter collectors before the first summon.
        for code in [GATEWAY, UNITED] {
            if !ctx.face_up_on_field(ctx.me, code) {
                if let Some(i) = t.activate_from(code, Location::Hand) {
                    return t.pick(i);
                }
            }
        }
        if let Some(i) = t.activate(SMOKE_SIGNAL) {
            return t.pick(i);
        }
        // Gateway: 6 counters revive a Shien, 4 search a Samurai.
        let counters = Self::gateway_counters(&ctx);
        let gateway = |t: &Turn, index: u64| {
            t.find_where(|c| c.kind == ChoiceKind::Activate && c.code().map(|k| ctx.canonical(k)) == Some(GATEWAY) && c.description & 0xf == index)
        };
        if counters >= 6 && ctx.graveyard(ctx.me).iter().any(|c| ctx.is(c, GREAT_SHOGUN_SHIEN)) {
            if let Some(i) = gateway(t, 2) {
                return t.pick(i);
            }
        }
        if counters >= 4 {
            if let Some(i) = gateway(t, 1) {
                return t.pick(i);
            }
        }
        let united = ctx.spell_traps(ctx.me).iter().find(|c| ctx.is(c, UNITED)).map_or(0, |c| c.counters);
        if united >= 2 {
            if let Some(i) = t.activate_from(UNITED, Location::SpellTrapZone) {
                return t.pick(i);
            }
        }
        // Removal paid with a spare Samurai.
        if let Some(target) = Self::removal_target(&ctx, false) {
            if self.spare(&ctx, MIZUHO).map_or(false, |v| v < ctx.threat(target) + 300) {
                if let Some(i) = t.activate_from(MIZUHO, Location::MonsterZone) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
        }
        if let Some(target) = Self::removal_target(&ctx, true) {
            if self.spare(&ctx, HAND_OF_THE_SIX).map_or(false, |v| v < ctx.threat(target) + 300) {
                if let Some(i) = t.activate_from(HAND_OF_THE_SIX, Location::MonsterZone) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
            // The Chancellor's destruction costs it this turn's attack.
            if target.position.face_up {
                if let Some(i) = t.activate_from(ENISHI_CHANCELLOR, Location::MonsterZone) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
            if target.position.face_up && ctx.main1() {
                if let Some(i) = t.activate_from(ENISHI, Location::MonsterZone) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
        }
        // Cunning: a spent Samurai for a better one in either Graveyard.
        let prize = ctx
            .graveyard(ctx.me)
            .into_iter()
            .chain(ctx.graveyard(ctx.opp))
            .filter(|c| Self::is_samurai(&ctx, c))
            .max_by_key(|c| value(self, &ctx, None, Some(c)));
        if let (Some(prize), Some(spare)) = (prize, self.spare(&ctx, 0)) {
            if value(self, &ctx, None, Some(prize)) >= spare + 800 {
                if let Some(i) = t.activate(CUNNING) {
                    return t.pick_targeting(i, vec![prize.at]);
                }
            }
        }
        // Spirit of the Six Samurai rides the strongest attacker.
        if let Some(i) = t.find_where(|c| c.kind == ChoiceKind::Activate && c.code().map(|k| ctx.canonical(k)) == Some(SPIRIT_OF_THE_SIX) && c.description == UNION_EQUIP) {
            let holder = ctx
                .monsters(ctx.me)
                .into_iter()
                .filter(|c| c.position.face_up && Self::is_samurai(&ctx, c) && !ctx.is(c, SPIRIT_OF_THE_SIX))
                .max_by_key(|c| c.attack);
            if let Some(holder) = holder {
                return t.pick_targeting(i, vec![holder.at]);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let samurai_up = Self::samurai_up(&ctx);
        let level3_up = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && Self::is_samurai(&ctx, c) && c.level == 3);
        Some(match (choice.kind, code) {
            // Kageki's summon brings another Samurai from the hand.
            (ChoiceKind::NormalSummon, KAGEKI) if Self::small_samurai_in_hand(&ctx) => Some(2600.0),
            // Kagemusha with a Level 3 Samurai is Shi En.
            (ChoiceKind::NormalSummon, KAGEMUSHA) if level3_up => Some(2400.0),
            (ChoiceKind::NormalSummon, ENISHI) if samurai_up >= 1 => Some(2200.0),
            (ChoiceKind::NormalSummon, HAND_OF_THE_SIX | MIZUHO) if samurai_up >= 1 => Some(1900.0),
            // Kizan and Grandmaster summon themselves beside another Samurai:
            // the Normal Summon goes to one that cannot.
            (ChoiceKind::NormalSummon, KIZAN | GRANDMASTER) if samurai_up >= 1 => None,
            (ChoiceKind::NormalSummon, SHIENS_SQUIRE) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // Both destroy our own side too.
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
        let incoming = ctx.incoming_attack();
        Some(match code {
            // Once per turn: their Spell/Trap is negated and destroyed.
            SHI_EN if hostile.matches(|link| !ctx.data(link.code).is_monster()) => Response::new(90.0),
            SHI_EN => Response::no(),
            // Offered only against their destruction.
            MUSAKANI_MAGATAMA if hostile.matches(|_| true) => Response::new(88.0),
            MUSAKANI_MAGATAMA => Response::no(),
            // Kagemusha takes the hit for a more valuable Samurai.
            KAGEMUSHA => match hostile {
                crate::agent::Hostile::Link(link) => {
                    let saved = link.targets.iter().filter_map(|at| ctx.card(*at)).map(|c| value(self, &ctx, None, Some(c))).max().unwrap_or(0);
                    if saved > 1600 { Response::new(70.0) } else { Response::no() }
                }
                _ => Response::no(),
            },
            // Legendary Enishi: bounce their attacker.
            ENISHI => match incoming {
                Some((attacker, target)) if attacker.position.face_up && ctx.attack_hurts(attacker, target) => {
                    Response::targeting(60.0, vec![attacker.at])
                }
                _ => Response::no(),
            },
            // From the hand: our battling Samurai survives.
            SHIENS_SQUIRE if choice.at().map_or(false, |a| a.location == Location::Hand) => {
                let (Some(attacker), Some(target)) = (ctx.battle_attacker(), ctx.battle_target()) else { return Some(Response::no()) };
                let (ours, theirs) = if attacker.at.controller == ctx.me { (attacker, target) } else { (target, attacker) };
                let loses = if ours.position.attack { ours.attack <= ctx.battle_stat(theirs) } else { false };
                if Self::is_samurai(&ctx, ours) && loses && value(self, &ctx, None, Some(ours)) >= 1800 { Response::new(50.0) } else { Response::no() }
            }
            // Free bodies (they die in the End Phase): a blocker, or Synchro material.
            RETURN_OF_THE_SIX | DOUBLE_EDGED_SWORD => {
                let blocker = incoming.is_some() && ctx.monsters(ctx.me).is_empty();
                let tuner_up = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).is_tuner());
                let material = ctx.my_turn() && ctx.main1() && tuner_up;
                if code == RETURN_OF_THE_SIX && (blocker || material) {
                    Response::new(if blocker { 45.0 } else { 20.0 })
                } else if code == DOUBLE_EDGED_SWORD && material && ctx.my_lp() > 4000 {
                    Response::new(15.0)
                } else {
                    Response::no()
                }
            }
            // A same-ATK Samurai from the Deck, for Synchro material.
            ASCETICISM => {
                let tuner_up = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).is_tuner());
                if ctx.my_turn() && ctx.main1() && tuner_up { Response::new(20.0) } else { Response::no() }
            }
            // Equipped, the union's other effect would unequip it.
            SPIRIT_OF_THE_SIX if choice.at().map_or(false, |a| a.location == Location::SpellTrapZone) => Response::no(),
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
            // Searches: Kageki starts a turn; otherwise the best follow-up.
            Hint::AddToHand if member.at.location == Location::Deck || member.at.location == Location::Graveyard => {
                let bonus = if code == KAGEKI && !ctx.in_hand(KAGEKI) { 1500.0 } else { 0.0 };
                Some(worth + bonus)
            }
            // Tributes for Mizuho / Hand: Shinai returns a Samurai when Tributed.
            Hint::Release | Hint::Tribute if member.at.location == Location::MonsterZone => {
                Some(if code == SHINAI { 500.0 } else { -worth })
            }
            _ => None,
        }
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let code = t.ctx.canonical(code);
        Some(t.ctx.data(code).is_trap() || matches!(code, ASCETICISM | CUNNING))
    }
}
