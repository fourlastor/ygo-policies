#!/usr/bin/env python3
"""Audit only the completed, preregistered Demise opponent-model diagnostic.

Run from any directory. Inputs are never modified. Requires all 64+64+4 rows
and their final metadata/summaries; refuses partial runs before scoring them.
"""
import contextlib
import copy
import gzip
import hashlib
import importlib.util
import io
import json
import math
from pathlib import Path
from statistics import NormalDist
import sys

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
ANALYZER = HERE.parents[1] / "generic-policy" / "analyze.py"
COUNTERS = ("mcts_opponent_nodes", "mcts_opponent_actions", "mcts_opponent_tree_hits")
LATENCY_VALUES = ("max", "p50", "p95", "total")


def logical_path(path):
    return path.with_suffix("") if path.suffix == ".gz" else path


def logical_bytes(path):
    data = path.read_bytes()
    return gzip.decompress(data) if path.suffix == ".gz" else data


def evidence_path(directory, name):
    """Prefer raw JSONL; accept its gzip archive without changing logical hashes."""
    raw = directory / f"{name}.jsonl"
    archive = directory / f"{name}.jsonl.gz"
    if raw.is_file():
        if archive.is_file():
            assert logical_bytes(raw) == logical_bytes(archive), f"Conflicting raw/archive: {name}"
        return raw
    assert archive.is_file(), f"Missing raw or compressed evidence: {directory.name}/{name}"
    return archive


def fingerprint(path):
    data = logical_bytes(path)
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def write_json(name, value):
    (HERE / name).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def numeric_times(values):
    assert isinstance(values, list)
    assert all(type(v) in (int, float) and math.isfinite(v) and v >= 0 for v in values)
    # Retain sample counts and positions: only elapsed values may differ.
    return [None] * len(values)


def normalize_parity(row, current):
    row = copy.deepcopy(row)
    assert type(row.pop("seconds")) in (int, float)
    game = row["game"]
    for key in ("decision_ms", "search_decision_ms"):
        game[key] = numeric_times(game[key])
    game["decision_ms_by_seat"] = [numeric_times(v) for v in game["decision_ms_by_seat"]]
    for key in ("latency_ms", "search_latency_ms"):
        assert set(game[key]) == {"count", *LATENCY_VALUES}
        for metric in LATENCY_VALUES:
            value = game[key].pop(metric)
            assert type(value) in (int, float) and math.isfinite(value) and value >= 0
    if current:
        assert game.pop("search_opponent_model") == "generic"
        for key in COUNTERS:
            assert game.pop(key) == 0
    else:
        assert "search_opponent_model" not in game
        assert all(key not in game for key in COUNTERS)
    return row


def differences(before, after, path=""):
    if type(before) is not type(after):
        return [path]
    if isinstance(before, dict):
        result = [path + "." + key for key in sorted(before.keys() ^ after.keys())]
        for key in sorted(before.keys() & after.keys()):
            result.extend(differences(before[key], after[key], path + "." + key))
        return result
    if isinstance(before, list):
        if len(before) != len(after):
            return [path + ".length"]
        return [p for i, (a, b) in enumerate(zip(before, after))
                for p in differences(a, b, f"{path}[{i}]")]
    return [] if before == after else [path]


def wilson(wins, n, z=1.96):
    p = wins / n
    center = (p + z * z / (2 * n)) / (1 + z * z / n)
    radius = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
    return [center - radius, center + radius]


def paired_newcombe(a, b, c, d):
    """Newcombe method 10, first marginal minus second (paired outcomes).

    Formula: statsmodels.stats.proportion.confint_proportions_paired, official
    development documentation consulted 2026-10-14. Keep repository normal CI
    separately; this score-based interval accounts for within-pair correlation.
    """
    n = a + b + c + d
    p1, p2 = (a + b) / n, (a + c) / n
    z = NormalDist().inv_cdf(0.975)
    low1, high1 = wilson(a + b, n, z)
    low2, high2 = wilson(a + c, n, z)
    marginals = (a + b) * (c + d) * (a + c) * (b + d)
    covariance = a * d - b * c
    if covariance > 0:
        covariance = max(0, covariance - n / 2)
    phi = covariance / math.sqrt(marginals) if marginals else 0
    def length(left, right):
        return math.sqrt(left * left + right * right - 2 * phi * left * right)
    return [p1 - p2 - length(p1 - low1, high2 - p2),
            p1 - p2 + length(high1 - p1, p2 - low2)]


def main():
    # Refuse partial evidence before loading or calculating any outcome totals.
    inputs = [HERE / "manifest.json", HERE / "default-parity-plan.json"]
    for name, count in (("generic", 64), ("specialist", 64), ("default-parity-v11", 4)):
        raw = evidence_path(HERE, name)
        assert len(logical_bytes(raw).splitlines()) == count, f"{name}: requires {count} rows"
        paths = [raw, HERE / f"{name}.metadata.json", HERE / f"{name}.summary.json"]
        assert all(path.is_file() for path in paths), f"{name}: final metadata/summary missing"
        inputs.extend(paths)
    before_fingerprints = {logical_path(p).name: fingerprint(p) for p in inputs}
    spec = importlib.util.spec_from_file_location("generic_analyze", ANALYZER)
    analyze = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(analyze)
    runs = {name: analyze.load(evidence_path(HERE, name))
            for name in ("generic", "specialist", "default-parity-v11")}
    baseline, _, baseline_meta = runs["generic"]
    candidate, _, candidate_meta = runs["specialist"]
    assert baseline_meta["workers"] == candidate_meta["workers"] == 4
    assert baseline_meta["seed"] == candidate_meta["seed"] == 2472000
    assert baseline_meta["policies"] == candidate_meta["policies"] == ["demise"]
    # Use the repository validator with the one declared metadata ablation.
    capture = io.StringIO()
    with contextlib.redirect_stdout(capture):
        analyze.main([str(evidence_path(HERE, "generic")), str(evidence_path(HERE, "specialist"))],
                     allow_metadata_ablation=["search.opponent_model"])
    comparison = json.loads(capture.getvalue())
    result = comparison["comparisons"][0]
    assert result["isolated_metadata_ablation"] is True
    assert [v["field"] for v in result["configuration_changes"]] == ["search.opponent_model"]

    old, _, _ = runs["default-parity-v11"]
    expected_first_four = sorted(baseline)[:4]
    assert sorted(old) == expected_first_four
    parity = {"passed": True, "pairs": [], "excluded_elapsed_values": [
        "seconds", "game.decision_ms[*]", "game.decision_ms_by_seat[*][*]",
        "game.search_decision_ms[*]", *[f"game.{group}.{metric}"
            for group in ("latency_ms", "search_latency_ms") for metric in LATENCY_VALUES]],
        "allowed_additive_fields": {"game.search_opponent_model": "generic",
                                    **{f"game.{key}": 0 for key in COUNTERS}},
        "sample_counts_latency_counts_snapshot_copy_counts_and_all_other_fields_retained": True}
    for key in expected_first_four:
        diff = differences(normalize_parity(old[key], False), normalize_parity(baseline[key], True))
        parity["pairs"].append({"key": list(key), "equal": not diff, "different_paths": diff})
        parity["passed"] &= not diff
    byte_fields = {"max_snapshot_bytes", "restored_bytes", "snapshotted_bytes"}
    # Separate diagnostic, not an exclusion or a change to the strict result.
    parity["behavior_diagnostic"] = {
        "scope": "All strict-retained fields other than native snapshot byte-size telemetry",
        "matched_games": sum(all(path.startswith(".game.mcts_snapshot_copies[")
                                  and path.rsplit(".", 1)[-1] in byte_fields
                                  for path in pair["different_paths"])
                             for pair in parity["pairs"]),
        "total_games": 4,
        "does_not_override_strict_parity_result": True,
    }
    parity["snapshot_byte_differences"] = []
    for key in expected_first_four:
        for index, (left, right) in enumerate(zip(old[key]["game"]["mcts_snapshot_copies"],
                                                 baseline[key]["game"]["mcts_snapshot_copies"])):
            if left != right:
                parity["snapshot_byte_differences"].append({"key": list(key), "search_index": index,
                    "v11": left, "v12": right, "delta": {field: right[field] - left[field] for field in left}})
    write_json("default-parity.json", parity)
    # A failed cross-version parity control must remain visible. It does not
    # change the independently validated same-binary two-arm comparison.

    paired = result["overall"]
    discordant = paired["improved"] + paired["regressed"]
    smaller = min(paired["improved"], paired["regressed"])
    exact_p = min(1.0, 2 * sum(math.comb(discordant, k) for k in range(smaller + 1))
                  / 2 ** discordant) if discordant else 1.0
    both_win = sum(analyze.won(baseline[key]) and analyze.won(candidate[key]) for key in baseline)
    neither_win = len(baseline) - both_win - discordant
    table = [[both_win, paired["improved"]], [paired["regressed"], neither_win]]
    # The official implementation's published example protects sign/formula.
    example = paired_newcombe(10, 5, 2, 20)
    assert math.isclose(example[0], -0.0643926050711, abs_tol=1e-12)
    assert math.isclose(example[1], 0.2211160178958, abs_tol=1e-12)
    newcombe = paired_newcombe(*table[0], *table[1])
    reverse = paired_newcombe(both_win, paired["regressed"], paired["improved"], neither_win)
    assert all(math.isclose(x, -y, abs_tol=1e-12) for x, y in zip(newcombe, reversed(reverse)))
    stats = {"planned_pairs": 64, "completed_pairs": len(baseline),
        "candidate_minus_reference": "specialist model minus generic model",
        "paired": paired, "exact_two_sided_discordant_binomial_p": exact_p,
        "paired_interval_method": "mean of paired binary-win differences +/- 1.96 * sample SD / sqrt(64); approximate normal 95% interval",
        "paired_newcombe_95": newcombe,
        "paired_newcombe_method": "Newcombe 1998 method 10; marginal Wilson intervals and corrected within-pair phi; z=Phi^-1(0.975)",
        "paired_newcombe_source": "https://www.statsmodels.org/dev/generated/statsmodels.stats.proportion.confint_proportions_paired.html",
        "paired_table": {"rows": ["specialist win", "specialist non-win"],
                         "columns": ["generic win", "generic non-win"], "counts": table},
        "arm_interval_method": "Wilson 95%, z=1.96; wins / all 64 planned games",
        "all_planned_games_required_before_analysis": True,
        "strict_default_parity_passed": parity["passed"], "arms": {}}
    for name in ("generic", "specialist"):
        rows, summary, _ = runs[name]
        overall = summary["overall"]
        wins = sum(analyze.won(row) for row in rows.values())
        times = [ms for row in rows.values() for ms in row["game"]["search_decision_ms"]]
        arm = {"wins": wins, "games": len(rows), "win_rate": wins / len(rows),
               "wilson_95": wilson(wins, len(rows)), "searches_over_200_ms": sum(ms > 200 for ms in times),
               "search_samples": len(times), "wall_seconds": summary["wall_seconds"]}
        for key in ("draws", "failures", "caps", "failed_searches", "failed_playouts", "searched",
                    "mcts_simulations", "mcts_completed", "mcts_cutoffs", "mcts_steps", "mcts_accepted",
                    "mcts_live_hits", "mcts_live_missing", "playouts", "truncated_playouts",
                    "unresolved_playouts", "incomplete_worlds", "budget_exhausted", "soft_budget_overruns",
                    "facing_set", "unsafe_selections", "decision_latency_ms", "search_latency_ms", *COUNTERS):
            arm[key] = overall[key]
        assert arm["mcts_simulations"] == arm["searched"] * 128
        assert arm["mcts_completed"] + arm["mcts_cutoffs"] == arm["mcts_simulations"]
        stats["arms"][name] = arm
    after_fingerprints = {logical_path(p).name: fingerprint(p) for p in inputs}
    assert before_fingerprints == after_fingerprints, "Input changed during analysis"
    stats["input_fingerprints"] = before_fingerprints
    stats["input_fingerprint_scope"] = "Logical decompressed bytes; accepts raw .jsonl or .jsonl.gz, requiring equality if both exist"
    stats["analysis_source"] = {"diagnostic": fingerprint(Path(__file__)),
                                "repository_analyzer": fingerprint(ANALYZER)}
    write_json("comparison.json", comparison)
    write_json("paired-statistics.json", stats)
    print(json.dumps({"parity_passed": parity["passed"], "paired": paired,
                      "exact_two_sided_p": exact_p, "arms": stats["arms"]}, indent=2))


if __name__ == "__main__":
    main()
