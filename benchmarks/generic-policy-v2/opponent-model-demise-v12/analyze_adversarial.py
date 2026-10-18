#!/usr/bin/env python3
"""Separate completed exploratory opponent-UCT ablation; leaves raw evidence intact."""
import contextlib
import io
import json
import math
from pathlib import Path
import sys

sys.dont_write_bytecode = True

from analyze_controls import module

HERE = Path(__file__).resolve().parent


def main():
    helper = module("diagnostic", HERE / "analyze_diagnostic.py")
    analyze = module("generic_analyze", helper.ANALYZER)
    paths = [HERE / "adversarial-manifest.json"]
    for name in ("generic", "adversarial"):
        raw = helper.evidence_path(HERE, name)
        assert len(helper.logical_bytes(raw).splitlines()) == 64
        paths.append(raw)
        paths.extend(HERE / f"{name}.{suffix}" for suffix in ("metadata.json", "summary.json"))
    fingerprints = {helper.logical_path(p).name: helper.fingerprint(p) for p in paths}
    generic, _, generic_meta = analyze.load(helper.evidence_path(HERE, "generic"))
    adversarial, summary, metadata = analyze.load(helper.evidence_path(HERE, "adversarial"))
    assert metadata["workers"] == generic_meta["workers"] == 4
    capture = io.StringIO()
    with contextlib.redirect_stdout(capture):
        analyze.main([str(helper.evidence_path(HERE, "generic")), str(helper.evidence_path(HERE, "adversarial"))],
                     allow_metadata_ablation=["search.mcts.adversarial_opponent"])
    comparison = json.loads(capture.getvalue())
    result = comparison["comparisons"][0]
    assert result["isolated_metadata_ablation"] is True
    assert [c["field"] for c in result["configuration_changes"]] == ["search.mcts.adversarial_opponent"]
    paired = result["overall"]
    b, c = paired["improved"], paired["regressed"]
    a = sum(analyze.won(generic[k]) and analyze.won(adversarial[k]) for k in generic)
    d = len(generic) - a - b - c
    overall = summary["overall"]
    assert overall["mcts_simulations"] == overall["searched"] * 128
    assert overall["mcts_completed"] + overall["mcts_cutoffs"] == overall["mcts_simulations"]
    assert all(overall[k] > 0 for k in helper.COUNTERS)
    stats = {"scope": "Separate exploratory shared-budget opponent UCT diagnostic, added after original diagnostic began; no multiplicity adjustment",
             "reference": "generic", "candidate": "adversarial", "planned_pairs": 64,
             "paired": paired, "paired_table": {"rows": ["adversarial win", "adversarial non-win"],
                "columns": ["generic win", "generic non-win"], "counts": [[a, b], [c, d]]},
             "paired_newcombe_95": helper.paired_newcombe(a, b, c, d),
             "exact_two_sided_discordant_binomial_p": min(1.0, 2 * sum(math.comb(b + c, k)
                 for k in range(min(b, c) + 1)) / 2 ** (b + c)) if b + c else 1.0,
             "wilson_95": helper.wilson(overall["wins"], 64), "overall": overall,
             "proposal_only_adversarial_search_frozen_validation_uses_generic": True,
             "interpretation": "Pessimistic sampled-world opposition; not an actor-fair belief model or unknown-opponent qualification",
             "all_64_planned_games_complete": True, "input_fingerprints": fingerprints,
             "input_fingerprint_scope": "Logical decompressed bytes; raw or gzip accepted with equality required if both exist",
             "analysis_source": helper.fingerprint(Path(__file__))}
    assert fingerprints == {helper.logical_path(p).name: helper.fingerprint(p) for p in paths}
    helper.write_json("adversarial-comparison.json", comparison)
    helper.write_json("adversarial-statistics.json", stats)
    print(json.dumps({k: stats[k] for k in ("paired", "paired_newcombe_95", "exact_two_sided_discordant_binomial_p", "wilson_95")}, indent=2))


if __name__ == "__main__":
    main()
