"""Focused provenance/telemetry checks; run with the standard-library unittest runner."""
import contextlib
import copy
import io
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import analyze


def metadata():
    fingerprint = {"bytes": 1, "fnv1a64": "1234567890abcdef"}
    return {
        "mode": "generic", "candidate_policy": "generic", "plain": False,
        "effect_profiles": True, "rules": "fixture", "life_points": [8000, 8000],
        "decision_limit": 8192, "shuffle": "fixture", "policy_seed": "seed+seat",
        "target": .55, "exceptions": [], "seed": 123, "games_per_matchup": 2,
        "planned_jobs": 2, "policies": ["deck"], "workers": 1,
        "baseline": dict(fingerprint), "candidate": dict(fingerprint),
        "executable": dict(fingerprint), "cards": dict(fingerprint),
        "decks": {"deck": dict(fingerprint)}, "engine": {"linked": True},
        "core": {"commit": "core", "status": ""},
        "scripts": {"commit": "scripts", "status": ""},
        "lua": {"commit": "lua", "status": ""},
        "search": {"method": "mcts", "beam": None, "validation": "heuristic",
                   "horizon": 1, "worlds": 4, "max_candidates": 8,
                   "max_playouts": 256, "millis": 0, "depth": 1, "strict": True,
                   "mcts": {"iterations": 128, "actions": 8, "nodes": 1024,
                            "decisions": 384, "replies": 1, "terminal": False,
                            "worlds": 4, "validate_roots": False}},
    }


def fixture(meta, raw_model=None, counters=False):
    rows = []
    for deck, seed, seat in sorted(analyze.expected_schedule(meta)):
        row = {"opponent": deck, "seed": seed, "seat": seat,
               "candidate_policy": "generic", "plain": False,
               "game": {"winner": seat, "limit": False}}
        if raw_model is not None:
            row["game"]["search_opponent_model"] = raw_model
        if counters:
            row["game"].update({key: 0 for key in analyze.OPPONENT_COUNTERS})
        rows.append(row)
    totals = {"completed": 2, "wins": 2, "losses": 0, "draws": 0}
    if counters:
        totals.update({key: 0 for key in analyze.OPPONENT_COUNTERS})
    summary = {"overall": {**totals, "complete": True, "planned": 2},
               "matchups": [{**totals, "opponent": "deck", "exception": False,
                             "meets_observed_target": True, "lower_95_meets_target": False}]}
    return rows, summary


class OpponentProvenanceTests(unittest.TestCase):
    def test_legacy_defaults_normalize_without_mutating_metadata(self):
        old = metadata()
        original = copy.deepcopy(old)
        explicit = copy.deepcopy(old)
        explicit["search"]["opponent_model"] = "generic"
        explicit["search"]["mcts"]["adversarial_opponent"] = False
        self.assertEqual(analyze.recorded_configuration(old), analyze.recorded_configuration(explicit))
        self.assertEqual(analyze.configuration_changes(old, explicit), [])
        self.assertEqual(old, original)
        old_identity, new_identity = analyze.identity(old), analyze.identity(explicit)
        self.assertEqual(old_identity["search_opponent_model_source"], "inferred_legacy_generic_opponent")
        self.assertEqual(old_identity["mcts_adversarial_opponent_source"], "inferred_legacy_viewer_only")
        self.assertEqual(new_identity["search_opponent_model_source"], "recorded")
        self.assertEqual(new_identity["mcts_adversarial_opponent_source"], "recorded")
        # New reports intentionally enrich old metadata with explicit inference.
        # A historical serialized identity omitting these fields is not byte-
        # identical to a new report; original evidence must not be rewritten.
        self.assertIn("opponent_model", old_identity["search"])
        self.assertIn("adversarial_opponent", old_identity["search"]["mcts"])

    def test_strict_boolean_and_opponent_model_values(self):
        for value in (None, 0, 1, "false", [], {}):
            with self.subTest(adversarial=value):
                meta = metadata()
                meta["search"]["mcts"]["adversarial_opponent"] = value
                with self.assertRaisesRegex(ValueError, "adversarial_opponent.*boolean"):
                    analyze.recorded_configuration(meta)
        for value in (None, True, "oracle", "Generic", [], {}):
            with self.subTest(model=value):
                meta = metadata()
                meta["search"]["opponent_model"] = value
                with self.assertRaisesRegex(ValueError, "opponent_model must"):
                    analyze.recorded_configuration(meta)

    def test_both_changes_need_their_own_ablation_declarations(self):
        old, new = metadata(), metadata()
        old["search"]["mcts"]["adversarial_opponent"] = True
        new["search"]["opponent_model"] = "specialist"
        new["search"]["mcts"]["adversarial_opponent"] = False
        for allowed in ([], ["search.opponent_model"], ["search.mcts.adversarial_opponent"]):
            with self.subTest(allowed=allowed):
                with self.assertRaisesRegex(ValueError, "Undeclared metadata ablation"):
                    analyze.validate_comparison(old, new, True, allowed)
        allowed = ["search.opponent_model", "search.mcts.adversarial_opponent"]
        self.assertEqual(analyze.validate_comparison(old, new, False, allowed), [])
        self.assertEqual(analyze.validate_comparison(old, new, False, ["search.opponent_model", "search.mcts"]), [])
        changes = {change["field"]: change for change in analyze.configuration_changes(old, new)}
        for field in allowed:
            self.assertTrue(changes[field]["after_recorded"])
            self.assertEqual(changes[field]["after_source"], "recorded")
        self.assertFalse(changes[allowed[0]]["before_recorded"])
        self.assertEqual(changes[allowed[0]]["before_source"], "inferred_legacy_generic_opponent")
        self.assertTrue(changes[allowed[1]]["before_recorded"])
        self.assertEqual(changes[allowed[1]]["before_source"], "recorded")

    def test_legacy_viewer_only_to_adversarial_needs_ablation(self):
        old, new = metadata(), metadata()
        new["search"]["mcts"]["adversarial_opponent"] = True
        with self.assertRaisesRegex(ValueError, "Undeclared metadata ablation"):
            analyze.validate_comparison(old, new, True)
        analyze.validate_comparison(old, new, False, ["search.mcts.adversarial_opponent"])
        change, = analyze.configuration_changes(old, new)
        self.assertFalse(change["before_recorded"])
        self.assertEqual(change["before_source"], "inferred_legacy_viewer_only")

    def test_unsupported_mode_combinations_are_rejected(self):
        for model, adversarial, plain in (("specialist", True, False),
                                          ("generic", True, True),
                                          ("specialist", False, True)):
            with self.subTest(model=model, adversarial=adversarial, plain=plain):
                meta = metadata()
                meta["plain"] = plain
                meta["search"]["opponent_model"] = model
                meta["search"]["mcts"]["adversarial_opponent"] = adversarial
                with self.assertRaisesRegex(ValueError, "require"):
                    analyze.recorded_configuration(meta)

    def test_recording_unchanged_defaults_is_not_an_ablation(self):
        old, new = metadata(), metadata()
        new["search"]["opponent_model"] = "generic"
        new["search"]["mcts"]["adversarial_opponent"] = False
        for field in ("search.opponent_model", "search.mcts.adversarial_opponent"):
            with self.assertRaisesRegex(ValueError, "does not change"):
                analyze.validate_comparison(old, new, False, [field])

    def test_overrides_do_not_relax_frozen_inputs(self):
        old, new = metadata(), metadata()
        new["search"]["opponent_model"] = "specialist"
        allowed = ["search.opponent_model"]
        for field in ("baseline", "cards"):
            changed = copy.deepcopy(new)
            changed[field]["fnv1a64"] = "different"
            with self.assertRaisesRegex(ValueError, "different fixed inputs"):
                analyze.validate_comparison(old, changed, True, allowed)

    def test_known_legacy_inference_preserves_isolated_ablation_label(self):
        old, new = metadata(), metadata()
        new["search"]["mcts"]["adversarial_opponent"] = True
        rows, summary = fixture(old)
        keyed = {(row["opponent"], row["seed"], row["seat"]): row for row in rows}
        with patch.object(analyze, "load", side_effect=[(keyed, summary, old), (keyed, summary, new)]):
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                analyze.main(["old", "new"], allow_metadata_ablation=["search.mcts"])
        result = json.loads(output.getvalue())["comparisons"][0]
        self.assertTrue(result["isolated_metadata_ablation"])
        self.assertFalse(result["isolated_search_configuration_comparison"])


class OpponentEvidenceTests(unittest.TestCase):
    def load_fixture(self, meta, rows, summary):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "run.jsonl"
            path.write_text("".join(json.dumps(row) + "\n" for row in rows))
            path.with_suffix(".metadata.json").write_text(json.dumps(meta))
            path.with_suffix(".summary.json").write_text(json.dumps(summary))
            return analyze.load(path)

    def test_legacy_model_field_may_be_absent(self):
        meta = metadata()
        rows, summary = fixture(meta)
        self.assertEqual(len(self.load_fixture(meta, rows, summary)[0]), 2)

    def test_explicit_metadata_requires_matching_raw_model(self):
        for model in ("generic", "specialist"):
            with self.subTest(model=model):
                meta = metadata()
                meta["search"]["opponent_model"] = model
                rows, summary = fixture(meta)
                with self.assertRaisesRegex(ValueError, "missing raw search_opponent_model"):
                    self.load_fixture(meta, rows, summary)
                rows, summary = fixture(meta, raw_model=model)
                self.assertEqual(len(self.load_fixture(meta, rows, summary)[0]), 2)
                rows[0]["game"]["search_opponent_model"] = "specialist" if model == "generic" else "generic"
                with self.assertRaisesRegex(ValueError, "search_opponent_model differs"):
                    self.load_fixture(meta, rows, summary)

    def test_conflicting_outer_and_game_models_are_rejected(self):
        meta = metadata()
        meta["search"]["opponent_model"] = "specialist"
        rows, summary = fixture(meta, raw_model="specialist")
        rows[0]["search_opponent_model"] = "generic"
        with self.assertRaisesRegex(ValueError, "search_opponent_model differs"):
            self.load_fixture(meta, rows, summary)

    def test_adversarial_mode_requires_opponent_diagnostics(self):
        meta = metadata()
        meta["search"]["mcts"]["adversarial_opponent"] = True
        rows, summary = fixture(meta)
        with self.assertRaisesRegex(ValueError, "missing adversarial opponent telemetry"):
            self.load_fixture(meta, rows, summary)
        rows, summary = fixture(meta, counters=True)
        self.assertEqual(len(self.load_fixture(meta, rows, summary)[0]), 2)

    def test_new_counter_presence_types_and_totals_are_validated(self):
        meta = metadata()
        meta["search"]["mcts"]["adversarial_opponent"] = True
        for counter in analyze.OPPONENT_COUNTERS:
            rows, summary = fixture(meta, counters=True)
            rows[0]["game"][counter] = 3
            summary["overall"][counter] = summary["matchups"][0][counter] = 3
            self.assertEqual(len(self.load_fixture(meta, rows, summary)[0]), 2)
            with self.subTest(counter=counter, case="missing"):
                bad = copy.deepcopy(rows)
                del bad[1]["game"][counter]
                with self.assertRaisesRegex(ValueError, "missing adversarial opponent telemetry"):
                    self.load_fixture(meta, bad, summary)
            for bad_value in (False, -1, 3.0):
                with self.subTest(counter=counter, raw=bad_value):
                    bad = copy.deepcopy(rows)
                    bad[0]["game"][counter] = bad_value
                    with self.assertRaisesRegex(ValueError, "invalid telemetry"):
                        self.load_fixture(meta, bad, summary)
            for bad_value in (False, 3.0, 4):
                with self.subTest(counter=counter, total=bad_value):
                    bad = copy.deepcopy(summary)
                    bad["overall"][counter] = bad_value
                    with self.assertRaisesRegex(ValueError, "stale telemetry total"):
                        self.load_fixture(meta, rows, bad)
            bad = copy.deepcopy(summary)
            bad["matchups"][0][counter] = 4
            with self.assertRaisesRegex(ValueError, "stale telemetry total"):
                self.load_fixture(meta, rows, bad)

    def test_opponent_tree_activity_requires_adversarial_mode(self):
        for explicit in (False, True):
            meta = metadata()
            if explicit:
                meta["search"]["mcts"]["adversarial_opponent"] = False
            for counter in analyze.OPPONENT_COUNTERS:
                with self.subTest(explicit=explicit, counter=counter):
                    rows, summary = fixture(meta, counters=True)
                    rows[0]["game"][counter] = 1
                    summary["overall"][counter] = summary["matchups"][0][counter] = 1
                    with self.assertRaisesRegex(ValueError, "opponent tree activity requires adversarial"):
                        self.load_fixture(meta, rows, summary)


class LegacyPerformanceCompatibilityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = Path(__file__).resolve().parent.parent / "generic-policy-v2/summarize_perf_v9.py"
        spec = importlib.util.spec_from_file_location("perf_summary", path)
        cls.helper = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.helper)

    def reports(self):
        current = {"identities": {"deck": {"control": analyze.identity(metadata())}}, "timing": 123}
        saved = copy.deepcopy(current)
        old = saved["identities"]["deck"]["control"]
        del old["search_opponent_model_source"]
        del old["mcts_adversarial_opponent_source"]
        del old["search"]["opponent_model"]
        del old["search"]["mcts"]["adversarial_opponent"]
        return current, saved

    def test_only_expected_new_inferences_are_projected(self):
        current, saved = self.reports()
        original = copy.deepcopy(current)
        self.assertEqual(self.helper.legacy_report_projection(current, saved), saved)
        self.assertEqual(current, original)
        current["timing"] += 1
        self.assertNotEqual(self.helper.legacy_report_projection(current, saved), saved)

    def test_recorded_or_nondefault_settings_are_never_hidden(self):
        for source in ("search_opponent_model_source", "mcts_adversarial_opponent_source"):
            current, saved = self.reports()
            current["identities"]["deck"]["control"][source] = "recorded"
            with self.assertRaisesRegex(ValueError, "Unexpected historical identity"):
                self.helper.legacy_report_projection(current, saved)
        for value in (True, 0):
            current, saved = self.reports()
            current["identities"]["deck"]["control"]["search"]["mcts"]["adversarial_opponent"] = value
            with self.assertRaisesRegex(ValueError, "Unexpected historical identity"):
                self.helper.legacy_report_projection(current, saved)
        current, saved = self.reports()
        current["identities"]["deck"]["control"]["search"]["opponent_model"] = "specialist"
        with self.assertRaisesRegex(ValueError, "Unexpected historical identity"):
            self.helper.legacy_report_projection(current, saved)

    def test_unmeasured_counters_are_omitted_not_zero_filled(self):
        rows = [{"game": {"old": 1}}, {"game": {"old": 2}}]
        self.assertEqual(self.helper.recorded_counter_totals(rows, ["old", "new"]), {"old": 3})
        rows[0]["game"]["new"] = 0
        with self.assertRaisesRegex(ValueError, "Incomplete historical counter"):
            self.helper.recorded_counter_totals(rows, ["old", "new"])


if __name__ == "__main__":
    unittest.main()
