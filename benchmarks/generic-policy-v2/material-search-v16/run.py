#!/usr/bin/env python3
"""Run one prespecified fixed-sample ablation using immutable FFI artifacts."""
import argparse
from contextlib import ExitStack
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SHARED = HERE.parents[1] / "generic-policy" / "analyze.py"


def fingerprint(path):
    path = Path(path)
    return {"path": str(path), "bytes": path.stat().st_size,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def write_new(path, value):
    with path.open("x") as stream:
        json.dump(value, stream, indent=2, sort_keys=True, allow_nan=False)
        stream.write("\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage", choices=("plain", "search", "sentinels", "latency"))
    args = parser.parse_args()
    plan = json.loads((HERE / "plan.json").read_text())
    stage = plan["stages"][args.stage]
    assert not list(HERE.glob(args.stage + "-*")), "Refusing overwrite/resume"
    assert stage["workers"] * (1 if stage.get("serial") else len(stage["arms"])) <= 16
    if args.stage != "plain":
        previous = json.loads((HERE / "plain-analysis.json").read_text())
        assert previous["gate_passed"], "Plain screen did not pass"
    if args.stage == "sentinels":
        assert json.loads((HERE / "search-analysis.json").read_text())["gate_passed"]
    artifacts = {key: fingerprint(plan[key]["path"]) for key in ("executable", "baseline")}
    assert all(artifacts[key] == plan[key] for key in artifacts)
    builds = {}
    for name in stage["arms"]:
        expected = plan["arms"][name]
        builds[name] = json.loads(Path(expected["path"]).with_name("build.json").read_text())
        actual = fingerprint(expected["path"])
        assert all(builds[name][key] == value for key, value in actual.items())
        assert builds[name]["source_commit"].startswith(expected["source_commit"])
        assert all(expected[key] == actual[key] for key in ("bytes", "sha256") if key in expected)
        artifacts[name] = actual
    jobs = []
    for name in stage["arms"]:
        output = HERE / f"{args.stage}-{name}.jsonl"
        flags = {**plan["common_arguments"], "--library": artifacts[name]["path"],
                 "--policies": ",".join(stage["policies"]), "--seed": str(stage["seed"]),
                 "--games": str(stage["games"]), "--workers": str(stage["workers"]),
                 "--plain": str(stage["plain"]).lower(), "--output": str(output)}
        command = ["taskset", "-c", plan["cpus"], artifacts["executable"]["path"], "generic"]
        command += [item for pair in flags.items() for item in pair]
        jobs.append({"name": name, "command": command, "arguments": flags, "output": str(output)})
    manifest = {"plan": plan, "stage": args.stage, "jobs": jobs, "artifacts": artifacts,
                "builds": builds, "started_utc": datetime.now(timezone.utc).isoformat(),
                "launch_checkout": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                "source_fingerprints": {key: fingerprint(path) for key, path in
                                        (("plan", HERE / "plan.json"), ("runner", Path(__file__)), ("analyzer", SHARED))}}
    write_new(HERE / f"{args.stage}-manifest.json", manifest)
    children, exits = {}, {}
    try:
        with ExitStack() as stack:
            for job in jobs:
                log = stack.enter_context((HERE / f"{args.stage}-{job['name']}.log").open("x"))
                child = subprocess.Popen(job["command"], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
                children[job["name"]] = child
                if stage.get("serial"):
                    exits[job["name"]] = child.wait()
            exits = {name: child.wait() for name, child in children.items()}
    finally:
        for child in children.values():
            if child.poll() is None:
                child.terminate()
        for child in children.values():
            child.wait()
    after = {name: fingerprint(value["path"]) for name, value in artifacts.items()}
    write_new(HERE / f"{args.stage}-exits.json", {"codes": exits, "artifacts_after": after,
                                               "finished_utc": datetime.now(timezone.utc).isoformat()})
    assert artifacts == after and all(code == 0 for code in exits.values())
    spec = importlib.util.spec_from_file_location("shared_analysis", SHARED)
    analyze = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(analyze)
    runs = {job["name"]: analyze.load(Path(job["output"])) for job in jobs}
    expected_pairs = len(stage["policies"]) * stage["games"]
    quality, identities = {}, {}
    for job in jobs:
        name = job["name"]
        rows, summary, meta = runs[name]
        assert len(rows) == expected_pairs and meta["arguments"] == job["arguments"]
        assert meta["search"]["evaluation_weights"] == plan["weights"]
        assert meta["checkout"]["commit"] == manifest["launch_checkout"]
        for key, artifact in (("executable", "executable"), ("candidate", name), ("baseline", "baseline")):
            assert all(meta[key][field] == artifacts[artifact][field] for field in ("path", "bytes"))
        quality[name] = {key: summary["overall"][key] for key in
                         ("failures", "caps", "failed_searches", "failed_playouts", "mcts_cutoffs",
                          "unresolved_playouts", "incomplete_worlds", "budget_exhausted")}
        assert not any(row["game"].get("search_errors") for row in rows.values())
        identities[name] = analyze.identity(meta)
    comparisons = {}
    for left, right in zip(stage["arms"], stage["arms"][1:]):
        a, b = runs[left], runs[right]
        assert analyze.validate_comparison(a[2], b[2], allow_candidate_change=True) == ["candidate_library"]
        assert analyze.configuration_changes(a[2], b[2]) == []
        per_deck = {}
        for deck in stage["policies"]:
            first = {key: value for key, value in a[0].items() if key[0] == deck}
            second = {key: value for key, value in b[0].items() if key[0] == deck}
            result = analyze.compare(first, second)
            result.update(control_wins=sum(map(analyze.won, first.values())),
                          candidate_wins=sum(map(analyze.won, second.values())))
            per_deck[deck] = result
        comparisons[left + "_to_" + right] = {"overall": analyze.compare(a[0], b[0]), "per_deck": per_deck}
    clean = all(not any(value.values()) for value in quality.values())
    regressions = [(pair, deck) for pair, comparison in comparisons.items()
                   for deck, result in comparison["per_deck"].items()
                   if deck != "countdown" and result["regressed"] - result["improved"] >= 8]
    assert manifest["source_fingerprints"] == {key: fingerprint(value["path"])
                                               for key, value in manifest["source_fingerprints"].items()}
    report = {"stage": args.stage, "pairs_per_comparison": expected_pairs, "comparisons": comparisons,
              "quality": quality, "identities": identities, "regression_holds": regressions,
              "gate_passed": clean and (args.stage != "plain" or not regressions),
              "overall_by_arm": {name: run[1]["overall"] for name, run in runs.items()},
              "scope": plan["interpretation"][args.stage]}
    write_new(HERE / f"{args.stage}-analysis.json", report)
    print(json.dumps({"stage": args.stage, "gate_passed": report["gate_passed"],
                      "comparisons": {key: value["overall"] for key, value in comparisons.items()}}, indent=2))


if __name__ == "__main__":
    main()
