//! "Ojama Brigade": Chazz's Ojamas, Armed Dragons and the XYZ union.
//!
//! The Ojamas are 0 ATK bodies that pay off together: Ojama Delta Hurricane!!
//! destroys every card the opponent controls once Ojama Green, Yellow and
//! Black are face-up, and Polymerization turns them into Ojama King (or two
//! of them into Ojama Knight), which also locks the opponent's zones.  Ojama
//! Red puts the hand's Ojamas on the field, Ojama Country revives one for an
//! Ojama card from the hand (Ojamagic as that cost searches all three) and
//! swaps every monster's ATK and DEF.  Ojama Trio fills the opponent's zones
//! with tokens that cost them 300 each when destroyed.
//!
//! Beside them, Armed Dragon LV3 levels up in the Standby Phase, LV5 and LV7
//! discard a monster to destroy theirs, and X-Head Cannon collects its unions
//! for XYZ-Dragon Cannon.  Shining Angel and Masked Dragon float into the
//! next piece.

use crate::agent::{value, Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Phase, Position};

pub const DECK: &str = "Ojama Brigade";

const OJAMA_YELLOW: u32 = 42941100;
const OJAMA_GREEN: u32 = 12482652;
const OJAMA_BLACK: u32 = 79335209;
const OJAMA_RED: u32 = 37132349;
const OJAMA_BLUE: u32 = 64627453;
const X_HEAD_CANNON: u32 = 62651957;
const Y_DRAGON_HEAD: u32 = 65622692;
const Z_METAL_TANK: u32 = 64500000;
const ARMED_DRAGON_LV3: u32 = 980973;
const ARMED_DRAGON_LV5: u32 = 46384672;
const ARMED_DRAGON_LV7: u32 = 73879377;
const SHINING_ANGEL: u32 = 95956346;
const MASKED_DRAGON: u32 = 39191307;
const POLYMERIZATION: u32 = 24094653;
const OJAMAGIC: u32 = 24643836;
const DELTA_HURRICANE: u32 = 8251996;
const OJAMA_COUNTRY: u32 = 90011152;
const LEVEL_UP: u32 = 25290459;
const STAMPING_DESTRUCTION: u32 = 81385346;
const CHTHONIAN_ALLIANCE: u32 = 46910446;
const OJAMA_TRIO: u32 = 29843091;
const OJAMA_KING: u32 = 90140980;
const OJAMA_KNIGHT: u32 = 40391316;
const XYZ_DRAGON_CANNON: u32 = 91998119;
const SET_OJAMA: u16 = 0xf;
/// The union effect that equips it (the other one Special Summons it back).
const UNION_EQUIP: u64 = 1068;

const TRIO: [u32; 3] = [OJAMA_GREEN, OJAMA_YELLOW, OJAMA_BLACK];

#[derive(Clone, Default)]
pub struct Ojama;

impl Ojama {
    fn is_ojama_monster(ctx: &Ctx, code: u32) -> bool {
        let d = ctx.data(code);
        d.is_monster() && d.in_set(SET_OJAMA) && !d.is_extra()
    }

    fn face_up(ctx: &Ctx, code: u32) -> bool {
        ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.is(c, code))
    }

    /// Trio members missing from our face-up monsters.
    fn missing_trio(ctx: &Ctx) -> Vec<u32> {
        TRIO.iter().copied().filter(|c| !Self::face_up(ctx, *c)).collect()
    }

    fn ojamas_in_hand(ctx: &Ctx) -> usize {
        ctx.hand().iter().filter(|c| c.code.map_or(false, |k| Self::is_ojama_monster(ctx, ctx.canonical(k)))).count()
    }

    fn country_up(ctx: &Ctx) -> bool {
        ctx.face_up_on_field(ctx.me, OJAMA_COUNTRY)
    }

    fn is_dragon_lv(ctx: &Ctx, card: &CardView) -> bool {
        [ARMED_DRAGON_LV3, ARMED_DRAGON_LV5].iter().any(|k| ctx.is(card, *k))
    }

    /// Opponent monsters Armed Dragon LV5 / LV7 could destroy by sending a
    /// monster with `attack` ATK: LV5 one of them, LV7 all of them.
    fn in_reach<'a>(ctx: &Ctx<'a>, attack: i32) -> Vec<&'a CardView> {
        ctx.monsters(ctx.opp).into_iter().filter(|c| c.position.face_up && c.attack <= attack).collect()
    }

    /// The hand monster to send for Armed Dragon's effect: the cheapest one
    /// that still reaches the target(s).  LV7 is the ideal cost, it can only
    /// come out through LV5.
    fn dragon_cost<'a>(&self, ctx: &Ctx<'a>, sweep: bool) -> Option<(&'a CardView, Vec<&'a CardView>)> {
        let mut best: Option<(i32, &CardView, Vec<&CardView>)> = None;
        for card in ctx.hand() {
            let data = ctx.view_data(card);
            if !data.is_monster() {
                continue;
            }
            let reach = Self::in_reach(ctx, data.attack);
            let gain: i32 = if sweep {
                reach.iter().map(|c| ctx.threat(c)).sum()
            } else {
                reach.iter().map(|c| ctx.threat(c)).max().unwrap_or(0)
            };
            let cost = if ctx.is(card, ARMED_DRAGON_LV7) { 0 } else { value(self, ctx, card.code, None) };
            let net = gain - cost;
            if gain >= 1200 && net > 0 && best.as_ref().map_or(true, |b| net > b.0) {
                best = Some((net, card, reach));
            }
        }
        best.map(|(_, card, reach)| (card, reach))
    }

    fn union_equip(t: &Turn, code: u32) -> Option<usize> {
        let ctx = t.ctx;
        t.find_where(|c| {
            c.kind == ChoiceKind::Activate
                && c.description == UNION_EQUIP
                && c.code().map(|k| ctx.canonical(k)) == Some(code)
        })
    }
}

impl Strategy for Ojama {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            OJAMA_KING => 3000,
            XYZ_DRAGON_CANNON | ARMED_DRAGON_LV7 => 2800,
            DELTA_HURRICANE => 2600,
            OJAMA_KNIGHT => 2500,
            ARMED_DRAGON_LV5 => 2400,
            OJAMA_COUNTRY => 2000,
            OJAMA_RED | X_HEAD_CANNON => 1800,
            POLYMERIZATION => 1700,
            LEVEL_UP | ARMED_DRAGON_LV3 => 1600,
            OJAMAGIC | Y_DRAGON_HEAD | Z_METAL_TANK => 1500,
            SHINING_ANGEL => 1450,
            MASKED_DRAGON | OJAMA_TRIO => 1400,
            STAMPING_DESTRUCTION => 1300,
            OJAMA_BLUE => 1100,
            OJAMA_GREEN | OJAMA_YELLOW | OJAMA_BLACK => 900,
            CHTHONIAN_ALLIANCE => 600,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let their_cards = ctx.monsters(ctx.opp).len() + ctx.spell_traps(ctx.opp).len();
        // Only offered with the Trio face-up: their whole side goes.
        if their_cards >= 2 || ctx.field_strength(ctx.opp) >= 1500 {
            if let Some(i) = t.activate(DELTA_HURRICANE) {
                return t.pick(i);
            }
        }
        if !Self::country_up(&ctx) {
            if let Some(i) = t.activate_from(OJAMA_COUNTRY, Location::Hand) {
                return t.pick(i);
            }
        }
        // Ojama Country: an Ojama card from the hand revives one.  With
        // Ojamagic as the cost, it also searches the Trio.
        if let Some(i) = t.activate_from(OJAMA_COUNTRY, Location::SpellTrapZone) {
            let ojamagic = ctx.in_hand(OJAMAGIC);
            let missing = Self::missing_trio(&ctx);
            let revives_piece = ctx.graveyard(ctx.me).iter().any(|c| missing.iter().any(|k| ctx.is(c, *k)));
            let spare = Self::ojamas_in_hand(&ctx) >= 2;
            if ojamagic || (revives_piece && spare) {
                return t.pick(i);
            }
        }
        // Ojama King (or Knight) from the Ojamas we have: after the Hurricane
        // if it is coming, since it needs them on the field.
        let hurricane_ready = ctx.in_hand(DELTA_HURRICANE) && Self::missing_trio(&ctx).is_empty();
        let red_first = ctx.in_hand(DELTA_HURRICANE) && ctx.in_hand(OJAMA_RED) && t.has(ChoiceKind::NormalSummon);
        if !hurricane_ready && !red_first {
            if let Some(i) = t.activate(POLYMERIZATION) {
                let trio_known = TRIO.iter().all(|k| {
                    ctx.in_hand(*k) || Self::face_up(&ctx, *k)
                });
                let ojamas = Self::ojamas_in_hand(&ctx)
                    + ctx.monsters(ctx.me).iter().filter(|c| c.code.map_or(false, |k| Self::is_ojama_monster(&ctx, ctx.canonical(k)))).count();
                let hurricane_later = ctx.in_hand(DELTA_HURRICANE) && !trio_known;
                if trio_known || (ojamas >= 2 && !hurricane_later) {
                    return t.pick(i);
                }
            }
        }
        // Armed Dragons: level up, then destroy with a discarded monster.
        if let Some(i) = t.activate(LEVEL_UP) {
            if ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && Self::is_dragon_lv(&ctx, c)) {
                return t.pick(i);
            }
        }
        for (code, sweep) in [(ARMED_DRAGON_LV7, true), (ARMED_DRAGON_LV5, false)] {
            if let Some(i) = t.activate_from(code, Location::MonsterZone) {
                if let Some((_, reach)) = self.dragon_cost(&ctx, sweep) {
                    let target = reach.iter().max_by_key(|c| ctx.threat(c)).map(|c| c.at);
                    return t.pick_targeting(i, target.into_iter().collect());
                }
            }
        }
        // XYZ-Dragon Cannon: a spare card for their best one.
        if let Some(i) = t.activate_from(XYZ_DRAGON_CANNON, Location::MonsterZone) {
            let target = ctx.monsters(ctx.opp).into_iter().chain(ctx.spell_traps(ctx.opp)).max_by_key(|c| ctx.threat(c));
            let cheapest = ctx.hand().iter().map(|c| if ctx.is(c, OJAMAGIC) { 0 } else { value(self, &ctx, c.code, None) }).min();
            if let (Some(target), Some(cheapest)) = (target, cheapest) {
                if ctx.threat(target) >= 1000 && cheapest < ctx.threat(target) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
        }
        // Unions: dress X-Head Cannon (and each other) up.
        if Self::face_up(&ctx, X_HEAD_CANNON) {
            for code in [Y_DRAGON_HEAD, Z_METAL_TANK] {
                if let Some(i) = Self::union_equip(t, code) {
                    return t.pick(i);
                }
            }
        }
        // Stamping Destruction: needs a Dragon of ours; their backrow first.
        let backrow = ctx.spell_traps(ctx.opp).into_iter().max_by_key(|c| ctx.threat(c));
        if let Some(target) = backrow {
            if let Some(i) = t.activate(STAMPING_DESTRUCTION) {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        None
    }

    fn main_phase_late(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Chthonian Alliance: only on a monster with namesakes beside it.
        let holder = ctx.monsters(ctx.me).into_iter().filter(|c| c.position.face_up && c.known()).find(|c| {
            ctx.monsters(ctx.me).iter().chain(ctx.monsters(ctx.opp).iter()).any(|o| o.at != c.at && o.position.face_up && o.code == c.code)
        });
        if let Some(holder) = holder {
            if let Some(i) = t.activate(CHTHONIAN_ALLIANCE) {
                return t.pick_targeting(i, vec![holder.at]);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let ojamas = Self::ojamas_in_hand(&ctx);
        let x_up = Self::face_up(&ctx, X_HEAD_CANNON);
        Some(match (choice.kind, code) {
            // Ojama Red brings the rest of the hand's Ojamas.
            (ChoiceKind::NormalSummon, OJAMA_RED) if ojamas >= 2 => Some(2500.0 + 300.0 * ojamas as f64),
            (ChoiceKind::NormalSummon, ARMED_DRAGON_LV3) => Some(2000.0),
            (ChoiceKind::NormalSummon, X_HEAD_CANNON) => Some(1900.0),
            (ChoiceKind::NormalSummon, Y_DRAGON_HEAD | Z_METAL_TANK) if x_up => Some(1850.0),
            // Ojama Blue searches two Ojama cards when it is destroyed in battle.
            (ChoiceKind::SetMonster, OJAMA_BLUE) => Some(1300.0),
            (ChoiceKind::SetMonster, SHINING_ANGEL | MASKED_DRAGON) => Some(1250.0),
            // The Trio pieces stay in hand for Red, Polymerization and Country's cost.
            (ChoiceKind::NormalSummon, OJAMA_GREEN | OJAMA_YELLOW | OJAMA_BLACK | OJAMA_BLUE | OJAMA_RED) => None,
            (ChoiceKind::SetMonster, OJAMA_GREEN | OJAMA_YELLOW | OJAMA_BLACK | OJAMA_RED) => {
                let keep = ctx.in_hand(OJAMA_RED) || ctx.in_hand(POLYMERIZATION) || ctx.in_hand(DELTA_HURRICANE);
                if keep && !ctx.monsters(ctx.me).is_empty() { None } else { Some(600.0) }
            }
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        Some(match code {
            // Three tokens on their side: the zones they fill and 300 each
            // when they die.  At the end of their turn, or before we attack.
            OJAMA_TRIO => {
                let free = ctx.free_monster_zones(ctx.opp);
                let end_of_their_turn = !ctx.my_turn() && ctx.phase() == Some(Phase::End);
                let before_attack = ctx.my_turn()
                    && ctx.main1()
                    && ctx.monsters(ctx.me).iter().any(|c| ctx.can_attack(c) && (c.attack > 1000 || Self::country_up(&ctx)));
                if free >= 3 && (end_of_their_turn || before_attack) { Response::new(25.0) } else { Response::no() }
            }
            // Equipped, the unions sit in the Spell & Trap Zone, where the
            // open window would unequip them: the Main Phase plan decides.
            Y_DRAGON_HEAD | Z_METAL_TANK => Response::no(),
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
        match t.memory.last_activated.map(|c| ctx.canonical(c)) {
            // Ojama Country's cost: Ojamagic searches the Trio on its way.
            Some(OJAMA_COUNTRY) if member.at.location == Location::Hand && t.decision.hint.is_cost() => {
                return Some(if code == OJAMAGIC { 5000.0 } else { -worth });
            }
            // Ojama Country revives a missing Trio piece first.
            Some(OJAMA_COUNTRY) if member.at.location == Location::Graveyard => {
                let missing = Self::missing_trio(&ctx).contains(&code);
                return Some(worth + if missing { 2000.0 } else { 0.0 });
            }
            // Armed Dragon LV5 / LV7: the cheapest monster that reaches the target.
            Some(ARMED_DRAGON_LV5 | ARMED_DRAGON_LV7) if member.at.location == Location::Hand && t.decision.hint.is_cost() => {
                let sweep = t.memory.last_activated.map(|c| ctx.canonical(c)) == Some(ARMED_DRAGON_LV7);
                let chosen = self.dragon_cost(&ctx, sweep).map(|(card, _)| card.at);
                return Some(if chosen == Some(member.at) { 5000.0 } else { -worth });
            }
            // XYZ-Dragon Cannon's discard.
            Some(XYZ_DRAGON_CANNON) if member.at.location == Location::Hand => {
                return Some(if code == OJAMAGIC { 5000.0 } else { -worth });
            }
            _ => {}
        }
        match t.decision.hint {
            // Searches and floaters: what the board needs.
            Hint::AddToHand | Hint::SpecialSummon if member.at.location == Location::Deck => {
                let missing = Self::missing_trio(&ctx).contains(&code) && ctx.in_hand(DELTA_HURRICANE);
                let bonus = match code {
                    DELTA_HURRICANE if !ctx.in_hand(DELTA_HURRICANE) => 1500.0,
                    OJAMAGIC if !ctx.in_hand(OJAMAGIC) => 800.0,
                    ARMED_DRAGON_LV3 => 400.0,
                    Y_DRAGON_HEAD | Z_METAL_TANK if Self::face_up(&ctx, X_HEAD_CANNON) => 600.0,
                    _ if missing => 1200.0,
                    _ => 0.0,
                };
                Some(worth + bonus)
            }
            // Fusion Material: spare Ojamas, never the pieces a Hurricane still needs.
            Hint::FusionMaterial => Some(-worth),
            _ => None,
        }
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        let ctx = t.ctx;
        let code = ctx.canonical(code);
        match code {
            // 0 ATK walls, unless Ojama Country turns their DEF into ATK.
            OJAMA_KING | OJAMA_KNIGHT if Self::country_up(&ctx) => Some(Position::FACE_UP_ATTACK),
            OJAMA_KING | OJAMA_KNIGHT => Some(Position::FACE_UP_DEFENSE),
            _ if Self::is_ojama_monster(&ctx, code) => Some(Position::FACE_UP_DEFENSE),
            _ => None,
        }
    }
}
