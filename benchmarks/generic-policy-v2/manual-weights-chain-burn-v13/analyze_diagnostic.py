#!/usr/bin/env python3
"""Analyze only the completed 64-pair Chain Burn manual-weight diagnostic.

Run after completion is confirmed. Accepts default/manual.jsonl or .jsonl.gz,
requires final metadata/summaries, and refuses to overwrite derived reports.
No matches are launched and no input evidence is changed.
"""
import contextlib
import gzip
import hashlib
import importlib.util
import io
import json
import math
from pathlib import Path
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ANALYZER = HERE.parents[1] / "generic-policy" / "analyze.py"
DEFAULT_WEIGHTS = {
    "accessible_cards": 0.8, "stranded_hand": 0.5, "body_power": 0.2,
    "dominant_attack": 0.45, "reusable_grave": 0.6, "interaction": 0.8,
    "effect_access": 0.45, "battle_card_gain": 0.6, "battle_damage": 0.45,
    "immediate_lethal": 2.5, "life_buffer": 0.55, "deck_reserve": 0.7,
    "alternate_win_progress": 1.0,
}
MANUAL_WEIGHTS = {**DEFAULT_WEIGHTS, "accessible_cards": 0.3, "life_buffer": 4.0}
SEARCH = {
    "method": "mcts", "beam": None, "depth": 1, "horizon": 1,
    "max_candidates": 8, "max_playouts": 256, "millis": 0, "worlds": 4,
    "strict": True, "validation": "heuristic", "opponent_model": "generic",
    "mcts": {"actions": 8, "adversarial_opponent": False, "decisions": 384,
             "iterations": 128, "nodes": 1024, "replies": 1, "terminal": False,
             "validate_roots": False, "worlds": 4},
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def logical_bytes(path):
    data = path.read_bytes()
    return gzip.decompress(data) if path.suffix == ".gz" else data


def fingerprint(path):
    data = logical_bytes(path)
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def evidence_path(name):
    raw, archive = HERE / f"{name}.jsonl", HERE / f"{name}.jsonl.gz"
    if raw.is_file():
        if archive.is_file():
            require(logical_bytes(raw) == logical_bytes(archive), f"{name}: conflicting raw/archive")
        return raw
    require(archive.is_file(), f"{name}: missing raw or compressed evidence")
    return archive


def validate_launch(name, meta, manifest, plan):
    require(manifest["plan"] == plan, "Manifest does not contain the prespecified plan")
    expected_plan = {"deck": "chain-burn", "games_per_arm": 64, "seed": 2474000,
                     "workers_per_arm": 8, "candidate_seats_per_arm": [32, 32],
                     "cpus": list(range(8, 24)), "maximum_simultaneous_match_workers": 16,
                     "weights": {"default": DEFAULT_WEIGHTS, "manual": MANUAL_WEIGHTS}}
    require(all(plan[key] == value for key, value in expected_plan.items()), "Plan differs from fixed diagnostic")
    jobs = {job["name"]: job for job in manifest["jobs"]}
    require(len(manifest["jobs"]) == 2 and set(jobs) == {"default", "manual"}, "Invalid manifest jobs")
    job, artifacts = jobs[name], manifest["artifacts"]
    command = job["command"]
    require(command[:5] == ["taskset", "-c", "8-23", artifacts["executable"]["path"], "generic"],
            f"{name}: unexpected launch executable/CPU binding")
    flags = command[5:]
    require(len(flags) % 2 == 0 and all(flag.startswith("--") for flag in flags[::2]),
            f"{name}: invalid manifest argument pairs")
    arguments = dict(zip(flags[::2], flags[1::2]))
    require(len(arguments) * 2 == len(flags) and meta["arguments"] == arguments,
            f"{name}: recorded arguments differ from manifest command")
    require(job["output"] == arguments["--output"] == str(HERE / f"{name}.jsonl"),
            f"{name}: unexpected output identity")
    require(meta["checkout"]["commit"] == manifest["source_commit"],
            f"{name}: source revision differs from frozen manifest")
    for artifact, field in (("executable", "executable"), ("library", "candidate"), ("opponent", "baseline")):
        require(all(meta[field][key] == artifacts[artifact][key] for key in ("path", "bytes")),
                f"{name}: {artifact} path/size differs from frozen manifest")
    expected_weights = DEFAULT_WEIGHTS if name == "default" else MANUAL_WEIGHTS
    require(meta["search"] == {**SEARCH, "evaluation_weights": expected_weights},
            f"{name}: effective weights or search budget differs from fixed plan")
    require(meta["effect_profiles"] is True and meta["decision_limit"] == 8192
            and meta["rules"] == "MasterRule1 (0xD0700)" and meta["life_points"] == [8000, 8000],
            f"{name}: effects/limit/rules/life points differ from fixed plan")


def main():
    outputs = [HERE / "comparison.json", HERE / "paired-statistics.json"]
    require(not any(path.exists() for path in outputs), "Derived reports already exist; refusing overwrite")
    paths, inputs = {}, {name: HERE / name for name in (
        "manifest.json", "plan.json", "artifact-verification.json")}
    # Check both arms before loading any outcomes; a partial run is never scored.
    for name in ("default", "manual"):
        paths[name] = evidence_path(name)
        require(len(logical_bytes(paths[name]).splitlines()) == 64, f"{name}: requires exactly 64 rows")
        inputs[f"{name}.jsonl"] = paths[name]
        for suffix in ("metadata", "summary"):
            inputs[f"{name}.{suffix}.json"] = HERE / f"{name}.{suffix}.json"
        status = HERE / f"{name}.status.json"
        if status.exists():
            inputs[status.name] = status
    require(all(path.is_file() for path in inputs.values()), "Final manifest/plan/metadata/summary missing")
    before = {name: fingerprint(path) for name, path in inputs.items()}
    manifest = json.loads(inputs["manifest.json"].read_text())
    plan = json.loads(inputs["plan.json"].read_text())
    sources = {"diagnostic": fingerprint(Path(__file__)), "repository_analyzer": fingerprint(ANALYZER)}
    spec = importlib.util.spec_from_file_location("generic_analyze", ANALYZER)
    analyze = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(analyze)
    runs = {name: analyze.load(path) for name, path in paths.items()}
    for name, (rows, _, meta) in runs.items():
        validate_launch(name, meta, manifest, plan)
        require(len(rows) == meta["planned_jobs"] == meta["games_per_matchup"] == 64,
                f"{name}: expected the complete 64-game schedule")
        require(meta["seed"] == 2474000 and meta["policies"] == ["chain-burn"]
                and meta["workers"] == 8 and meta["plain"] is False,
                f"{name}: differs from planned seed/deck/workers/search mode")
    # This enforces identical candidate/executable, fixed inputs and exact seeds.
    capture = io.StringIO()
    with contextlib.redirect_stdout(capture):
        analyze.main([str(paths["default"]), str(paths["manual"])],
                     allow_metadata_ablation=["search.evaluation_weights"])
    comparison = json.loads(capture.getvalue())
    result = comparison["comparisons"][0]
    expected_changes = {"search.evaluation_weights.accessible_cards", "search.evaluation_weights.life_buffer"}
    require(result["isolated_metadata_ablation"] is True
            and {change["field"] for change in result["configuration_changes"]} == expected_changes,
            "Comparison must isolate exactly the two planned weight changes")
    default, manual = runs["default"][0], runs["manual"][0]
    paired = result["overall"]
    discordant = paired["improved"] + paired["regressed"]
    smaller = min(paired["improved"], paired["regressed"])
    exact_p = min(1.0, 2 * sum(math.comb(discordant, k) for k in range(smaller + 1))
                  / 2 ** discordant) if discordant else 1.0
    both_win = sum(analyze.won(default[key]) and analyze.won(manual[key]) for key in default)
    neither_win = 64 - both_win - discordant
    stats = {
        "planned_pairs": 64, "completed_pairs": 64,
        "candidate_minus_reference": "manual weights minus default weights",
        "paired": paired,
        "paired_interval_method": "Mean paired binary-win difference +/- 1.96 * sample SD / sqrt(64); approximate normal 95% interval",
        "exact_two_sided_mcnemar_p": exact_p,
        "paired_table": {"rows": ["manual win", "manual non-win"],
                         "columns": ["default win", "default non-win"],
                         "counts": [[both_win, paired["improved"]], [paired["regressed"], neither_win]]},
        "draws_count_as_wins": False,
        "all_planned_games_required_before_analysis": True,
        "arms": {name: {"identity": analyze.identity(meta), "overall": summary["overall"],
                        "wall_seconds": summary.get("wall_seconds")}
                 for name, (_, summary, meta) in runs.items()},
        "input_fingerprints": before,
        "input_fingerprint_scope": "Logical decompressed bytes; raw and gzip must agree if both exist",
        "analysis_source": sources,
        "interpretation": "One preselected weak matchup and one manually chosen two-weight change; not evidence of a 55% per-deck guarantee or general improvement. Concurrent-arm timing is descriptive only.",
    }
    require(before == {name: fingerprint(path) for name, path in inputs.items()},
            "Input evidence changed during analysis")
    require(sources == {"diagnostic": fingerprint(Path(__file__)), "repository_analyzer": fingerprint(ANALYZER)},
            "Analysis code changed during analysis")
    for path, value in zip(outputs, (comparison, stats)):
        with path.open("x") as stream:
            stream.write(json.dumps(value, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"paired": paired, "exact_two_sided_mcnemar_p": exact_p}, indent=2))


if __name__ == "__main__":
    main()
