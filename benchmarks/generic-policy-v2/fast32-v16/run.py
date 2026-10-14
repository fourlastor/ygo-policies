#!/usr/bin/env python3
"""One fixed 32-iteration arm. Default prints its command; --execute runs it.

The caller must wait until other host benchmarks finish. This script refuses
visible active policy-bench processes, never resumes paused jobs, and launches
only five workers on CPUs8-23. No builds or automatic later variants.
"""
import argparse
import contextlib
from datetime import datetime, timezone
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SHARED = ROOT / "benchmarks/generic-policy/analyze.py"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_new(name, value):
    with (HERE / name).open("x") as stream:
        json.dump(value, stream, indent=2, sort_keys=True, allow_nan=False)
        stream.write("\n")


def verify_artifacts(plan):
    for name, expected in plan["artifacts"].items():
        path = Path(expected["path"])
        require(path.stat().st_size == expected["bytes"] and digest(path) == expected["sha256"],
                f"Frozen artifact changed: {name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    plan = json.loads((HERE / "plan.json").read_text())
    if not args.execute:
        print(json.dumps(plan["command"], indent=2))
        return
    require(not any((HERE / name).exists() for name in (
        "manifest.json", "exit.json", "comparison.json", "fast32.jsonl", "fast32.jsonl.gz", "fast32.log")),
        "Output exists; refusing overwrite or partial-run resume")
    require(set(plan["cpus"]) <= os.sched_getaffinity(0), "CPUs8-23 unavailable")
    for proc in Path("/proc").glob("[0-9]*"):
        try:
            argv = (proc / "cmdline").read_bytes().split(b"\0")
            if argv and Path(os.fsdecode(argv[0])).name == "policy-bench":
                state = (proc / "stat").read_text().rsplit(")", 1)[1].split()[0]
                require(state in ("T", "t", "Z"), f"Other visible benchmark active: PID{proc.name}")
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
    verify_artifacts(plan)
    reference = plan["reference"]
    for name in ("manifest", "metadata"):
        require(digest(reference[name]) == reference[name + "_sha256"], f"Reference {name} changed")
    previous = json.loads(Path(reference["manifest"]).read_text())
    expected = next(job["command"].copy() for job in previous["jobs"] if job["name"] == "v16")
    expected[expected.index("--mcts-iterations") + 1] = "32"
    expected[expected.index("--output") + 1] = str(HERE / "fast32.jsonl")
    require(plan["command"] == expected, "Only iterations and output may differ from the reference command")
    spec = importlib.util.spec_from_file_location("fast32_shared_analysis", SHARED)
    analysis = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(analysis)
    old_path = Path(reference["path"])
    if not old_path.exists():
        old_path = Path(str(old_path) + ".gz")
    old_rows, _, old_meta = analysis.load(old_path)
    require(len(old_rows) == 64 and old_meta["seed"] == 2482000
            and old_meta["policies"] == ["demise"] and old_meta["workers"] == 5,
            "Unexpected reference schedule")
    sources = {"plan": digest(HERE / "plan.json"), "runner": digest(Path(__file__)), "analyzer": digest(SHARED)}
    write_new("manifest.json", {"plan": plan, "source_sha256": sources,
        "launch_checkout": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "started_utc": datetime.now(timezone.utc).isoformat(),
        "process_check_scope": "Visible /proc only; caller confirms no concurrent host benchmarks"})
    with (HERE / "fast32.log").open("x") as log:
        result = subprocess.run(plan["command"], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    verify_artifacts(plan)
    write_new("exit.json", {"returncode": result.returncode, "artifacts_after_verified": True,
                            "finished_utc": datetime.now(timezone.utc).isoformat()})
    require(result.returncode == 0, "Benchmark exited unsuccessfully; see fast32.log")
    require(sources == {"plan": digest(HERE / "plan.json"), "runner": digest(Path(__file__)), "analyzer": digest(SHARED)},
            "Analysis or plan changed during the run")
    _, _, new_meta = analysis.load(HERE / "fast32.jsonl")
    require(new_meta["arguments"] == dict(zip(expected[5::2], expected[6::2])), "Recorded launch flags differ")
    require(analysis.configuration_changes(old_meta, new_meta) == plan["expected_configuration_changes"],
            "Unexpected effective configuration change")
    capture = io.StringIO()
    with contextlib.redirect_stdout(capture):
        analysis.main([str(old_path), str(HERE / "fast32.jsonl")],
                      allow_metadata_ablation=["search.mcts.iterations"])
    comparison = json.loads(capture.getvalue())
    write_new("comparison.json", comparison)
    print(json.dumps(comparison["comparisons"][0]["overall"], indent=2))


if __name__ == "__main__":
    main()
