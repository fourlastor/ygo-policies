//! `.ydk` deck lists: `#main`, `#extra` and `!side` sections of passcodes.

use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Deck {
    pub main: Vec<u32>,
    pub extra: Vec<u32>,
    pub side: Vec<u32>,
}

pub fn parse(text: &str) -> Deck {
    let mut deck = Deck::default();
    let mut section = 0;
    for line in text.lines().map(str::trim) {
        match line {
            "#main" => section = 0,
            "#extra" => section = 1,
            "!side" => section = 2,
            _ => {
                if let Ok(code) = line.parse::<u32>() {
                    [&mut deck.main, &mut deck.extra, &mut deck.side][section].push(code);
                }
            }
        }
    }
    deck
}

pub fn load(path: &Path) -> std::io::Result<Deck> {
    Ok(parse(&std::fs::read_to_string(path)?))
}

#[cfg(test)]
mod tests {
    #[test]
    fn sections() {
        let deck = super::parse("#created by x\n#main\n1\n2\n#extra\n3\n!side\n4\n");
        assert_eq!((deck.main, deck.extra, deck.side), (vec![1, 2], vec![3], vec![4]));
    }
}
