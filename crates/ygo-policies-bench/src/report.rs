//! Statistics from recorded results. Errors are never counted as losses/draws.
use crate::engine::Result;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub fn comparison(rows: &[Value], output: &Path) -> Result<()> {
    let failures = rows
        .iter()
        .filter(|r| r["baseline"]["error"].is_string() || r["candidate"]["error"].is_string())
        .count();
    let valid: Vec<_> = rows
        .iter()
        .filter(|r| r["baseline"]["score"].is_number() && r["candidate"]["score"].is_number())
        .collect();
    let pairs: BTreeSet<_> = rows
        .iter()
        .map(|r| (r["a"].as_str().unwrap(), r["b"].as_str().unwrap()))
        .collect();
    let matchups: Vec<_> = pairs
        .iter()
        .map(|(a, b)| {
            let sample: Vec<_> = valid
                .iter()
                .copied()
                .filter(|r| r["a"] == *a && r["b"] == *b)
                .collect();
            let mut result = paired(&sample);
            result["policy"] = json!(a);
            result["opponent"] = json!(b);
            result
        })
        .collect();
    let summary = json!({"pairs": rows.len(), "failures": failures, "overall": paired(&valid), "matchups": matchups});
    save(output, &summary)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&summary["overall"]).unwrap()
    );
    if failures > 0 {
        return Err(format!("{failures} pairs failed; results are incomplete"));
    }
    Ok(())
}

fn paired(rows: &[&Value]) -> Value {
    if rows.is_empty() {
        return json!({"pairs": 0});
    }
    let n = rows.len() as f64;
    let before: f64 = rows
        .iter()
        .map(|r| r["baseline"]["score"].as_f64().unwrap())
        .sum();
    let after: f64 = rows
        .iter()
        .map(|r| r["candidate"]["score"].as_f64().unwrap())
        .sum();
    let deltas: Vec<_> = rows
        .iter()
        .map(|r| {
            r["candidate"]["score"].as_f64().unwrap() - r["baseline"]["score"].as_f64().unwrap()
        })
        .collect();
    let delta = (after - before) / n;
    let se = if n > 1.0 {
        (deltas.iter().map(|d| (d - delta).powi(2)).sum::<f64>() / (n * (n - 1.0))).sqrt()
    } else {
        0.0
    };
    json!({"pairs": rows.len(), "baseline": before / n, "candidate": after / n,
        "delta": delta, "paired_95_half_width": 1.96 * se,
        "improved": deltas.iter().filter(|d| **d > 0.0).count(), "regressed": deltas.iter().filter(|d| **d < 0.0).count(),
        "limits": rows.iter().filter(|r| r["baseline"]["limit"] == true || r["candidate"]["limit"] == true).count(),
        "identical": rows.iter().filter(|r| r["baseline"]["digest"] == r["candidate"]["digest"]).count()})
}

#[derive(Default, Clone, Copy)]
struct Score {
    points: f64,
    games: usize,
}
impl Score {
    fn rate(self) -> Option<f64> {
        (self.games > 0).then(|| self.points / self.games as f64)
    }
    fn add(&mut self, score: f64) {
        self.points += score;
        self.games += 1;
    }
    fn interval(self) -> Option<[f64; 2]> {
        let p = self.rate()?;
        let n = self.games as f64;
        let z2 = 1.96f64.powi(2);
        let center = (p + z2 / (2.0 * n)) / (1.0 + z2 / n);
        let half = 1.96 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / (1.0 + z2 / n);
        Some([center - half, center + half])
    }
}

/// Bradley-Terry MM fit with half a win per side per observed edge. This weak
/// symmetric prior makes undefeated/defeated schedules finite. Unplayed edges
/// add no evidence. Ratings are centered at 1500 within the measured pool.
fn ratings(scores: &[Vec<Score>]) -> Vec<f64> {
    let n = scores.len();
    let mut ability = vec![1.0; n];
    for _ in 0..10000 {
        let next: Vec<f64> = (0..n)
            .map(|i| {
                let wins: f64 = (0..n)
                    .filter(|j| scores[i][*j].games > 0)
                    .map(|j| scores[i][j].points + 0.5)
                    .sum();
                let denom: f64 = (0..n)
                    .filter(|j| scores[i][*j].games > 0)
                    .map(|j| (scores[i][j].games as f64 + 1.0) / (ability[i] + ability[j]))
                    .sum();
                if denom > 0.0 {
                    wins / denom
                } else {
                    1.0
                }
            })
            .collect();
        let mean = next.iter().map(|a| a.ln()).sum::<f64>() / n as f64;
        let next: Vec<_> = next.iter().map(|a| a / mean.exp()).collect();
        let change = next
            .iter()
            .zip(&ability)
            .map(|(a, b)| (a.ln() - b.ln()).abs())
            .fold(0.0, f64::max);
        ability = next;
        if change < 1e-9 {
            break;
        }
    }
    ability
        .into_iter()
        .map(|a| 1500.0 + 400.0 * a.log10())
        .collect()
}

pub fn ranking(
    rows: &[Value],
    metadata: &Value,
    output: &Path,
    markdown: Option<&Path>,
) -> Result<()> {
    if metadata["mode"] == "compare" {
        return Err("Paired comparisons are not round-robin ranking evidence".into());
    }
    let planned = metadata["planned_jobs"]
        .as_u64()
        .ok_or("Missing planned job count")? as usize;
    if rows.len() != planned {
        return Err(format!(
            "Incomplete run: {}/{} games; refusing to publish ranking",
            rows.len(),
            planned
        ));
    }
    let mut seen = BTreeSet::new();
    for row in rows {
        let a = row["a"].as_str().ok_or("Missing policy a")?;
        let b = row["b"].as_str().ok_or("Missing policy b")?;
        let seed = row["seed"].as_u64().ok_or("Missing seed")?;
        let seat = row["seat"].as_u64().ok_or("Missing seat")?;
        if a == b || seat > 1 || !seen.insert((a, b, seed, seat)) {
            return Err("Invalid or duplicate game record".into());
        }
        let score = row["game"]["score"]
            .as_f64()
            .ok_or("Failed or missing game: refusing to publish ranking")?;
        if ![0.0, 0.5, 1.0].contains(&score) || row["game"]["error"].is_string() {
            return Err("Invalid/failed game record".into());
        }
    }
    let names: Vec<_> = rows
        .iter()
        .flat_map(|r| [r["a"].as_str().unwrap(), r["b"].as_str().unwrap()])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let indices: BTreeMap<_, _> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (*name, i))
        .collect();
    let mut scores = vec![vec![Score::default(); names.len()]; names.len()];
    for row in rows {
        let a = indices[row["a"].as_str().unwrap()];
        let b = indices[row["b"].as_str().unwrap()];
        let score = row["game"]["score"].as_f64().unwrap();
        scores[a][b].add(score);
        scores[b][a].add(1.0 - score);
    }
    let elo = ratings(&scores);
    let reference: Vec<_> = metadata["reference"]
        .as_array()
        .ok_or("Missing reference pool")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let mut order: Vec<_> = (0..names.len()).collect();
    order.sort_by(|a, b| elo[*b].total_cmp(&elo[*a]).then(names[*a].cmp(names[*b])));
    let mut ranking = Vec::new();
    let mut matrix = BTreeMap::new();
    for &i in &order {
        let mut all = Score::default();
        let mut existing = Score::default();
        let mut covered = 0;
        let mut row = BTreeMap::new();
        for (j, name) in names.iter().enumerate() {
            all.points += scores[i][j].points;
            all.games += scores[i][j].games;
            if reference.contains(name) && scores[i][j].games > 0 {
                existing.points += scores[i][j].points;
                existing.games += scores[i][j].games;
                covered += 1;
            }
            row.insert(
                *name,
                json!({"games": scores[i][j].games, "rate": scores[i][j].rate()}),
            );
        }
        matrix.insert(names[i], row);
        let needed = reference.iter().filter(|name| **name != names[i]).count();
        let complete = covered == needed && needed > 0;
        let tier = if complete {
            existing.rate().map(|rate| {
                if rate < 0.447 {
                    "weak"
                } else if rate <= 0.646 {
                    "mid"
                } else {
                    "strong"
                }
            })
        } else {
            None
        };
        ranking.push(json!({"policy": names[i], "deck": metadata["catalog"][names[i]], "elo": elo[i],
            "all": {"rate": all.rate(), "games": all.games, "ci95": all.interval()}, "reference": {"rate": existing.rate(), "games": existing.games, "ci95": existing.interval(),
            "opponents": covered, "required_opponents": needed}, "tier": tier}));
    }
    let limits = rows.iter().filter(|r| r["game"]["limit"] == true).count();
    let summary = json!({"games": rows.len(), "failures": 0, "limits": limits, "ranking": ranking, "matrix": matrix});
    save(output, &summary)?;
    if let Some(path) = markdown {
        let mut text = format!("# Deck tier list\n\nGenerated by `policy-bench` from {} completed games ({} decision-limit draws).\n\n\
All games start at 8000 LP under Master Rule 1. Seats alternate. Draws count as half a win. Brackets show Wilson 95% intervals (draws use fractional wins).\n\
The JSONL and adjacent metadata record seeds, deck/library/database fingerprints and source revisions.\n\
Ratings are Bradley–Terry fits with a half-win prior on each observed matchup, centered at 1500 in this pool.\n\
Tiers use the reference-pool win rate: weak below 44.7%, mid through 64.6%, strong above.\n\
A tier is shown only when every required reference opponent was played. Unplayed matchups are blank.\n\n\
Reference pool: {}.\n\n\
| # | Deck | Policy | Elo | vs reference | vs measured pool | Tier |\n\
|---:|---|---|---:|---:|---:|---|\n", rows.len(), limits, reference.join(", "));
        for (rank, row) in summary["ranking"].as_array().unwrap().iter().enumerate() {
            let rate = |v: &Value| {
                v["rate"].as_f64().map_or("—".into(), |r| {
                    format!(
                        "{:.1}% [{:.1}, {:.1}]",
                        100.0 * r,
                        100.0 * v["ci95"][0].as_f64().unwrap(),
                        100.0 * v["ci95"][1].as_f64().unwrap()
                    )
                })
            };
            text += &format!(
                "| {} | {} | `{}` | {:.0} | {} (n={}) | {} (n={}) | {} |\n",
                rank + 1,
                row["deck"].as_str().unwrap_or(""),
                row["policy"].as_str().unwrap(),
                row["elo"].as_f64().unwrap(),
                rate(&row["reference"]),
                row["reference"]["games"],
                rate(&row["all"]),
                row["all"]["games"],
                row["tier"].as_str().unwrap_or("unrated")
            );
        }
        text += "\n## Win-rate matrix\n\nRow policy's win rate against the column policy, in percent.\n\n| Policy |";
        for &i in &order {
            text += &format!(" {} |", names[i]);
        }
        text += "\n|---|";
        for _ in &order {
            text += "---:|";
        }
        text += "\n";
        for &i in &order {
            text += &format!("| **{}** |", names[i]);
            for &j in &order {
                text += &scores[i][j]
                    .rate()
                    .map_or(" — |".into(), |r| format!(" {:.1} |", 100.0 * r));
            }
            text += "\n";
        }
        std::fs::write(path, text).map_err(|e| e.to_string())?;
    }
    println!(
        "{} games, {} policies, {} limits; {}",
        rows.len(),
        names.len(),
        limits,
        output.display()
    );
    Ok(())
}

fn save(path: &Path, value: &Value) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paired_statistics_use_within_seed_changes() {
        let rows: Vec<_> = [(1., 1.), (0., 1.), (1., 0.), (0., 1.)]
            .into_iter()
            .map(|(a, b)| json!({"baseline":{"score":a}, "candidate":{"score":b}}))
            .collect();
        let result = paired(&rows.iter().collect::<Vec<_>>());
        assert_eq!(result["delta"], 0.25);
        assert_eq!(result["improved"], 2);
        assert_eq!(result["regressed"], 1);
        assert!(
            (result["paired_95_half_width"].as_f64().unwrap() - 1.96 * (2.75f64 / 12.).sqrt())
                .abs()
                < 1e-10
        );
    }
    #[test]
    fn bradley_terry_is_centered_and_respects_results() {
        let mut scores = vec![vec![Score::default(); 2]; 2];
        scores[0][1] = Score {
            points: 8.,
            games: 10,
        };
        scores[1][0] = Score {
            points: 2.,
            games: 10,
        };
        let elo = ratings(&scores);
        assert!((elo[0] + elo[1] - 3000.).abs() < 1e-6);
        assert!((elo[0] - elo[1] - 400. * (8.5f64 / 2.5).log10()).abs() < 1e-6);
    }
    #[test]
    fn failed_or_incomplete_runs_cannot_publish_a_tier_list() {
        let meta = json!({"mode":"round-robin", "planned_jobs":2});
        assert!(ranking(&[], &meta, Path::new("unused"), None).is_err());
        let meta = json!({"mode":"round-robin", "planned_jobs":1});
        let row = json!({"a":"a", "b":"b", "seed":1, "seat":0, "game":{"error":"script failure"}});
        assert!(ranking(&[row], &meta, Path::new("unused"), None).is_err());
    }
}
