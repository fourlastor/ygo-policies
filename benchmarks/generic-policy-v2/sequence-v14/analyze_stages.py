#!/usr/bin/env python3
"""Validate and analyze one completed fixed sequence stage; never launch jobs."""
import argparse
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
PLAN = HERE / "staged-plan.json"
SHARED = HERE.parents[1] / "generic-policy" / "analyze.py"
FROZEN_REFERENCE = HERE.parent / "manual-weights-chain-burn-v13" / "default.metadata.json"
QUALITY_COUNTERS = ("failed_searches", "failed_playouts", "unresolved_playouts",
                    "incomplete_worlds", "budget_exhausted")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def logical_bytes(path):
    data = Path(path).read_bytes()
    return gzip.decompress(data) if Path(path).suffix == ".gz" else data


def fingerprint(path):
    data = logical_bytes(path)
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def shared_analyzer():
    spec = importlib.util.spec_from_file_location("sequence_shared_analysis", SHARED)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def expected_arguments(plan, stage, name):
    settings = plan["stages"][stage]
    return {**plan["common_arguments"], "--library": plan["artifacts"][name]["path"],
            "--policies": ",".join(settings["policies"]),
            "--plain": str(settings["plain"]).lower(), "--seed": str(settings["seed"]),
            "--games": str(settings["games_per_deck"]),
            "--output": str(HERE / f"{stage}-{name}.jsonl")}


def expected_search(plan, settings):
    args = plan["common_arguments"]
    number = lambda key: int(args["--" + key])
    flag = lambda key: args["--" + key] == "true"
    return {
        "method": args["--method"], "beam": None, "depth": number("depth"),
        "horizon": number("horizon"), "max_candidates": number("max-candidates"),
        # The CLI records its effective zero budget in plain mode.
        "max_playouts": 0 if settings["plain"] else number("max-playouts"),
        "millis": number("millis"), "worlds": number("worlds"), "strict": flag("strict"),
        "validation": "terminal" if number("horizon") == 0 else "heuristic",
        "opponent_model": args["--opponent-model"],
        "evaluation_weights": plan["default_evaluation_weights"],
        "mcts": {"iterations": number("mcts-iterations"), "actions": number("mcts-actions"),
                 "nodes": number("mcts-nodes"), "decisions": number("mcts-decisions"),
                 "replies": number("mcts-replies"), "worlds": number("mcts-worlds"),
                 "terminal": flag("mcts-terminal"), "validate_roots": flag("mcts-validate-roots"),
                 "adversarial_opponent": flag("mcts-adversarial-opponent")},
    }


def invariant_inputs(analysis, metadata):
    context = analysis.pinned_context(metadata)
    for key in ("seed", "games_per_matchup", "planned_jobs", "policies", "decks"):
        context.pop(key)
    return context


def quality(rows, summary):
    games = [row["game"] for row in rows.values()]
    result = {
        "invalid_games": sum(bool(game.get("error")) or game.get("winner") not in (0, 1, 2)
                             or bool(game.get("search_errors")) for game in games),
        "capped_games": sum(game.get("limit") is not False for game in games),
    }
    result["invalid_or_capped_games"] = sum(
        bool(game.get("error")) or game.get("winner") not in (0, 1, 2)
        or bool(game.get("search_errors")) or game.get("limit") is not False for game in games)
    for key in QUALITY_COUNTERS:
        require(all(type(game.get(key)) is int and game[key] >= 0 for game in games),
                f"Missing or invalid required diagnostic counter: {key}")
        result[key] = sum(game[key] for game in games)
        require(type(summary["overall"].get(key)) is int
                and summary["overall"][key] == result[key], f"Stale diagnostic counter: {key}")
    for key in ("failures", "caps"):
        require(type(summary["overall"].get(key)) is int and summary["overall"][key] == 0,
                f"Invalid completed summary: {key}")
    return result


def paired(analysis, control, candidate):
    result = analysis.compare(control, candidate)
    result.update(control_wins=sum(map(analysis.won, control.values())),
                  candidate_wins=sum(map(analysis.won, candidate.values())),
                  net_wins=result["improved"] - result["regressed"])
    return result


def analyze_stage(stage, write=False):
    """Recompute validated evidence and gates; write=True creates a new report."""
    require(stage in ("plain", "demise"), "Unknown stage")
    output = HERE / f"{stage}-analysis.json"
    require(not write or not output.exists(), "Analysis already exists; refusing overwrite")
    inputs, before = {}, {}

    def track(name, path):
        path = Path(path)
        require(path.is_file(), f"Missing required input: {path}")
        inputs[name], before[name] = path, fingerprint(path)
        return path

    def read(name, path):
        return json.loads(track(name, path).read_text())

    plan = read("staged-plan.json", PLAN)
    settings = plan["stages"][stage]
    reference_path = (HERE / plan["fixed_reference_metadata"]).resolve()
    reference = read("fixed_reference_metadata", reference_path)
    frozen_reference = read("frozen_v13_metadata", FROZEN_REFERENCE)
    require(plan["schema"] == 1 and plan["exceptions"] == ["countdown"] and plan["target"] == .55,
            "Unexpected plan schema, exceptions or target")
    require(plan["cpus"] == list(range(8, 24)) and plan["workers_per_arm"] == 8
            and plan["maximum_simultaneous_match_workers"] == 16, "Unexpected worker/CPU plan")
    require(plan["stages"]["plain"]["policies"] == reference["policies"]
            and len(reference["policies"]) == 71
            and plan["stages"]["demise"]["policies"] == ["demise"], "Unexpected stage roster")
    for name, seed in (("plain", 2477000), ("demise", 2478000)):
        config = plan["stages"][name]
        require(config["plain"] is (name == "plain") and config["seed"] == seed
                and config["games_per_deck"] == 64
                and config["planned_pairs"] == 64 * len(config["policies"]), "Unexpected fixed stage schedule")
    require(plan["plain_promotion_gate"] == {
        "required_roster_net_wins_minimum": 1, "demise_net_wins_minimum": 1,
        "required_deck_regression_hold_at_net_losses": 8, "invalid_or_capped_games": 0,
        **dict.fromkeys(QUALITY_COUNTERS, 0)}, "Unexpected plain promotion thresholds")
    require(plan["demise_expansion_recommendation"] == {
        "net_wins_minimum": 4, "automatic_later_jobs": False}, "Unexpected Demise recommendation thresholds")

    prerequisite = None
    if stage == "demise":
        prerequisite = analyze_stage("plain", write=False)
        require(prerequisite["promotion_gate"]["passed"], "Plain promotion gate failed")
        if (HERE / "plain-analysis.json").exists():
            track("plain-analysis.json", HERE / "plain-analysis.json")
        for name, value in prerequisite["input_fingerprints"].items():
            path = Path(prerequisite["input_paths"][name])
            require(fingerprint(path) == value, "Plain evidence changed after gate recomputation")
            track("plain_prerequisite/" + name, path)

    manifest = read(f"{stage}-manifest.json", HERE / f"{stage}-manifest.json")
    exits = read(f"{stage}-exits.json", HERE / f"{stage}-exits.json")
    require(manifest["stage"] == exits["stage"] == stage and manifest["plan"] == plan,
            "Stage manifest differs from fixed plan")
    require(manifest["artifacts"] == exits["artifacts_after"] == plan["artifacts"],
            "Frozen artifacts differ before/after stage")
    require(exits["exit_codes"] == {"control": 0, "candidate": 0}
            and all(type(code) is int for code in exits["exit_codes"].values())
            and exits.get("failure") is None, "Both arms must finish successfully")
    build_path = Path(plan["artifacts"]["candidate"]["path"]).with_name("build.json")
    build = read("candidate_build.json", build_path)
    require(manifest["candidate_build"] == build and build["source_commit"] == plan["candidate_library_source_commit"]
            and all(build[key] == plan["artifacts"]["candidate"][key] for key in ("path", "bytes", "sha256")),
            "Candidate build provenance differs from plan")
    for name, path in (("plan", PLAN), ("runner", HERE / "run_stages.py"), ("analyzer", Path(__file__))):
        track("source/" + name, path)
        require(before["source/" + name] == manifest["source_fingerprints"][name],
                f"Source changed since launch: {name}")
    track("source/shared_analyzer", SHARED)
    analysis = shared_analyzer()
    jobs = {job["name"]: job for job in manifest["jobs"]}
    require(len(manifest["jobs"]) == 2 and set(jobs) == {"control", "candidate"}, "Invalid manifest jobs")
    paths = {}
    # Require both complete fixed-size files and sidecars before scoring either.
    for name in ("control", "candidate"):
        base = HERE / f"{stage}-{name}.jsonl"
        alternatives = [path for path in (base, Path(str(base) + ".gz")) if path.is_file()]
        require(alternatives, f"Missing {name} evidence")
        for index, path in enumerate(alternatives):
            track(f"{stage}-{name}.jsonl" + (".duplicate-gzip" if index else ""), path)
        require(all(logical_bytes(path) == logical_bytes(alternatives[0]) for path in alternatives),
                "Raw and compressed records conflict")
        paths[name] = alternatives[0]
        require(len(logical_bytes(paths[name]).splitlines()) == settings["planned_pairs"],
                f"{name}: incomplete or excessive stage records")
        for suffix in ("metadata.json", "summary.json"):
            track(f"{stage}-{name}.{suffix}", base.with_suffix("." + suffix))
        if base.with_suffix(".status.json").exists():
            track(f"{stage}-{name}.status.json", base.with_suffix(".status.json"))
    runs = {name: analysis.load(path) for name, path in paths.items()}
    diagnostics = {}
    for name, (rows, summary, meta) in runs.items():
        job = jobs[name]
        command = job["command"]
        require(command[:5] == ["taskset", "-c", "8-23", plan["artifacts"]["executable"]["path"], "generic"],
                f"{name}: invalid executable/CPU command")
        flags = command[5:]
        require(len(flags) % 2 == 0 and all(flag.startswith("--") for flag in flags[::2]), "Invalid flag pairs")
        arguments = dict(zip(flags[::2], flags[1::2]))
        require(len(arguments) * 2 == len(flags) and arguments == expected_arguments(plan, stage, name)
                and meta["arguments"] == arguments and job["output"] == arguments["--output"],
                f"{name}: launch/recorded arguments differ from plan")
        require(meta["seed"] == settings["seed"] and meta["games_per_matchup"] == settings["games_per_deck"]
                and meta["planned_jobs"] == len(rows) == settings["planned_pairs"]
                and meta["policies"] == settings["policies"] and meta["workers"] == plan["workers_per_arm"]
                and meta["plain"] is settings["plain"], f"{name}: recorded stage schedule differs")
        require(meta["search"] == expected_search(plan, settings) and meta["effect_profiles"] is True,
                f"{name}: effective settings/weights differ from plan")
        require(meta["checkout"]["commit"] == manifest["launch_checkout"]["commit"],
                f"{name}: live launch checkout mismatch")
        for artifact, field in ((name, "candidate"), ("executable", "executable"), ("baseline", "baseline")):
            require(all(meta[field][key] == plan["artifacts"][artifact][key] for key in ("path", "bytes")),
                    f"{name}: frozen {artifact} path/size mismatch")
        for field in (("candidate", "executable") if name == "control" else ("executable",)):
            require(analysis.file_identity(meta[field], field) == analysis.file_identity(frozen_reference[field], field),
                    f"{name}: recorded frozen v13 {field} fingerprint differs")
        require(invariant_inputs(analysis, meta) == invariant_inputs(analysis, reference),
                f"{name}: pinned inputs differ from pilot-v10 reference")
        require(set(meta["decks"]) == set(settings["policies"]) and all(
            analysis.file_identity(meta["decks"][deck], deck) == analysis.file_identity(reference["decks"][deck], deck)
            for deck in settings["policies"]), f"{name}: deck subset/fingerprints differ from reference")
        diagnostics[name] = quality(rows, summary)
    control, candidate = runs["control"][0], runs["candidate"][0]
    changes = analysis.validate_comparison(runs["control"][2], runs["candidate"][2], allow_candidate_change=True)
    require(changes == ["candidate_library"], "Only the candidate policy library may change")
    require(analysis.configuration_changes(runs["control"][2], runs["candidate"][2]) == [], "Configuration changed between arms")
    per_deck = {deck: paired(analysis, {k: v for k, v in control.items() if k[0] == deck},
                            {k: v for k, v in candidate.items() if k[0] == deck}) for deck in settings["policies"]}
    required = paired(analysis, {k: v for k, v in control.items() if k[0] not in plan["exceptions"]},
                      {k: v for k, v in candidate.items() if k[0] not in plan["exceptions"]})
    required["decks"] = len(set(settings["policies"]) - set(plan["exceptions"]))
    holds = {deck: values["net_wins"] for deck, values in per_deck.items()
             if deck not in plan["exceptions"] and values["net_wins"] <= -8}
    clean = all(all(value == 0 for value in counters.values()) for counters in diagnostics.values())
    checks = {"zero_invalid_caps_failures_unresolved_incomplete_budget": clean}
    if stage == "plain":
        checks.update(required_roster_net_wins_positive=required["net_wins"] >= 1,
                      demise_net_wins_positive=per_deck["demise"]["net_wins"] >= 1,
                      no_required_deck_net_losses_at_least_eight=not holds)
    else:
        checks.update(plain_gate_recomputed_passed=prerequisite["promotion_gate"]["passed"],
                      demise_net_wins_at_least_four=per_deck["demise"]["net_wins"] >= 4)
    gate = {"passed": all(checks.values()), "checks": checks,
            "failed_checks": [key for key, value in checks.items() if not value],
            "required_deck_regression_holds": holds, "automatic_later_jobs": False,
            "meaning": "Permission gate for separate Demise stage" if stage == "plain" else "Recommendation for wider search sentinels only"}
    report = {"stage": stage, "planned_pairs": settings["planned_pairs"], "completed_pairs": len(control),
              "candidate_revision_changes": changes, "configuration_changes": [],
              "overall_including_countdown": paired(analysis, control, candidate),
              "required_roster": required, "countdown": per_deck.get("countdown"), "per_deck": per_deck,
              "quality": diagnostics, "promotion_gate": gate,
              "artifacts": plan["artifacts"], "candidate_build": build,
              "source_commits": {key: plan[key] for key in ("executable_source_commit", "control_library_source_commit", "candidate_library_source_commit")},
              "launch_checkout": manifest["launch_checkout"],
              "arms": {name: {"identity": analysis.identity(meta), "live_checkout": meta["checkout"],
                               "overall": summary["overall"]} for name, (_, summary, meta) in runs.items()},
              "input_fingerprints": before, "input_paths": {name: str(path) for name, path in inputs.items()},
              "fingerprint_scope": "Logical decompressed bytes; both raw and gzip checked when present",
              "artifact_verification": "Frozen SHA256/path/bytes verified by runner before and after; metadata library/host fingerprints also compared across arms and known control reference",
              "interpretation": "Fixed candidate-library comparison. Countdown excluded from the required roster. Concurrent timing is descriptive. No general 55% guarantee or automatic later jobs."}
    if prerequisite is not None:
        report["prerequisite_plain_gate"] = prerequisite["promotion_gate"]
    require(before == {name: fingerprint(path) for name, path in inputs.items()}, "Evidence/source changed during analysis")
    if write:
        with output.open("x") as stream:
            json.dump(report, stream, indent=2, sort_keys=True, allow_nan=False)
            stream.write("\n")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", choices=("plain", "demise"), required=True)
    args = parser.parse_args()
    report = analyze_stage(args.stage, write=True)
    print(json.dumps({"stage": args.stage, "promotion_gate": report["promotion_gate"]}, indent=2))


if __name__ == "__main__":
    main()
