#!/usr/bin/env python3
"""Compare completed generic mirror runs with pinned inputs and matching schedules.

Usage: python3 analyze.py [--allow-candidate-change]
    [--allow-metadata-ablation FIELD] REFERENCE.jsonl[.gz] CANDIDATE.jsonl[.gz] ...
Ordinary rollout budgets may differ. Effect-profile, search-method and beam-setting
changes require explicit ablation paths (for example effect_profiles, search.method,
search.beam, search.mcts, or search.validation). Repeat the flag to declare several paths; a named object covers
its children. Revision changes require their separate flag. Neither override can
relax the fixed opponent, cards, scripts, decks, rules or exact paired schedule.
A searched reference requires an explicit metadata-ablation or candidate-revision
declaration, and the declared kind of change must actually be present.
"""
import argparse
import gzip
import json
import math
from pathlib import Path
import statistics


def file_identity(value, label):
    if (not isinstance(value, dict) or value.get("error")
            or not isinstance(value.get("fnv1a64"), str)
            or not isinstance(value.get("bytes"), int)):
        raise ValueError(f"Missing valid fingerprint for {label}")
    return {key: value[key] for key in ("bytes", "fnv1a64")}


def pinned_context(metadata):
    context = {key: metadata[key] for key in (
        "mode", "candidate_policy", "rules", "life_points", "decision_limit",
        "shuffle", "policy_seed", "target", "exceptions", "seed", "games_per_matchup", "planned_jobs",
    )}
    context["policies"] = sorted(metadata["policies"])
    for key in ("baseline", "cards"):
        context[key] = file_identity(metadata[key], key)
    context["decks"] = {
        name: file_identity(value, f"deck {name}")
        for name, value in metadata["decks"].items()
    }
    context["engine"] = ({"linked": True} if metadata["engine"].get("linked") is True
                         else file_identity(metadata["engine"], "engine"))
    for key in ("core", "scripts", "lua"):
        revision = metadata[key]
        if not revision.get("commit") or revision.get("status") != "":
            raise ValueError(f"Cannot verify {key} equivalence without a clean pinned revision")
        context[key] = {"commit": revision["commit"], "status": revision["status"]}
    return context


def search_validation(metadata):
    search = metadata["search"]
    if "validation" in search:
        if search["validation"] not in ("terminal", "heuristic"):
            raise ValueError("search.validation must be terminal or heuristic")
        return search["validation"], "recorded"
    # Older rollout binaries used horizon=0 for terminal results and a positive
    # horizon for heuristic leaves. Beam and MCTS must record their validation
    # mode explicitly; proposal-return settings do not establish validation mode.
    if (search.get("method", "rollout") != "rollout" or search.get("beam") is not None
            or search.get("mcts") is not None):
        raise ValueError("Beam/MCTS search requires explicit search.validation metadata")
    horizon = search.get("horizon")
    if type(horizon) is not int or horizon < 0:
        raise ValueError("Cannot infer legacy rollout validation without a recorded nonnegative horizon")
    return ("terminal" if horizon == 0 else "heuristic"), "inferred_from_rollout_horizon"


def mcts_root_validation(metadata):
    mcts = metadata["search"].get("mcts")
    if mcts is None:
        return None, "not_applicable"
    if not isinstance(mcts, dict):
        raise ValueError("search.mcts must be an object or null")
    if "validate_roots" not in mcts:
        # Historical MCTS validated only its selected proposal, rather than all
        # measured root actions on fresh common worlds.
        return False, "inferred_legacy_best_only"
    if type(mcts["validate_roots"]) is not bool:
        raise ValueError("search.mcts.validate_roots must be a recorded boolean")
    return mcts["validate_roots"], "recorded"


def recorded_configuration(metadata):
    effects = metadata.get("effect_profiles")
    if "effect_profiles" in metadata and type(effects) is not bool:
        raise ValueError("effect_profiles must be a recorded boolean")
    if not isinstance(metadata["search"], dict):
        raise ValueError("search configuration must be an object")
    search = dict(metadata["search"])
    search["validation"], _ = search_validation(metadata)
    if "method" in search and search["method"] not in ("rollout", "beam", "mcts"):
        raise ValueError("search.method must be rollout, beam or mcts")
    beam = search.get("beam")
    if beam is not None:
        if not isinstance(beam, dict):
            raise ValueError("search.beam must be an object or null")
        for key in ("width", "actions", "nodes", "decisions", "replies"):
            minimum = 0 if key == "replies" else 1
            if type(beam.get(key)) is not int or beam[key] < minimum:
                raise ValueError(f"search.beam.{key} must be a recorded integer >= {minimum}")
    if search.get("method") == "beam" and beam is None:
        raise ValueError("Beam search requires its recorded search.beam settings")
    mcts = search.get("mcts")
    if mcts is not None:
        if not isinstance(mcts, dict):
            raise ValueError("search.mcts must be an object or null")
        for key in ("iterations", "actions", "nodes", "decisions", "replies", "worlds"):
            minimum = 0 if key == "replies" else 1
            if type(mcts.get(key)) is not int or mcts[key] < minimum:
                raise ValueError(f"search.mcts.{key} must be a recorded integer >= {minimum}")
        if type(mcts.get("terminal")) is not bool:
            raise ValueError("search.mcts.terminal must be a recorded boolean")
        if search.get("method") != "mcts" or beam is not None:
            raise ValueError("search.mcts settings require method=mcts and no beam settings")
        search["mcts"] = dict(mcts)
        search["mcts"]["validate_roots"], _ = mcts_root_validation(metadata)
    if search.get("method") == "mcts" and mcts is None:
        raise ValueError("MCTS search requires its recorded search.mcts settings")
    configuration = {"plain": metadata["plain"], "search": search}
    if "effect_profiles" in metadata:
        configuration["effect_profiles"] = effects
    return configuration


def configuration_changes(before_metadata, after_metadata):
    def differences(before, after, prefix=""):
        result = []
        for key in sorted(before.keys() | after.keys()):
            field = f"{prefix}.{key}" if prefix else key
            old, new = before.get(key), after.get(key)
            if key in before and key in after and isinstance(old, dict) and isinstance(new, dict):
                result.extend(differences(old, new, field))
            elif key not in before or key not in after or old != new:
                result.append({"field": field, "before": old, "after": new,
                               "before_recorded": key in before, "after_recorded": key in after})
        return result
    result = differences(recorded_configuration(before_metadata), recorded_configuration(after_metadata))
    for change in result:
        source_reader = {"search.validation": search_validation,
                         "search.mcts.validate_roots": mcts_root_validation}.get(change["field"])
        if source_reader:
            for side, metadata in (("before", before_metadata), ("after", after_metadata)):
                _, source = source_reader(metadata)
                change[side + "_source"] = source
                change[side + "_recorded"] = source == "recorded"
    return result


def declared_ablation(field, allowed):
    return any(field == name or field.startswith(name + ".") for name in allowed)


def identity(metadata):
    configuration = recorded_configuration(metadata)
    return {
        "candidate_library": file_identity(metadata["candidate"], "candidate library"),
        "candidate_executable": file_identity(metadata["executable"], "candidate executable"),
        "fixed_inputs": pinned_context(metadata),
        "plain": metadata["plain"],
        "effect_profiles": configuration.get("effect_profiles"),
        "effect_profiles_recorded": "effect_profiles" in configuration,
        "search": configuration["search"],
        "search_validation_source": search_validation(metadata)[1],
        "search_validation_note": metadata.get("validation_note"),
        "mcts_root_validation_source": mcts_root_validation(metadata)[1],
        "seed": metadata["seed"],
        "games_per_matchup": metadata["games_per_matchup"],
        "workers": metadata["workers"],
    }


def expected_schedule(metadata):
    expected = set()
    for deck in metadata["policies"]:
        salt = 0xCBF29CE484222325
        for byte in ("generic-mirror\0" + deck).encode():
            salt = ((salt ^ byte) * 0x100000001B3) & ((1 << 64) - 1)
        for index in range(metadata["games_per_matchup"]):
            seed = (metadata["seed"] + salt + index) & 0x7FFFFFFF
            expected.add((deck, seed, index % 2))
    return expected


TELEMETRY_COUNTERS = (
    "failed_playouts", "failed_searches", "searched", "playouts", "budget_exhausted",
    "incomplete_worlds", "truncated_playouts", "in_chain", "facing_set", "inner_playouts",
    "inner_searches", "unresolved_playouts", "unsafe_selections", "soft_budget_overruns",
    "proposal_nodes", "proposal_leaves", "proposal_improvements", "proposal_completed_prefixes",
    "proposal_responses", "plan_evaluations", "plan_applied", "plan_mismatches",
    "live_plan_applied", "live_plan_mismatches", "mcts_simulations", "mcts_completed",
    "mcts_nodes", "mcts_steps", "mcts_tree_hits", "mcts_cutoffs", "mcts_live_hits",
    "mcts_live_missing", "mcts_frozen_actions", "mcts_accepted", "mcts_root_actions",
    "mcts_visited_root_actions",
)


def validate_telemetry(rows, summary, label):
    for counter in TELEMETRY_COUNTERS:
        present = [counter in row["game"] for row in rows]
        if counter not in summary and not any(present):
            continue  # This counter was not recorded by an older build.
        if counter not in summary or not all(present):
            raise ValueError(f"{label}: incomplete telemetry for {counter}")
        values = [row["game"][counter] for row in rows]
        if any(type(value) is not int or value < 0 for value in values):
            raise ValueError(f"{label}: invalid telemetry for {counter}")
        if summary[counter] != sum(values):
            raise ValueError(f"{label}: stale telemetry total for {counter}")
    for raw, aggregate in (("decision_ms", "decision_latency_ms"),
                           ("search_decision_ms", "search_latency_ms")):
        values = []
        for row in rows:
            samples = row["game"].get(raw, [])
            if not isinstance(samples, list) or any(
                type(value) not in (int, float) or not math.isfinite(value) or value < 0
                for value in samples
            ):
                raise ValueError(f"{label}: invalid latency samples for {raw}")
            values.extend(samples)
        recorded = summary.get(aggregate)
        if not values:
            if recorded is not None:
                raise ValueError(f"{label}: latency summary without samples for {aggregate}")
            continue
        if not isinstance(recorded, dict):
            raise ValueError(f"{label}: missing latency summary for {aggregate}")
        values.sort()
        total = sum(values)
        expected = {"samples": len(values), "mean": total / len(values), "total": total,
                    "max": values[-1]}
        for key, fraction in (("p50", 0.5), ("p95", 0.95), ("p99", 0.99)):
            expected[key] = values[max(0, math.ceil(len(values) * fraction) - 1)]
        for key, value in expected.items():
            actual = recorded.get(key)
            if (type(actual) not in (int, float) or not math.isfinite(actual)
                    or not math.isclose(actual, value, rel_tol=1e-9, abs_tol=1e-9)):
                raise ValueError(f"{label}: stale {aggregate}.{key}")


def load(path):
    path = Path(path)
    uncompressed = path.with_suffix("") if path.suffix == ".gz" else path
    status_path = uncompressed.with_suffix(".status.json")
    if status_path.exists():
        status = json.loads(status_path.read_text())
        if not isinstance(status, dict):
            raise ValueError(f"{path}: invalid evidence status document {status_path}")
        if status.get("valid") is False or status.get("status") not in ("valid", "complete"):
            reason = status.get("reason", status.get("reasons", "No reason recorded"))
            raise ValueError(f"{path}: evidence status {status.get('status', 'unspecified')}: {reason}")
    summary = json.loads(uncompressed.with_suffix(".summary.json").read_text())
    metadata = json.loads(uncompressed.with_suffix(".metadata.json").read_text())
    if metadata["mode"] != "generic" or summary["overall"]["complete"] is not True:
        raise ValueError(f"{path}: benchmark is not valid and complete")
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt") as stream:
        rows = [json.loads(line) for line in stream]
    keyed = {}
    for row in rows:
        key = (row["opponent"], row["seed"], row["seat"])
        game = row["game"]
        if key in keyed:
            raise ValueError(f"{path}: duplicate record {key}")
        if (type(row["seat"]) is not int or row["seat"] not in (0, 1)
                or type(game.get("winner")) is not int or game["winner"] not in (0, 1, 2)
                or game.get("error") or game.get("limit") is not False
                or game.get("failed_playouts", 0) != 0 or game.get("failed_searches", 0) != 0):
            raise ValueError(f"{path}: invalid, capped or failed game/search at {key}")
        if row.get("candidate_policy") != metadata["candidate_policy"] or row.get("plain") != metadata["plain"]:
            raise ValueError(f"{path}: row/metadata candidate configuration differs at {key}")
        for field in ("effect_profiles", "search"):
            if field in row and (field not in metadata or row[field] != metadata[field]):
                raise ValueError(f"{path}: row/metadata {field} differs at {key}")
        keyed[key] = row
    expected = expected_schedule(metadata)
    if (set(keyed) != expected or len(rows) != metadata["planned_jobs"]
            or len(rows) != summary["overall"]["planned"]):
        raise ValueError(f"{path}: missing, unexpected or incorrectly planned records")
    wins = sum(map(won, rows))
    draws = sum(row["game"]["winner"] == 2 for row in rows)
    for key, expected_value in (("completed", len(rows)), ("wins", wins), ("draws", draws),
                                ("losses", len(rows) - wins - draws)):
        if summary["overall"][key] != expected_value:
            raise ValueError(f"{path}: stale summary {key}")
    validate_telemetry(rows, summary["overall"], f"{path}: overall")
    matchups = {row["opponent"]: row for row in summary["matchups"]}
    if set(matchups) != set(metadata["policies"]) or len(matchups) != len(summary["matchups"]):
        raise ValueError(f"{path}: stale or duplicate per-deck summary")
    for deck, matchup in matchups.items():
        sample = [row for row in rows if row["opponent"] == deck]
        wins = sum(map(won, sample))
        draws = sum(row["game"]["winner"] == 2 for row in sample)
        n = len(sample)
        validate_telemetry(sample, matchup, f"{path}: {deck}")
        for key, expected_value in (("completed", n), ("wins", wins), ("draws", draws),
                                    ("losses", n - wins - draws)):
            if matchup[key] != expected_value:
                raise ValueError(f"{path}: stale {deck} summary {key}")
        rate, z2 = wins / n, 1.96 ** 2
        center = (rate + z2 / (2 * n)) / (1 + z2 / n)
        half = 1.96 * math.sqrt(rate * (1 - rate) / n + z2 / (4 * n * n)) / (1 + z2 / n)
        for key, expected_value in (
            ("exception", deck in metadata["exceptions"]),
            ("meets_observed_target", rate >= metadata["target"]),
            ("lower_95_meets_target", center - half >= metadata["target"]),
        ):
            if matchup[key] is not expected_value:
                raise ValueError(f"{path}: stale {deck} summary {key}")
    identity(metadata)  # Validate all fingerprints before using the evidence.
    return keyed, summary, metadata


def won(row):
    return int(row["game"]["winner"] == row["seat"])


def compare(plain, candidate):
    if plain.keys() != candidate.keys():
        raise ValueError("Runs have different deck/seed/seat schedules")
    differences = [won(candidate[key]) - won(plain[key]) for key in plain]
    n = len(differences)
    mean = statistics.mean(differences)
    width = 1.96 * statistics.stdev(differences) / math.sqrt(n) if n > 1 else None
    return {
        "pairs": n,
        "baseline_win_rate": sum(map(won, plain.values())) / n,
        "candidate_win_rate": sum(map(won, candidate.values())) / n,
        "paired_gain": mean,
        "paired_normal_95": [mean - width, mean + width] if width is not None else None,
        "improved": differences.count(1),
        "regressed": differences.count(-1),
        "same_outcome": differences.count(0),
    }


def validate_comparison(plain_metadata, candidate_metadata, allow_candidate_change,
                        allow_metadata_ablation=()):
    before, after = identity(plain_metadata), identity(candidate_metadata)
    if before["fixed_inputs"] != after["fixed_inputs"]:
        different = [key for key in before["fixed_inputs"]
                     if before["fixed_inputs"][key] != after["fixed_inputs"].get(key)]
        raise ValueError(f"Runs have different fixed inputs: {', '.join(different)}")
    changed = [key for key in ("candidate_library", "candidate_executable") if before[key] != after[key]]
    if changed and not allow_candidate_change:
        raise ValueError(f"Candidate revision changed ({', '.join(changed)}); use --allow-candidate-change "
                         "only for an explicitly labeled revision comparison")
    allowed = set(allow_metadata_ablation)
    for field in allowed:
        if field != "effect_profiles" and not field.startswith("search."):
            raise ValueError(f"Unsupported metadata-ablation path: {field}")
    differences = configuration_changes(plain_metadata, candidate_metadata)
    # These existing budgets have always been compared as a search configuration.
    # New methods/planners and policy-side metadata must never change silently.
    rollout_budgets = {"search." + key for key in (
        "worlds", "max_candidates", "max_playouts", "horizon", "depth", "millis", "strict",
    )}
    required = [change["field"] for change in differences
                if change["field"] != "plain" and change["field"] not in rollout_budgets
                and not declared_ablation(change["field"], allowed)]
    if required:
        raise ValueError("Undeclared metadata ablation: " + ", ".join(required)
                         + "; explicitly name each changed field/object with --allow-metadata-ablation")
    for field in allowed:
        if not any(declared_ablation(change["field"], [field]) for change in differences):
            raise ValueError(f"Declared metadata ablation does not change: {field}")
    return changed


def main(paths, allow_candidate_change=False, allow_metadata_ablation=()):
    if len(paths) < 2:
        raise ValueError(__doc__)
    plain, _, plain_metadata = load(paths[0])
    if (plain_metadata["plain"] is not True and not allow_metadata_ablation
            and not allow_candidate_change):
        raise ValueError("A searched reference requires explicit --allow-metadata-ablation paths "
                         "or --allow-candidate-change for a changed candidate revision")
    results = []
    for path in paths[1:]:
        candidate, summary, metadata = load(path)
        changed = validate_comparison(plain_metadata, metadata, allow_candidate_change, allow_metadata_ablation)
        if plain_metadata["plain"] is not True and not allow_metadata_ablation and not changed:
            raise ValueError("A searched reference needs a declared metadata change or an actual candidate revision change")
        differences = configuration_changes(plain_metadata, metadata)
        effect_changed = any(change["field"] == "effect_profiles" for change in differences)
        missing_configuration = any(
            not change[side + "_recorded"]
            and change.get(side + "_source") not in (
                "inferred_from_rollout_horizon", "inferred_legacy_best_only",
            )
            for change in differences for side in ("before", "after")
        )
        isolated_search = (not changed and not effect_changed and not missing_configuration
                           and plain_metadata["plain"] is True and metadata["plain"] is False)
        isolated_ablation = (not changed and bool(differences) and not missing_configuration
                             and all(declared_ablation(change["field"], allow_metadata_ablation)
                                     for change in differences))
        if changed:
            interpretation = "Candidate revisions differ; this is not an isolated configuration ablation"
        elif missing_configuration:
            interpretation = "Some changed settings were not recorded on both sides; attribution is not isolated"
        elif isolated_ablation:
            interpretation = "Same candidate and fixed inputs; only the explicitly declared metadata settings changed"
        elif isolated_search:
            interpretation = "Same candidate and effect-profile setting; the recorded search configuration changed"
        elif differences:
            interpretation = "Several configuration dimensions changed; do not attribute gain to one setting"
        else:
            interpretation = "Candidate, recorded configuration and fixed inputs match"
        result = {
            "candidate": str(path), "identity": identity(metadata),
            "candidate_revision_changes": changed,
            "configuration_changes": differences,
            "declared_metadata_ablations": sorted(set(allow_metadata_ablation)),
            "isolated_search_configuration_comparison": isolated_search,
            "isolated_metadata_ablation": isolated_ablation,
            "interpretation": interpretation,
            "overall": compare(plain, candidate),
        }
        result["per_deck"] = {
            deck: compare({k: v for k, v in plain.items() if k[0] == deck},
                          {k: v for k, v in candidate.items() if k[0] == deck})
            for deck in sorted({k[0] for k in plain})
        }
        result["required_observed_passes"] = sum(
            not row["exception"] and row["meets_observed_target"] for row in summary["matchups"]
        )
        result["required_confident_passes"] = sum(
            not row["exception"] and row["lower_95_meets_target"] for row in summary["matchups"]
        )
        result["search_latency_ms"] = summary["overall"].get("search_latency_ms")
        results.append(result)
    print(json.dumps({"baseline": str(paths[0]), "baseline_identity": identity(plain_metadata),
                      "allow_candidate_change": allow_candidate_change,
                      "allow_metadata_ablation": sorted(set(allow_metadata_ablation)),
                      "comparisons": results}, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-candidate-change", action="store_true")
    parser.add_argument("--allow-metadata-ablation", action="append", default=[], metavar="FIELD",
                        help="Explicitly permit a changed configuration field/object; repeat for multiple paths")
    parser.add_argument("paths", nargs="+")
    args = parser.parse_args()
    main(args.paths, args.allow_candidate_change, args.allow_metadata_ablation)
