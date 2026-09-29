//! "Spellcaster's Command": Spell Counters.
//!
//! Every Spell we resolve puts Spell Counters on the counter collectors
//! (Magical Citadel of Endymion, Dark Red Enchanter, Marionette, Blast
//! Magician, Exemplar, Royal Magical Library, Cerberus...) and Breaker /
//! Hannibal / Defender come with one.  Counters are the currency: Breaker
//! destroys a Spell/Trap, Hannibal a Trap, Marionette and Blast Magician
//! monsters, Dark Red Enchanter hits the hand, the Library draws, Exemplar
//! summons, and six on the Citadel bring out Endymion.  Counters we place
//! ourselves (Apprentice Magician, Spell Power Grasp, Pitch-Black Power
//! Stone) go where they pay most.  Tower of Babel burns whoever resolves the
//! 4th Spell -- the Spell deck -- so it stays in the Deck's spirit only.

use crate::agent::{Response, Strategy, Turn};
use crate::ctx::Ctx;
use crate::model::{CardRef, CardView, Choice, ChoiceKind, Location, Position};

pub const DECK: &str = "14 - Spellcaster's Command";

const APPRENTICE: u32 = 9156135;
const POWER_STONE: u32 = 34029630;
const ENDYMION: u32 = 40732515;
const DISENCHANTER: u32 = 76137614;
const DEFENDER: u32 = 2525268;
const HANNIBAL: u32 = 5640330;
const SUMMONER_MONK: u32 = 423585;
const DARK_RED_ENCHANTER: u32 = 45462639;
const SKILLED_DARK_MAGICIAN: u32 = 73752131;
const OLD_VINDICTIVE: u32 = 45141844;
const MARIONETTE: u32 = 8034697;
const BREAKER: u32 = 71413901;
const MANDRAGOLA: u32 = 7802006;
const LIBRARY: u32 = 70791313;
const BLAST_MAGICIAN: u32 = 21051146;
const CERBERUS: u32 = 55424270;
const MEI_KOU: u32 = 47731128;
const CRYSTAL_SEER: u32 = 82099401;
const EXEMPLAR: u32 = 6061630;
const CITADEL: u32 = 39910367;
const SPELL_POWER_GRASP: u32 = 75014062;
const MAGICIANS_UNITE: u32 = 36045450;
const MIST_BODY: u32 = 47529357;
const NUZZLER: u32 = 99597615;
const FISSURE: u32 = 66788016;
const SWORDS: u32 = 72302403;
const MAGE_POWER: u32 = 83746708;
const TERRAFORMING: u32 = 73628505;
const ENEMY_CONTROLLER: u32 = 98045062;
const MAGICAL_BLAST: u32 = 91819979;
const MAGICAL_DIMENSION: u32 = 28553439;
const TWISTER: u32 = 45939841;
const FIELD_BARRIER: u32 = 7153114;
const MAGICIANS_CIRCLE: u32 = 50755;
const TOWER_OF_BABEL: u32 = 94256039;
const MAGIC_CYLINDER: u32 = 62279055;

#[derive(Default)]
pub struct Spellcaster;

impl Spellcaster {
    fn mine<'a>(ctx: &Ctx<'a>, code: u32) -> Option<&'a CardView> {
        ctx.monsters(ctx.me)
            .into_iter()
            .chain(ctx.spell_traps(ctx.me))
            .find(|c| c.position.face_up && ctx.is(c, code))
    }

    fn counters(ctx: &Ctx, code: u32) -> u32 {
        Self::mine(ctx, code).map_or(0, |c| c.counters)
    }

    /// Where a Spell Counter we place ourselves does the most.
    fn counter_target(ctx: &Ctx) -> Option<CardRef> {
        let rank = |code: u32| match code {
            CITADEL => 100,
            BREAKER | MARIONETTE | BLAST_MAGICIAN => 80,
            LIBRARY => 70,
            DARK_RED_ENCHANTER | EXEMPLAR | HANNIBAL => 60,
            CERBERUS | DEFENDER => 40,
            _ => 0,
        };
        ctx.monsters(ctx.me)
            .into_iter()
            .chain(ctx.spell_traps(ctx.me))
            .filter(|c| c.position.face_up)
            .filter_map(|c| c.code.map(|code| (rank(ctx.canonical(code)), c.at)))
            .filter(|(r, _)| *r > 0)
            .max_by_key(|(r, _)| *r)
            .map(|(_, at)| at)
    }

    fn best_opponent_monster<'a>(ctx: &Ctx<'a>, min_threat: i32) -> Option<&'a CardView> {
        ctx.monsters(ctx.opp).into_iter().max_by_key(|c| ctx.threat(c)).filter(|c| ctx.threat(c) >= min_threat)
    }

    fn best_opponent_backrow<'a>(ctx: &Ctx<'a>) -> Option<&'a CardView> {
        ctx.spell_traps(ctx.opp).into_iter().max_by_key(|c| if c.position.face_up { ctx.threat(c) } else { 1500 })
    }

    fn spells_in_hand(ctx: &Ctx) -> usize {
        ctx.hand().iter().filter(|c| ctx.view_data(c).is_spell()).count()
    }

    fn spellcasters(ctx: &Ctx) -> usize {
        ctx.monsters(ctx.me)
            .iter()
            .filter(|c| c.position.face_up && ctx.view_data(c).race & crate::cards::races::SPELLCASTER != 0)
            .count()
    }
}

impl Strategy for Spellcaster {
    fn value(&self, _ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            ENDYMION => 2800,
            CITADEL => 2400,
            MARIONETTE | DARK_RED_ENCHANTER | DISENCHANTER => 2100,
            BREAKER | MAGICAL_DIMENSION => 2000,
            SKILLED_DARK_MAGICIAN | BLAST_MAGICIAN | EXEMPLAR => 1800,
            SWORDS | ENEMY_CONTROLLER | FISSURE => 1700,
            DEFENDER | HANNIBAL | MEI_KOU | LIBRARY => 1600,
            CERBERUS | SUMMONER_MONK | TERRAFORMING | OLD_VINDICTIVE => 1500,
            SPELL_POWER_GRASP | MAGE_POWER | TWISTER | POWER_STONE => 1300,
            APPRENTICE | CRYSTAL_SEER | NUZZLER | MIST_BODY | MAGICAL_BLAST => 1200,
            MANDRAGOLA | FIELD_BARRIER | MAGICIANS_CIRCLE => 1000,
            MAGICIANS_UNITE => 900,
            MAGIC_CYLINDER => 1700,
            TOWER_OF_BABEL => 300,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // The Citadel collects every counter and pays for everything.
        if !ctx.face_up_on_field(ctx.me, CITADEL) {
            if let Some(i) = t.activate_from(CITADEL, Location::Hand) {
                return t.pick(i);
            }
            if let Some(i) = t.activate(TERRAFORMING) {
                return t.pick(i);
            }
        } else if !ctx.face_up_on_field(ctx.me, FIELD_BARRIER) {
            if let Some(i) = t.activate_from(FIELD_BARRIER, Location::Hand) {
                return t.pick(i);
            }
        }
        // Spend counters.
        if let Some(i) = t.activate_from(LIBRARY, Location::MonsterZone) {
            return t.pick(i);
        }
        if let Some(target) = Self::best_opponent_backrow(&ctx) {
            if let Some(i) = t.activate_from(BREAKER, Location::MonsterZone) {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        let face_up_trap = ctx.spell_traps(ctx.opp).into_iter().find(|c| c.position.face_up && ctx.view_data(c).is_trap());
        if let Some(trap) = face_up_trap {
            if let Some(i) = t.activate_from(HANNIBAL, Location::MonsterZone) {
                return t.pick_targeting(i, vec![trap.at]);
            }
        }
        let face_up_continuous = ctx
            .spell_traps(ctx.opp)
            .into_iter()
            .find(|c| c.position.face_up && ctx.view_data(c).is(crate::cards::types::CONTINUOUS));
        if let Some(target) = face_up_continuous {
            if let Some(i) = t.activate_from(MEI_KOU, Location::MonsterZone) {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        if let Some(target) = Self::best_opponent_monster(&ctx, 1500) {
            for code in [MARIONETTE, ENDYMION] {
                if let Some(i) = t.activate_from(code, Location::MonsterZone) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
        }
        // Blast Magician reaches face-up monsters with up to 700 ATK per
        // counter it holds, on either field: past its reach, the only targets
        // left would be our own.
        if let Some(blast) = Self::mine(&ctx, BLAST_MAGICIAN) {
            let reach = 700 * blast.counters as i32;
            let target = ctx
                .monsters(ctx.opp)
                .into_iter()
                .filter(|c| c.position.face_up && c.attack <= reach)
                .max_by_key(|c| ctx.threat(c))
                .filter(|c| ctx.threat(c) >= 1000);
            if let Some(target) = target {
                if let Some(i) = t.activate_from(BLAST_MAGICIAN, Location::MonsterZone) {
                    return t.pick_targeting(i, vec![target.at]);
                }
            }
        }
        if ctx.hand_size(ctx.opp) >= 2 && Self::counters(&ctx, DARK_RED_ENCHANTER) >= 4 {
            if let Some(i) = t.activate_from(DARK_RED_ENCHANTER, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        let their_face_up_spell = ctx
            .spell_traps(ctx.opp)
            .into_iter()
            .find(|c| c.position.face_up && ctx.view_data(c).is_spell());
        if let Some(target) = their_face_up_spell {
            if let Some(i) = t.activate_from(DISENCHANTER, Location::MonsterZone) {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        if let Some(i) = t.activate_from(EXEMPLAR, Location::MonsterZone) {
            return t.pick(i);
        }
        if Self::spells_in_hand(&ctx) >= 2 {
            if let Some(i) = t.activate_from(SUMMONER_MONK, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        // Place counters where they pay most.
        if let Some(target) = Self::counter_target(&ctx) {
            for code in [SPELL_POWER_GRASP, POWER_STONE] {
                if let Some(i) = t.activate(code) {
                    return t.pick_targeting(i, vec![target]);
                }
            }
        }
        // Removal and board spells (each also feeds the counters).
        if ctx.monsters(ctx.opp).iter().any(|c| c.position.face_up) {
            if let Some(i) = t.activate(FISSURE) {
                return t.pick(i);
            }
        }
        if ctx.opp_best_attack() > ctx.my_best_attack() && !ctx.monsters(ctx.opp).is_empty() {
            if let Some(i) = t.activate(SWORDS) {
                return t.pick(i);
            }
        }
        // Magical Dimension: swap a small Spellcaster for a big one and destroy a monster.
        let big_in_hand = ctx.hand().iter().any(|c| {
            let d = ctx.view_data(c);
            d.race & crate::cards::races::SPELLCASTER != 0 && d.level >= 5
        });
        if big_in_hand && Self::best_opponent_monster(&ctx, 1000).is_some() {
            if let Some(i) = t.activate(MAGICAL_DIMENSION) {
                return t.pick(i);
            }
        }
        let face_up_their_st = ctx.spell_traps(ctx.opp).into_iter().filter(|c| c.position.face_up).max_by_key(|c| ctx.threat(c));
        if let Some(target) = face_up_their_st {
            if let Some(i) = t.activate(TWISTER) {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        let blast = 200 * Self::spellcasters(&ctx) as i32;
        if blast >= ctx.opp_lp() || Self::spellcasters(&ctx) >= 3 {
            if let Some(i) = t.activate(MAGICAL_BLAST) {
                return t.pick(i);
            }
        }
        // Mist Body keeps our most valuable monster alive in battle when theirs are bigger.
        let keeper = ctx.monsters(ctx.me).into_iter().filter(|c| c.position.face_up).max_by_key(|c| c.attack);
        if let Some(keeper) = keeper {
            if ctx.opp_best_attack() > keeper.attack.max(keeper.defense) {
                if let Some(i) = t.activate(MIST_BODY) {
                    return t.pick_targeting(i, vec![keeper.at]);
                }
            }
        }
        // Equips on our best attacker before battle.
        if ctx.main1() {
            let attacker = ctx.monsters(ctx.me).into_iter().filter(|c| ctx.can_attack(c)).max_by_key(|c| c.attack);
            if let Some(attacker) = attacker {
                let best = ctx.opp_best_attack();
                if attacker.attack <= best && attacker.attack + 700 > best {
                    for code in [NUZZLER, MAGE_POWER] {
                        if let Some(i) = t.activate(code) {
                            return t.pick_targeting(i, vec![attacker.at]);
                        }
                    }
                }
            }
        }
        None
    }

    fn allow_staple(&self, t: &Turn, code: u32) -> bool {
        // Giant Trunade would bounce our own Citadel and its counters.
        !(code == crate::staples::GIANT_TRUNADE && t.ctx.face_up_on_field(t.ctx.me, CITADEL))
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, BREAKER) => Some(1900.0 + if ctx.spell_traps(ctx.opp).is_empty() { 0.0 } else { 300.0 }),
            (ChoiceKind::NormalSummon, SKILLED_DARK_MAGICIAN) => Some(1800.0),
            (ChoiceKind::NormalSummon, EXEMPLAR | MEI_KOU) => Some(1700.0),
            (ChoiceKind::NormalSummon, DEFENDER) => Some(1650.0),
            (ChoiceKind::NormalSummon, HANNIBAL | BLAST_MAGICIAN | CERBERUS) => Some(1500.0),
            (ChoiceKind::NormalSummon, SUMMONER_MONK) if Self::spells_in_hand(&ctx) >= 2 => Some(1550.0),
            (ChoiceKind::NormalSummon, LIBRARY) => Some(1300.0),
            (ChoiceKind::NormalSummon, APPRENTICE) if Self::counter_target(&ctx).is_some() => Some(1250.0),
            (ChoiceKind::SetMonster, OLD_VINDICTIVE) if !ctx.monsters(ctx.opp).is_empty() => Some(1500.0),
            (ChoiceKind::SetMonster, CRYSTAL_SEER) => Some(1200.0),
            (ChoiceKind::SetMonster, MANDRAGOLA) => Some(1100.0),
            (ChoiceKind::NormalSummon, OLD_VINDICTIVE | CRYSTAL_SEER | MANDRAGOLA | APPRENTICE) => None,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        Some(match code {
            // Our own summon triggers that place a counter: aim it.
            APPRENTICE => match Self::counter_target(&ctx) {
                Some(target) => Response::targeting(40.0, vec![target]),
                None => Response::no(),
            },
            ENEMY_CONTROLLER => match ctx.incoming_attack() {
                Some((attacker, target)) if attacker.position.face_up && ctx.attack_hurts(attacker, target) => {
                    Response::targeting(60.0, vec![attacker.at])
                }
                _ => Response::no(),
            },
            // From the Graveyard it costs the normal draw: only for lethal.
            MAGICAL_BLAST if choice.at().map(|a| a.location) == Some(Location::Graveyard) => {
                if 200 * (Self::spellcasters(&ctx) as i32).max(1) >= ctx.opp_lp() { Response::new(50.0) } else { Response::no() }
            }
            MAGIC_CYLINDER => match ctx.incoming_attack() {
                Some((attacker, target)) if attacker.attack >= 1500 && ctx.attack_hurts(attacker, target) => Response::new(70.0),
                _ => Response::no(),
            },
            // Our attacks only: both players summon a Spellcaster.
            MAGICIANS_CIRCLE if ctx.my_turn() => Response::new(40.0),
            POWER_STONE if !ctx.my_turn() && ctx.phase() == Some(crate::model::Phase::End) => Response::new(20.0),
            MAGICIANS_CIRCLE | POWER_STONE | TOWER_OF_BABEL | MAGICIANS_UNITE => Response::no(),
            _ => return None,
        })
    }

    fn yes_no(&self, t: &Turn) -> Option<bool> {
        let ctx = t.ctx;
        // Magical Blast back from the Graveyard costs the normal draw: only for lethal.
        (t.decision.subject.map(|c| ctx.canonical(c)) == Some(MAGICAL_BLAST))
            .then(|| 200 * (Self::spellcasters(&ctx) as i32).max(1) >= ctx.opp_lp())
    }

    fn position(&self, t: &Turn, code: u32) -> Option<Position> {
        matches!(t.ctx.canonical(code), LIBRARY | SUMMONER_MONK).then_some(Position::FACE_UP_DEFENSE)
    }

    fn option(&self, _t: &Turn) -> Option<usize> {
        // Enemy Controller: change the attacker's battle position.
        Some(0)
    }

    fn set_spell_trap(&self, t: &Turn, code: u32) -> Option<bool> {
        let code = t.ctx.canonical(code);
        Some(match code {
            TOWER_OF_BABEL => false,
            ENEMY_CONTROLLER => true,
            other => t.ctx.data(other).is_trap(),
        })
    }
}
