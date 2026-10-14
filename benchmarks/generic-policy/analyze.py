#!/usr/bin/env python3
"""Compare completed generic mirror runs with pinned inputs and matching schedules.

Usage: python3 analyze.py [--allow-candidate-change] PLAIN.jsonl[.gz] SEARCH.jsonl[.gz] ...
Search budgets may differ. Candidate-library or executable changes require the
explicit flag and are reported as revision comparisons, never isolated search gains.
"""
import argparse
import gzip
import json
import math
from pathlib import Path
import statistics


def file_identity(value, label):
    if (not isinstance(value, dict) or value.get("error")
            or not isinstance(value.get("fnv1a64"), str)
            or not isinstance(value.get("bytes"), int)):
        raise ValueError(f"Missing valid fingerprint for {label}")
    return {key: value[key] for key in ("bytes", "fnv1a64")}


def pinned_context(metadata):
    context = {key: metadata[key] for key in (
        "mode", "candidate_policy", "rules", "life_points", "decision_limit",
        "shuffle", "policy_seed", "target", "exceptions",
    )}
    for key in ("baseline", "cards"):
        context[key] = file_identity(metadata[key], key)
    context["decks"] = {
        name: file_identity(value, f"deck {name}")
        for name, value in metadata["decks"].items()
    }
    context["engine"] = ({"linked": True} if metadata["engine"].get("linked") is True
                         else file_identity(metadata["engine"], "engine"))
    for key in ("core", "scripts", "lua"):
        revision = metadata[key]
        if not revision.get("commit") or revision.get("status") != "":
            raise ValueError(f"Cannot verify {key} equivalence without a clean pinned revision")
        context[key] = {"commit": revision["commit"], "status": revision["status"]}
    return context


def identity(metadata):
    return {
        "candidate_library": file_identity(metadata["candidate"], "candidate library"),
        "candidate_executable": file_identity(metadata["executable"], "candidate executable"),
        "fixed_inputs": pinned_context(metadata),
        "plain": metadata["plain"],
        "search": metadata["search"],
        "seed": metadata["seed"],
        "games_per_matchup": metadata["games_per_matchup"],
        "workers": metadata["workers"],
    }


def expected_schedule(metadata):
    expected = set()
    for deck in metadata["policies"]:
        salt = 0xCBF29CE484222325
        for byte in ("generic-mirror\0" + deck).encode():
            salt = ((salt ^ byte) * 0x100000001B3) & ((1 << 64) - 1)
        for index in range(metadata["games_per_matchup"]):
            seed = (metadata["seed"] + salt + index) & 0x7FFFFFFF
            expected.add((deck, seed, index % 2))
    return expected


def load(path):
    path = Path(path)
    uncompressed = path.with_suffix("") if path.suffix == ".gz" else path
    status_path = uncompressed.with_suffix(".status.json")
    if status_path.exists():
        status = json.loads(status_path.read_text())
        if not isinstance(status, dict):
            raise ValueError(f"{path}: invalid evidence status document {status_path}")
        if status.get("valid") is False or status.get("status") not in ("valid", "complete"):
            reason = status.get("reason", status.get("reasons", "No reason recorded"))
            raise ValueError(f"{path}: evidence status {status.get('status', 'unspecified')}: {reason}")
    summary = json.loads(uncompressed.with_suffix(".summary.json").read_text())
    metadata = json.loads(uncompressed.with_suffix(".metadata.json").read_text())
    if metadata["mode"] != "generic" or summary["overall"]["complete"] is not True:
        raise ValueError(f"{path}: benchmark is not valid and complete")
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt") as stream:
        rows = [json.loads(line) for line in stream]
    keyed = {}
    for row in rows:
        key = (row["opponent"], row["seed"], row["seat"])
        game = row["game"]
        if key in keyed:
            raise ValueError(f"{path}: duplicate record {key}")
        if (type(row["seat"]) is not int or row["seat"] not in (0, 1)
                or type(game.get("winner")) is not int or game["winner"] not in (0, 1, 2)
                or game.get("error") or game.get("limit") is not False
                or game.get("failed_playouts", 0) != 0 or game.get("failed_searches", 0) != 0):
            raise ValueError(f"{path}: invalid, capped or failed game/search at {key}")
        if row.get("candidate_policy") != metadata["candidate_policy"] or row.get("plain") != metadata["plain"]:
            raise ValueError(f"{path}: row/metadata candidate configuration differs at {key}")
        keyed[key] = row
    expected = expected_schedule(metadata)
    if (set(keyed) != expected or len(rows) != metadata["planned_jobs"]
            or len(rows) != summary["overall"]["planned"]):
        raise ValueError(f"{path}: missing, unexpected or incorrectly planned records")
    wins = sum(map(won, rows))
    draws = sum(row["game"]["winner"] == 2 for row in rows)
    for key, expected_value in (("completed", len(rows)), ("wins", wins), ("draws", draws),
                                ("losses", len(rows) - wins - draws)):
        if summary["overall"][key] != expected_value:
            raise ValueError(f"{path}: stale summary {key}")
    matchups = {row["opponent"]: row for row in summary["matchups"]}
    if set(matchups) != set(metadata["policies"]) or len(matchups) != len(summary["matchups"]):
        raise ValueError(f"{path}: stale or duplicate per-deck summary")
    for deck, matchup in matchups.items():
        sample = [row for row in rows if row["opponent"] == deck]
        wins = sum(map(won, sample))
        draws = sum(row["game"]["winner"] == 2 for row in sample)
        n = len(sample)
        for key, expected_value in (("completed", n), ("wins", wins), ("draws", draws),
                                    ("losses", n - wins - draws)):
            if matchup[key] != expected_value:
                raise ValueError(f"{path}: stale {deck} summary {key}")
        rate, z2 = wins / n, 1.96 ** 2
        center = (rate + z2 / (2 * n)) / (1 + z2 / n)
        half = 1.96 * math.sqrt(rate * (1 - rate) / n + z2 / (4 * n * n)) / (1 + z2 / n)
        for key, expected_value in (
            ("exception", deck in metadata["exceptions"]),
            ("meets_observed_target", rate >= metadata["target"]),
            ("lower_95_meets_target", center - half >= metadata["target"]),
        ):
            if matchup[key] is not expected_value:
                raise ValueError(f"{path}: stale {deck} summary {key}")
    identity(metadata)  # Validate all fingerprints before using the evidence.
    return keyed, summary, metadata


def won(row):
    return int(row["game"]["winner"] == row["seat"])


def compare(plain, candidate):
    if plain.keys() != candidate.keys():
        raise ValueError("Runs have different deck/seed/seat schedules")
    differences = [won(candidate[key]) - won(plain[key]) for key in plain]
    n = len(differences)
    mean = statistics.mean(differences)
    width = 1.96 * statistics.stdev(differences) / math.sqrt(n) if n > 1 else None
    return {
        "pairs": n,
        "baseline_win_rate": sum(map(won, plain.values())) / n,
        "candidate_win_rate": sum(map(won, candidate.values())) / n,
        "paired_gain": mean,
        "paired_normal_95": [mean - width, mean + width] if width is not None else None,
        "improved": differences.count(1),
        "regressed": differences.count(-1),
        "same_outcome": differences.count(0),
    }


def validate_comparison(plain_metadata, candidate_metadata, allow_candidate_change):
    before, after = identity(plain_metadata), identity(candidate_metadata)
    if before["fixed_inputs"] != after["fixed_inputs"]:
        different = [key for key in before["fixed_inputs"]
                     if before["fixed_inputs"][key] != after["fixed_inputs"].get(key)]
        raise ValueError(f"Runs have different fixed inputs: {', '.join(different)}")
    changed = [key for key in ("candidate_library", "candidate_executable") if before[key] != after[key]]
    if changed and not allow_candidate_change:
        raise ValueError(f"Candidate revision changed ({', '.join(changed)}); use --allow-candidate-change "
                         "only for an explicitly labeled revision comparison")
    return changed


def main(paths, allow_candidate_change=False):
    if len(paths) < 2:
        raise ValueError(__doc__)
    plain, _, plain_metadata = load(paths[0])
    if plain_metadata["plain"] is not True:
        raise ValueError("The reference run must be the plain generic policy")
    results = []
    for path in paths[1:]:
        candidate, summary, metadata = load(path)
        changed = validate_comparison(plain_metadata, metadata, allow_candidate_change)
        result = {
            "candidate": str(path), "identity": identity(metadata),
            "candidate_revision_changes": changed,
            "isolated_search_configuration_comparison": not changed and metadata["plain"] is False,
            "interpretation": ("Candidate revisions differ; the gain is not an isolated search-configuration effect"
                               if changed else "Candidate library and executable match; fixed inputs match"),
            "overall": compare(plain, candidate),
        }
        result["per_deck"] = {
            deck: compare({k: v for k, v in plain.items() if k[0] == deck},
                          {k: v for k, v in candidate.items() if k[0] == deck})
            for deck in sorted({k[0] for k in plain})
        }
        result["required_observed_passes"] = sum(
            not row["exception"] and row["meets_observed_target"] for row in summary["matchups"]
        )
        result["required_confident_passes"] = sum(
            not row["exception"] and row["lower_95_meets_target"] for row in summary["matchups"]
        )
        result["search_latency_ms"] = summary["overall"].get("search_latency_ms")
        results.append(result)
    print(json.dumps({"baseline": str(paths[0]), "baseline_identity": identity(plain_metadata),
                      "allow_candidate_change": allow_candidate_change, "comparisons": results}, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-candidate-change", action="store_true")
    parser.add_argument("paths", nargs="+")
    args = parser.parse_args()
    main(args.paths, args.allow_candidate_change)
