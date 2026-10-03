//! "Claudi-oh's Countdown": a clock nothing can stop, behind a wall nothing
//! gets through.
//!
//! Final Countdown costs 2000 Life Points and wins the duel 20 turns after
//! it resolves; once resolved there is nothing left on the field to answer.
//! The rest of the deck makes the opponent's ten Battle Phases not matter:
//!
//! - locks that stay: Messenger of Peace (1500 ATK and up), Level Limit -
//!   Area B and Gravity Bind (Level 4 and up), and Skill Drain so that no
//!   monster effect takes them away;
//! - locks that run out: Nightmare's Steelcage (two of their turns) and
//!   Swords of Revealing Light (three);
//! - covers for one turn: Threatening Roar, Waboku, Rainbow Life (which
//!   turns the turn's damage, burn included, into Life Points), and from the
//!   hand Battle Fader and Swift Scarecrow;
//! - Cyber Valley, which ends the Battle Phase when attacked and draws a
//!   card, and Shining Angel, which brings a Cyber Valley when it falls.
//!
//! One cover a turn is enough, so the policy spends one and keeps the rest:
//! a lock already up, then a Set Trap (which could be destroyed before it is
//! used), the hand traps last.  A cover that gets negated is seen in the
//! chain and answered with the next one.

use std::cell::Cell;

use crate::agent::{Hostile, Response, Strategy, Turn};
use crate::cards::types;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Location, Member, Phase, Position};

pub const DECK: &str = "Claudi-oh's Countdown";

const FINAL_COUNTDOWN: u32 = 95308449;
const POT_OF_DUALITY: u32 = 98645731;
const UPSTART_GOBLIN: u32 = 70368879;
const NIGHTMARES_STEELCAGE: u32 = 58775978;
const SWORDS: u32 = 72302403;
const MESSENGER_OF_PEACE: u32 = 44656491;
const LEVEL_LIMIT_AREA_B: u32 = 3136426;
const GRAVITY_BIND: u32 = 85742772;
const SKILL_DRAIN: u32 = 82732705;
const THREATENING_ROAR: u32 = 36361633;
const WABOKU: u32 = 12607053;
const RAINBOW_LIFE: u32 = 34002992;
const SWIFT_SCARECROW: u32 = 18964575;
const BATTLE_FADER: u32 = 19665973;
const CYBER_VALLEY: u32 = 3657444;
const SHINING_ANGEL: u32 = 95956346;

/// Negates one Spell or Trap a turn: what matters goes second.
const SHI_EN: u32 = 29981921;
/// Negates a Spell or Trap for a card from the hand, every time.
const HERAKLINOS: u32 = 27346636;
/// Turns each Life Point gain of its controller into 500 damage.
const FIRE_PRINCESS: u32 = 64752646;
/// Calls a card type on a random card of our hand: 700 damage if right.
const OMINOUS_FORTUNETELLING: u32 = 56995655;
/// Take every Spell and Trap off the field without targeting, or switch the
/// Traps off: a Set cover is used in response or not at all.  Giant Trunade,
/// Black Rose Dragon, Judgment Dragon, Trishula, Trap Stun, Royal Decree,
/// Heavy Storm.
const SWEEPS: &[u32] = &[42703248, 73580471, 57774843, 52687916, 59616123, 51452091, 19613556];
/// Effect damage of 800 and more at once: worth a Rainbow Life.  Tremendous
/// Fire, Ookazi, Wave-Motion Cannon, Secret Barrel, Magical Explosion, Meteor
/// of Destruction, Poison of the Old Man, Just Desserts, Chain Strike,
/// Ceasefire.
const BLASTS: &[u32] = &[46918794, 19523799, 38992735, 27053506, 32723153, 33767325, 8842266, 24068492, 91623717, 36468556];
/// Smaller burn: with the blasts, the sign of a deck that wins by effect
/// damage.  Final Flame, Goblin Thief, Hinotama, Stealth Bird, Solar Flare
/// Dragon, Dark Room of Nightmare, Wattcannon.
const EMBERS: &[u32] = &[FIRE_PRINCESS, OMINOUS_FORTUNETELLING, 73134081, 45311864, 46130346, 3510565, 45985838, 85562745, 95084054];

/// What stops a whole turn of attacks once activated.
const COVERS: &[u32] = &[THREATENING_ROAR, WABOKU, RAINBOW_LIFE, CYBER_VALLEY, SWIFT_SCARECROW, BATTLE_FADER];
/// The covers that let the attacks come and take their damage away.
const SHIELDS: &[u32] = &[WABOKU, RAINBOW_LIFE];

/// ATK a face-down monster of theirs is counted for.
const FACE_DOWN: i32 = 1000;
/// Life Points kept for what is not a cost of ours.
const RESERVE: i32 = 1000;

#[derive(Default)]
pub struct Countdown {
    /// The turn Final Countdown was activated: the other copies are spare cards.
    started: Cell<Option<u32>>,
    /// The turn a shield was last activated, and our Life Points then.
    shield: Cell<(u32, i32)>,
    /// A turn, and how many of the covers we activated in it were negated.
    void: Cell<(u32, usize)>,
}

/// The locks face-up on our field.
#[derive(Clone, Copy)]
struct Locks {
    /// Nothing attacks.
    cage: bool,
    /// Nothing with 1500 ATK or more attacks.
    peace: bool,
    /// Nothing of Level 4 or higher attacks.
    levels: bool,
}

impl Locks {
    fn stops(&self, card: &CardView) -> bool {
        self.cage || (card.position.face_up && ((self.peace && card.attack >= 1500) || (self.levels && card.level >= 4)))
    }
}

impl Countdown {
    fn face_up(ctx: &Ctx, code: u32) -> bool {
        ctx.spell_traps(ctx.me).iter().any(|c| c.position.face_up && ctx.is(c, code))
    }

    fn set(ctx: &Ctx, code: u32) -> bool {
        ctx.spell_traps(ctx.me).iter().any(|c| !c.position.face_up && ctx.is(c, code))
    }

    fn locks(ctx: &Ctx) -> Locks {
        let up = |code| Self::face_up(ctx, code);
        Locks {
            cage: up(NIGHTMARES_STEELCAGE) || up(SWORDS),
            peace: up(MESSENGER_OF_PEACE),
            levels: up(LEVEL_LIMIT_AREA_B) || up(GRAVITY_BIND),
        }
    }

    /// Battle damage their monsters can still deal this turn through our
    /// locks: the attack being declared, and the others, strongest first.
    fn threat(ctx: &Ctx) -> (i32, Vec<i32>) {
        let locks = Self::locks(ctx);
        let declared = ctx.incoming_attack();
        let battle = ctx.phase().map_or(false, |p| p.is_battle());
        let this = match declared {
            Some((attacker, None)) => attacker.attack.max(0),
            Some((attacker, Some(target))) if target.position.face_up && target.position.attack => (attacker.attack - target.attack).max(0),
            _ => 0,
        };
        let mut others: Vec<i32> = ctx
            .monsters(ctx.opp)
            .iter()
            .filter(|c| declared.map_or(true, |(attacker, _)| attacker.at != c.at))
            .filter(|c| !locks.stops(c))
            // In their Battle Phase only what stands in Attack Position and
            // has not attacked; before it, anything can be turned.
            .filter(|c| !battle || (c.position.face_up && c.position.attack && ctx.can_attack(c)))
            .map(|c| if c.position.face_up { c.attack.max(0) } else { FACE_DOWN })
            .collect();
        others.sort_by(|a, b| b.cmp(a));
        // Each monster of ours takes one attack in our place: their weakest.
        let attacked = declared.and_then(|(_, target)| target).map(|c| c.at);
        let walls = ctx.monsters(ctx.me).iter().filter(|c| Some(c.at) != attacked).count();
        others.truncate(others.len().saturating_sub(walls));
        (this, others)
    }

    /// Damage we can take this turn without a cover: on their last turn
    /// before the clock runs out, whatever does not end the duel.
    fn spare_life(&self, ctx: &Ctx) -> i32 {
        match self.started.get() {
            Some(start) if !ctx.my_turn() && ctx.obs.turn >= start + 19 => ctx.my_lp() - 1,
            _ => 0,
        }
    }

    /// The covers we activated this turn, in order.
    fn spent(t: &Turn) -> Vec<u32> {
        t.memory.activated.iter().map(|c| t.ctx.canonical(*c)).filter(|c| COVERS.contains(c)).collect()
    }

    /// Their monster's effect or Counter Trap sits right on top of the cover
    /// we just activated: it is being negated.
    fn negated(t: &Turn) -> bool {
        let ctx = t.ctx;
        let [.., ours, theirs] = ctx.obs.chain.as_slice() else { return false };
        ours.controller == ctx.me
            && theirs.controller == ctx.opp
            && COVERS.contains(&ctx.canonical(ours.code))
            && (theirs.source.location == Location::MonsterZone || ctx.data(theirs.code).is(types::COUNTER))
    }

    /// A cover already used this turn, and still holding: no second one.
    /// A cover that was negated (Shi En, a Counter Trap) shows: the negation
    /// in the chain, an attack declared after all, damage through a shield.
    fn covered(&self, t: &Turn) -> bool {
        let ctx = t.ctx;
        let spent = Self::spent(t);
        if Self::negated(t) {
            self.void.set((ctx.obs.turn, spent.len()));
        }
        let (turn, dead) = self.void.get();
        let alive = if turn == ctx.obs.turn { spent.get(dead..).unwrap_or(&[]) } else { &spent[..] };
        match alive.last() {
            None => false,
            Some(code) if SHIELDS.contains(code) => {
                let (turn, lp) = self.shield.get();
                turn != ctx.obs.turn || ctx.my_lp() >= lp
            }
            Some(_) => ctx.incoming_attack().is_none(),
        }
    }

    /// Their chain link is about to take this Set card of ours away.
    fn under_fire(t: &Turn, choice: &Choice) -> bool {
        match (t.hostile_top(), choice.at()) {
            (Hostile::Link(link), Some(at)) => link.targets.contains(&at) || SWEEPS.contains(&t.ctx.canonical(link.code)),
            _ => false,
        }
    }

    /// A monster of ours that answers an attack by itself: a face-up Cyber
    /// Valley, or a Shining Angel that brings one for the next attack.
    fn sentinel(ctx: &Ctx) -> bool {
        ctx.monsters(ctx.me)
            .iter()
            .any(|c| (c.position.face_up && ctx.is(c, CYBER_VALLEY)) || (ctx.is(c, SHINING_ANGEL) && Self::valleys_left(ctx) > 0))
    }

    /// Cyber Valleys still in our Deck.
    fn valleys_left(ctx: &Ctx) -> usize {
        let seen: usize = [Location::Hand, Location::MonsterZone, Location::Graveyard, Location::Banished]
            .into_iter()
            .map(|location| ctx.count_in(ctx.me, location, CYBER_VALLEY))
            .sum();
        3usize.saturating_sub(seen)
    }

    /// The opponent has shown a card that deals effect damage.
    fn burning(ctx: &Ctx) -> bool {
        [Location::MonsterZone, Location::SpellTrapZone, Location::Graveyard, Location::Banished].into_iter().any(|location| {
            ctx.pile(ctx.opp, location).iter().any(|c| {
                c.code.map_or(false, |code| {
                    let code = ctx.canonical(code);
                    BLASTS.contains(&code) || EMBERS.contains(&code)
                })
            })
        })
    }

    /// Skill Drain's 1000 Life Points can be paid without touching what the
    /// clock still needs.
    fn drain_affordable(&self, ctx: &Ctx) -> bool {
        let kept = RESERVE + if self.started.get().is_none() { 2000 } else { 0 };
        ctx.my_lp() - 1000 >= kept
    }

    /// A face-up monster of theirs that negates the Traps we activate, with
    /// its effect in force.
    fn trap_negator(ctx: &Ctx) -> bool {
        !ctx.effects_drained() && [SHI_EN, HERAKLINOS].iter().any(|code| ctx.face_up_on_field(ctx.opp, *code))
    }

    /// The codes we could activate at this prompt.
    fn offered(t: &Turn) -> Vec<u32> {
        t.choices()
            .filter(|(i, c)| c.kind == ChoiceKind::Activate && t.fresh(*i))
            .filter_map(|(_, c)| c.code().map(|code| t.ctx.canonical(code)))
            // What a monster of theirs negates every time is no cover.
            .filter(|code| !t.ctx.wasted(*code))
            .collect()
    }

    fn free_zones(ctx: &Ctx) -> usize {
        5usize.saturating_sub(ctx.spell_traps(ctx.me).iter().filter(|c| c.at.sequence < 5).count())
    }

    /// Zones the Traps leave free: one for the Spells still to be played.
    fn kept_zones(&self, ctx: &Ctx) -> usize {
        let spell = ctx.hand().iter().any(|c| c.code.map_or(false, |code| ctx.data(code).is_spell() && !self.spare(ctx, ctx.canonical(code))));
        usize::from(self.started.get().is_none() || spell)
    }

    /// A card in hand that does nothing any more.
    fn spare(&self, ctx: &Ctx, code: u32) -> bool {
        match code {
            FINAL_COUNTDOWN => self.started.get().is_some() || ctx.count_in(ctx.me, Location::Hand, FINAL_COUNTDOWN) > 1,
            MESSENGER_OF_PEACE | LEVEL_LIMIT_AREA_B => Self::face_up(ctx, code),
            _ => false,
        }
    }

    /// The cover to activate at this prompt of their turn, if any.
    fn cover(&self, t: &Turn) -> Option<u32> {
        let ctx = t.ctx;
        if ctx.my_turn() || self.covered(t) {
            return None;
        }
        let offered = Self::offered(t);
        let has = |code: u32| offered.contains(&code);
        let (this, others) = Self::threat(&ctx);
        let total = this + others.iter().sum::<i32>();
        let spare_life = self.spare_life(&ctx);
        if let Some((_, target)) = ctx.incoming_attack() {
            // A Cyber Valley under attack ends the Battle Phase for a card.
            if target.map_or(false, |c| ctx.is(c, CYBER_VALLEY)) && has(CYBER_VALLEY) {
                return Some(CYBER_VALLEY);
            }
            if this == 0 || total <= spare_life {
                return None;
            }
            // Set Traps before the hand traps.  Rainbow Life needs a card
            // to discard: it goes first while there is one to spare, when
            // the hand is about to run out, or when the turn is worth it.
            // Under a monster that negates Traps, what is not a Trap -- or
            // the Trap that draws the negation, with Skill Drain to chain.
            let spare = ctx.hand().iter().any(|c| c.code.map_or(false, |code| self.spare(&ctx, ctx.canonical(code))));
            let bait = Self::set(&ctx, SKILL_DRAIN) && self.drain_affordable(&ctx);
            let order: &[u32] = if Self::trap_negator(&ctx) && !bait {
                &[SWIFT_SCARECROW, BATTLE_FADER, WABOKU, RAINBOW_LIFE, THREATENING_ROAR]
            } else if spare || total >= 4000 || ctx.hand().len() <= 2 {
                &[RAINBOW_LIFE, WABOKU, SWIFT_SCARECROW, BATTLE_FADER, THREATENING_ROAR]
            } else {
                &[WABOKU, RAINBOW_LIFE, SWIFT_SCARECROW, BATTLE_FADER, THREATENING_ROAR]
            };
            return order.iter().copied().find(|c| has(*c));
        }
        // Threatening Roar waits for their Battle Phase, when the board is
        // the one that would attack.
        let roar = ctx.phase() == Some(Phase::BattleStart) && total > spare_life && !Self::sentinel(&ctx) && has(THREATENING_ROAR);
        roar.then_some(THREATENING_ROAR)
    }
}

impl Strategy for Countdown {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        if self.spare(ctx, code) {
            return Some(0);
        }
        Some(match code {
            FINAL_COUNTDOWN => 5000,
            SWORDS => 2600,
            SKILL_DRAIN if Self::face_up(ctx, SKILL_DRAIN) => 300,
            SKILL_DRAIN => 2500,
            NIGHTMARES_STEELCAGE => 2400,
            MESSENGER_OF_PEACE => 2300,
            BATTLE_FADER | SWIFT_SCARECROW | CYBER_VALLEY | SHINING_ANGEL => 2200,
            LEVEL_LIMIT_AREA_B | GRAVITY_BIND => 2100,
            RAINBOW_LIFE => 1900,
            THREATENING_ROAR | WABOKU => 1800,
            POT_OF_DUALITY => 1200,
            UPSTART_GOBLIN => 600,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        let locks = Self::locks(&ctx);
        // The clock first: every turn it waits is one more turn to survive.
        // Shi En negates the first Spell of the turn: another one goes first.
        let negated = ctx.face_up_on_field(ctx.opp, SHI_EN) && t.memory.activated.is_empty();
        if self.started.get().is_none() && ctx.my_lp() > 2000 && !negated {
            if let Some(i) = t.activate(FINAL_COUNTDOWN) {
                self.started.set(Some(ctx.obs.turn));
                return t.pick(i);
            }
        }
        // Upstart Goblin's Life Points are 500 damage under Fire Princess.
        if !ctx.face_up_on_field(ctx.opp, FIRE_PRINCESS) || ctx.effects_drained() {
            if let Some(i) = t.activate(UPSTART_GOBLIN) {
                return t.pick(i);
            }
        }
        if let Some(i) = t.activate(POT_OF_DUALITY) {
            return t.pick(i);
        }
        // Locks that stay: as soon as drawn, while a zone is left for the clock.
        if Self::free_zones(&ctx) > usize::from(self.started.get().is_none()) {
            for code in [MESSENGER_OF_PEACE, LEVEL_LIMIT_AREA_B] {
                if !Self::face_up(&ctx, code) {
                    if let Some(i) = t.activate_from(code, Location::Hand) {
                        return t.pick(i);
                    }
                }
            }
        }
        // Locks that run out: one at a time, when something of theirs gets
        // through the others, or could once they Summon.
        if !locks.cage {
            let (_, others) = Self::threat(&ctx);
            if others.iter().sum::<i32>() > 0 || (!locks.peace && ctx.hand_size(ctx.opp) > 0) {
                for code in [NIGHTMARES_STEELCAGE, SWORDS] {
                    if let Some(i) = t.activate_from(code, Location::Hand) {
                        return t.pick(i);
                    }
                }
            }
        }
        None
    }

    /// The staples would play Swords on their own terms.
    fn allow_staple(&self, _t: &Turn, code: u32) -> bool {
        code != SWORDS
    }

    fn wants_battle(&self, _t: &Turn) -> Option<bool> {
        Some(false)
    }

    /// Cyber Valley stands guard face-up, Shining Angel Set, one at a time.
    /// Battle Fader and Swift Scarecrow work from the hand.
    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let fortunetelling = ctx.face_up_on_field(ctx.opp, OMINOUS_FORTUNETELLING);
        let (kind, score) = match choice.code().map(|c| ctx.canonical(c)) {
            Some(CYBER_VALLEY) => (ChoiceKind::NormalSummon, 200.0),
            Some(SHINING_ANGEL) => (ChoiceKind::SetMonster, 150.0),
            Some(SWIFT_SCARECROW) if fortunetelling => (ChoiceKind::SetMonster, 90.0),
            Some(BATTLE_FADER) if fortunetelling => (ChoiceKind::SetMonster, 80.0),
            _ => return Some(None),
        };
        // A Battle Fader left on the field is no guard.
        let guarded = ctx.monsters(ctx.me).iter().any(|c| !ctx.is(c, BATTLE_FADER));
        if guarded && fortunetelling {
            // Every monster in hand is 700 damage waiting: they go to the field.
            return Some((choice.kind == ChoiceKind::SetMonster).then_some(score));
        }
        Some((choice.kind == kind && !guarded).then_some(score))
    }

    fn allow_reposition(&self, _t: &Turn, _card: &CardView) -> bool {
        false
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let ctx = t.ctx;
        let code = ctx.canonical(code);
        if !ctx.data(code).is_trap() {
            return Some(false);
        }
        // One Skill Drain at a time: the next stays in hand, out of reach.
        if code == SKILL_DRAIN && (Self::face_up(&ctx, code) || Self::set(&ctx, code)) {
            return Some(false);
        }
        Some(Self::free_zones(&ctx) > self.kept_zones(&ctx))
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let fired = Self::under_fire(t, choice);
        match code {
            // The lock that has to be Set first: face-up at the first chance.
            GRAVITY_BIND => return Some(if fired { Response::no() } else { Response::new(88.0) }),
            // Skill Drain, once, when it can be paid for.  Chained to the
            // effect of a monster of theirs it takes that effect away, even
            // one aimed at Skill Drain itself; a Spell or Trap aimed at it
            // would only make us pay.  Never into Shi En's negation, and not
            // against a deck that burns unless Fire Princess does the burning.
            SKILL_DRAIN => {
                if Self::face_up(&ctx, SKILL_DRAIN) || !self.drain_affordable(&ctx) {
                    return Some(Response::no());
                }
                let go = match t.hostile_top() {
                    Hostile::Link(link) if link.source.location == Location::MonsterZone => true,
                    _ => {
                        let burn_only = Self::burning(&ctx) && !ctx.face_up_on_field(ctx.opp, FIRE_PRINCESS);
                        !fired && !Self::trap_negator(&ctx) && !burn_only
                    }
                };
                return Some(if go { Response::new(96.0) } else { Response::no() });
            }
            _ => {}
        }
        if !COVERS.contains(&code) {
            return None;
        }
        // Cyber Valley only has its answer to an attack for us.
        if code == CYBER_VALLEY && (ctx.my_turn() || ctx.incoming_attack().is_none()) {
            return Some(Response::no());
        }
        // Rainbow Life turns their burn into Life Points for the turn.
        if code == RAINBOW_LIFE && !self.covered(t) {
            if let Hostile::Link(link) = t.hostile_top() {
                if BLASTS.contains(&ctx.canonical(link.code)) {
                    self.shield.set((ctx.obs.turn, ctx.my_lp()));
                    return Some(Response::new(94.0));
                }
            }
        }
        // About to be taken away: use it now if it still does something.
        if fired && !self.covered(t) && !ctx.my_turn() {
            if SHIELDS.contains(&code) {
                self.shield.set((ctx.obs.turn, ctx.my_lp()));
            }
            return Some(Response::new(95.0));
        }
        if self.cover(t) != Some(code) {
            return Some(Response::no());
        }
        if SHIELDS.contains(&code) {
            self.shield.set((ctx.obs.turn, ctx.my_lp()));
        }
        Some(Response::new(90.0))
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        matches!(t.ctx.canonical(code), BATTLE_FADER | CYBER_VALLEY).then_some(Position::FACE_UP_DEFENSE)
    }

    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        let ctx = t.ctx;
        if member.at.controller != ctx.me || member.at.location != Location::Deck {
            return None;
        }
        let code = ctx.canonical(member.code?);
        match t.memory.last_activated.map(|c| ctx.canonical(c)) {
            // Shining Angel's replacement: the Cyber Valley that ends the Battle Phase.
            Some(SHINING_ANGEL) => Some(match code {
                CYBER_VALLEY => 9000.0,
                SHINING_ANGEL => 5000.0,
                _ => 0.0,
            }),
            // Pot of Duality: the clock, then what stops the most turns.
            Some(POT_OF_DUALITY) => Some(match code {
                FINAL_COUNTDOWN if self.started.get().is_none() && !ctx.in_hand(FINAL_COUNTDOWN) => 9000.0,
                FINAL_COUNTDOWN => 0.0,
                // A second copy of a lock that is up, Set or in hand can wait.
                MESSENGER_OF_PEACE | LEVEL_LIMIT_AREA_B | GRAVITY_BIND | SKILL_DRAIN
                    if Self::face_up(&ctx, code) || Self::set(&ctx, code) || ctx.in_hand(code) =>
                {
                    800.0
                }
                other => self.value(&ctx, other).unwrap_or(500) as f64,
            }),
            _ => None,
        }
    }
}
