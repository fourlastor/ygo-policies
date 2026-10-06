//! "Harpie Sisters": Mai's Harpie Ladies and their hunting ground.
//!
//! Harpies' Hunting Ground turns every Summon of a "Harpie Lady" (Harpie
//! Queen, Harpie Lady 1-3 and Cyber Harpie Lady all count) into the
//! destruction of a Spell or Trap, so the deck summons Ladies as often as it
//! can: Normal Summons, Elegant Egotist (another Lady, or Harpie Lady
//! Sisters, from the Deck), and Hysteric Party (Ladies back from the
//! Graveyard). Mist Valley Falcon returns set backrow before other attackers.
//! Queen searches Hunting Ground when opposing backrow makes it useful.
//! Harpie's Pet Baby Dragon
//! shields the other Harpies from attacks, doubles, and with three Harpies
//! destroys a card each turn; Harpie's Pet Dragon grows with the Ladies.
//! Triangle Ecstasy Spark raises the Sisters to 2700 and shuts off Traps.

use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::races;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member, Phase};

pub const DECK: &str = "Harpie Sisters";

const HARPIE_QUEEN: u32 = 75064463;
const HARPIE_LADY_1: u32 = 91932350;
const HARPIE_LADY_2: u32 = 27927359;
const HARPIE_LADY_3: u32 = 54415063;
const HARPIE_LADY: u32 = 76812113;
const CYBER_HARPIE_LADY: u32 = 80316585;
const HARPIE_GIRL: u32 = 34100324;
const HARPIE_LADY_SISTERS: u32 = 12206212;
const HARPIES_PET_DRAGON: u32 = 52040216;
const HARPIES_PET_BABY_DRAGON: u32 = 6924874;
const BIRDFACE: u32 = 45547649;
const MIST_VALLEY_FALCON: u32 = 82199284;
const HUNTING_GROUND: u32 = 75782277;
const ELEGANT_EGOTIST: u32 = 90219263;
const TRIANGLE_ECSTASY_SPARK: u32 = 12181376;
const RISING_AIR_CURRENT: u32 = 45778932;
const ICARUS_ATTACK: u32 = 53567095;
const HYSTERIC_PARTY: u32 = 77778835;
const SET_HARPIE: u16 = 0x64;

/// Cards whose name is "Harpie Lady" on the field and in the Graveyard.
const LADIES: [u32; 6] = [HARPIE_LADY, HARPIE_QUEEN, HARPIE_LADY_1, HARPIE_LADY_2, HARPIE_LADY_3, CYBER_HARPIE_LADY];

#[derive(Clone, Default)]
pub struct Harpie;

impl Harpie {
    fn is_lady(ctx: &Ctx, card: &CardView) -> bool {
        LADIES.iter().any(|k| ctx.is(card, *k))
    }

    fn is_harpie(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.in_set(SET_HARPIE)
    }

    fn ladies_up(ctx: &Ctx) -> usize {
        ctx.monsters(ctx.me).iter().filter(|c| c.position.face_up && Self::is_lady(ctx, c)).count()
    }

    fn ground_up(ctx: &Ctx) -> bool {
        ctx.face_up_on_field(ctx.me, HUNTING_GROUND)
    }

    /// A Lady's Summon would destroy one of their Spells/Traps.
    fn ground_pays(ctx: &Ctx) -> bool {
        Self::ground_up(ctx) && !ctx.spell_traps(ctx.opp).is_empty()
    }

    fn field_spell_up(ctx: &Ctx) -> bool {
        Self::ground_up(ctx) || ctx.face_up_on_field(ctx.me, RISING_AIR_CURRENT)
    }

    /// Icarus Attack: our cheapest Winged Beast for two of their cards.
    fn icarus_targets<'a>(ctx: &Ctx<'a>) -> Vec<&'a CardView> {
        let mut targets: Vec<_> = ctx.monsters(ctx.opp).into_iter().chain(ctx.spell_traps(ctx.opp)).collect();
        targets.sort_by_key(|c| -ctx.threat(c));
        targets
    }
}

impl Strategy for Harpie {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        Some(match code {
            HUNTING_GROUND => 2600,
            HARPIES_PET_DRAGON => 2000 + 300 * Self::ladies_up(ctx) as i32,
            HARPIE_LADY_SISTERS => 2300,
            HARPIES_PET_BABY_DRAGON | ELEGANT_EGOTIST => 2000,
            HARPIE_QUEEN | CYBER_HARPIE_LADY => 1900,
            HARPIE_LADY_1 | MIST_VALLEY_FALCON | HYSTERIC_PARTY => 1800,
            ICARUS_ATTACK => 1700,
            HARPIE_LADY_3 | BIRDFACE => 1600,
            HARPIE_LADY_2 | HARPIE_LADY | TRIANGLE_ECSTASY_SPARK => 1500,
            RISING_AIR_CURRENT => 1300,
            HARPIE_GIRL => 700,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Its destruction is mandatory: save Ground and Queen until it has
        // an opposing target, rather than sacrificing our own backrow.
        if !Self::ground_up(&ctx) && !ctx.spell_traps(ctx.opp).is_empty() {
            if let Some(i) = t.activate(HUNTING_GROUND) {
                return t.pick(i);
            }
            // Harpie Queen fetches it.
            if !ctx.in_hand(HUNTING_GROUND) {
                if let Some(i) = t.activate_from(HARPIE_QUEEN, Location::Hand) {
                    return t.pick(i);
                }
            }
        }
        if !Self::field_spell_up(&ctx) && !ctx.in_hand(HUNTING_GROUND) {
            if let Some(i) = t.activate_from(RISING_AIR_CURRENT, Location::Hand) {
                return t.pick(i);
            }
        }
        // Even one Lady is an attacker or an Egotist enabler this turn.
        if ctx.main1() && ctx.free_monster_zones(ctx.me) >= 1 && ctx.hand_size(ctx.me) >= 1
            && ctx.graveyard(ctx.me).iter().filter(|c| Self::is_lady(&ctx, c)).count() >= 1 {
            if let Some(i) = t.activate(HYSTERIC_PARTY) {
                return t.pick(i);
            }
        }
        // Another Lady (or the Sisters) from the Deck: a free Hunting Ground hit.
        if Self::ladies_up(&ctx) >= 1 && ctx.free_monster_zones(ctx.me) > 0 {
            if let Some(i) = t.activate(ELEGANT_EGOTIST) {
                return t.pick(i);
            }
        }
        // Pet Baby Dragon with three Harpies: their best card.
        if let Some(i) = t.activate_from(HARPIES_PET_BABY_DRAGON, Location::MonsterZone) {
            if let Some(target) = Self::icarus_targets(&ctx).first() {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        // The Sisters at 2700, with their Traps shut off, right before battle.
        let sisters_attack = ctx.monsters(ctx.me).iter().any(|c| ctx.is(c, HARPIE_LADY_SISTERS) && ctx.can_attack(c));
        if ctx.main1() && t.has(ChoiceKind::EnterBattle) && sisters_attack {
            if let Some(i) = t.activate(TRIANGLE_ECSTASY_SPARK) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let ground = if Self::ground_pays(&ctx) { 500.0 } else { 0.0 };
        let harpies = ctx.monsters(ctx.me).iter().filter(|c| c.position.face_up && Self::is_harpie(&ctx, c)).count();
        Some(match (choice.kind, code) {
            (ChoiceKind::NormalSummon, CYBER_HARPIE_LADY | HARPIE_QUEEN) => Some(1900.0 + ground),
            (ChoiceKind::NormalSummon, HARPIE_LADY_1) => Some(1850.0 + ground),
            (ChoiceKind::NormalSummon, HARPIE_LADY_3 | HARPIE_LADY_2 | HARPIE_LADY) => Some(1500.0 + ground),
            // The Baby Dragon shields the Harpies already out.
            (ChoiceKind::NormalSummon, HARPIES_PET_BABY_DRAGON) if harpies >= 1 => Some(1800.0 + 200.0 * harpies as f64),
            (ChoiceKind::SetMonster, BIRDFACE) => Some(1300.0),
            (ChoiceKind::NormalSummon | ChoiceKind::SetMonster, HARPIE_GIRL) => None,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let incoming = ctx.incoming_attack();
        Some(match code {
            ICARUS_ATTACK => {
                let fodder = ctx.monsters(ctx.me).iter().any(|c| ctx.view_data(c).race & races::WINGED_BEAST != 0);
                let targets = Self::icarus_targets(&ctx);
                if !fodder || targets.len() < 2 {
                    return Some(Response::no());
                }
                match incoming {
                    Some((attacker, target)) if ctx.attack_hurts(attacker, target) => {
                        let other = targets.iter().find(|c| c.at != attacker.at).map(|c| c.at);
                        Response::targeting(70.0, std::iter::once(attacker.at).chain(other).collect())
                    }
                    _ => Response::no(),
                }
            }
            // Revive a group whenever possible, or one Lady to rebuild an empty
            // field or attack this turn. A group Summon triggers Ground once.
            HYSTERIC_PARTY if t.view(choice).map_or(false, |v| !v.position.face_up) => {
                let ladies = ctx.graveyard(ctx.me).iter().filter(|c| Self::is_lady(&ctx, c)).count();
                let attacking = ctx.my_turn() && ctx.phase() == Some(Phase::Main1);
                let rebuild = ctx.monsters(ctx.me).is_empty();
                if ladies >= 1 && ctx.hand_size(ctx.me) >= 1 && (attacking || ladies >= 2 || rebuild) {
                    Response::new(35.0)
                } else {
                    Response::no()
                }
            }
            HYSTERIC_PARTY | TRIANGLE_ECSTASY_SPARK => Response::no(),
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
        // Falcon can return and re-set backrow while keeping our attackers.
        if t.decision.hint == Hint::ReturnToHand && member.at.location == Location::SpellTrapZone
            && ctx.card(member.at).map_or(false, |c| !c.position.face_up) {
            return Some(3000.0);
        }
        // If a mandatory Ground trigger has only our cards to choose from,
        // sacrifice Ground before support such as a Party sustaining Ladies.
        if t.decision.hint == Hint::Destroy && code == HUNTING_GROUND
            && t.memory.last_activated.map(|c| ctx.canonical(c)) == Some(HUNTING_GROUND)
            && ctx.spell_traps(ctx.opp).is_empty() {
            return Some(0.0);
        }
        match t.decision.hint {
            // Otherwise return a Lady to summon again, preserving face-up
            // Ground and Party (its Ladies die when it leaves the field).
            Hint::ReturnToHand if member.at.location.is_field() => Some(match code {
                HUNTING_GROUND | HYSTERIC_PARTY => -10_000.0,
                _ if LADIES.contains(&code) => 500.0 - worth / 10.0,
                _ => -worth,
            }),
            // Elegant Egotist: the Sisters first.
            Hint::SpecialSummon if member.at.location == Location::Deck || member.at.location == Location::Hand => Some(worth),
            _ => None,
        }
    }
}
