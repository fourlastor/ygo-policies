use std::collections::HashMap;

#[test]
fn exodia_list_respects_wc2011_pool_and_limits() {
    let limits: HashMap<u32, usize> = include_str!("../../../data/wc2011.lflist.conf")
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            Some((words.next()?.parse().ok()?, words.next()?.parse().ok()?))
        })
        .collect();
    let mut piles = [Vec::new(), Vec::new(), Vec::new()];
    let mut pile = 0;
    for line in include_str!("../../../decks/Astra's Exodia.ydk").lines() {
        match line.trim() {
            "#main" => pile = 0,
            "#extra" => pile = 1,
            "!side" => pile = 2,
            value => if let Ok(code) = value.parse::<u32>() { piles[pile].push(code); },
        }
    }
    assert_eq!(piles[0].len(), 40);
    assert!(piles[1].len() <= 15 && piles[2].len() <= 15);
    let mut counts = HashMap::new();
    for &code in piles.iter().flatten() {
        let limit = limits.get(&code).expect("card must belong to the WC2011 pool");
        let count = counts.entry(code).or_insert(0);
        *count += 1;
        assert!(*count <= *limit, "card {code} exceeds its WC2011 limit");
    }
    for code in [33396948, 70903634, 7902349, 8124921, 44519536] {
        assert_eq!(piles[0].iter().filter(|&&c| c == code).count(), 1);
    }
}
