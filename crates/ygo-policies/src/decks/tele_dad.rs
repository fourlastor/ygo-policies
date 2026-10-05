//! "Tele-DAD": Emergency Teleport Synchros and Dark Armed Dragon.
//!
//! Emergency Teleport (a Quick-Play) Special Summons Krebons or Psychic
//! Commander from the Deck; beside any monster they are a Synchro Summon, so
//! the Tuner is picked for the Level it reaches (Stardust, Scrap,
//! Trishula...).  The DARK half fills the Graveyard for Dark Armed Dragon,
//! which needs exactly three DARK monsters there and then banishes them one
//! at a time to destroy a card each: Armageddon Knight and Dark Grepher send
//! DARKs from the Deck, Destiny HERO - Malicious leaves the Graveyard for
//! another copy, Destiny Draw and Allure of Darkness trade them for cards.
//! Caius banishes a card on its Tribute Summon, Gorz punishes an empty field,
//! and a heavy trap line (Solemns, Bottomless, Mirror Force, Torrential)
//! holds the board.

use crate::agent::{value, Response, Strategy, Turn};
use crate::cards::attributes;
use crate::ctx::Ctx;
use crate::model::{CardView, Choice, ChoiceKind, Hint, Location, Member};
use crate::tactics;

pub const DECK: &str = "Tele-DAD";

const DARK_ARMED_DRAGON: u32 = 65192027;
const KREBONS: u32 = 59575539;
const PSYCHIC_COMMANDER: u32 = 21454943;
const MALICIOUS: u32 = 9411399;
const DARK_GREPHER: u32 = 14536035;
const ARMAGEDDON_KNIGHT: u32 = 28985331;
const CAIUS: u32 = 9748752;
const CARD_TROOPER: u32 = 85087012;
const SANGAN: u32 = 26202165;
const GORZ: u32 = 44330098;
const PLAGUESPREADER: u32 = 33420078;
const SPIRIT_REAPER: u32 = 23205979;
const EMERGENCY_TELEPORT: u32 = 67723438;
const DESTINY_DRAW: u32 = 45809008;
const MAGICAL_ANDROID: u32 = 43385557;
const THOUGHT_RULER: u32 = 70780151;
const STARDUST_DRAGON: u32 = 44508094;
const BRIONAC: u32 = 50321796;
const TRISHULA: u32 = 52687916;
const BLACK_ROSE_DRAGON: u32 = 73580471;
const COLOSSAL_FIGHTER: u32 = 23693634;
const MIST_WURM: u32 = 27315304;
const SCRAP_DRAGON: u32 = 76774528;
const RED_DRAGON_ARCHFIEND: u32 = 70902743;
const CATASTOR: u32 = 26593852;
const ARMORY_ARM: u32 = 29071332;

#[derive(Default)]
pub struct TeleDad;

impl TeleDad {
    fn is_dark(ctx: &Ctx, card: &CardView) -> bool {
        let d = ctx.view_data(card);
        d.is_monster() && d.attribute & attributes::DARK != 0
    }

    fn darks_in_graveyard(ctx: &Ctx) -> usize {
        ctx.graveyard(ctx.me).iter().filter(|c| Self::is_dark(ctx, c)).count()
    }

    /// The Level 3 or lower Psychic Tuner Emergency Teleport should bring:
    /// the one whose Synchro Summon is worth most.
    fn teleport_pick(&self, ctx: &Ctx) -> Option<(u32, i32)> {
        [KREBONS, PSYCHIC_COMMANDER]
            .into_iter()
            .filter_map(|code| tactics::synchro_with_tuner(self, ctx, ctx.data(code).level).map(|worth| (code, worth)))
            .max_by_key(|(_, worth)| *worth)
    }

    /// What we would rather lose from the hand (discards, costs).
    fn discard_rank(ctx: &Ctx, code: u32) -> i32 {
        match code {
            MALICIOUS => 300,
            DARK_ARMED_DRAGON if ctx.count_in(ctx.me, Location::Hand, DARK_ARMED_DRAGON) >= 2 => 200,
            PLAGUESPREADER | SPIRIT_REAPER => 100,
            _ => 0,
        }
    }
}

impl Strategy for TeleDad {
    fn value(&self, ctx: &Ctx, code: u32) -> Option<i32> {
        let their_cards = (ctx.monsters(ctx.opp).len() + ctx.spell_traps(ctx.opp).len()) as i32;
        Some(match code {
            TRISHULA => 3300,
            DARK_ARMED_DRAGON | STARDUST_DRAGON | SCRAP_DRAGON | COLOSSAL_FIGHTER => 2800,
            THOUGHT_RULER => 2700,
            MIST_WURM => 2600,
            BRIONAC => 2300 + 100 * their_cards.min(3),
            MAGICAL_ANDROID | CAIUS => 2400,
            GORZ => 2300,
            CATASTOR => 2200,
            BLACK_ROSE_DRAGON | RED_DRAGON_ARCHFIEND => 2000,
            EMERGENCY_TELEPORT => 2000,
            DARK_GREPHER => 1900,
            ARMAGEDDON_KNIGHT | DESTINY_DRAW | ARMORY_ARM => 1800,
            KREBONS => 1700,
            PSYCHIC_COMMANDER | SANGAN | MALICIOUS => 1500,
            CARD_TROOPER => 1400,
            PLAGUESPREADER | SPIRIT_REAPER => 1300,
            _ => return None,
        })
    }

    fn main_phase(&mut self, t: &mut Turn) -> Option<usize> {
        let ctx = t.ctx;
        // Dark Armed Dragon the moment the Graveyard holds exactly three DARKs.
        if let Some(i) = t.find(ChoiceKind::SpecialSummon, Some(DARK_ARMED_DRAGON), Some(Location::Hand)) {
            return t.pick(i);
        }
        // ... then it destroys a card per DARK it banishes.
        if let Some(i) = t.activate_from(DARK_ARMED_DRAGON, Location::MonsterZone) {
            let target = ctx
                .monsters(ctx.opp)
                .into_iter()
                .chain(ctx.spell_traps(ctx.opp))
                .max_by_key(|c| ctx.threat(c))
                .filter(|c| ctx.threat(c) >= 1000);
            if let Some(target) = target {
                return t.pick_targeting(i, vec![target.at]);
            }
        }
        // Card advantage that also fills the Graveyard.
        if ctx.hand().iter().any(|c| ctx.is(c, MALICIOUS)) {
            if let Some(i) = t.activate(DESTINY_DRAW) {
                return t.pick(i);
            }
        }
        let darks = Self::darks_in_graveyard(&ctx);
        let dad_in_hand = ctx.in_hand(DARK_ARMED_DRAGON);
        // Dark Grepher: a DARK from the hand, one from the Deck.
        if darks < 3 || !dad_in_hand {
            if ctx.hand().iter().any(|c| Self::is_dark(&ctx, c) && c.code.map_or(false, |k| Self::discard_rank(&ctx, ctx.canonical(k)) > 0)) {
                if let Some(i) = t.activate_from(DARK_GREPHER, Location::MonsterZone) {
                    return t.pick(i);
                }
            }
        }
        // Emergency Teleport: a Tuner from the Deck when it makes a Synchro.
        if ctx.main1() && ctx.free_monster_zones(ctx.me) > 0 && self.teleport_pick(&ctx).is_some() {
            if let Some(i) = t.activate(EMERGENCY_TELEPORT) {
                return t.pick(i);
            }
        }
        // Malicious: another copy from the Deck, as Synchro material or a body.
        let tuner_up = ctx.monsters(ctx.me).iter().any(|c| c.position.face_up && ctx.view_data(c).is_tuner());
        let dad_count_ok = !dad_in_hand || darks != 3;
        if (tuner_up || ctx.monsters(ctx.me).is_empty()) && dad_count_ok && ctx.free_monster_zones(ctx.me) > 0 {
            if let Some(i) = t.activate_from(MALICIOUS, Location::Graveyard) {
                return t.pick(i);
            }
        }
        // Plaguespreader from the Graveyard beside a non-Tuner: a Synchro.
        if tactics::synchro_with_tuner(self, &ctx, 2).is_some() && ctx.hand_size(ctx.me) >= 1 {
            if let Some(i) = t.activate_from(PLAGUESPREADER, Location::Graveyard) {
                return t.pick(i);
            }
        }
        // Card Trooper mills toward the DARK count.
        if dad_in_hand && darks < 3 {
            if let Some(i) = t.activate_from(CARD_TROOPER, Location::MonsterZone) {
                return t.pick(i);
            }
        }
        None
    }

    fn summon_score(&self, t: &Turn, choice: &Choice) -> Option<Option<f64>> {
        let ctx = t.ctx;
        let code = ctx.canonical(choice.code()?);
        let teleport = ctx.in_hand(EMERGENCY_TELEPORT);
        Some(match (choice.kind, code) {
            // A DARK from the Deck on the way in.
            (ChoiceKind::NormalSummon, ARMAGEDDON_KNIGHT) => Some(2000.0 + if teleport { 200.0 } else { 0.0 }),
            (ChoiceKind::NormalSummon, DARK_GREPHER) => Some(1900.0),
            (ChoiceKind::SetMonster, SANGAN) => Some(1500.0),
            (ChoiceKind::NormalSummon, CARD_TROOPER) if ctx.in_hand(DARK_ARMED_DRAGON) => Some(1600.0),
            (ChoiceKind::NormalSummon, KREBONS | PSYCHIC_COMMANDER) if tactics::synchro_with_tuner(self, &ctx, ctx.data(code).level).is_some() => {
                Some(2100.0)
            }
            (_, GORZ) => None,
            _ => return None,
        })
    }

    fn special_summon(&self, t: &Turn, choice: &Choice) -> Option<bool> {
        let ctx = t.ctx;
        Some(match ctx.canonical(choice.code().unwrap_or(0)) {
            // Its discard should be a DARK we want in the Graveyard.
            DARK_GREPHER => ctx.hand().iter().any(|c| ctx.is(c, MALICIOUS) || (ctx.is(c, DARK_ARMED_DRAGON) && Self::discard_rank(&ctx, DARK_ARMED_DRAGON) > 0)),
            BLACK_ROSE_DRAGON => ctx.field_strength(ctx.opp) > ctx.field_strength(ctx.me) + 2500,
            // Its End Phase destroys our monsters that did not attack.
            RED_DRAGON_ARCHFIEND => ctx.monsters(ctx.me).len() <= 2,
            _ => return None,
        })
    }

    fn chain(&mut self, t: &Turn, index: usize) -> Option<Response> {
        let ctx = t.ctx;
        let choice = t.choice(index);
        let code = ctx.canonical(choice.code()?);
        let incoming = ctx.incoming_attack();
        Some(match code {
            // Krebons pays 800 to stop an attack on it.
            KREBONS => match incoming {
                Some((attacker, target)) if ctx.attack_hurts(attacker, target) && ctx.my_lp() > 2000 => Response::new(60.0),
                _ => Response::no(),
            },
            // Negates a Spell/Trap aimed at one Psychic (1000 LP).
            THOUGHT_RULER if ctx.my_lp() > 2000 => Response::new(80.0),
            THOUGHT_RULER | PSYCHIC_COMMANDER => Response::no(),
            // On their turn, Emergency Teleport is a blocker.
            EMERGENCY_TELEPORT => {
                let blocker = incoming.is_some() && ctx.monsters(ctx.me).is_empty();
                if blocker { Response::new(40.0) } else { Response::no() }
            }
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
        match (t.memory.last_activated.map(|c| ctx.canonical(c)), t.decision.hint) {
            // Emergency Teleport: the Tuner for the best Synchro.
            (Some(EMERGENCY_TELEPORT), Hint::SpecialSummon) => {
                let pick = self.teleport_pick(&ctx).map(|(c, _)| c);
                Some(if pick == Some(code) { 5000.0 } else { worth })
            }
            // DARKs sent from the Deck: Malicious can leave the Graveyard again.
            (_, Hint::ToGraveyard) if member.at.location == Location::Deck => Some(match code {
                MALICIOUS => 3000.0,
                PLAGUESPREADER => 2500.0,
                SPIRIT_REAPER | KREBONS => 2000.0,
                _ => -worth,
            }),
            // Discards: what works from the Graveyard.
            (_, Hint::Discard) | (_, Hint::ToGraveyard) if member.at.location == Location::Hand => {
                Some(Self::discard_rank(&ctx, code) as f64 - worth / 10.0)
            }
            _ => None,
        }
    }
}
