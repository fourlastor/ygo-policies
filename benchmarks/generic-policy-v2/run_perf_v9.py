#!/usr/bin/env python3
"""Frozen v9 bundled-performance parity experiment; default action only prints a plan.

Run with host /proc visibility after other benchmarks finish. The sole permitted
background benchmark is the exact v8 roster process below (four workers). Its
threads are temporarily narrowed from CPUs 8-23 to 12-23, then restored. Measurement
children use CPUs 8-11 and one worker. SIGKILL or machine failure cannot run cleanup;
the fsynced events.jsonl preserves process identities and original affinity masks.
No policy-strength claim follows from these eight fixed game pairs.
"""
import argparse
import copy
import datetime
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import select
import signal
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / "benchmarks/generic-policy-v2"
DECKS = ("blackwing", "fabled", "monarch", "worm")
V6_PID, V8_PID = 236006, 329692
V8_EXE = "/tmp/ygo-generic-v8/policy-bench"
V8_OUTPUT = HERE / "mcts-v8-roster.jsonl"
ORIGINAL_CPUS, BACKGROUND_CPUS, MEASURE_CPUS = set(range(8, 24)), set(range(12, 24)), set(range(8, 12))
VARIANTS = {
    "control": {
        "directory": "/tmp/ygo-generic-v9-diversity", "revision": "1576a59",
        "executable_sha256": "ed4c6f8a119d03a3650f4db02705fc1dda3065a49c184de7e1f6a84aaa2e65d7",
        "library_sha256": "9c3e60325a835b2025191cdf8cc5f86bb2b42f8650c980415ca5c1cce0d307d1",
    },
    "optimized": {
        "directory": "/tmp/ygo-generic-v9-perf", "revision": "2b289c6",
        "executable_sha256": "d1280e127eb55a4372f75a585f4a3339bb22b0f59c9d946d7e7a7e0177079eec",
        "library_sha256": "3df62caf14688b50bc8c69c8d82c253310bb32cb6b345f30db0ca0d4f67303e0",
    },
}
COMMON = {
    "--baseline": "/tmp/ygo-generic-baseline/libygo_policies.so",
    "--cards": str(ROOT / "vendor/BabelCdb/cards.cdb"),
    "--scripts": str(ROOT / "vendor/CardScripts"), "--decks": str(ROOT / "decks"),
    "--games": "2", "--seed": "2468000", "--workers": "1", "--limit": "8192",
    "--method": "mcts", "--plain": "false", "--effects": "true", "--strict": "true",
    "--depth": "1", "--worlds": "4", "--max-candidates": "8", "--max-playouts": "256",
    "--horizon": "1", "--millis": "0", "--mcts-iterations": "128", "--mcts-worlds": "4",
    "--mcts-actions": "8", "--mcts-nodes": "1024", "--mcts-decisions": "384",
    "--mcts-replies": "1", "--mcts-terminal": "false", "--mcts-validate-roots": "false",
    "--trace": "false", "--record": "false",
}
TIMING_FIELDS = {"decision_ms", "decision_ms_by_seat", "latency_ms", "search_decision_ms", "search_latency_ms"}


def analysis_module():
    spec = importlib.util.spec_from_file_location("generic_analysis", ROOT / "benchmarks/generic-policy/analyze.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_new(path, value):
    with path.open("x") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def schedule(directory):
    result = []
    for index, deck in enumerate(DECKS):
        for variant in (("control", "optimized") if index % 2 == 0 else ("optimized", "control")):
            frozen = Path(VARIANTS[variant]["directory"])
            output = directory / f"perf-v9-{deck}-{variant}.jsonl"
            options = {**COMMON, "--library": str(frozen / "libygo_policies.so"),
                       "--policies": deck, "--output": str(output)}
            command = ["taskset", "-c", "8-11", str(frozen / "policy-bench"), "generic"]
            command.extend(item for pair in options.items() for item in pair)
            result.append({"deck": deck, "variant": variant, "output": str(output), "command": command})
    return result


def archive_or_raw(path):
    return path if path.exists() else Path(str(path) + ".gz")


def process_identity(pid):
    proc = Path("/proc") / str(pid)
    try:
        stat = (proc / "stat").read_text().rsplit(")", 1)[1].split()
        return {"pid": pid, "start_ticks": stat[19], "state": stat[0],
                "exe": os.readlink(proc / "exe"),
                "argv": [part.decode() for part in (proc / "cmdline").read_bytes().split(b"\0") if part],
                "boot_id": Path("/proc/sys/kernel/random/boot_id").read_text().strip()}
    except FileNotFoundError:
        return None


def option(argv, name):
    if argv.count(name) != 1:
        return None
    index = argv.index(name)
    return argv[index + 1] if index + 1 < len(argv) else None


def output_path(identity):
    value = option(identity["argv"], "--output")
    if value is None:
        return None
    path = Path(value)
    return path.resolve() if path.is_absolute() else (Path("/proc") / str(identity["pid"]) / "cwd" / path).resolve()


def same_process(expected):
    actual = process_identity(expected["pid"])
    return actual is not None and all(actual[key] == expected[key] for key in ("pid", "start_ticks", "exe", "argv", "boot_id"))


def verify_background(analysis):
    # A validated final summary is independent evidence even in a PID namespace.
    analysis.load(archive_or_raw(HERE / "mcts-v6-large-roster.jsonl"))
    if process_identity(V6_PID) is not None:
        raise RuntimeError(f"PID {V6_PID} still exists; v6 process exit must be confirmed before measurement")
    background = process_identity(V8_PID)
    if background is None:
        # Missing /proc alone never proves the host process finished.
        analysis.load(archive_or_raw(V8_OUTPUT))
    elif (background["exe"] != V8_EXE or output_path(background) != V8_OUTPUT
          or option(background["argv"], "--workers") != "4"
          or option(background["argv"], "--millis") != "0"
          or background["state"] in ("T", "t", "Z")):
        raise RuntimeError(f"V8 PID identity/configuration mismatch: {background}")
    check_other_benchmarks(background)
    return background


def check_other_benchmarks(background):
    pids = [int(path.name) for path in Path("/proc").iterdir() if path.name.isdigit()]
    # The desktop host has many processes. A tiny sandbox /proc cannot establish
    # that competing host benchmarks are absent, even if both summaries exist.
    if len(pids) < 20:
        raise RuntimeError("Insufficient host /proc visibility; run from the host, outside the tool PID namespace")
    for pid in pids:
        try:
            identity = process_identity(pid)
        except PermissionError:
            # Reading an unrelated user's exe may be denied; cmdline is enough
            # to reject a visible policy-bench rather than silently ignoring it.
            cmdline = (Path("/proc") / str(pid) / "cmdline").read_bytes()
            if b"policy-bench" in cmdline:
                raise RuntimeError(f"Cannot inspect possible benchmark PID {pid}")
            continue
        if identity and Path(identity["exe"].removesuffix(" (deleted)")).name == "policy-bench":
            if background is None or not all(identity[k] == background[k] for k in ("pid", "start_ticks", "exe", "argv")):
                raise RuntimeError(f"Unexpected active benchmark: {identity}")


class Journal:
    def __init__(self, directory):
        self.stream = (directory / "events.jsonl").open("x", buffering=1)

    def __call__(self, event, **details):
        self.stream.write(json.dumps({"utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                                      "event": event, **details}, allow_nan=False) + "\n")
        self.stream.flush()
        os.fsync(self.stream.fileno())


class BackgroundAffinity:
    def __init__(self, identity, journal):
        self.identity, self.journal, self.fd = identity, journal, None
        self.changed = False
        self.restoring = False

    def log(self, event, **details):
        try:
            self.journal(event, **details)
        except OSError:
            if not self.restoring:
                raise
            # A full disk must not prevent restoring the user's CPU masks.

    def thread_identity(self, tid):
        task = Path("/proc") / str(tid)
        try:
            stat = (task / "stat").read_text().rsplit(")", 1)[1].split()
            tgid = next(line.split()[1] for line in (task / "status").read_text().splitlines()
                        if line.startswith("Tgid:"))
            return {"tid": tid, "start_ticks": stat[19], "tgid": int(tgid)}
        except FileNotFoundError:
            return None

    def alive(self):
        if self.identity is None:
            return False
        return not select.select([self.fd], [], [], 0)[0] and same_process(self.identity)

    def masks(self):
        result = {}
        for task in (Path("/proc") / str(self.identity["pid"]) / "task").iterdir():
            try:
                result[int(task.name)] = sorted(os.sched_getaffinity(int(task.name)))
            except ProcessLookupError:
                continue
        return result

    def narrow(self):
        if self.identity is None:
            self.log("background_already_complete")
            return
        self.fd = os.pidfd_open(self.identity["pid"])
        if not self.alive():
            raise RuntimeError("Background process exited or changed during preflight")
        original = self.masks()
        if not original or any(set(mask) != ORIGINAL_CPUS for mask in original.values()):
            raise RuntimeError(f"Expected all background threads on CPUs8-23 before narrowing, got {original}")
        self.log("affinity_original", identity=self.identity, thread_masks=original,
                     restore_new_thread_mask=sorted(ORIGINAL_CPUS))
        self.changed = True  # A partially applied change must also restore.
        self.set_all(BACKGROUND_CPUS)

    def set_all(self, cpus):
        for _ in range(8):
            if not self.alive():
                self.log("background_exited_before_affinity", cpus=sorted(cpus))
                return
            masks = self.masks()
            # Leader first: newly created workers inherit its current mask.
            ordered = sorted(masks, key=lambda tid: (tid != self.identity["pid"], tid))
            for tid in ordered:
                if set(masks[tid]) == cpus:
                    continue
                if not self.alive():
                    return
                thread = self.thread_identity(tid)
                if thread is None or thread["tgid"] != self.identity["pid"]:
                    continue
                self.log("affinity_before_change", thread=thread, before=masks[tid], after=sorted(cpus))
                if self.thread_identity(tid) != thread or not self.alive():
                    continue
                try:
                    os.sched_setaffinity(tid, cpus)
                except ProcessLookupError:
                    continue
            if self.alive():
                after = self.masks()
                if after and all(set(mask) == cpus for mask in after.values()):
                    self.log("affinity_verified", thread_masks=after)
                    return
        raise RuntimeError("Background thread affinities did not stabilize")

    def restore(self):
        self.restoring = True
        try:
            if self.changed:
                self.set_all(ORIGINAL_CPUS)
                self.log("affinity_cleanup_finished", original_identity=self.identity)
        finally:
            if self.fd is not None:
                os.close(self.fd)


def deterministic_row(row):
    value = copy.deepcopy(row)
    value.pop("seconds", None)
    for key in TIMING_FIELDS:
        value["game"].pop(key, None)
    return value


def sample_stats(values):
    if not values:
        return {"count": 0}
    ordered = sorted(values)
    def percentile(q):
        pos = (len(ordered) - 1) * q
        lo, hi = math.floor(pos), math.ceil(pos)
        return ordered[lo] + (ordered[hi] - ordered[lo]) * (pos - lo)
    return {"count": len(values), "total": sum(values), "mean": statistics.mean(values),
            "median": statistics.median(values), "p95": percentile(.95), "max": max(values)}


def search_samples(row):
    game = row["game"]
    samples = game["search_decision_ms"]
    if len(samples) != game["searched"]:
        raise ValueError("Search sample count does not equal searched roots")
    if (len(game["decision_ms_by_seat"]) != 2
            or sum(map(len, game["decision_ms_by_seat"])) != game["decisions"]):
        raise ValueError("Seat decision sample counts do not equal total decisions")
    for values in [samples, game["decision_ms"], *game["decision_ms_by_seat"]]:
        if any(type(value) not in (int, float) or not math.isfinite(value) or value < 0 for value in values):
            raise ValueError("Invalid latency sample")
    if game["decision_ms"] != game["decision_ms_by_seat"][row["seat"]]:
        raise ValueError("Candidate-seat decision samples are misaligned")
    return samples


def report(directory):
    analysis = analysis_module()
    manifest = json.loads((directory / "manifest.json").read_text())
    if manifest["jobs"] != schedule(directory) or manifest["common_arguments"] != COMMON:
        raise ValueError("Manifest does not describe the frozen performance schedule")
    for variant, expected in VARIANTS.items():
        frozen = manifest["variants"][variant]
        if not frozen["revision"].startswith(expected["revision"]):
            raise ValueError("Manifest candidate revision mismatch")
        for filename, key in (("policy-bench", "executable_sha256"), ("libygo_policies.so", "library_sha256")):
            if frozen["files"][filename]["sha256"] != expected[key]:
                raise ValueError("Manifest frozen binary fingerprint mismatch")
    events = [json.loads(line) for line in (directory / "events.jsonl").read_text().splitlines()]
    validated = {event["output"]: event for event in events if event["event"] == "measurement_validated"}
    shared_inputs, candidate_fingerprints = None, {}
    pairs, mismatches, identities = [], [], {}
    pooled = {"control": [], "optimized": []}
    for deck in DECKS:
        loaded = {variant: analysis.load(archive_or_raw(directory / f"perf-v9-{deck}-{variant}.jsonl"))
                  for variant in VARIANTS}
        control, _, control_meta = loaded["control"]
        optimized, _, optimized_meta = loaded["optimized"]
        for variant, (_, _, metadata) in loaded.items():
            job = next(job for job in manifest["jobs"] if job["deck"] == deck and job["variant"] == variant)
            arguments = dict(zip(job["command"][5::2], job["command"][6::2]))
            if metadata["arguments"] != arguments or metadata["workers"] != 1:
                raise ValueError("Measurement arguments differ from frozen manifest")
            if (metadata["policies"] != [deck] or metadata["seed"] != 2468000
                    or metadata["games_per_matchup"] != 2 or metadata["planned_jobs"] != 2):
                raise ValueError("Measurement schedule differs from frozen manifest")
            event = validated.get(job["output"])
            if event is None:
                raise ValueError("Missing validated measurement event")
            for suffix, key in ((".metadata.json", "metadata_sha256"), (".summary.json", "summary_sha256")):
                if sha256(Path(job["output"]).with_suffix(suffix)) != event[key]:
                    raise ValueError("Measurement sidecar changed after validation")
            raw = Path(job["output"])
            if not raw.exists() or sha256(raw) != event["raw_sha256"]:
                raise ValueError("Measurement raw evidence changed after validation; keep these small runs uncompressed")
            fingerprints = {key: analysis.file_identity(metadata[key], key) for key in ("candidate", "executable")}
            if (event.get("frozen_sha256_verified_before_and_after") is not True
                    or event.get("recorded_candidate_identities") != fingerprints):
                raise ValueError("Measurement fingerprints are not bound to the frozen binary verification")
            if variant in candidate_fingerprints and candidate_fingerprints[variant] != fingerprints:
                raise ValueError("Candidate binary changed between deck blocks")
            candidate_fingerprints[variant] = fingerprints
            for key, filename in (("candidate", "libygo_policies.so"), ("executable", "policy-bench")):
                frozen_file = manifest["variants"][variant]["files"][filename]
                if (metadata[key]["path"] != frozen_file["path"]
                        or metadata[key]["bytes"] != frozen_file["bytes"]):
                    raise ValueError("Measurement binary does not match frozen path/size")
            shared = analysis.pinned_context(metadata)
            for key in ("decks", "policies"):
                shared.pop(key)
            if shared_inputs is not None and shared_inputs != shared:
                raise ValueError("Shared inputs changed across deck blocks")
            shared_inputs = shared
        analysis.validate_comparison(control_meta, optimized_meta, allow_candidate_change=True)
        if analysis.recorded_configuration(control_meta) != analysis.recorded_configuration(optimized_meta):
            raise ValueError("Performance experiment changed search configuration")
        if control.keys() != optimized.keys():
            raise ValueError("Performance experiment has different paired schedules")
        identities[deck] = {variant: analysis.identity(value[2]) for variant, value in loaded.items()}
        for key in sorted(control):
            before, after = control[key], optimized[key]
            old, new = deterministic_row(before), deterministic_row(after)
            if old != new:
                mismatches.append({"key": key, "control": old, "optimized": new})
                continue
            a, b = search_samples(before), search_samples(after)
            if len(a) != len(b):
                raise ValueError("Parity passed but search sample lengths differ")
            da, db = before["game"]["decision_ms"], after["game"]["decision_ms"]
            if len(da) != len(db) or any(len(x) != len(y) for x, y in zip(
                    before["game"]["decision_ms_by_seat"], after["game"]["decision_ms_by_seat"])):
                raise ValueError("Parity passed but decision sample lengths differ")
            for variant, samples in (("control", a), ("optimized", b)):
                pooled[variant].extend(samples)
            pairs.append({"key": key, "digest": old["game"]["digest"],
                          "winner": old["game"]["winner"], "searched": len(a),
                          "non_timing_row_sha256": hashlib.sha256(json.dumps(old, sort_keys=True).encode()).hexdigest(),
                          "control_search_ms": sample_stats(a), "optimized_search_ms": sample_stats(b),
                          "search_total_ratio_optimized_over_control": sum(b) / sum(a) if sum(a) else None,
                          "paired_search_delta_ms": sample_stats([y - x for x, y in zip(a, b)]),
                          "control_candidate_decision_ms": sample_stats(da), "optimized_candidate_decision_ms": sample_stats(db),
                          "paired_candidate_decision_delta_ms": sample_stats([y - x for x, y in zip(da, db)]),
                          "control_game_seconds": before["seconds"], "optimized_game_seconds": after["seconds"]})
    result = {"kind": "bundled_host_and_library_performance_revision", "exact_non_timing_parity": not mismatches,
              "frozen_variants": manifest["variants"],
              "expected_pairs": 8, "matched_pairs": len(pairs), "mismatches": mismatches,
              "identities": identities, "pairs": pairs,
              "pooled_search_ms": {key: sample_stats(values) for key, values in pooled.items()},
              "caveats": ["Eight fixed game pairs across four development decks; no 55% strength claim.",
                          "Remaining background v8 memory and shared-cache load may affect timing.",
                          "Ordinal search pairing is used only after exact full deterministic-row parity.",
                          "Decisions within a game are correlated; pooled decisions are not independent experimental units.",
                          "Both host and FFI library changed; this measures the bundled performance revision."]}
    ratios = [pair["search_total_ratio_optimized_over_control"] for pair in pairs
              if pair["search_total_ratio_optimized_over_control"] is not None]
    result["per_game_search_total_ratios"] = sample_stats(ratios)
    write_new(directory / "comparison.json", result)
    if mismatches or len(pairs) != 8:
        raise RuntimeError("Exact deterministic parity failed; comparison.json records discrepancies, no speedup conclusion")
    return result


def verify_frozen():
    result = {}
    for name, variant in VARIANTS.items():
        directory = Path(variant["directory"])
        revision = (directory / "source-commit.txt").read_text().strip()
        if not revision.startswith(variant["revision"]):
            raise RuntimeError(f"Unexpected {name} source revision: {revision}")
        result[name] = {"revision": revision, "files": {}}
        for filename, expected in (("policy-bench", variant["executable_sha256"]),
                                   ("libygo_policies.so", variant["library_sha256"])):
            path = directory / filename
            digest = sha256(path)
            if digest != expected:
                raise RuntimeError(f"Frozen fingerprint mismatch: {path}")
            result[name]["files"][filename] = {"path": str(path), "bytes": path.stat().st_size, "sha256": digest}
    # These source directories contain the generic policy and fixed evaluator.
    unchanged = subprocess.run(["git", "diff", "--exit-code", "1576a59", "2b289c6", "--",
                                "crates/ygo-policies/src", "crates/ygo-policies-bench/src/engine/evaluation.rs"],
                               cwd=ROOT, capture_output=True, text=True)
    if unchanged.returncode:
        raise RuntimeError("Generic policy/evaluator sources differ between frozen revisions")
    return result


def run(directory):
    analysis = analysis_module()
    frozen = verify_frozen()
    background = verify_background(analysis)
    directory.mkdir(parents=False, exist_ok=False)  # Protect raw data AND overwrite-prone CLI sidecars.
    jobs = schedule(directory)
    write_new(directory / "manifest.json", {"runner_sha256": sha256(__file__), "variants": frozen,
              "common_arguments": COMMON, "jobs": jobs, "background": background,
              "measurement_cpus": sorted(MEASURE_CPUS), "background_cpus": sorted(BACKGROUND_CPUS),
              "external_benchmark_workers": 4 if background else 0, "measurement_workers": 1,
              "scope": "Bundled host snapshot/field and FFI message/query allocation changes; no policy/evaluator changes"})
    journal = Journal(directory)
    affinity = BackgroundAffinity(background, journal)
    child = None
    completed = False
    def interrupted(signum, frame):
        raise KeyboardInterrupt(f"Received signal {signum}")
    old_handlers = {sig: signal.signal(sig, interrupted) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        affinity.narrow()
        for job in jobs:
            if background and not affinity.alive():
                # Allow a cleanly completed background game set, never a crash.
                analysis.load(archive_or_raw(V8_OUTPUT))
                background = None
            check_other_benchmarks(background)
            if background and any(set(mask) != BACKGROUND_CPUS for mask in affinity.masks().values()):
                raise RuntimeError("Background affinity changed during measurement schedule")
            if verify_frozen() != frozen:
                raise RuntimeError("Frozen files changed before measurement")
            output = Path(job["output"])
            journal("measurement_start", job=job, background_alive=affinity.alive(),
                    background_thread_masks=affinity.masks() if affinity.alive() else {})
            start = time.monotonic()
            with output.with_suffix(".log").open("x") as log:
                child = subprocess.Popen(job["command"], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                # taskset applies affinity before exec; wait briefly for that
                # transition, then verify every measurement thread's mask.
                for _ in range(100):
                    identity = process_identity(child.pid)
                    if identity and identity["exe"] == job["command"][3]:
                        masks = {int(task.name): sorted(os.sched_getaffinity(int(task.name)))
                                 for task in (Path("/proc") / str(child.pid) / "task").iterdir()}
                        if not masks or any(set(mask) != MEASURE_CPUS for mask in masks.values()):
                            raise RuntimeError("Measurement child affinity mismatch")
                        journal("measurement_affinity_verified", identity=identity, thread_masks=masks)
                        break
                    if child.poll() is not None:
                        raise RuntimeError("Measurement exited before affinity verification")
                    time.sleep(.01)
                else:
                    raise RuntimeError("Measurement did not enter frozen executable")
                code = child.wait()
                child = None
            journal("measurement_finish", output=str(output), returncode=code, elapsed_seconds=time.monotonic() - start,
                    background_alive=affinity.alive())
            if code:
                raise RuntimeError(f"Measurement failed: {output}, exit {code}")
            _, _, metadata = analysis.load(output)
            if verify_frozen() != frozen:
                raise RuntimeError("Frozen files changed during measurement")
            journal("measurement_validated", output=str(output), raw_sha256=sha256(output),
                    metadata_sha256=sha256(output.with_suffix(".metadata.json")),
                    summary_sha256=sha256(output.with_suffix(".summary.json")),
                    frozen_sha256_verified_before_and_after=True,
                    recorded_candidate_identities={key: analysis.file_identity(metadata[key], key)
                                                   for key in ("candidate", "executable")})
        report(directory)
        completed = True
    except BaseException as error:
        journal("failed", error=repr(error))
        raise
    finally:
        # Avoid a second SIGTERM interrupting restoration. SIGKILL is unrecoverable.
        for sig in old_handlers:
            signal.signal(sig, signal.SIG_IGN)
        try:
            if child is not None and child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
            affinity.restore()
        except BaseException as error:
            journal("cleanup_failed", error=repr(error))
            raise
        else:
            if completed:
                journal("complete", exact_non_timing_parity=True, affinity_cleanup_successful=True)
        finally:
            journal.stream.close()
            for sig, handler in old_handlers.items():
                signal.signal(sig, handler)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("plan", "run", "report"), default="plan", nargs="?")
    parser.add_argument("--output-dir", type=Path, default=HERE / "perf-v9-2b289c6")
    args = parser.parse_args()
    directory = args.output_dir.resolve()
    if args.action == "plan":
        print(json.dumps({"output_directory": str(directory), "variants": VARIANTS, "jobs": schedule(directory),
                          "requires": "Host /proc visibility, completed v6 large, no other benchmark except exact v8 with four workers",
                          "no_measurements_launched": True}, indent=2))
    elif args.action == "run":
        run(directory)
    else:
        report(directory)


if __name__ == "__main__":
    main()
