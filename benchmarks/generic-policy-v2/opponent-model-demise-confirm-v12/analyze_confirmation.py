#!/usr/bin/env python3
"""Analyze the fixed fresh 256-pair confirmation only after both arms finish.

The original 64 pairs are excluded from the primary test. An explicitly
secondary pooled result is kept separately. No raw evidence is modified.
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
PRIOR = HERE.parent / "opponent-model-demise-v12"


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def inference(analyze, helper, generic, specialist):
    paired = analyze.compare(generic, specialist)
    b, c = paired["improved"], paired["regressed"]
    a = sum(analyze.won(generic[k]) and analyze.won(specialist[k]) for k in generic)
    d = len(generic) - a - b - c
    return {"paired": paired,
            "paired_table": {"rows": ["specialist win", "specialist non-win"],
                             "columns": ["generic win", "generic non-win"], "counts": [[a, b], [c, d]]},
            "paired_newcombe_95": helper.paired_newcombe(a, b, c, d),
            "exact_two_sided_discordant_binomial_p": min(1.0, 2 * sum(math.comb(b + c, k)
                for k in range(min(b, c) + 1)) / 2 ** (b + c)) if b + c else 1.0}


def main():
    helper = module("diagnostic", PRIOR / "analyze_diagnostic.py")
    analyze = module("generic_analyze", helper.ANALYZER)
    paths = [HERE / "manifest.json", PRIOR / "manifest.json"]
    # Refuse to inspect outcomes until all fresh planned games and summaries exist.
    for directory, count in ((HERE, 256), (PRIOR, 64)):
        for name in ("generic", "specialist"):
            assert (directory / f"{name}.summary.json").is_file(), f"{directory.name}/{name}: incomplete"
            raw = helper.evidence_path(directory, name)
            assert len(helper.logical_bytes(raw).splitlines()) == count
            paths.append(raw)
            paths.extend(directory / f"{name}.{suffix}" for suffix in ("metadata.json", "summary.json"))
    fingerprints = {str(helper.logical_path(p).relative_to(HERE.parent)): helper.fingerprint(p) for p in paths}
    runs = {name: analyze.load(helper.evidence_path(HERE, name)) for name in ("generic", "specialist")}
    prior = {name: analyze.load(helper.evidence_path(PRIOR, name)) for name in ("generic", "specialist")}
    for name, (_, _, metadata) in runs.items():
        assert metadata["seed"] == 2473000 and metadata["workers"] == 8
        assert metadata["policies"] == ["demise"]
        assert metadata["search"]["opponent_model"] == name
        prior_metadata = prior[name][2]
        assert metadata["search"] == prior_metadata["search"]
        assert metadata["plain"] == prior_metadata["plain"] is False
        assert metadata["effect_profiles"] == prior_metadata["effect_profiles"] is True
        for field in ("candidate", "executable"):
            assert analyze.file_identity(metadata[field], field) == analyze.file_identity(prior_metadata[field], field)
        old, new = analyze.pinned_context(prior_metadata), analyze.pinned_context(metadata)
        assert old.pop("seed") == 2472000 and new.pop("seed") == 2473000
        for field in ("planned_jobs", "games_per_matchup"):
            assert old.pop(field) == 64 and new.pop(field) == 256
        assert old == new
        assert not set(runs[name][0]).intersection(prior[name][0]), "Fresh schedule overlaps original study"
    capture = io.StringIO()
    with contextlib.redirect_stdout(capture):
        analyze.main([str(helper.evidence_path(HERE, "generic")), str(helper.evidence_path(HERE, "specialist"))],
                     allow_metadata_ablation=["search.opponent_model"])
    comparison = json.loads(capture.getvalue())
    result = comparison["comparisons"][0]
    assert result["isolated_metadata_ablation"] is True
    assert [c["field"] for c in result["configuration_changes"]] == ["search.opponent_model"]
    stats = inference(analyze, helper, runs["generic"][0], runs["specialist"][0])
    stats.update({"scope": "Primary: fresh fixed 256 matched pairs only; original 64 excluded",
                  "planned_pairs": 256, "completed_pairs": 256,
                  "all_planned_games_required_before_analysis": True,
                  "paired_interval_method": "Newcombe1998 method10, standard normal97.5% quantile; repository paired-normal interval retained separately",
                  "arm_interval_method": "Wilson95%, z=1.96, wins/all256plannedgames", "arms": {}})
    for name, (rows, summary, _) in runs.items():
        overall = summary["overall"]
        times = [ms for row in rows.values() for ms in row["game"]["search_decision_ms"]]
        assert overall["mcts_simulations"] == overall["searched"] * 128
        assert overall["mcts_completed"] + overall["mcts_cutoffs"] == overall["mcts_simulations"]
        stats["arms"][name] = {"overall": overall, "wilson_95": helper.wilson(overall["wins"], 256),
                               "searches_over_200_ms": sum(ms > 200 for ms in times),
                               "search_samples": len(times), "wall_seconds": summary["wall_seconds"]}
    pooled = inference(analyze, helper, {**prior["generic"][0], **runs["generic"][0]},
                       {**prior["specialist"][0], **runs["specialist"][0]})
    pooled["scope"] = "Secondary pooled320pairs; includes initial exploratory64whichmotivatedconfirmation; not the primary confirmation test"
    pooled["fresh_and_prior_schedules_disjoint"] = True
    stats["secondary_pooled"] = pooled
    assert fingerprints == {str(helper.logical_path(p).relative_to(HERE.parent)): helper.fingerprint(p) for p in paths}
    stats["input_fingerprints"] = fingerprints
    stats["input_fingerprint_scope"] = "Logical decompressed bytes; raw or gzip accepted with equality required if both exist"
    stats["analysis_source"] = {"confirmation": helper.fingerprint(Path(__file__)),
                                "shared_statistics": helper.fingerprint(PRIOR / "analyze_diagnostic.py"),
                                "repository_analyzer": helper.fingerprint(helper.ANALYZER)}
    for name, data in (("comparison.json", comparison), ("paired-statistics.json", stats)):
        (HERE / name).write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"primary": {k: stats[k] for k in ("paired", "paired_newcombe_95", "exact_two_sided_discordant_binomial_p")},
                      "secondary_pooled": pooled}, indent=2))


if __name__ == "__main__":
    main()
