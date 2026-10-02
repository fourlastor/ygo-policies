//! The shared decision loop.  A deck policy is a [`Strategy`] (hooks with
//! defaults) wrapped in an [`Agent`], which implements [`Policy`].

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use crate::cards::CardDatabase;
use crate::ctx::Ctx;
use crate::model::{
    CardRef, CardView, ChainLink, Choice, ChoiceKind, Decision, DecisionKind, Hint, Location,
    Member, Observation, Phase, Position,
};
use crate::{staples, tactics};

/// Anything that answers engine decisions for one seat.
pub trait Policy: Send {
    /// Return an index into `decision.choices`.
    fn choose(&mut self, obs: &Observation, decision: &Decision) -> usize;
}

/// Seeded tie-breaking.  Equally scored choices are taken at random rather
/// than in engine order, which follows zone order: an opponent could exploit
/// "the first face-down card is always the one destroyed", a guess cannot be.
/// The generator is part of the agent's state, so an agent rebuilt from the
/// same seed and message stream makes the same choices.
#[derive(Debug, Default)]
pub struct TieBreak(Cell<u64>);

impl TieBreak {
    pub fn new(seed: u64) -> Self {
        TieBreak(Cell::new(seed))
    }

    /// SplitMix64.
    fn next(&self) -> u64 {
        let state = self.0.get().wrapping_add(0x9E37_79B9_7F4A_7C15);
        self.0.set(state);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform index below `n` (`n > 0`).
    pub fn index(&self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// The highest-scored item; equal scores are broken at random.  A draw is
    /// taken only when there is a tie.
    pub fn best<T: Copy>(&self, scored: impl IntoIterator<Item = (f64, T)>) -> Option<(f64, T)> {
        let mut top: Vec<(f64, T)> = Vec::new();
        for item in scored {
            match top.first() {
                Some(first) if item.0 < first.0 => {}
                Some(first) if item.0 == first.0 => top.push(item),
                _ => top = vec![item],
            }
        }
        match top.len() {
            0 => None,
            1 => Some(top[0]),
            n => Some(top[self.index(n)]),
        }
    }
}

/// Per-agent memory that survives between prompts.
#[derive(Default, Debug)]
pub struct Memory {
    /// Cards the next target prompt should pick (set when we activate/attack),
    /// with the card each slot held then: once another card sits there, the
    /// slot is no longer the one we meant.
    pub intent: Vec<(CardRef, Option<u32>)>,
    /// Passcode of the last card effect we activated.
    pub last_activated: Option<u32>,
    picks: HashMap<(ChoiceKind, Option<u32>, Option<CardRef>), u32>,
    turn: u32,
    /// Board signature when we last responded in a chain, for engines that do
    /// not expose the chain: an unchanged board means it is still building.
    chain_mark: Option<u64>,
    pub ties: TieBreak,
}

/// A chain response: how much we want it, and what its targets should be.
#[derive(Clone, Debug, Default)]
pub struct Response {
    pub score: f64,
    pub intent: Vec<CardRef>,
}

impl Response {
    pub fn new(score: f64) -> Self {
        Response { score, intent: Vec::new() }
    }
    pub fn targeting(score: f64, intent: Vec<CardRef>) -> Self {
        Response { score, intent }
    }
    pub fn no() -> Self {
        Response::new(0.0)
    }
}

/// What a chain response would be answering.
#[derive(Clone, Copy, Debug)]
pub enum Hostile<'a> {
    /// The opponent's link on top of the chain.
    Link(&'a ChainLink),
    /// The engine does not expose the chain, and it is not our own response:
    /// an engine-gated card (Seven Tools, Sandman...) is being offered
    /// against something the opponent did.
    Unseen,
    /// Nothing of the opponent's to answer.
    No,
}

impl Hostile<'_> {
    /// Does the answered link satisfy `pred`?  Unseen links are given the
    /// benefit of the doubt: the engine only offers the card when it applies.
    pub fn matches(&self, pred: impl FnOnce(&ChainLink) -> bool) -> bool {
        match self {
            Hostile::Link(link) => pred(link),
            Hostile::Unseen => true,
            Hostile::No => false,
        }
    }
}

/// What changes when the opponent responds to our chain link, but not when
/// our own activation pays its cost: their Spell/Trap zone (a set card they
/// activate flips face-up).  Our tributes, discards, LP payments and our own
/// flipped backrow must not look like a new chain, or we would answer our own
/// link (Seven Tools of the Bandit negating our own Icarus Attack).
fn chain_signature(ctx: &Ctx) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ctx.obs.turn.hash(&mut hasher);
    format!("{:?}", ctx.obs.phase).hash(&mut hasher);
    for card in ctx.obs.pile(ctx.opp, Location::SpellTrapZone) {
        (card.at, card.code, card.position).hash(&mut hasher);
    }
    hasher.finish()
}

/// Result of a battle between one of our attackers and a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Target destroyed, attacker survives.  `trick` = needs a hand trick.
    Win { trick: bool },
    /// Both destroyed.
    Trade,
    /// Nothing destroyed (and maybe some damage to us).
    Bounce,
    /// Attacker destroyed.
    Lose,
}

/// One prompt being answered: context, decision and mutable memory.
pub struct Turn<'a> {
    pub ctx: Ctx<'a>,
    pub decision: &'a Decision,
    pub memory: &'a mut Memory,
}

impl<'a> Turn<'a> {
    pub fn choices(&self) -> impl Iterator<Item = (usize, &'a Choice)> + 'a {
        self.decision.choices.iter().enumerate()
    }

    fn pick_key(choice: &Choice) -> (ChoiceKind, Option<u32>, Option<CardRef>) {
        (choice.kind, choice.code(), choice.at())
    }

    /// Guard against re-choosing an action that keeps failing / looping.
    pub fn fresh(&self, index: usize) -> bool {
        let key = Self::pick_key(&self.decision.choices[index]);
        self.memory.picks.get(&key).copied().unwrap_or(0) < 3
    }

    pub fn find_where(&self, mut pred: impl FnMut(&Choice) -> bool) -> Option<usize> {
        self.choices().find(|(i, c)| pred(c) && self.fresh(*i)).map(|(i, _)| i)
    }

    pub fn find(&self, kind: ChoiceKind, code: Option<u32>, location: Option<Location>) -> Option<usize> {
        self.find_where(|c| {
            c.kind == kind
                && code.map_or(true, |code| c.code().map(|c| self.ctx.canonical(c)) == Some(code))
                && location.map_or(true, |loc| c.at().map(|a| a.location) == Some(loc))
        })
    }

    pub fn has(&self, kind: ChoiceKind) -> bool {
        self.decision.choices.iter().any(|c| c.kind == kind)
    }

    pub fn activate(&self, code: u32) -> Option<usize> {
        self.find(ChoiceKind::Activate, Some(code), None)
    }

    pub fn activate_from(&self, code: u32, location: Location) -> Option<usize> {
        self.find(ChoiceKind::Activate, Some(code), Some(location))
    }

    /// Commit to a choice.
    pub fn pick(&mut self, index: usize) -> Option<usize> {
        let choice = &self.decision.choices[index];
        *self.memory.picks.entry(Self::pick_key(choice)).or_insert(0) += 1;
        if choice.kind == ChoiceKind::Activate {
            self.memory.last_activated = choice.code();
            // Whatever the prompt, our activation is now the chain's top link.
            self.memory.chain_mark = Some(chain_signature(&self.ctx));
        }
        Some(index)
    }

    /// Commit to a choice whose follow-up target prompt should pick `intent`.
    pub fn pick_targeting(&mut self, index: usize, intent: Vec<CardRef>) -> Option<usize> {
        let ctx = self.ctx;
        self.memory.intent = intent.into_iter().map(|at| (at, ctx.card(at).and_then(|c| c.code))).collect();
        self.pick(index)
    }

    /// Is the top of the chain our own response?
    pub fn own_top(&self) -> bool {
        if self.ctx.obs.chain_known {
            self.ctx.obs.chain.last().map_or(false, |link| link.controller == self.ctx.me)
        } else {
            self.memory.chain_mark == Some(chain_signature(&self.ctx))
        }
    }

    /// The opponent's action a response in this chain window would answer.
    pub fn hostile_top(&self) -> Hostile<'a> {
        let ctx = self.ctx;
        if ctx.obs.chain_known {
            return ctx.obs.chain.last().filter(|l| l.controller == ctx.opp).map_or(Hostile::No, Hostile::Link);
        }
        if self.own_top() { Hostile::No } else { Hostile::Unseen }
    }

    pub fn choice(&self, index: usize) -> &'a Choice {
        &self.decision.choices[index]
    }

    pub fn view(&self, choice: &Choice) -> Option<&'a CardView> {
        choice.at().and_then(|at| self.ctx.card(at))
    }
}

/// Deck-specific knowledge.  Every hook has a sensible default, so a deck
/// only overrides what makes it different.
#[allow(unused_variables)]
pub trait Strategy: Send {
    /// Worth of keeping one of our own cards (costs, searches, trades).
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        None
    }
    /// Deck plan at the start of each Main Phase command (before staples).
    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        None
    }
    /// Deck plan after the generic summons (effects, extra deck, combos).
    fn main_phase_late(&mut self, t: &mut Turn) -> Option<usize> {
        None
    }
    /// Veto a shared staple (e.g. Pot of Duality while we still want to Special Summon).
    fn allow_staple(&self, t: &Turn, code: u32) -> bool {
        true
    }
    /// Score a Normal Summon / Set choice.  `Some(None)` vetoes it.
    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        None
    }
    /// Allow a Special Summon choice from the hand/graveyard/extra deck.
    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        None
    }
    /// Veto a position change whose triggered effect makes it undesirable.
    fn allow_reposition(&self, t: &Turn, card: &CardView) -> bool {
        true
    }
    fn wants_battle(&self, t: &Turn) -> Option<bool> {
        None
    }
    fn battle(&mut self, t: &mut Turn) -> Option<usize> {
        None
    }
    fn attack_outcome(&self, ctx: &Ctx, attacker: &CardView, target: &CardView) -> Option<Outcome> {
        None
    }
    /// ATK a trick in hand can add to this attacker in the Damage Step.
    fn attack_trick(&self, ctx: &Ctx, attacker: &CardView) -> i32 {
        0
    }
    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        None
    }
    fn select(&self, t: &Turn, member: &Member) -> Option<f64> {
        None
    }
    fn yes_no(&self, t: &Turn) -> Option<bool> {
        None
    }
    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        None
    }
    /// Set this Spell/Trap from the hand now?
    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        None
    }
    fn option(&self, t: &Turn) -> Option<usize> {
        None
    }
}

pub struct Agent<S> {
    pub strategy: S,
    db: Arc<dyn CardDatabase>,
    memory: Memory,
}

impl<S: Strategy> Agent<S> {
    pub fn new(strategy: S, db: Arc<dyn CardDatabase>) -> Self {
        Self::seeded(strategy, db, 0)
    }

    /// An agent whose ties are broken by a generator seeded with `seed`.
    pub fn seeded(strategy: S, db: Arc<dyn CardDatabase>, seed: u64) -> Self {
        Agent { strategy, db, memory: Memory { ties: TieBreak::new(seed), ..Memory::default() } }
    }
}

impl<S: Strategy> Policy for Agent<S> {
    fn choose(&mut self, obs: &Observation, decision: &Decision) -> usize {
        if decision.choices.is_empty() {
            return 0;
        }
        if obs.turn != self.memory.turn {
            self.memory.turn = obs.turn;
            self.memory.picks.clear();
        }
        let db = self.db.clone();
        let mut t = Turn { ctx: Ctx::new(obs, db.as_ref()), decision, memory: &mut self.memory };
        let s = &mut self.strategy;
        let answer = match decision.kind {
            DecisionKind::Idle => {
                t.memory.intent.clear();
                t.memory.chain_mark = None;
                idle(s, &mut t)
            }
            DecisionKind::Battle => {
                t.memory.intent.clear();
                t.memory.chain_mark = None;
                battle(s, &mut t)
            }
            DecisionKind::Chain { .. } => chain(s, &mut t),
            DecisionKind::YesNo => yes_no(s, &mut t),
            DecisionKind::Position => position(s, &t),
            DecisionKind::SelectCards | DecisionKind::SelectSum => select(s, &t),
            DecisionKind::SelectToggle => toggle(s, &t),
            DecisionKind::Option => s.option(&t),
            // No card depends on zones: any zone, at random.
            DecisionKind::Place => Some(t.memory.ties.index(decision.choices.len())),
            _ => None,
        };
        answer.filter(|i| *i < decision.choices.len()).unwrap_or(0)
    }
}

/// Worth of keeping one of our own cards.
pub fn value<S: Strategy + ?Sized>(s: &S, ctx: &Ctx, code: Option<u32>, view: Option<&CardView>) -> i32 {
    let code = view.and_then(|v| v.code).or(code);
    let Some(code) = code else { return 500 };
    let data = ctx.data(code);
    let code = ctx.canonical(code);
    let mut base = s
        .value(ctx, code)
        .or_else(|| staples::value(code))
        .unwrap_or_else(|| if data.is_monster() { data.attack.max(data.defense) } else { 1000 });
    if let Some(v) = view {
        if v.at.location == Location::MonsterZone && v.position.face_up {
            base = base.max(v.attack);
        }
    }
    base
}

// ---- Main Phase --------------------------------------------------------------

fn idle<S: Strategy>(s: &mut S, t: &mut Turn) -> Option<usize> {
    if tactics::lethal_on_board(t) {
        // Clear their backrow and add a free attacker, then swing.
        if let Some(i) = staples::clear_backrow(s, t) {
            return Some(i);
        }
        let attacker = tactics::normal_summon(s, t).filter(|i| {
            let choice = t.choice(*i);
            choice.kind == ChoiceKind::NormalSummon && choice.code().map_or(false, |c| t.ctx.data(c).tributes() == 0)
        });
        if let Some(i) = attacker {
            return t.pick(i);
        }
        if let Some(i) = t.find(ChoiceKind::EnterBattle, None, None) {
            return t.pick(i);
        }
    }
    if let Some(i) = s.main_phase(t) {
        return Some(i);
    }
    if let Some(i) = staples::main_phase(s, t) {
        return Some(i);
    }
    if let Some(i) = tactics::normal_summon(s, t) {
        return t.pick(i);
    }
    let special = t.find_where(|c| {
        c.kind == ChoiceKind::SpecialSummon && c.at().map(|a| a.location) != Some(Location::Extra)
    });
    if let Some(i) = special {
        let choice = t.choice(i);
        if s.special_summon(t, choice).unwrap_or(true) {
            return t.pick(i);
        }
    }
    if let Some(i) = s.main_phase_late(t) {
        return Some(i);
    }
    if let Some(i) = tactics::extra_deck_summon(s, t) {
        return t.pick(i);
    }
    if let Some(i) = tactics::reposition(s, t) {
        return t.pick(i);
    }
    let ctx = t.ctx;
    let can_attack = ctx.monsters(ctx.me).iter().any(|c| ctx.can_attack(c));
    if ctx.main1() && can_attack && s.wants_battle(t).unwrap_or(true) {
        if let Some(i) = t.find(ChoiceKind::EnterBattle, None, None) {
            return t.pick(i);
        }
    }
    if let Some(i) = tactics::set_spell_trap(s, t) {
        return t.pick(i);
    }
    // A monster that must attack keeps us from ending Main Phase 1 any
    // other way than through the Battle Phase.
    t.find(ChoiceKind::EndTurn, None, None)
        .or_else(|| t.find(ChoiceKind::EnterMain2, None, None))
        .or_else(|| t.find(ChoiceKind::EnterBattle, None, None))
        .and_then(|i| t.pick(i))
}

// ---- Battle Phase ------------------------------------------------------------

fn battle<S: Strategy>(s: &mut S, t: &mut Turn) -> Option<usize> {
    if let Some(i) = s.battle(t) {
        return Some(i);
    }
    if let Some((i, target)) = tactics::plan_attack(s, t) {
        return t.pick_targeting(i, target.into_iter().collect());
    }
    if let Some(i) = t.find(ChoiceKind::EnterMain2, None, None).or_else(|| t.find(ChoiceKind::EndTurn, None, None)) {
        return t.pick(i);
    }
    // No way out of the Battle Phase: a monster must attack.
    let (i, target) = tactics::forced_attack(s, t)?;
    t.pick_targeting(i, target.into_iter().collect())
}

// ---- Chain windows -------------------------------------------------------------

/// A response must beat this to be chained on top of our own chain link:
/// stacking a second answer to the same threat usually wastes one of them.
const CHAIN_ON_OWN_LINK: f64 = 100.0;

/// Our own Main Phase with nothing on the chain, and not a trigger window.
/// After a summon or a resolved chain the engine gives the turn player
/// priority here and offers its monsters' Ignition effects besides Quick
/// ones (OCGCore's obsolete ignition ruling).  Anything used here could also
/// wait for the next Main Phase command.
fn open_window(t: &Turn) -> bool {
    let ctx = t.ctx;
    matches!(t.decision.kind, DecisionKind::Chain { triggers: false, .. })
        && ctx.my_turn()
        && matches!(ctx.phase(), Some(Phase::Main1 | Phase::Main2))
        && ctx.obs.chain_known
        && ctx.obs.chain.is_empty()
}

fn chain<S: Strategy>(s: &mut S, t: &mut Turn) -> Option<usize> {
    let open = open_window(t);
    if open {
        // Using an effect now is using it in the Main Phase: the plan decides.
        let planned = s.main_phase(t).or_else(|| staples::main_phase(s, t)).or_else(|| s.main_phase_late(t));
        if let Some(i) = planned.filter(|i| t.choice(*i).kind == ChoiceKind::Activate) {
            return Some(i);
        }
    }
    let own_top = t.own_top();
    let mut best: Option<(f64, usize, Vec<CardRef>)> = None;
    for (i, choice) in t.choices() {
        if choice.kind != ChoiceKind::Activate || !t.fresh(i) {
            continue;
        }
        // The plan passed on our monsters' Ignition effects; resolving them
        // anyway, with no aim, spends them on our own cards.
        let ignition = open && choice.at().map_or(false, |at| at.controller == t.ctx.me && at.location == Location::MonsterZone);
        let response = s
            .chain(t, i)
            .or_else(|| staples::chain(s, t, i))
            .unwrap_or_else(|| if ignition { Response::no() } else { default_trigger(t, choice) });
        let threshold = if own_top { CHAIN_ON_OWN_LINK } else { 0.0 };
        if response.score > threshold && best.as_ref().map_or(true, |b| response.score > b.0) {
            best = Some((response.score, i, response.intent));
        }
    }
    if let Some((_, i, intent)) = best {
        return t.pick_targeting(i, intent);
    }
    t.find(ChoiceKind::Pass, None, None).or(Some(0))
}

/// Our own optional triggers / effects of face-up cards are worth resolving.
fn default_trigger(t: &Turn, choice: &Choice) -> Response {
    let Some(at) = choice.at() else { return Response::no() };
    let own = at.controller == t.ctx.me;
    let face_up = t.view(choice).map_or(true, |v| v.position.face_up);
    if own && at.location != Location::Hand && face_up {
        Response::new(10.0)
    } else {
        Response::no()
    }
}

// ---- small prompts ---------------------------------------------------------------

fn yes_no<S: Strategy>(s: &S, t: &mut Turn) -> Option<usize> {
    let yes = s.yes_no(t).unwrap_or(true);
    // Yes to "use this card's effect?" starts a new activation: the targets
    // of the previous one (Test Tiger's Beast) are not this one's, and the
    // selections that follow are this card's.
    if yes && t.decision.choices.iter().any(|c| c.kind == ChoiceKind::Yes && c.card.is_some()) {
        t.memory.intent.clear();
        if let Some(code) = t.decision.subject {
            t.memory.last_activated = Some(code);
        }
    }
    let wanted = if yes { ChoiceKind::Yes } else { ChoiceKind::No };
    t.choices().find(|(_, c)| c.kind == wanted).map(|(i, _)| i)
}

fn position<S: Strategy>(s: &S, t: &Turn) -> Option<usize> {
    let code = t.decision.subject.unwrap_or(0);
    let ctx = t.ctx;
    let preferred = s.position(t, code).unwrap_or_else(|| {
        let data = ctx.data(code);
        if data.attack >= ctx.opp_best_attack() || ctx.monsters(ctx.opp).is_empty() {
            Position::FACE_UP_ATTACK
        } else if data.defense > data.attack {
            Position::FACE_UP_DEFENSE
        } else {
            Position::FACE_DOWN_DEFENSE
        }
    });
    let order = [
        preferred,
        Position::FACE_UP_DEFENSE,
        Position::FACE_DOWN_DEFENSE,
        Position::FACE_UP_ATTACK,
    ];
    order.iter().find_map(|p| t.choices().find(|(_, c)| c.kind == ChoiceKind::Position(*p)).map(|(i, _)| i))
}

// ---- card selection ------------------------------------------------------------------

/// How good it is for us that `member` is part of the answer.
pub fn member_score<S: Strategy + ?Sized>(s: &S, t: &Turn, member: &Member) -> f64 {
    let intended = |(at, code): &(CardRef, Option<u32>)| {
        *at == member.at && (code.is_none() || member.code.is_none() || *code == member.code)
    };
    if let Some(rank) = t.memory.intent.iter().position(intended) {
        return 10_000.0 - rank as f64;
    }
    if let Some(score) = s.select(t, member) {
        return score;
    }
    let ctx = &t.ctx;
    let view = ctx.card(member.at);
    let mine = member.at.controller == ctx.me;
    let hint = t.decision.hint;
    let worth = if mine {
        value(s, ctx, member.code, view.filter(|v| v.known())) as f64
    } else {
        view.map_or(500, |v| ctx.threat(v)) as f64
    };
    if hint.is_gain() || (hint == Hint::Target && member.at.location == Location::Graveyard && t.memory.last_activated.map_or(false, |c| staples::is_revival(ctx.canonical(c)))) {
        return worth;
    }
    if hint.is_cost() {
        return if mine { -worth } else { worth };
    }
    // Hostile/targeting default: hit theirs, spare ours.
    if mine {
        -worth
    } else {
        worth
    }
}

fn select<S: Strategy>(s: &S, t: &Turn) -> Option<usize> {
    if t.choices().any(|(_, c)| c.kind == ChoiceKind::Toggle) {
        return select_step(s, t);
    }
    let best = t.memory.ties.best(t.choices().filter(|(_, c)| c.kind != ChoiceKind::Cancel).map(|(i, choice)| {
        let score: f64 = choice.members.iter().filter(|m| !m.required).map(|m| member_score(s, t, m)).sum();
        (score, i)
    }));
    let cancel = t.choices().find(|(_, c)| c.kind == ChoiceKind::Cancel).map(|(i, _)| i);
    match best {
        None => cancel,
        Some((score, _)) if t.decision.hint == Hint::AttackTarget && t.memory.intent.is_empty() && score < 0.0 && cancel.is_some() => cancel,
        Some((_, i)) => Some(i),
    }
}

/// One step of a sequential card/sum selection.  Scores are per card, so
/// taking the best card while it helps (or while the minimum is unmet) picks
/// the same set as maximising the summed score over whole subsets: the
/// smallest best set, without zero-score extras.
fn select_step<S: Strategy>(s: &S, t: &Turn) -> Option<usize> {
    let finish = t.choices().find(|(_, c)| c.kind == ChoiceKind::Finish).map(|(i, _)| i);
    let cancel = t.choices().find(|(_, c)| c.kind == ChoiceKind::Cancel).map(|(i, _)| i);
    let best = t.memory.ties.best(
        t.choices()
            .filter(|(_, c)| c.kind == ChoiceKind::Toggle)
            .filter_map(|(i, c)| c.card.map(|m| (member_score(s, t, &m), i))),
    );
    let first = t.decision.selected.is_empty();
    if let Some((score, _)) = best {
        if first && t.decision.hint == Hint::AttackTarget && t.memory.intent.is_empty() && score < 0.0 && cancel.is_some() {
            return cancel;
        }
    }
    let enough = t.decision.selected.len() as u32 >= t.decision.minimum;
    if enough && finish.is_some() && best.map_or(true, |b| b.0 <= 0.0) {
        return finish;
    }
    best.map(|b| b.1).or(finish).or(cancel)
}

fn toggle<S: Strategy>(s: &S, t: &Turn) -> Option<usize> {
    let finish = t.choices().find(|(_, c)| c.kind == ChoiceKind::Finish).map(|(i, _)| i);
    let scored: Vec<(f64, usize)> = t
        .choices()
        .filter(|(_, c)| c.kind == ChoiceKind::Toggle)
        .filter_map(|(i, c)| c.card.map(|m| (member_score(s, t, &m), i)))
        .collect();
    let enough = t.decision.selected.len() as u32 >= t.decision.minimum;
    let best = t.memory.ties.best(scored.iter().copied());
    if finish.is_some() && enough && (t.decision.hint.is_cost() || best.map_or(true, |b| b.0 < 0.0)) {
        return finish;
    }
    best.map(|b| b.1)
        .or(finish)
        .or_else(|| t.choices().find(|(_, c)| c.kind == ChoiceKind::Cancel).map(|(i, _)| i))
}
