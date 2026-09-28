//! OCGCore's card-announcement filter (`is_declarable`): a small stack
//! program sent with `MSG_ANNOUNCE_CARD` that decides which names are legal.

use ygo_policies::cards::{types, CardData};

const BASE: u64 = 0x4000000000000000;
const ADD: u64 = BASE;
const SUB: u64 = BASE | 0x100000000;
const MUL: u64 = BASE | 0x200000000;
const DIV: u64 = BASE | 0x300000000;
const AND: u64 = BASE | 0x400000000;
const OR: u64 = BASE | 0x500000000;
const NEG: u64 = BASE | 0x600000000;
const NOT: u64 = BASE | 0x700000000;
const BAND: u64 = BASE | 0x800000000;
const BOR: u64 = BASE | 0x900000000;
const BNOT: u64 = BASE | 0x1000000000;
const BXOR: u64 = BASE | 0x1100000000;
const LSHIFT: u64 = BASE | 0x1200000000;
const RSHIFT: u64 = BASE | 0x1300000000;
const ALLOW_ALIASES: u64 = BASE | 0x1400000000;
const ALLOW_TOKENS: u64 = BASE | 0x1500000000;
const IS_CODE: u64 = BASE | 0x10000000000;
const IS_SETCARD: u64 = BASE | 0x10100000000;
const IS_TYPE: u64 = BASE | 0x10200000000;
const IS_RACE: u64 = BASE | 0x10300000000;
const IS_ATTRIBUTE: u64 = BASE | 0x10400000000;
const GET_CODE: u64 = BASE | 0x10500000000;
const GET_SETCARD: u64 = BASE | 0x10600000000;
const GET_CARDTYPE: u64 = BASE | 0x10700000000;
const GET_RACE: u64 = BASE | 0x10800000000;
const GET_ATTRIBUTE: u64 = BASE | 0x10900000000;

pub fn matches(card: &CardData, program: &[u64]) -> bool {
    let mut stack: Vec<i64> = Vec::new();
    let (mut aliases, mut tokens) = (false, false);
    for &op in program {
        match op {
            ALLOW_ALIASES => aliases = true,
            ALLOW_TOKENS => tokens = true,
            GET_CODE => stack.push(card.code as i64),
            GET_CARDTYPE => stack.push(card.kind as i64),
            GET_RACE => stack.push(card.race as i64),
            GET_ATTRIBUTE => stack.push(card.attribute as i64),
            GET_SETCARD => stack.push(card.setcodes.first().copied().unwrap_or(0) as i64),
            IS_CODE | IS_TYPE | IS_RACE | IS_ATTRIBUTE | IS_SETCARD | NEG | NOT | BNOT if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                stack.push(match op {
                    IS_CODE => (a == card.code as i64) as i64,
                    IS_TYPE => (a & card.kind as i64 != 0) as i64,
                    IS_RACE => (a & card.race as i64 != 0) as i64,
                    IS_ATTRIBUTE => (a & card.attribute as i64 != 0) as i64,
                    IS_SETCARD => card.in_set(a as u16) as i64,
                    NEG => -a,
                    NOT => (a == 0) as i64,
                    _ => !a,
                });
            }
            ADD | SUB | MUL | DIV | AND | OR | BAND | BOR | BXOR | LSHIFT | RSHIFT if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(match op {
                    ADD => a.wrapping_add(b),
                    SUB => a.wrapping_sub(b),
                    MUL => a.wrapping_mul(b),
                    DIV => a.checked_div(b).unwrap_or(0),
                    AND => (a != 0 && b != 0) as i64,
                    OR => (a != 0 || b != 0) as i64,
                    BAND => a & b,
                    BOR => a | b,
                    BXOR => a ^ b,
                    LSHIFT => a.wrapping_shl(b as u32),
                    _ => a.wrapping_shr(b as u32),
                });
            }
            value => stack.push(value as i64),
        }
    }
    let token = card.is(types::TOKEN);
    stack.len() == 1 && stack[0] != 0 && (aliases || card.alias == 0) && (tokens || !token)
}
