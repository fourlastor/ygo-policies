#!/usr/bin/env python3
"""Analyze only the completed 256-pair fresh Chain Burn weight confirmation.

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
from statistics import NormalDist

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ANALYZER = HERE.parents[1] / "generic-policy" / "analyze.py"
PILOT_MANIFEST = HERE.parent / "manual-weights-chain-burn-v13" / "manifest.json"
PILOT_METADATA = PILOT_MANIFEST.with_name("default.metadata.json")
PILOT_SEEDS = range(1196572947, 1196573011)
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


def validate_launch(name, meta, manifest, plan, pilot_manifest):
    require(manifest["plan"] == plan, "Manifest does not contain the prespecified plan")
    expected_plan = {"deck": "chain-burn", "games_per_arm": 256, "seed": 2475000,
                     "workers_per_arm": 8, "candidate_seats_per_arm": [128, 128],
                     "cpus": list(range(8, 24)), "maximum_simultaneous_match_workers": 16,
                     "weights": {"default": DEFAULT_WEIGHTS, "manual": MANUAL_WEIGHTS}}
    require(all(plan[key] == value for key, value in expected_plan.items()), "Plan differs from fixed diagnostic")
    require(manifest["source_commit"] == pilot_manifest["source_commit"]
            == "5705ae27e15fa7b223f2eeb1833232fb4eb6265f",
            "Frozen executable source differs from the original weight diagnostic")
    require(manifest["artifacts"] == pilot_manifest["artifacts"]
            and manifest["policy_library_source_commit"] == pilot_manifest["policy_library_source_commit"],
            "Frozen executable, policy library or opponent differs from the pilot")
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
    require(meta["checkout"]["commit"] == manifest["launch_checkout_commit"],
            f"{name}: launch checkout differs from manifest (distinct from frozen binary source)")
    for artifact, field in (("executable", "executable"), ("library", "candidate"), ("opponent", "baseline")):
        require(all(meta[field][key] == artifacts[artifact][key] for key in ("path", "bytes")),
                f"{name}: {artifact} path/size differs from frozen manifest")
    expected_weights = DEFAULT_WEIGHTS if name == "default" else MANUAL_WEIGHTS
    require(meta["search"] == {**SEARCH, "evaluation_weights": expected_weights},
            f"{name}: effective weights or search budget differs from fixed plan")
    require(meta["effect_profiles"] is True and meta["decision_limit"] == 8192
            and meta["rules"] == "MasterRule1 (0xD0700)" and meta["life_points"] == [8000, 8000],
            f"{name}: effects/limit/rules/life points differ from fixed plan")


def paired_newcombe(a, b, c, d):
    """Method 10: first marginal minus second, matching the prior diagnostic formula."""
    n = a + b + c + d
    p1, p2 = (a + b) / n, (a + c) / n
    z = NormalDist().inv_cdf(0.975)
    def wilson(wins):
        p = wins / n
        center = (p + z * z / (2 * n)) / (1 + z * z / n)
        radius = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
        return center - radius, center + radius
    low1, high1 = wilson(a + b)
    low2, high2 = wilson(a + c)
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
    # Published example protects the supplement's formula and sign without data.
    require(all(math.isclose(a, b, abs_tol=1e-12) for a, b in zip(
        paired_newcombe(10, 5, 2, 20), [-0.0643926050711, 0.2211160178958])), "Newcombe formula check failed")
    outputs = [HERE / "comparison.json", HERE / "paired-statistics.json"]
    require(not any(path.exists() for path in outputs), "Derived reports already exist; refusing overwrite")
    paths, inputs = {}, {name: HERE / name for name in (
        "manifest.json", "plan.json", "artifact-verification.json")}
    inputs["pilot_manifest.json"] = PILOT_MANIFEST
    inputs["pilot_default.metadata.json"] = PILOT_METADATA
    # Check both arms before loading any outcomes; a partial run is never scored.
    for name in ("default", "manual"):
        paths[name] = evidence_path(name)
        require(len(logical_bytes(paths[name]).splitlines()) == 256, f"{name}: requires exactly 256 rows")
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
    pilot_manifest = json.loads(PILOT_MANIFEST.read_text())
    pilot_metadata = json.loads(PILOT_METADATA.read_text())
    sources = {"diagnostic": fingerprint(Path(__file__)), "repository_analyzer": fingerprint(ANALYZER)}
    spec = importlib.util.spec_from_file_location("generic_analyze", ANALYZER)
    analyze = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(analyze)
    pilot_fixed = analyze.pinned_context(pilot_metadata)
    schedule_fields = {"seed", "games_per_matchup", "planned_jobs"}
    require({key[1] for key in analyze.expected_schedule(pilot_metadata)} == set(PILOT_SEEDS),
            "Prior metadata does not reconstruct the fixed 64 pilot seeds")
    runs = {name: analyze.load(path) for name, path in paths.items()}
    for name, (rows, _, meta) in runs.items():
        validate_launch(name, meta, manifest, plan, pilot_manifest)
        fixed = analyze.pinned_context(meta)
        require({key: value for key, value in fixed.items() if key not in schedule_fields}
                == {key: value for key, value in pilot_fixed.items() if key not in schedule_fields},
                f"{name}: fixed inputs differ from the pilot beyond the intended schedule change")
        for field in ("candidate", "executable"):
            require(analyze.file_identity(meta[field], field)
                    == analyze.file_identity(pilot_metadata[field], f"pilot {field}"),
                    f"{name}: recorded {field} fingerprint differs from the pilot")
        require(len(rows) == meta["planned_jobs"] == meta["games_per_matchup"] == 256,
                f"{name}: expected the complete 256-game schedule")
        require(meta["seed"] == 2475000 and meta["policies"] == ["chain-burn"]
                and meta["workers"] == 8 and meta["plain"] is False,
                f"{name}: differs from planned seed/deck/workers/search mode")
        require(not any(key[1] in PILOT_SEEDS for key in rows), f"{name}: overlap with the 64 pilot seeds")
        require(all(sum(key[2] == seat for key in rows) == 128 for seat in (0, 1)),
                f"{name}: candidate seats must be exactly balanced")
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
    neither_win = 256 - both_win - discordant
    stats = {
        "planned_pairs": 256, "completed_pairs": 256,
        "primary_sample": "Fresh 256 pairs only; the pilot is excluded",
        "pilot_pairs_included": 0, "pilot_seed_overlap": False,
        "fixed_inputs_match_pilot_except_schedule": True,
        "frozen_source_commit": manifest["source_commit"],
        "launch_checkout_commit": manifest["launch_checkout_commit"],
        "candidate_minus_reference": "manual weights minus default weights",
        "paired": paired,
        "paired_interval_method": "Mean paired binary-win difference +/- 1.96 * sample SD / sqrt(256); approximate normal 95% interval",
        "exact_two_sided_mcnemar_p": exact_p,
        "paired_newcombe_95": paired_newcombe(both_win, paired["improved"], paired["regressed"], neither_win),
        "paired_newcombe_method": "Supplement: Newcombe 1998 method 10, marginal Wilson intervals and corrected within-pair phi; z=Phi^-1(0.975)",
        "paired_newcombe_source": "https://www.statsmodels.org/dev/generated/statsmodels.stats.proportion.confint_proportions_paired.html",
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
        "interpretation": "Fresh confirmation of one preset on one preselected weak matchup; not evidence of a 55% per-deck guarantee or general improvement. Concurrent-arm timing is descriptive only.",
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
