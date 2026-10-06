use std::collections::HashMap;

#[test]
fn harpie_list_respects_wc2011_pool_and_shared_name_limit() {
    let limits: HashMap<u32, usize> = include_str!("../../../data/wc2011.lflist.conf")
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            Some((words.next()?.parse().ok()?, words.next()?.parse().ok()?))
        })
        .collect();
    let mut piles = [Vec::new(), Vec::new(), Vec::new()];
    let mut pile = 0;
    for line in include_str!("../../../decks/Harpie Sisters.ydk").lines() {
        match line.trim() {
            "#main" => pile = 0,
            "#extra" => pile = 1,
            "!side" => pile = 2,
            value => if let Ok(code) = value.parse::<u32>() { piles[pile].push(code); },
        }
    }
    assert!((40..=60).contains(&piles[0].len()));
    assert!(piles[1].len() <= 15 && piles[2].len() <= 15);
    let mut counts = HashMap::new();
    for &code in piles.iter().flatten() {
        let limit = limits.get(&code).expect("card must belong to the WC2011 pool");
        let count = counts.entry(code).or_insert(0);
        *count += 1;
        assert!(*count <= *limit, "card {code} exceeds its WC2011 limit");
    }
    // These names always count as Harpie Lady, including during deck building.
    // Queen changes name only on the field/in the GY, so has its own limit.
    // https://www.db.yugioh-card.com/yugiohdb/faq_search.action?ope=4&cid=6186&request_locale=ja
    let ladies: usize = [76812113, 80316585, 91932350, 27927359, 54415063]
        .iter().map(|code| counts.get(code).copied().unwrap_or(0)).sum();
    assert!(ladies <= 3, "Harpie Lady variants share a three-copy limit");
}
