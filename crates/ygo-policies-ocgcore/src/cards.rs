//! `cards.cdb` (the OCGCore card database) as a [`CardDatabase`].

use std::collections::HashMap;
use std::path::Path;

use ygo_policies::cards::{CardData, CardDatabase};

pub struct SqliteCards {
    cards: HashMap<u32, CardData>,
}

impl SqliteCards {
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let db = rusqlite::Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mut statement =
            db.prepare("select id, alias, type, atk, def, level, race, attribute, setcode from datas")?;
        let rows = statement.query_map([], |row| {
            let setcode: i64 = row.get(8)?;
            let setcodes = (0..4)
                .map(|i| ((setcode as u64 >> (16 * i)) & 0xffff) as u16)
                .filter(|s| *s != 0)
                .collect();
            Ok(CardData {
                code: row.get::<_, i64>(0)? as u32,
                alias: row.get::<_, i64>(1)? as u32,
                kind: row.get::<_, i64>(2)? as u32,
                attack: (row.get::<_, i64>(3)? as i32).max(0),
                defense: (row.get::<_, i64>(4)? as i32).max(0),
                level: (row.get::<_, i64>(5)? as u32) & 0xff,
                race: row.get::<_, i64>(6)? as u32,
                attribute: row.get::<_, i64>(7)? as u32,
                setcodes,
            })
        })?;
        let mut cards = HashMap::new();
        for card in rows {
            let card = card?;
            cards.insert(card.code, card);
        }
        Ok(SqliteCards { cards })
    }
}

impl CardDatabase for SqliteCards {
    fn card(&self, code: u32) -> Option<&CardData> {
        self.cards.get(&code)
    }

    fn all(&self) -> Box<dyn Iterator<Item = &CardData> + '_> {
        Box::new(self.cards.values())
    }
}
