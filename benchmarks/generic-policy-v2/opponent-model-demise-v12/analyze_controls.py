#!/usr/bin/env python3
"""Explanatory controls added after the original paired diagnostic completed.

Keeps these post hoc comparisons separate from the original 64-pair primary
test. Does not read the separately running adversarial-opponent experiment.
"""
import contextlib
import importlib.util
import io
import json
import math
from pathlib import Path
import sys

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def main():
    helpers = module("diagnostic", HERE / "analyze_diagnostic.py")
    analyze = module("generic_analyze", helpers.ANALYZER)
    paths = [HERE / "plain-plan.json", HERE / "allocation-replay-plan.json"]
    for name, count in (("plain", 64), ("allocation-replay", 4), ("generic", 64), ("specialist", 64)):
        raw = helpers.evidence_path(HERE, name)
        assert len(helpers.logical_bytes(raw).splitlines()) == count
        paths.append(raw)
        paths.extend(HERE / f"{name}.{suffix}" for suffix in ("metadata.json", "summary.json"))
    fingerprints = {helpers.logical_path(p).name: helpers.fingerprint(p) for p in paths}
    runs = {name: analyze.load(helpers.evidence_path(HERE, name))
            for name in ("plain", "allocation-replay", "generic", "specialist")}
    plain, plain_summary, plain_meta = runs["plain"]
    secondary = {"scope": "Explanatory controls added after the original two-arm result; no multiplicity adjustment",
                 "plain_overall": plain_summary["overall"], "comparisons": []}
    for name in ("generic", "specialist"):
        candidate, _, _ = runs[name]
        capture = io.StringIO()
        with contextlib.redirect_stdout(capture):
            analyze.main([str(helpers.evidence_path(HERE, "plain")), str(helpers.evidence_path(HERE, name))],
                         allow_metadata_ablation=["search.opponent_model"] if name == "specialist" else [])
        result = json.loads(capture.getvalue())["comparisons"][0]
        paired = result["overall"]
        b, c = paired["improved"], paired["regressed"]
        a = sum(analyze.won(plain[key]) and analyze.won(candidate[key]) for key in plain)
        d = len(plain) - a - b - c
        result["paired_table"] = {"rows": [f"{name} win", f"{name} non-win"],
                                  "columns": ["plain win", "plain non-win"], "counts": [[a, b], [c, d]]}
        result["paired_newcombe_95"] = helpers.paired_newcombe(a, b, c, d)
        result["exact_two_sided_discordant_binomial_p"] = (
            min(1.0, 2 * sum(math.comb(b + c, k) for k in range(min(b, c) + 1)) / 2 ** (b + c))
            if b + c else 1.0)
        secondary["comparisons"].append(result)

    original, _, original_meta = runs["generic"]
    replay, _, replay_meta = runs["allocation-replay"]
    assert sorted(replay) == sorted(original)[:4]
    assert analyze.recorded_configuration(original_meta) == analyze.recorded_configuration(replay_meta)
    for field in ("candidate", "executable"):
        assert analyze.file_identity(original_meta[field], field) == analyze.file_identity(replay_meta[field], field)
    fixed_before, fixed_after = analyze.pinned_context(original_meta), analyze.pinned_context(replay_meta)
    for field in ("planned_jobs", "games_per_matchup"):
        assert fixed_before.pop(field) == 64 and fixed_after.pop(field) == 4
    assert fixed_before == fixed_after
    audit = {"same_executable_library_configuration_and_fixed_inputs": True,
             "planned_games_before": 64, "planned_games_replay": 4,
             "workers_before": original_meta["workers"], "workers_replay": replay_meta["workers"],
             "strict_non_timing_parity": True, "pairs": [], "byte_differences": [],
             "excluded_elapsed_values": json.loads((HERE / "default-parity.json").read_text())["excluded_elapsed_values"],
             "additive_fields_ignored": [],
             "note": "Both builds have the four v12 fields; their exact required values and presence are checked on both sides."}
    for key in sorted(replay):
        diff = helpers.differences(helpers.normalize_parity(original[key], True),
                                   helpers.normalize_parity(replay[key], True))
        audit["pairs"].append({"key": list(key), "equal": not diff, "different_paths": diff})
        audit["strict_non_timing_parity"] &= not diff
        for index, (left, right) in enumerate(zip(original[key]["game"]["mcts_snapshot_copies"],
                                                 replay[key]["game"]["mcts_snapshot_copies"])):
            if left != right:
                audit["byte_differences"].append({"key": list(key), "search_index": index,
                    "original": left, "replay": right, "delta": {field: right[field] - left[field] for field in left}})
    byte_fields = {"max_snapshot_bytes", "restored_bytes", "snapshotted_bytes"}
    audit["behavior_diagnostic_matched_games"] = sum(
        all(p.startswith(".game.mcts_snapshot_copies[") and p.rsplit(".", 1)[-1] in byte_fields
            for p in pair["different_paths"]) for pair in audit["pairs"])
    audit["same_build_snapshot_byte_variability_observed"] = bool(audit["byte_differences"])
    assert fingerprints == {helpers.logical_path(p).name: helpers.fingerprint(p) for p in paths}
    secondary["input_fingerprints"] = fingerprints
    secondary["input_fingerprint_scope"] = "Logical decompressed bytes; raw or gzip accepted with equality required if both exist"
    secondary["analysis_source"] = helpers.fingerprint(Path(__file__))
    audit["input_fingerprints"] = fingerprints
    helpers.write_json("secondary-comparisons.json", secondary)
    helpers.write_json("allocation-replay-audit.json", audit)
    print(json.dumps({"secondary": [{"candidate": c["candidate"], "paired": c["overall"],
                        "paired_newcombe_95": c["paired_newcombe_95"],
                        "exact_p": c["exact_two_sided_discordant_binomial_p"]} for c in secondary["comparisons"]],
                      "allocation_strict_parity": audit["strict_non_timing_parity"],
                      "allocation_behavior_matches": audit["behavior_diagnostic_matched_games"],
                      "allocation_changed_snapshot_entries": len(audit["byte_differences"])}, indent=2))


if __name__ == "__main__":
    main()
