#!/usr/bin/python3
"""Validate a completed v9 performance run and print latency groups as JSON.

Uses the unchanged measurement runner to revalidate all evidence and reproduce
comparison.json in memory. It launches no games and does not modify input files.
"""
import importlib.util
import copy
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("perf_runner", HERE / "run_perf_v9.py")
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def legacy_report_projection(reconstructed, saved):
    """Remove only newly inferred defaults absent from the original identities."""
    result = copy.deepcopy(reconstructed)
    for deck, variants in saved["identities"].items():
        for variant, old in variants.items():
            current = result["identities"][deck][variant]
            for source, container, field, expected_source, expected in (
                ("search_opponent_model_source", "search", "opponent_model",
                 "inferred_legacy_generic_opponent", "generic"),
                ("mcts_adversarial_opponent_source", "mcts", "adversarial_opponent",
                 "inferred_legacy_viewer_only", False),
            ):
                if source in old:
                    continue
                previous_settings = old["search"] if container == "search" else old["search"]["mcts"]
                current_settings = current["search"] if container == "search" else current["search"]["mcts"]
                if (field in previous_settings or current.get(source) != expected_source
                        or type(current_settings.get(field)) is not type(expected)
                        or current_settings[field] != expected):
                    raise ValueError(f"Unexpected historical identity difference: {source}")
                del current[source]
                del current_settings[field]
    return result


def recorded_counter_totals(rows, counters):
    totals = {}
    for key in counters:
        present = [key in row["game"] for row in rows]
        if not any(present):
            continue  # Older binaries did not measure this counter.
        if not all(present):
            raise ValueError(f"Incomplete historical counter: {key}")
        totals[key] = sum(row["game"][key] for row in rows)
    return totals


def summarize(directory):
    directory = directory.resolve()
    manifest = json.loads((directory / "manifest.json").read_text())
    if runner.sha256(HERE / "run_perf_v9.py") != manifest["runner_sha256"]:
        raise ValueError("Measurement runner source changed since execution")
    comparison = json.loads((directory / "comparison.json").read_text())
    def verify_report(path, reconstructed):
        normalized = legacy_report_projection(json.loads(json.dumps(reconstructed)), comparison)
        if path != directory / "comparison.json" or normalized != comparison:
            raise ValueError("Saved comparison does not reproduce exactly")
    runner.write_new = verify_report
    runner.report(directory)
    events = [json.loads(line) for line in (directory / "events.jsonl").read_text().splitlines()]
    if (events[-1].get("event") != "complete" or not events[-1].get("affinity_cleanup_successful")
            or any(event["event"] in ("failed", "cleanup_failed") for event in events)):
        raise ValueError("Run did not finish successfully with affinity cleanup")
    starts = {event["job"]["output"]: event for event in events if event["event"] == "measurement_start"}
    ends = {event["output"]: event for event in events if event["event"] == "measurement_finish"}
    if len(starts) != 8 or starts.keys() != ends.keys() or any(event["returncode"] for event in ends.values()):
        raise ValueError("Incomplete successful measurement schedule")
    states, rows = {}, {}
    analysis = runner.analysis_module()
    for job in manifest["jobs"]:
        path = job["output"]
        states[(job["deck"], job["variant"])] = {
            "start": starts[path]["utc"], "finish": ends[path]["utc"],
            "background_at_start": starts[path]["background_alive"],
            "background_at_finish": ends[path]["background_alive"],
            "process_seconds": ends[path]["elapsed_seconds"],
        }
        rows[(job["deck"], job["variant"])] = analysis.load(Path(path))[0]
    clean = {deck for deck in runner.DECKS if all(
        not states[(deck, variant)][field]
        for variant in runner.VARIANTS for field in ("background_at_start", "background_at_finish"))}
    groups = {}
    for label, decks in (("all_pairs", set(runner.DECKS)),
                         ("background_affected", set(runner.DECKS) - clean),
                         ("no_benchmark_background", clean)):
        selected = [pair for pair in comparison["pairs"] if pair["key"][0] in decks]
        samples, decisions = {}, {}
        for variant in runner.VARIANTS:
            games = [row for deck in sorted(decks) for row in rows[(deck, variant)].values()]
            samples[variant] = runner.sample_stats([value for row in games for value in row["game"]["search_decision_ms"]])
            decisions[variant] = runner.sample_stats([value for row in games for value in row["game"]["decision_ms"]])
        ratios = [pair["search_total_ratio_optimized_over_control"] for pair in selected]
        groups[label] = {
            "decks": sorted(decks), "game_pairs": len(selected),
            "search_latency_ms": samples, "candidate_decision_latency_ms": decisions,
            "search_total_ratio_optimized_over_control": samples["optimized"]["total"] / samples["control"]["total"],
            "candidate_decision_total_ratio_optimized_over_control": decisions["optimized"]["total"] / decisions["control"]["total"],
            "per_game_search_total_ratios": runner.sample_stats(ratios),
            "per_game_search_total_ratio_min": min(ratios),
            "per_game_search_total_ratio_max": max(ratios),
            "all_game_search_totals_lower": all(ratio < 1 for ratio in ratios),
        }
    control_games = [row for deck in runner.DECKS for row in rows[(deck, "control")].values()]
    return {
        "kind": "descriptive_paired_performance_with_background_groups",
        "source_sha256": {name: runner.sha256(directory / name) for name in ("manifest.json", "events.jsonl", "comparison.json")},
        "frozen_variants": comparison["frozen_variants"],
        "exact_non_timing_parity": comparison["exact_non_timing_parity"],
        "game_pairs": comparison["matched_pairs"],
        "identical_counter_totals_per_variant": recorded_counter_totals(control_games, analysis.TELEMETRY_COUNTERS),
        "groups": groups,
        "per_deck_scheduling": {deck: {variant: states[(deck, variant)] for variant in runner.VARIANTS}
                                for deck in runner.DECKS},
        "interpretation": [
            "Both executable and FFI library changed; generic policy and evaluator sources were unchanged.",
            "Eight fixed game pairs establish parity for this sample, not universal policy equivalence.",
            "The v8 background job ended during optimized Blackwing; its two pairs have asymmetric shared-load exposure.",
            "The remaining six pairs had no competing policy-bench process, but unrelated system activity was not controlled.",
            "Per-decision samples within each game are correlated; no confidence interval or global speed claim is inferred.",
            "This is a performance comparison, not a test of the 55% per-specialist target.",
        ],
    }


if __name__ == "__main__":
    directory = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE / "perf-v9-2b289c6-r2"
    print(json.dumps(summarize(directory), indent=2, allow_nan=False))
