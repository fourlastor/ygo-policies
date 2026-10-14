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

/// Read only the projection's fields, without allocating an intermediate
/// `RawRecord`. Keep the raw reader's framing and first-occurrence semantics,
/// including short scalar fields and a zero-size terminator after fields.
pub(crate) fn read_query(r: &mut Reader<'_>) -> Result<Option<Query>> {
    let mut record = Query::default();
    let mut any_field = false;
    let mut seen = 0u32;
    loop {
        let size = r.u16()? as usize;
        if size == 0 {
            return Ok(any_field.then_some(record));
        }
        let flag = r.u32()?;
        let value = r.bytes(size.saturating_sub(4))?;
        any_field = true;
        if flag == query::END {
            return Ok(Some(record));
        }
        // Unknown flags, including combinations of recognized flag bits, do
        // not consume the first occurrence of any recognized scalar field.
        if !matches!(flag, query::CODE | query::POSITION | query::LEVEL | query::RANK
            | query::ATTACK | query::DEFENSE | query::COUNTERS | query::IS_PUBLIC)
            || seen & flag != 0
        { continue; }
        seen |= flag;
        let scalar = || value.get(..4).map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()));
        match flag {
            query::CODE => record.code = scalar(),
            query::POSITION => record.position = scalar(),
            query::LEVEL => record.level = scalar(),
            query::RANK => record.rank = scalar(),
            query::ATTACK => record.attack = scalar().map(|value| value as i32),
            query::DEFENSE => record.defense = scalar().map(|value| value as i32),
            query::IS_PUBLIC => record.is_public = Some(value.first().is_some_and(|value| *value != 0)),
            query::COUNTERS => {
                let mut counters = Reader::new(value);
                let count = counters.u32().unwrap_or(0) as usize;
                // The raw parser ignores missing counter entries. Stop once
                // no full entry remains instead of repeating failed reads.
                record.counters = Some((0..count.min(counters.remaining() / 4))
                    .map(|_| counters.u32().unwrap() >> 16).sum());
            }
            _ => unreachable!(),
        }
    }
}

/// The typed counterpart of `read_location`; only the final pile allocates.
pub(crate) fn read_query_location(data: &[u8]) -> Result<Vec<Option<Query>>> {
    let mut r = Reader::new(data);
    let size = r.u32()? as usize;
    let mut r = Reader::new(r.bytes(size)?);
    let mut records = Vec::new();
    while r.remaining() > 0 {
        records.push(read_query(&mut r)?);
    }
    Ok(records)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn field(out: &mut Vec<u8>, flag: u32, value: &[u8]) {
        out.extend_from_slice(&((value.len() + 4) as u16).to_le_bytes());
        out.extend_from_slice(&flag.to_le_bytes());
        out.extend_from_slice(value);
    }

    // The unchanged raw APIs are the pre-optimization parser, retained only
    // as the typed parser's differential reference in these tests.
    fn reference(r: &mut Reader<'_>) -> Result<Option<Query>> {
        Ok(read_record(r)?.map(|record| record.parse()))
    }

    fn assert_record_parity(bytes: &[u8]) {
        let mut streamed = Reader::new(bytes);
        let mut legacy = Reader::new(bytes);
        assert_eq!(read_query(&mut streamed), reference(&mut legacy), "record {bytes:?}");
        assert_eq!(streamed.position(), legacy.position(), "cursor for {bytes:?}");
    }

    fn assert_location_parity(bytes: &[u8]) {
        let legacy = read_location(bytes).map(|records| {
            records.into_iter().map(|record| record.map(|record| record.parse())).collect::<Vec<_>>()
        });
        assert_eq!(read_query_location(bytes), legacy, "location {bytes:?}");
    }

    #[test]
    fn typed_queries_preserve_first_fields_and_partial_values() {
        let mut record = Vec::new();
        field(&mut record, query::CODE | query::POSITION, &99u32.to_le_bytes());
        field(&mut record, query::CODE, &[1, 2, 3]); // Present but short still wins.
        field(&mut record, query::CODE, &123u32.to_le_bytes());
        field(&mut record, query::POSITION, &8u32.to_le_bytes());
        field(&mut record, query::POSITION, &1u32.to_le_bytes());
        field(&mut record, query::ATTACK, &(-2i32).to_le_bytes());
        field(&mut record, query::DEFENSE, &(-1i32).to_le_bytes());
        field(&mut record, query::IS_PUBLIC, &[]);
        field(&mut record, query::IS_PUBLIC, &[1]);
        let counters: Vec<_> = [3u32, (2 << 16) | 9, (5 << 16) | 7]
            .into_iter().flat_map(u32::to_le_bytes).chain([42, 43]).collect();
        field(&mut record, query::COUNTERS, &counters);
        field(&mut record, query::COUNTERS, &0u32.to_le_bytes());
        field(&mut record, query::END, &[11, 12]);
        record.extend_from_slice(&[99, 100]); // Not part of this record.
        assert_record_parity(&record);
        assert_eq!(read_query(&mut Reader::new(&record)).unwrap(), Some(Query {
            code: None, position: Some(8), attack: Some(-2), defense: Some(-1),
            counters: Some(7), is_public: Some(false), ..Query::default()
        }));

        for first in [vec![], vec![1], vec![1, 2, 3]] {
            let mut record = Vec::new();
            field(&mut record, query::COUNTERS, &first);
            field(&mut record, query::COUNTERS, &counters);
            record.extend_from_slice(&0u16.to_le_bytes());
            assert_record_parity(&record);
            assert_eq!(read_query(&mut Reader::new(&record)).unwrap().unwrap().counters, Some(0));
        }
    }

    #[test]
    fn typed_queries_preserve_empty_records_and_large_counter_counts() {
        assert_eq!(read_query(&mut Reader::new(&[0, 0])).unwrap(), None);
        let mut end_only = Vec::new();
        field(&mut end_only, query::END, &[]);
        let mut unknown_only = Vec::new();
        field(&mut unknown_only, 0x4000_0000, &[1, 2]);
        unknown_only.extend_from_slice(&[0, 0]);
        for record in [end_only, unknown_only] {
            assert_record_parity(&record);
            assert_eq!(read_query(&mut Reader::new(&record)).unwrap(), Some(Query::default()));
        }
        for (count, expected) in [(0, 0), (u32::MAX, 7)] {
            let counters: Vec<_> = [count, (2 << 16) | 9, (5 << 16) | 7]
                .into_iter().flat_map(u32::to_le_bytes).chain([42, 43]).collect();
            let mut record = Vec::new();
            field(&mut record, query::COUNTERS, &counters);
            field(&mut record, query::END, &[]);
            // A huge declaration previously repeated failed reads billions
            // of times. Its value is still just the complete entries present;
            // do not run that legacy loop as part of an ordinary unit test.
            assert_eq!(read_query(&mut Reader::new(&record)).unwrap().unwrap().counters, Some(expected));
            if count == 0 { assert_record_parity(&record); }
        }
    }

    #[test]
    fn typed_queries_match_legacy_framing_and_exact_errors() {
        let mut record = Vec::new();
        field(&mut record, query::CODE, &123u32.to_le_bytes());
        field(&mut record, 0x4000_0000, &[7, 8, 9]);
        field(&mut record, query::END, &[]);
        for end in 0..=record.len() { assert_record_parity(&record[..end]); }
        for size in 0u16..=5 {
            let mut short = size.to_le_bytes().to_vec();
            short.extend_from_slice(&query::CODE.to_le_bytes());
            short.push(7);
            short.extend_from_slice(&0u16.to_le_bytes());
            assert_record_parity(&short);
        }
        let mut pile = vec![0, 0]; // Empty zone before a populated one.
        pile.extend_from_slice(&record);
        pile.extend_from_slice(&[0, 0]);
        let mut location = (pile.len() as u32).to_le_bytes().to_vec();
        location.extend_from_slice(&pile);
        for end in 0..=location.len() { assert_location_parity(&location[..end]); }
        location.extend_from_slice(&[0xff; 7]); // Outside the declared pile length.
        assert_location_parity(&location);
        for size in [0, 1, 2, 3, pile.len() as u32 - 1, u32::MAX] {
            location[..4].copy_from_slice(&size.to_le_bytes());
            assert_location_parity(&location);
        }
    }

    #[test]
    fn typed_queries_match_deterministic_malformed_corpus() {
        let mut random = 0x811c_9dc5u32;
        let mut next = || {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            random
        };
        let flags = [query::CODE, query::POSITION, query::LEVEL, query::RANK,
            query::ATTACK, query::DEFENSE, query::COUNTERS, query::IS_PUBLIC,
            query::ALIAS, query::CODE | query::POSITION, 0, 0x4000_0000];
        for case in 0..512 {
            let mut record = Vec::new();
            for _ in 0..1 + next() % 6 {
                let flag = flags[next() as usize % flags.len()];
                let mut payload: Vec<_> = (0..next() % 13).map(|_| next() as u8).collect();
                if flag == query::COUNTERS && payload.len() >= 4 {
                    // Bound the old parser's deliberately forgiving loop;
                    // declared counts still include truncated-entry cases.
                    payload[..4].copy_from_slice(&(next() % 7).to_le_bytes());
                }
                field(&mut record, flag, &payload);
            }
            if case % 2 == 0 { field(&mut record, query::END, &[]); }
            else { record.extend_from_slice(&0u16.to_le_bytes()); }
            for end in 0..=record.len() { assert_record_parity(&record[..end]); }
            let declared = if case % 3 == 0 { next() % (record.len() as u32 + 3) }
                else { record.len() as u32 };
            let mut location = declared.to_le_bytes().to_vec();
            location.extend_from_slice(&record);
            assert_location_parity(&location);
            // Unstructured frames mostly fail in framing. Compare the exact
            // error and cursor, rather than merely accepting two failures.
            let random_bytes: Vec<_> = (0..next() % 65).map(|_| next() as u8).collect();
            assert_record_parity(&random_bytes);
            assert_location_parity(&random_bytes);
        }
    }
}
