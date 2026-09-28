//! OCGCore card queries (`OCG_DuelQuery*`), as carried by `MSG_UPDATE_DATA`
//! and `MSG_UPDATE_CARD`.
//!
//! A card record is a run of `[u16 size][u32 flag][size - 4 bytes]` fields
//! closed by a `QUERY_END` field; an empty zone is a lone `u16 0`.

use crate::wire::{query, Reader, Result};

/// One card record, field by field, exactly as the engine wrote it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawRecord<'a> {
    pub fields: Vec<(u32, &'a [u8])>,
}

impl<'a> RawRecord<'a> {
    pub fn get(&self, flag: u32) -> Option<&'a [u8]> {
        self.fields.iter().find(|(f, _)| *f == flag).map(|(_, v)| *v)
    }

    fn u32(&self, flag: u32) -> Option<u32> {
        self.get(flag).filter(|v| v.len() >= 4).map(|v| u32::from_le_bytes(v[..4].try_into().unwrap()))
    }

    fn u8(&self, flag: u32) -> Option<u8> {
        self.get(flag).and_then(|v| v.first().copied())
    }

    pub fn position(&self) -> Option<u32> {
        self.u32(query::POSITION)
    }

    pub fn is_public(&self) -> bool {
        self.u8(query::IS_PUBLIC).map_or(false, |v| v != 0)
    }

    pub fn parse(&self) -> Query {
        let counters = self.get(query::COUNTERS).map(|v| {
            let mut r = Reader::new(v);
            let n = r.u32().unwrap_or(0);
            (0..n).filter_map(|_| r.u32().ok()).map(|packed| packed >> 16).sum()
        });
        Query {
            code: self.u32(query::CODE),
            position: self.position(),
            level: self.u32(query::LEVEL),
            rank: self.u32(query::RANK),
            attack: self.u32(query::ATTACK).map(|v| v as i32),
            defense: self.u32(query::DEFENSE).map(|v| v as i32),
            counters,
            is_public: self.get(query::IS_PUBLIC).map(|_| self.is_public()),
        }
    }

    /// Re-encode, keeping only `keep` fields.
    pub fn write(&self, keep: impl Fn(u32) -> bool, out: &mut Vec<u8>) {
        for (flag, value) in &self.fields {
            if *flag == query::END || !keep(*flag) {
                continue;
            }
            out.extend_from_slice(&((value.len() + 4) as u16).to_le_bytes());
            out.extend_from_slice(&flag.to_le_bytes());
            out.extend_from_slice(value);
        }
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&query::END.to_le_bytes());
    }
}

/// The subset of a record the projection uses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    pub code: Option<u32>,
    pub position: Option<u32>,
    pub level: Option<u32>,
    pub rank: Option<u32>,
    pub attack: Option<i32>,
    pub defense: Option<i32>,
    pub counters: Option<u32>,
    pub is_public: Option<bool>,
}

/// One record; `None` for an empty zone.
pub fn read_record<'a>(r: &mut Reader<'a>) -> Result<Option<RawRecord<'a>>> {
    let mut record = RawRecord::default();
    loop {
        let size = r.u16()? as usize;
        if size == 0 {
            return Ok(if record.fields.is_empty() { None } else { Some(record) });
        }
        let flag = r.u32()?;
        let value = r.bytes(size.saturating_sub(4))?;
        record.fields.push((flag, value));
        if flag == query::END {
            return Ok(Some(record));
        }
    }
}

/// An `OCG_DuelQueryLocation` result: `u32` byte count, then records.
pub fn read_location(data: &[u8]) -> Result<Vec<Option<RawRecord<'_>>>> {
    let mut r = Reader::new(data);
    let size = r.u32()? as usize;
    let mut r = Reader::new(r.bytes(size)?);
    let mut records = Vec::new();
    while r.remaining() > 0 {
        records.push(read_record(&mut r)?);
    }
    Ok(records)
}
