//! From what the probes saw to the facts the policies use
//! (`ygo_policies::knowledge`), and the generated table that holds them.
//!
//! A fact is written down only when a probe showed it.  Where the probes
//! disagree in a way the table cannot hold, the card gets a note for review
//! and no fact: the policies then treat it as a plain monster, as before.

use std::collections::BTreeMap;
use std::fmt::Write;

use ygo_policies::agent::Outcome;
use ygo_policies::knowledge::{kind, Against, AtkChange, Attacked, Burn, Facts, Fate as Becomes, Striking, ALWAYS};

use crate::probe::{self, Act, Attacker, Cards, Examined, Fate, Seen};

/// The facts about one monster, and what a reviewer should look at.
pub struct Derived {
    pub facts: Facts,
    pub notes: Vec<String>,
    /// The staged monster had left before anything was done to it.
    pub unstaged: bool,
}

fn becomes(fate: Fate) -> Becomes {
    match fate {
        Fate::Stays => Becomes::Unharmed,
        Fate::Banished => Becomes::Banished,
        Fate::Hand | Fate::Deck => Becomes::Returned,
        Fate::Battle | Fate::Destroyed | Fate::Tributed | Fate::Taken | Fate::Other => Becomes::Destroyed,
    }
}

/// What the cards a player got are worth to it, roughly: a monster on the
/// field 1000, a token 300, a card in hand 700, a Spell/Trap on the field 500.
fn payoff(cards: &Cards) -> i32 {
    let monsters = (cards.monsters - cards.tokens) as i32;
    (monsters * 1000 + cards.tokens as i32 * 300 + cards.hand as i32 * 700 + cards.spell_traps as i32 * 500).min(2500)
}

/// Other cards lost: two monsters or more is read as "every monster".
fn collateral(cards: &Cards) -> u8 {
    if cards.monsters >= 2 {
        ALWAYS
    } else {
        cards.monsters + cards.spell_traps + cards.hand
    }
}

/// What one attack that reached the subject met.  `None`: not an attack on
/// the subject, or a negated one.  `second`: the same attack by a monster
/// with another ATK, to tell a fixed amount from one that follows the ATK.
fn read_attack(act: &Act, second: Option<&Act>) -> Option<Attacked> {
    if !act.at_subject || act.negated {
        return None;
    }
    let mut met = Attacked::PLAIN;
    let gone = act.subject != Fate::Stays;
    let Some(calc) = act.calc else {
        // No battle took place.
        if act.counterpart != Fate::Stays {
            met.attacker = becomes(act.counterpart);
            met.before_damage = true;
            met.leaves = gone;
        }
        if gone {
            met.payoff = payoff(&act.gained[1]);
        }
        met.collateral = collateral(&act.lost[0]);
        return Some(met);
    };
    let attacker = act.attacker_visible.unwrap_or(calc.attacker_atk);
    let (attack, defense) = act.target_visible.unwrap_or((calc.target_atk, calc.target_def));
    met.stat = calc.stat() - if calc.target_defending { defense } else { attack };
    // The attacker's ATK at damage calculation, against what the board showed.
    let rules = |visible: i32, at_calc: i32| -> Vec<AtkChange> {
        let mut rules = Vec::new();
        if at_calc == visible {
            rules.push(AtkChange::None);
        }
        if at_calc == 0 {
            rules.push(AtkChange::Zero);
        }
        if at_calc == (visible + 1) / 2 {
            rules.push(AtkChange::Halved);
        }
        if at_calc < visible {
            rules.push(AtkChange::Minus(visible - at_calc));
        }
        rules
    };
    let other = second.and_then(|a| Some((a.attacker_visible?, a.calc?.attacker_atk)));
    met.attacker_atk = rules(attacker, calc.attacker_atk)
        .into_iter()
        .find(|rule| other.map_or(true, |(visible, at_calc)| rules(visible, at_calc).contains(rule)))
        .unwrap_or(AtkChange::None);
    let beaten = calc.attacker_atk > calc.stat() || (calc.attacker_atk == calc.stat() && !calc.target_defending && calc.stat() > 0);
    let attacker_beaten = !calc.target_defending && calc.attacker_atk <= calc.stat();
    if beaten && !gone {
        met.survives = 1;
    }
    if !attacker_beaten && act.counterpart != Fate::Stays {
        met.attacker = becomes(act.counterpart);
        // Destroyed by the battle is not "leaves along with it".
        met.leaves = !matches!(act.subject, Fate::Stays | Fate::Battle);
    }
    // Battle damage as the numbers say, against the damage taken.
    let owed = |a: &Act| {
        let c = a.calc?;
        let theirs = if c.target_defending { 0 } else { (c.attacker_atk - c.stat()).max(0) };
        let ours = (c.stat() - c.attacker_atk).max(0);
        Some((theirs, ours, a.attacker_visible.unwrap_or(c.attacker_atk)))
    };
    let (theirs, _, _) = owed(act)?;
    met.no_damage = theirs > 0 && act.damage[1] == 0;
    let burns = |a: &Act| -> Vec<Burn> {
        let Some((theirs, ours, visible)) = owed(a) else { return Vec::new() };
        let excess = a.damage[0] - ours;
        let mut burns = Vec::new();
        if excess <= 0 {
            burns.push(Burn::None);
            return burns;
        }
        if theirs > 0 && a.damage[1] == 0 && excess == theirs {
            burns.push(Burn::Reflected);
        }
        if excess == visible {
            burns.push(Burn::AttackerAtk);
        }
        burns.push(Burn::Fixed(excess));
        burns
    };
    let again = second.map(burns);
    met.burn = burns(act)
        .into_iter()
        .find(|burn| again.as_ref().map_or(true, |b| b.is_empty() || b.contains(burn)))
        .unwrap_or(Burn::None);
    if gone {
        met.payoff = payoff(&act.gained[1]);
    }
    met.collateral = collateral(&act.lost[0]);
    Some(met)
}

fn first(seen: &Seen) -> Option<&Act> {
    seen.acts.first()
}

fn end_position(start: usize, act: &Act) -> usize {
    match act.switched {
        Some(position) if position & probe::pos::DEFENSE != 0 => 1,
        Some(_) => 0,
        None => start,
    }
}

/// The attackers that met `met` among the ones tried, as a condition; or
/// why none could be written.
fn against(tried: &[(Attacker, Option<Attacked>)], met: &Attacked) -> Result<Against, String> {
    let base = Attacker::BASE;
    let same = |a: &Attacker| tried.iter().find(|(t, _)| t == a).and_then(|(_, r)| r.as_ref()).map(|r| r == met);
    let differing = tried.iter().filter(|(_, r)| r.as_ref().is_some_and(|r| r != met)).count();
    if differing == 0 {
        return Ok(Against::All);
    }
    // The table has no room for "attackers of this Type" (Ryu Kokki): the
    // plain attacker's Type must not be what decides.
    let plain = same(&base);
    if probe::race::OTHERS.iter().any(|r| same(&Attacker { race: *r, ..base }).is_some_and(|s| Some(s) != plain)) {
        return Err("depends on the attacker's Type".into());
    }
    let by_attribute: Vec<(u32, bool)> = probe::attribute::ALL.iter().filter_map(|a| Some((*a, same(&Attacker { attribute: *a, ..base })?))).collect();
    let by_level: Vec<(u32, bool)> = (1..=12).filter_map(|l| Some((l, same(&Attacker { level: l, ..base })?))).collect();
    let mut by_attack: Vec<(i32, bool)> =
        tried.iter().filter(|(a, _)| a.level == base.level && a.attribute == base.attribute && a.race == base.race).filter_map(|(a, r)| Some((a.attack, r.as_ref()? == met))).collect();
    by_attack.sort_unstable();
    let varies = |axis: &[bool]| axis.iter().any(|x| *x) && axis.iter().any(|x| !*x);
    let axes = [
        varies(&by_attribute.iter().map(|x| x.1).collect::<Vec<_>>()),
        varies(&by_level.iter().map(|x| x.1).collect::<Vec<_>>()),
        varies(&by_attack.iter().map(|x| x.1).collect::<Vec<_>>()),
    ];
    match axes {
        [true, false, false] => {
            let mask = by_attribute.iter().filter(|(_, same)| *same).fold(0u8, |m, (a, _)| m | *a as u8);
            Ok(Against::Attributes(mask))
        }
        [false, true, false] | [false, false, true] => {
            // The attackers it holds against must be all those from some
            // Level (or ATK) up, or all those below one.
            let axis: Vec<(i32, bool)> = if axes[1] { by_level.iter().map(|(l, s)| (*l as i32, *s)).collect() } else { by_attack };
            let flips = axis.windows(2).filter(|w| w[0].1 != w[1].1).count();
            if flips != 1 {
                return Err("holds against some attackers and not others, in no order".into());
            }
            let edge = axis.windows(2).find(|w| w[0].1 != w[1].1).unwrap()[1].0;
            // An edge at the strongest attacker tried is an amount that
            // follows the attacker's ATK, not a threshold (Mirage Knight).
            if !axes[1] && edge >= base.attack {
                return Err("follows the attacker's ATK in a way the table cannot hold".into());
            }
            Ok(match (axes[1], axis[axis.len() - 1].1) {
                (true, true) => Against::LevelFrom(edge as u8),
                (true, false) => Against::LevelBelow(edge as u8),
                (false, true) => Against::AttackFrom(edge),
                (false, false) => Against::AttackBelow(edge),
            })
        }
        _ => Err("depends on the attacker in more than one way".into()),
    }
}

/// Attacked in battle position `start` (0 Attack, 1 Defense): the position
/// it battles in, whether that is another one, and what the attack meets.
fn attacked(e: &Examined, start: usize, notes: &mut Vec<String>) -> (usize, bool, Attacked) {
    let position = ["Attack Position", "Defense Position"][start];
    let seen = &e.attacked[start];
    let plain = (start, false, Attacked::PLAIN);
    if seen.before.is_none() || seen.acts.is_empty() {
        return plain;
    }
    let acts = &seen.acts;
    let mut met = Attacked::PLAIN;
    // Three attacks in a row: the negated ones come first.
    let negated = acts.iter().take_while(|a| a.at_subject && a.negated && a.subject == Fate::Stays).count();
    if negated > 0 {
        met.negates = if negated == acts.len() && negated >= 3 { ALWAYS } else { negated as u8 };
        met.negate_cost = acts[0].paid[1];
        if acts[0].spent[1].any() {
            notes.push(format!("{position}: negating the attack costs its controller cards, not Life Points"));
        }
    }
    // One plain attacker alone is the cleanest reading; when its attack is
    // negated, the first of the three that goes through.
    let alone = e.variants[start].iter().find(|(a, _)| *a == Attacker::BASE).and_then(|(_, seen)| first(seen));
    let weaker = e.variants[start].iter().filter(|(a, _)| a.level == 4 && a.attribute == Attacker::BASE.attribute && a.race == Attacker::BASE.race && a.attack < Attacker::BASE.attack).map(|(a, s)| (a.attack, s)).min_by_key(|x| x.0).and_then(|(_, s)| first(s));
    let through = acts.get(negated);
    let reading = match alone {
        Some(act) if !act.negated => Some(act),
        _ => through,
    };
    let Some(act) = reading else { return (start, false, met) };
    let Some(read) = read_attack(act, weaker) else { return (start, false, met) };
    let mut bystanders = 0;
    // The same attack with two more attackers standing by should read the same.
    if let Some(among) = through.filter(|_| negated == 0 && alone.is_some_and(|a| !a.negated)) {
        let crowd = read_attack(among, None);
        if crowd.is_some_and(|c| (c.survives, c.attacker, c.before_damage) != (read.survives, read.attacker, read.before_damage)) {
            notes.push(format!("{position}: differs with how many monsters the attacking player controls"));
            return (start, false, met);
        }
        // The attacker alone has nothing else to lose.
        bystanders = crowd.map_or(0, |c| c.collateral);
    }
    let end = end_position(start, act);
    let (negates, negate_cost) = (met.negates, met.negate_cost);
    met = read;
    met.collateral = met.collateral.max(bystanders);
    // Does it take its attacker along whenever it battles, or only when the
    // battle destroys it?  An attacker too weak to destroy it tells.
    let spared = e.under.as_ref().and_then(first).is_some_and(|a| a.at_subject && a.calc.is_some() && a.counterpart == Fate::Stays && a.subject == Fate::Stays);
    met.when_destroyed = met.attacker != Becomes::Unharmed && !met.before_damage && act.subject == Fate::Battle && spared;
    met.negates = negates;
    met.negate_cost = negate_cost;
    // How many battles in a row it survives.
    if met.survives > 0 {
        let run = acts[negated..].iter().take_while(|a| read_attack(a, None).is_some_and(|r| r.survives > 0)).count();
        met.survives = if negated + run >= acts.len() && run >= 2 { ALWAYS } else { run.max(1) as u8 };
    }
    // Which attackers it holds against.
    let tried: Vec<(Attacker, Option<Attacked>)> = e.variants[start]
        .iter()
        .map(|(attacker, seen)| {
            let read = first(seen).and_then(|a| read_attack(a, None)).map(|mut r| {
                // One attack shows whether it survives, not how often.
                r.survives = r.survives.min(1) * met.survives;
                r.negates = met.negates;
                r.negate_cost = met.negate_cost;
                r.when_destroyed = met.when_destroyed && r.attacker != Becomes::Unharmed;
                // Alone, an attacker has no other cards to lose.
                if r.attacker == met.attacker && (r.payoff > 0) == (met.payoff > 0) {
                    r.collateral = met.collateral;
                }
                // Amounts that follow the attacker's ATK were settled above.
                if std::mem::discriminant(&r.burn) == std::mem::discriminant(&met.burn) || matches!((r.burn, met.burn), (Burn::Fixed(_), Burn::AttackerAtk | Burn::Reflected)) {
                    r.burn = met.burn;
                }
                if matches!((r.attacker_atk, met.attacker_atk), (AtkChange::Minus(_) | AtkChange::Zero | AtkChange::Halved, AtkChange::Zero | AtkChange::Halved)) {
                    r.attacker_atk = met.attacker_atk;
                }
                r
            });
            (*attacker, read)
        })
        .collect();
    let special = Attacked { negates: 0, negate_cost: 0, ..met } != Attacked::PLAIN;
    if special {
        match against(&tried, &met) {
            Ok(condition) => met.against = condition,
            Err(why) => {
                notes.push(format!("{position}: {why}; left as a plain monster"));
                return (start, false, Attacked { negates, negate_cost, ..Attacked::PLAIN });
            }
        }
    } else {
        // Plain against the plain attacker: some other attacker may still
        // meet something (an Ally of Justice against LIGHT).
        let others: Vec<Attacked> = tried.iter().filter_map(|(_, r)| *r).filter(|r| *r != met).collect();
        if let Some(other) = others.first() {
            if others.iter().all(|o| o == other) {
                match against(&tried, other) {
                    Ok(condition) => {
                        met = Attacked { against: condition, negates, negate_cost, ..*other };
                    }
                    Err(why) => notes.push(format!("{position}: {why}; left as a plain monster")),
                }
            } else {
                notes.push(format!("{position}: other attackers meet different things; left as a plain monster"));
            }
        }
    }
    let switches = end != start;
    (end, switches, met)
}

fn striking(e: &Examined, notes: &mut Vec<String>) -> Striking {
    // Piercing: battle damage through a monster in Defense Position.
    let finish = |mut striking: Striking| {
        if let Some((act, calc)) = first(&e.strike[1]).and_then(|a| a.calc.map(|c| (a, c))) {
            striking.piercing = calc.target_defending && act.damage[1] > 0 && act.damage[1] == calc.attacker_atk - calc.target_def;
        }
        striking
    };
    let mut striking = Striking::PLAIN;
    let seen = &e.strike[0];
    if seen.before.is_none() {
        return striking;
    }
    striking.direct = seen.noted("direct-attack-offered");
    let Some(act) = first(seen) else { return striking };
    let gone = act.subject != Fate::Stays;
    // ATK gained at damage calculation: the same against a monster in
    // Attack and in Defense Position, or the smaller of the two.
    let gained = |seen: &Seen| first(seen).and_then(|a| Some((a.calc?.attacker_atk - a.attacker_visible?).max(0))).unwrap_or(0);
    striking.bonus = gained(seen).min(gained(&e.strike[1]));
    match act.calc {
        Some(_) => {
            if !matches!(act.counterpart, Fate::Battle | Fate::Stays) {
                striking.target = becomes(act.counterpart);
                striking.leaves = gone;
            }
        }
        None if act.counterpart != Fate::Stays => {
            striking.target = becomes(act.counterpart);
            striking.before_damage = true;
            striking.leaves = gone;
        }
        None => {}
    }
    if striking.target != Becomes::Unharmed {
        let same = |seen: &Seen| first(seen).is_some_and(|a| a.counterpart == act.counterpart && a.calc.is_some() == act.calc.is_some());
        // A monster with 4000 ATK and DEF must fare the same: the table
        // has no room for "only monsters weaker than..." (Blackwing - Jin).
        if !e.strike_strong.as_ref().is_some_and(same) {
            notes.push("its attack removes some monsters only, by their stats".into());
            striking.target = Becomes::Unharmed;
            striking.before_damage = false;
            striking.leaves = false;
            return finish(striking);
        }
        // Which targets, by Attribute.
        let mask = e.strike_attributes.iter().filter(|(_, seen)| same(seen)).fold(probe::attribute::EARTH as u8, |m, (a, _)| m | *a as u8);
        if mask != 0x3f {
            striking.against = Against::Attributes(mask);
        }
    }
    if seen.acts.is_empty() && !seen.noted("cannot-attack") {
        notes.push("its attack could not be staged".into());
    }
    finish(striking)
}

fn effects(e: &Examined, facts: &mut Facts, notes: &mut Vec<String>) {
    let seen = |card: u32| e.effects.iter().find(|(c, _)| *c == card).map(|(_, s)| s);
    let blocked = |s: &Seen| s.noted("cannot-activate");
    // Negated, with the monster still there afterwards: a monster that
    // leaves to negate (Stardust Dragon, Tytannial) is removed all the same.
    let countered = |s: &Seen| s.acts.iter().take_while(|a| a.countered && a.subject == Fate::Stays).count();
    // Negated every time, at no cost: nothing spent or paid, the monster as it was.
    let free = |s: &Seen| {
        s.acts.len() >= 2
            && countered(s) == s.acts.len()
            && s.acts.iter().all(|a| !a.spent[1].any() && a.paid[1] == 0)
            && s.before.zip(s.after).is_some_and(|(b, a)| (b.attack, b.defense) == (a.attack, a.defense))
    };
    let times = std::cell::Cell::new(0u8);
    let discards = std::cell::Cell::new(false);
    let negated = |s: &Seen| {
        let n = countered(s);
        if n > 0 {
            times.set(times.get().max(if n >= 2 { ALWAYS } else { 1 }));
            discards.set(discards.get() | (s.acts[0].spent[1].hand > 0));
        }
        n > 0
    };
    let tools = [
        (kind::SPELL, probe::SOUL_TAKER, probe::BOOK_OF_MOON, [probe::DIAN_KETO, probe::OOKAZI]),
        (kind::TRAP, probe::RAIGEKI_BREAK, probe::COMPULSORY_EVACUATION_DEVICE, [probe::JAR_OF_GREED, probe::WABOKU]),
        (kind::MONSTER, probe::EXILED_FORCE, probe::BRIONAC, [probe::CANNON_SOLDIER; 2]),
    ];
    for (kind, destroyer, other, unrelated) in tools {
        let (Some(destroyer), Some(other), Some(first_unrelated), Some(second_unrelated)) = (seen(destroyer), seen(other), seen(unrelated[0]), seen(unrelated[1])) else { continue };
        // Cards that have nothing to do with the monster: when two that do
        // different things both fail, it is their kind that is stopped.  A
        // monster effect is one card only here, with a cost: no conclusion.
        let stopped = |s: &Seen| blocked(s) || free(s);
        let playable = !blocked(first_unrelated) || !blocked(second_unrelated);
        if kind != kind::MONSTER && stopped(first_unrelated) && stopped(second_unrelated) {
            facts.negates_any |= kind;
            times.set(ALWAYS);
        } else if stopped(first_unrelated) || first_unrelated.acts.iter().chain(&second_unrelated.acts).any(|a| a.countered) {
            notes.push(format!("stops some {} effects that do not involve it (at a cost, once a turn, or only some of them)", kind_name(kind)));
        }
        // A card that targets and destroys it, and one that targets it and
        // does something else.  "Cannot be targeted" when neither finds it
        // (the first may fail for its cost alone).
        let unreachable = |s: &Seen| (blocked(s) && playable) || s.noted("target-not-offered");
        if unreachable(destroyer) && unreachable(other) {
            facts.untargetable |= kind;
            continue;
        }
        if unreachable(destroyer) {
            notes.push(format!("a {} that targets and destroys could not be used on it", kind_name(kind)));
        }
        // Negated: for targeting it when both are, for destroying it when
        // only the destroyer is.
        match (negated(destroyer), negated(other)) {
            (_, true) => facts.negates_aimed |= kind,
            (true, false) => facts.negates_destruction |= kind,
            (false, false) => {}
        }
        if let Some(act) = first(destroyer).filter(|a| !a.countered) {
            if act.targeted && act.subject == Fate::Stays {
                facts.immune |= kind;
            }
            if act.subject != Fate::Stays {
                facts.effect_payoff = facts.effect_payoff.max(payoff(&act.gained[1]));
                facts.effect_collateral = facts.effect_collateral.max(collateral(&act.lost[0]));
            }
        }
        if let Some(act) = first(other).filter(|a| !a.countered) {
            let flipped = act.switched.is_some_and(|p| p & probe::pos::FACEDOWN != 0);
            if act.targeted && act.subject == Fate::Stays && !flipped {
                facts.unaffected |= kind;
            }
        }
    }
    // A Spell that destroys without targeting.
    if let Some(sweep) = seen(probe::SMASHING_GROUND) {
        if negated(sweep) {
            facts.negates_destruction |= kind::SPELL;
        } else if let Some(act) = first(sweep).filter(|a| !a.countered) {
            if act.subject == Fate::Stays {
                facts.immune |= kind::SPELL;
            } else {
                facts.effect_payoff = facts.effect_payoff.max(payoff(&act.gained[1]));
                facts.effect_collateral = facts.effect_collateral.max(collateral(&act.lost[0]));
            }
        }
    }
    facts.negate_times = times.get();
    facts.negate_discards = discards.get();
}

fn kind_name(mask: u8) -> &'static str {
    match mask {
        kind::SPELL => "Spell",
        kind::TRAP => "Trap",
        _ => "monster",
    }
}

pub fn derive(e: &Examined) -> Derived {
    let mut notes = Vec::new();
    let mut facts = Facts::NONE;
    if e.attacked[0].before.is_none() {
        return Derived { facts, notes, unstaged: true };
    }
    // Facts are kept for the position the monster battles in: a run that
    // started there and stayed says it best, else the run that ended there.
    let runs = [(0usize, attacked(e, 0, &mut notes)), (1, attacked(e, 1, &mut notes))];
    let mut filled = [false; 2];
    for (start, (_, switches, met)) in runs {
        if !switches {
            facts.attacked[start] = met;
            filled[start] = true;
        }
    }
    for (start, (end, switches, met)) in runs {
        if switches {
            facts.attacked[start].switches = true;
            if !filled[end] {
                let switches = facts.attacked[end].switches;
                facts.attacked[end] = Attacked { switches, ..met };
                filled[end] = true;
            }
        }
    }
    facts.striking = striking(e, &mut notes);
    effects(e, &mut facts, &mut notes);
    Derived { facts, notes, unstaged: false }
}

// ---- how well the shared tactics call a battle ------------------------------

/// What an attack turned out to be, in the tactics' terms.  `None`: negated,
/// or not an attack on the monster.
pub fn actual(act: &Act) -> Option<Outcome> {
    if !act.at_subject || act.negated {
        return None;
    }
    Some(match (act.subject != Fate::Stays, act.counterpart != Fate::Stays) {
        (true, false) => Outcome::Win { trick: false },
        (true, true) => Outcome::Trade,
        (false, false) => Outcome::Bounce,
        (false, true) => Outcome::Lose,
    })
}

/// (attacks, attacks the tactics called right) over every attack on a
/// monster the probes staged, and the same over the attacks that did not
/// end as printed stats alone say.
pub fn agreement(examined: &[Examined]) -> ([usize; 2], [usize; 2], Vec<(u32, String)>) {
    let (mut all, mut special) = ([0, 0], [0, 0]);
    let mut misses = Vec::new();
    for e in examined {
        for (p, seen) in e.attacked.iter().enumerate() {
            let weak = e.under.as_ref().filter(|_| p == 1).map(|s| (Attacker { attack: 0, ..Attacker::BASE }, s));
            let runs = std::iter::once((Attacker::BASE, seen)).chain(e.variants[p].iter().map(|(a, s)| (*a, s))).chain(weak);
            for (attacker, seen) in runs {
                for (act, predicted) in seen.acts.iter().zip(&seen.predicted) {
                    let Some(outcome) = actual(act) else { continue };
                    let right = *predicted == outcome;
                    all[0] += 1;
                    all[1] += right as usize;
                    let by_stats = act.calc.is_some_and(|c| {
                        let plain = if c.attacker_atk > c.stat() { Outcome::Win { trick: false } } else if c.target_defending { Outcome::Bounce } else if c.attacker_atk == c.stat() { Outcome::Trade } else { Outcome::Lose };
                        plain == outcome && act.attacker_visible == Some(c.attacker_atk)
                    });
                    if !by_stats {
                        special[0] += 1;
                        special[1] += right as usize;
                    }
                    if !right {
                        misses.push((e.code, format!("{} by {attacker:?}: expected {predicted:?}, was {outcome:?}", ["Attack Position", "Defense Position"][p])));
                    }
                }
            }
        }
    }
    // The monster's own attacks.
    for e in examined {
        let runs = e.strike.iter().chain(e.strike_attributes.iter().map(|(_, s)| s)).chain(&e.strike_strong);
        for seen in runs {
            for (act, predicted) in seen.acts.iter().zip(&seen.predicted) {
                let outcome = match (act.counterpart != Fate::Stays, act.subject != Fate::Stays) {
                    (true, false) => Outcome::Win { trick: false },
                    (true, true) => Outcome::Trade,
                    (false, false) => Outcome::Bounce,
                    (false, true) => Outcome::Lose,
                };
                let right = *predicted == outcome;
                all[0] += 1;
                all[1] += right as usize;
                let by_stats = act.calc.is_some_and(|c| act.attacker_visible == Some(c.attacker_atk))
                    && matches!((act.counterpart, act.subject), (Fate::Battle, Fate::Stays) | (Fate::Stays, Fate::Battle) | (Fate::Battle, Fate::Battle) | (Fate::Stays, Fate::Stays));
                if !by_stats {
                    special[0] += 1;
                    special[1] += right as usize;
                }
                if !right {
                    misses.push((e.code, format!("its own attack: expected {predicted:?}, was {outcome:?}")));
                }
            }
        }
    }
    (all, special, misses)
}

// ---- the generated table -------------------------------------------------------

fn times(n: u8) -> String {
    if n == ALWAYS { "ALWAYS".into() } else { n.to_string() }
}

fn kinds(mask: u8) -> String {
    if mask == kind::ALL {
        return "kind::ALL".into();
    }
    let names: Vec<&str> =
        [(kind::SPELL, "kind::SPELL"), (kind::TRAP, "kind::TRAP"), (kind::MONSTER, "kind::MONSTER")].iter().filter(|(k, _)| mask & k != 0).map(|(_, n)| *n).collect();
    names.join(" | ")
}

fn emit_attacked(a: &Attacked) -> String {
    let p = Attacked::PLAIN;
    if *a == p {
        return "P".into();
    }
    let mut f = Vec::new();
    if a.survives != p.survives {
        f.push(format!("survives: {}", times(a.survives)));
    }
    if a.negates != p.negates {
        f.push(format!("negates: {}", times(a.negates)));
    }
    if a.negate_cost != p.negate_cost {
        f.push(format!("negate_cost: {}", a.negate_cost));
    }
    if a.attacker != p.attacker {
        f.push(format!("attacker: Fate::{:?}", a.attacker));
    }
    if a.before_damage {
        f.push("before_damage: true".into());
    }
    if a.leaves {
        f.push("leaves: true".into());
    }
    if a.when_destroyed {
        f.push("when_destroyed: true".into());
    }
    if a.stat != 0 {
        f.push(format!("stat: {}", a.stat));
    }
    if a.attacker_atk != p.attacker_atk {
        f.push(format!("attacker_atk: AtkChange::{:?}", a.attacker_atk));
    }
    if a.no_damage {
        f.push("no_damage: true".into());
    }
    if a.burn != p.burn {
        f.push(format!("burn: Burn::{:?}", a.burn));
    }
    if a.payoff != 0 {
        f.push(format!("payoff: {}", a.payoff));
    }
    if a.collateral != 0 {
        f.push(format!("collateral: {}", times(a.collateral)));
    }
    if a.switches {
        f.push("switches: true".into());
    }
    if a.against != p.against {
        f.push(format!("against: Against::{:?}", a.against));
    }
    format!("Attacked {{ {}, ..P }}", f.join(", "))
}

fn emit_facts(facts: &Facts) -> String {
    let n = Facts::NONE;
    let mut f = Vec::new();
    if facts.attacked != n.attacked {
        if facts.attacked[0] == facts.attacked[1] {
            f.push(format!("attacked: [{}; 2]", emit_attacked(&facts.attacked[0])));
        } else {
            f.push(format!("attacked: [{}, {}]", emit_attacked(&facts.attacked[0]), emit_attacked(&facts.attacked[1])));
        }
    }
    let s = facts.striking;
    if s != Striking::PLAIN {
        let mut g = Vec::new();
        if s.bonus != 0 {
            g.push(format!("bonus: {}", s.bonus));
        }
        if s.piercing {
            g.push("piercing: true".into());
        }
        if s.direct {
            g.push("direct: true".into());
        }
        if s.target != Becomes::Unharmed {
            g.push(format!("target: Fate::{:?}", s.target));
        }
        if s.before_damage {
            g.push("before_damage: true".into());
        }
        if s.leaves {
            g.push("leaves: true".into());
        }
        if s.against != Against::All {
            g.push(format!("against: Against::{:?}", s.against));
        }
        f.push(format!("striking: Striking {{ {}, ..S }}", g.join(", ")));
    }
    for (name, mask) in [
        ("untargetable", facts.untargetable),
        ("negates_any", facts.negates_any),
        ("negates_aimed", facts.negates_aimed),
        ("negates_destruction", facts.negates_destruction),
        ("immune", facts.immune),
        ("unaffected", facts.unaffected),
    ] {
        if mask != 0 {
            f.push(format!("{name}: {}", kinds(mask)));
        }
    }
    if facts.negate_times != 0 {
        f.push(format!("negate_times: {}", times(facts.negate_times)));
    }
    if facts.negate_discards {
        f.push("negate_discards: true".into());
    }
    if facts.effect_payoff != 0 {
        f.push(format!("effect_payoff: {}", facts.effect_payoff));
    }
    if facts.effect_collateral != 0 {
        f.push(format!("effect_collateral: {}", times(facts.effect_collateral)));
    }
    format!("Facts {{ {}, ..N }}", f.join(", "))
}

/// The Rust source of the pool table.
pub fn table(derived: &BTreeMap<u32, Derived>, names: &dyn Fn(u32) -> String, pool: &str) -> String {
    let mut out = String::new();
    let kept: Vec<(&u32, &Derived)> = derived.iter().filter(|(_, d)| d.facts != Facts::NONE).collect();
    writeln!(out, "//! Generated by `policy-bench knowledge`; do not edit.").unwrap();
    writeln!(out, "//!").unwrap();
    writeln!(out, "//! What the engine did when each monster of {pool} was staged").unwrap();
    writeln!(out, "//! on a plain board and attacked, aimed at with a Spell, a Trap and a monster").unwrap();
    writeln!(out, "//! effect, and made to attack: {} monsters probed, {} with something to", derived.len(), kept.len()).unwrap();
    writeln!(out, "//! know.  Regenerate it after changing the card scripts, the pool or the").unwrap();
    writeln!(out, "//! probes; corrections go in `knowledge::facts`, not here.").unwrap();
    writeln!(out, "#[allow(unused_imports)]").unwrap();
    writeln!(out, "use super::{{kind, Against, AtkChange, Attacked, Burn, Facts, Fate, Striking, ALWAYS}};").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "const P: Attacked = Attacked::PLAIN;").unwrap();
    writeln!(out, "const S: Striking = Striking::PLAIN;").unwrap();
    writeln!(out, "const N: Facts = Facts::NONE;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "/// Sorted by code.").unwrap();
    writeln!(out, "#[rustfmt::skip]").unwrap();
    writeln!(out, "pub(super) static POOL: &[(u32, Facts)] = &[").unwrap();
    for (code, d) in kept {
        writeln!(out, "    // {}", names(*code)).unwrap();
        writeln!(out, "    ({code}, {}),", emit_facts(&d.facts)).unwrap();
    }
    writeln!(out, "];").unwrap();
    out
}

// ---- the review listing -------------------------------------------------------

fn describe_attacked(a: &Attacked) -> Vec<String> {
    let mut out = Vec::new();
    match a.survives {
        0 => {}
        ALWAYS => out.push("battle does not destroy it".into()),
        n => out.push(format!("survives {n} battle{} a turn", if n == 1 { "" } else { "s" })),
    }
    match a.negates {
        0 => {}
        ALWAYS => out.push(format!("its controller can negate every attack{}", if a.negate_cost > 0 { format!(" for {} LP", a.negate_cost) } else { String::new() })),
        n => out.push(format!("its controller can negate {n} attack{} a turn", if n == 1 { "" } else { "s" })),
    }
    if a.attacker != Becomes::Unharmed {
        let how = match a.attacker {
            Becomes::Destroyed => "destroyed",
            Becomes::Banished => "banished",
            _ => "returned to the hand or Deck",
        };
        out.push(format!(
            "the attacker is {how}{}{}{}",
            if a.before_damage { " before damage calculation" } else { "" },
            if a.when_destroyed { " when the battle destroys it" } else { "" },
            if a.leaves && !a.when_destroyed { ", and it leaves the field too" } else { "" }
        ));
    }
    if a.stat != 0 {
        out.push(format!("its battle stat changes by {:+} at damage calculation", a.stat));
    }
    match a.attacker_atk {
        AtkChange::None => {}
        AtkChange::Zero => out.push("the attacker's ATK becomes 0 at damage calculation".into()),
        AtkChange::Halved => out.push("the attacker's ATK is halved at damage calculation".into()),
        AtkChange::Minus(n) => out.push(format!("the attacker loses {n} ATK at damage calculation")),
    }
    if a.no_damage {
        out.push("its controller takes no battle damage".into());
    }
    match a.burn {
        Burn::None => {}
        Burn::Fixed(n) => out.push(format!("the attacking player takes {n} damage")),
        Burn::AttackerAtk => out.push("the attacking player takes damage equal to the attacker's ATK".into()),
        Burn::Reflected => out.push("the attacking player takes the battle damage instead".into()),
    }
    if a.payoff != 0 {
        out.push(format!("its controller gets cards back when it is destroyed (worth about {})", a.payoff));
    }
    match a.collateral {
        0 => {}
        ALWAYS => out.push("the attacking player loses its other monsters".into()),
        n => out.push(format!("the attacking player loses {n} other card{}", if n == 1 { "" } else { "s" })),
    }
    if a.against != Against::All && !out.is_empty() {
        let who = match a.against {
            Against::All => String::new(),
            Against::AttackFrom(n) => format!("attackers with {n} or more ATK"),
            Against::AttackBelow(n) => format!("attackers with less than {n} ATK"),
            Against::LevelFrom(n) => format!("attackers of Level {n} or higher"),
            Against::LevelBelow(n) => format!("attackers below Level {n}"),
            Against::Attributes(mask) => format!("{} attackers", attributes(mask)),
        };
        out = vec![format!("against {who}: {}", out.join("; "))];
    }
    if a.switches {
        out.insert(0, "changes battle position when attacked".into());
    }
    out
}

fn attributes(mask: u8) -> String {
    let names = [(0x01, "EARTH"), (0x02, "WATER"), (0x04, "FIRE"), (0x08, "WIND"), (0x10, "LIGHT"), (0x20, "DARK")];
    names.iter().filter(|(bit, _)| mask & bit != 0).map(|(_, n)| *n).collect::<Vec<_>>().join("/")
}

fn kind_list(mask: u8) -> String {
    let names: Vec<&str> = [(kind::SPELL, "Spells"), (kind::TRAP, "Traps"), (kind::MONSTER, "monster effects")].iter().filter(|(k, _)| mask & k != 0).map(|(_, n)| *n).collect();
    names.join(", ")
}

/// What the table says about a card, in words.
pub fn describe(facts: &Facts) -> Vec<String> {
    let mut out = Vec::new();
    if facts.attacked[0] == facts.attacked[1] {
        for line in describe_attacked(&facts.attacked[0]) {
            out.push(format!("attacked: {line}"));
        }
    } else {
        for (label, a) in [("attacked in Attack Position", &facts.attacked[0]), ("attacked in Defense Position", &facts.attacked[1])] {
            for line in describe_attacked(a) {
                out.push(format!("{label}: {line}"));
            }
        }
    }
    let s = facts.striking;
    if s.bonus != 0 {
        out.push(format!("attacking: gains {} ATK at damage calculation", s.bonus));
    }
    if s.piercing {
        out.push("attacking: piercing battle damage".into());
    }
    if s.direct {
        out.push("attacking: can attack directly past monsters".into());
    }
    if s.target != Becomes::Unharmed {
        let how = match s.target {
            Becomes::Destroyed => "destroyed",
            Becomes::Banished => "banished",
            _ => "returned to the hand or Deck",
        };
        let who = match s.against {
            Against::Attributes(mask) => format!(" ({} monsters)", attributes(mask)),
            _ => String::new(),
        };
        out.push(format!(
            "attacking: the monster it attacks is {how}{who}{}{}",
            if s.before_damage { " before damage calculation" } else { "" },
            if s.leaves { ", and it leaves the field too" } else { "" }
        ));
    }
    for (label, mask) in [
        ("cannot be targeted by", facts.untargetable),
        ("its controller negates any", facts.negates_any),
        ("its controller negates, when they target it,", facts.negates_aimed),
        ("its controller negates, when they would destroy it (targeted or not),", facts.negates_destruction),
        ("not destroyed by", facts.immune),
        ("unaffected by", facts.unaffected),
    ] {
        if mask != 0 {
            out.push(format!("effects: {label} {}", kind_list(mask)));
        }
    }
    if facts.negate_times != 0 && facts.negate_times != ALWAYS {
        out.push(format!("effects: negates {} time{} a turn", facts.negate_times, if facts.negate_times == 1 { "" } else { "s" }));
    }
    if facts.negate_discards {
        out.push("effects: each negation costs its controller a card from the hand".into());
    }
    if facts.effect_payoff != 0 {
        out.push(format!("effects: its controller gets cards back when an effect destroys it (worth about {})", facts.effect_payoff));
    }
    match facts.effect_collateral {
        0 => {}
        ALWAYS => out.push("effects: the player that destroys it loses its monsters".into()),
        n => out.push(format!("effects: the player that destroys it loses {n} card{}", if n == 1 { "" } else { "s" })),
    }
    out
}

/// A listing to read through: every monster with facts, the ones that could
/// not be staged, and the ones a probe left unclear.
pub fn report(examined: &[Examined], derived: &BTreeMap<u32, Derived>, names: &dyn Fn(u32) -> String) -> String {
    let mut out = String::new();
    let mut by_name: Vec<(String, u32)> = derived.keys().map(|c| (names(*c), *c)).collect();
    by_name.sort();
    let known = derived.values().filter(|d| d.facts != Facts::NONE).count();
    writeln!(out, "# What the engine shows about the pool's monsters\n").unwrap();
    writeln!(out, "Generated by `policy-bench knowledge --report`. {} monsters probed, {known} with something to know.\n", examined.len()).unwrap();
    writeln!(out, "## Monsters with facts\n").unwrap();
    for (name, code) in &by_name {
        let d = &derived[code];
        if d.facts == Facts::NONE {
            continue;
        }
        writeln!(out, "- **{name}** ({code})").unwrap();
        for line in describe(&d.facts) {
            writeln!(out, "  - {line}").unwrap();
        }
    }
    writeln!(out, "\n## Left for review\n").unwrap();
    writeln!(out, "The probes showed something the table cannot hold, or could not show it cleanly.\n").unwrap();
    for (name, code) in &by_name {
        let d = &derived[code];
        for note in &d.notes {
            writeln!(out, "- **{name}** ({code}): {note}").unwrap();
        }
    }
    writeln!(out, "\n## Could not be staged\n").unwrap();
    writeln!(out, "These leave the field as soon as they are placed on a plain board (they need a Field Spell, counters, or a proper Summon).\n").unwrap();
    let unstaged: Vec<&str> = by_name.iter().filter(|(_, c)| derived[c].unstaged).map(|(n, _)| n.as_str()).collect();
    writeln!(out, "{}", unstaged.join(", ")).unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::Calc;

    fn battle(attacker: i32, attack: i32, subject: Fate) -> Act {
        Act {
            at_subject: true,
            calc: Some(Calc { attacker_atk: attacker, target_atk: attack, target_def: 0, target_defending: false }),
            attacker_visible: Some(attacker),
            target_visible: Some((attack, 0)),
            subject,
            damage: [0, (attacker - attack).max(0)],
            ..Act::default()
        }
    }

    #[test]
    fn a_plain_battle_reads_as_plain() {
        assert_eq!(read_attack(&battle(5100, 1500, Fate::Battle), None), Some(Attacked::PLAIN));
        // Not an attack on the monster, or a negated one: nothing to read.
        assert_eq!(read_attack(&Act::default(), None), None);
        assert_eq!(read_attack(&Act { negated: true, ..battle(5100, 1500, Fate::Stays) }, None), None);
    }

    #[test]
    fn what_a_battle_shows() {
        // Beaten on the numbers and still there.
        let survived = read_attack(&battle(5100, 1500, Fate::Stays), None).unwrap();
        assert_eq!(survived, Attacked { survives: 1, ..Attacked::PLAIN });
        // The attacker is gone before damage calculation, the monster stays.
        let gone = Act { at_subject: true, counterpart: Fate::Destroyed, before_calc: true, ..Act::default() };
        assert_eq!(read_attack(&gone, None).unwrap(), Attacked { attacker: Becomes::Destroyed, before_damage: true, ..Attacked::PLAIN });
        // Destroyed, and its controller got a monster for it.
        let mut floats = battle(5100, 1400, Fate::Battle);
        floats.gained[1].monsters = 1;
        assert_eq!(read_attack(&floats, None).unwrap().payoff, 1000);
        // The attacker's ATK became 0, whatever it was.
        let mut zeroed = battle(0, 1500, Fate::Stays);
        zeroed.attacker_visible = Some(5100);
        zeroed.counterpart = Fate::Battle;
        zeroed.damage = [1500, 0];
        let mut again = zeroed.clone();
        again.attacker_visible = Some(1600);
        assert_eq!(read_attack(&zeroed, Some(&again)).unwrap().attacker_atk, AtkChange::Zero);
    }

    #[test]
    fn conditions_are_read_from_the_attackers_tried() {
        let base = Attacker::BASE;
        let special = Attacked { survives: ALWAYS, ..Attacked::PLAIN };
        let met = |holds: bool| Some(if holds { special } else { Attacked::PLAIN });
        // Every attacker: no condition.
        let all: Vec<_> = probe::attribute::ALL.iter().map(|a| (Attacker { attribute: *a, ..base }, met(true))).collect();
        assert_eq!(against(&all, &special), Ok(Against::All));
        // Every Attribute but LIGHT.
        let light: Vec<_> = probe::attribute::ALL.iter().map(|a| (Attacker { attribute: *a, ..base }, met(*a != probe::attribute::LIGHT))).collect();
        assert_eq!(against(&light, &special), Ok(Against::Attributes(0x2f)));
        // From 1900 ATK up.
        let strong: Vec<_> = [1500, 1800, 1900, 3500, 5100].iter().map(|a| (Attacker { attack: *a, ..base }, met(*a >= 1900))).collect();
        assert_eq!(against(&strong, &special), Ok(Against::AttackFrom(1900)));
        // Below Level 8.
        let low: Vec<_> = (1..=12).map(|l| (Attacker { level: l, ..base }, met(l < 8))).collect();
        assert_eq!(against(&low, &special), Ok(Against::LevelBelow(8)));
        // Two things at once: not written down.
        let both: Vec<_> = light.iter().cloned().chain(strong.iter().cloned().filter(|(a, _)| a.attack != base.attack)).collect();
        assert!(against(&both, &special).is_err());
    }
}
