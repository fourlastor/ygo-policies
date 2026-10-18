#!/usr/bin/env python3
"""Print or execute one fixed sequence-v14 stage; never build or overwrite runs.

Use --stage plain --execute, then --stage demise --execute after reviewing the
plain gate. Each stage launches exactly two eight-worker arms on CPUs 8-23.
The caller must ensure no other host benchmarks run; visible active benchmarks
are rejected, but /proc visibility may be namespace-limited. Paused jobs stay paused.
"""
import argparse
from contextlib import ExitStack
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PLAN = HERE / "staged-plan.json"
ANALYZER = HERE / "analyze_stages.py"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def fingerprint(path):
    path = Path(path)
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return {"bytes": path.stat().st_size, "sha256": digest.hexdigest()}


def write_new(path, value):
    with path.open("x") as stream:
        json.dump(value, stream, indent=2, sort_keys=True, allow_nan=False)
        stream.write("\n")


def verify_artifacts(plan):
    verified = {}
    for name, expected in plan["artifacts"].items():
        actual = {"path": expected["path"], **fingerprint(expected["path"])}
        require(actual == expected, f"Frozen {name} artifact differs from plan")
        verified[name] = actual
    return verified


def jobs(plan, stage):
    settings = plan["stages"][stage]
    result = []
    for name in ("control", "candidate"):
        output = HERE / f"{stage}-{name}.jsonl"
        args = {**plan["common_arguments"], "--library": plan["artifacts"][name]["path"],
                "--policies": ",".join(settings["policies"]),
                "--plain": str(settings["plain"]).lower(), "--seed": str(settings["seed"]),
                "--games": str(settings["games_per_deck"]), "--output": str(output)}
        command = ["taskset", "-c", "8-23", plan["artifacts"]["executable"]["path"], "generic"]
        command.extend(item for pair in args.items() for item in pair)
        result.append({"name": name, "output": str(output), "command": command})
    return result


def analysis_module():
    spec = importlib.util.spec_from_file_location("sequence_analysis", ANALYZER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def visible_benchmarks():
    active, paused = [], []
    processes = list(Path("/proc").glob("[0-9]*"))
    for process in processes:
        try:
            argv = [part.decode() for part in (process / "cmdline").read_bytes().split(b"\0") if part]
            if not argv or Path(argv[0]).name != "policy-bench":
                continue
            state = (process / "stat").read_text().rsplit(")", 1)[1].split()[0]
            item = {"pid": int(process.name), "state": state, "argv": argv}
            if state in ("T", "t"):
                paused.append(item)
            elif state != "Z":
                active.append(item)
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
    require(not active, f"Other visible benchmarks are active: {active}")
    return {"visible_process_count": len(processes), "paused_benchmarks_untouched": paused,
            "scope": "Visible /proc only; caller confirms host-wide worker availability"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", choices=("plain", "demise"), required=True)
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    plan = json.loads(PLAN.read_text())
    scheduled = jobs(plan, args.stage)
    if not args.execute:
        print(json.dumps({"stage": args.stage, "jobs": scheduled}, indent=2))
        return
    require(set(plan["cpus"]) <= os.sched_getaffinity(0), "CPUs 8-23 are not available to this runner")
    require(plan["workers_per_arm"] == 8 and plan["maximum_simultaneous_match_workers"] == 16,
            "Unexpected worker budget")
    require(not list(HERE.glob(f"{args.stage}-*")), "Stage outputs already exist; refusing overwrite/resume")
    analysis = analysis_module()
    if args.stage == "demise":
        plain = analysis.analyze_stage("plain", write=False)
        require(plain["promotion_gate"]["passed"], "Plain-stage gate failed; Demise search is not permitted")
    artifacts = verify_artifacts(plan)
    build_path = Path(plan["artifacts"]["candidate"]["path"]).with_name("build.json")
    build = json.loads(build_path.read_text())
    require(build["source_commit"] == plan["candidate_library_source_commit"]
            and all(build[key] == artifacts["candidate"][key] for key in ("path", "bytes", "sha256")),
            "Candidate build record differs from frozen plan")
    sources = {"plan": fingerprint(PLAN), "runner": fingerprint(Path(__file__)), "analyzer": fingerprint(ANALYZER)}
    manifest = {"stage": args.stage, "plan": plan, "artifacts": artifacts, "jobs": scheduled,
                "candidate_build": build, "source_fingerprints": sources,
                "launch_checkout": {
                    "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                    "status": subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True)},
                "preflight_processes": visible_benchmarks(),
                "started_utc": datetime.now(timezone.utc).isoformat()}
    write_new(HERE / f"{args.stage}-manifest.json", manifest)
    children, exit_codes = {}, {}
    def interrupted(signum, frame):
        raise KeyboardInterrupt(f"Runner interrupted by signal {signum}")
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    failure = None
    try:
        with ExitStack() as stack:
            for job in scheduled:
                log = stack.enter_context((HERE / f"{args.stage}-{job['name']}.log").open("x"))
                children[job["name"]] = subprocess.Popen(job["command"], cwd=ROOT, stdout=log,
                                                         stderr=subprocess.STDOUT, start_new_session=True)
            while any(child.poll() is None for child in children.values()):
                time.sleep(1)
            exit_codes = {name: child.wait() for name, child in children.items()}
    except BaseException as error:
        failure = f"{type(error).__name__}: {error}"
    finally:
        for child in children.values():
            if child.poll() is None:
                child.terminate()
        for name, child in children.items():
            try:
                exit_codes[name] = child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                child.kill()
                exit_codes[name] = child.wait()
    after = verify_artifacts(plan)
    require(sources == {"plan": fingerprint(PLAN), "runner": fingerprint(Path(__file__)), "analyzer": fingerprint(ANALYZER)},
            "Plan or analysis scripts changed during the stage")
    write_new(HERE / f"{args.stage}-exits.json", {
        "stage": args.stage, "exit_codes": exit_codes, "artifacts_after": after,
        "failure": failure, "finished_utc": datetime.now(timezone.utc).isoformat()})
    require(failure is None and exit_codes == {"control": 0, "candidate": 0}, f"Stage did not complete: {failure or exit_codes}")
    report = analysis.analyze_stage(args.stage, write=True)
    print(json.dumps({"stage": args.stage, "report": str(HERE / f"{args.stage}-analysis.json"),
                      "promotion_gate": report["promotion_gate"]}, indent=2))


if __name__ == "__main__":
    main()
