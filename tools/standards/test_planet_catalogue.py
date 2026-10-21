"""Boundaries and negative cases for the six PlanetSim catalogue cross-checks."""
import copy
import contextlib
import io
import json
import tempfile
from unittest.mock import patch
from pathlib import Path
import unittest
from planet_catalogue import main, audit, derived_ladder, example_sources, LEVELS, VIEWS
from validate import validate, schema, size_ranges
from planet_lod import context

SCHEMA = str(Path(__file__).resolve().parents[2] / "standards/PlanetSim/schema/planet-feature.schema.yaml")


def feature(name="parent"):
    return {"identity": {"key": "planet-feature." + name, "name": name, "about": "Meaningful ground"},
            "register": "feature", "family": "volcanic", "trigger": "eruption",
            "description": "Form, origin, orbit and ground appearance", "size": {"across_m": [100, 1000],
            "basis": [{"of": ["across_m"], "tier": "invented", "review": True, "note": "test"}]},
            "ladder": derived_ladder([100, 1000]), "shape": {"plan": "cone", "blend": "max"}}


def records():
    result = {key: {"span": span, "spacing": [span / 1000, span / 1000]} for key, span in zip(LEVELS, VIEWS)}
    result["planet-feature.parent"] = feature()
    for line, key in (("river", "trunk-river"), ("divide", "drainage-divide")):
        rec = feature(key)
        rec["line_kinds"] = ["planet-line." + line]
        result["planet-feature." + key] = rec
        result["planet-line." + line] = {"feature_kinds": ["planet-feature." + key]}
    return result


class PlanetCatalogue(unittest.TestCase):
    def test_inclusive_decade_boundaries(self):
        for index, view in enumerate(VIEWS):
            got = derived_ladder([view / 100, view / 100])
            self.assertEqual(got[0], LEVELS[index])
        self.assertEqual(derived_ladder([100, 1000]), [LEVELS[2], LEVELS[5]])
        self.assertEqual(derived_ladder([1e6, 1.5e7]), [LEVELS[0], LEVELS[1]])
        self.assertEqual(derived_ladder([.01, .01]), [LEVELS[7], LEVELS[7]])

    def test_context_includes_intermediate_ladder_levels(self):
        recs = records()
        recs["planet-feature.parent"]["levels"] = [LEVELS[6]]  # obsolete context hint
        grouped = {kind: {k: r for k, r in recs.items() if k.startswith(kind + ".")}
                   for kind in ("planet-level", "planet-feature", "planet-line")}
        self.assertIn("planet-feature.parent", context(grouped, "site")["records"]["planet-feature"])
        self.assertNotIn("planet-feature.parent", context(grouped, "ground")["records"].get("planet-feature", []))

    def test_unrepresentable_or_invalid_sizes(self):
        for size in ([0, 1], [-1, 1], [2, 1], [.001, .001], [1e8, 1e8], [float("nan"), 1]):
            with self.subTest(size=size), self.assertRaises(ValueError):
                derived_ladder(size)

    def test_valid_and_wrong_ladder(self):
        recs = records()
        self.assertEqual(audit(recs, ["planet-feature.parent"]), [])
        recs["planet-feature.parent"]["ladder"] = [LEVELS[0], LEVELS[7]]
        self.assertTrue(any(i["check"] == 1 for i in audit(recs, [])))

    def test_child_scale_missing_refs_and_cycles(self):
        recs = records()
        child = feature("child")
        child["size"]["across_m"] = [1e5, 1e6]
        child["ladder"] = derived_ladder(child["size"]["across_m"])
        recs["planet-feature.child"] = child
        recs["planet-feature.parent"]["children"] = ["planet-feature.child", "planet-feature.missing"]
        child["grows_into"] = [{"kind": "planet-feature.parent", "rule": "test"}]
        messages = [i["message"] for i in audit(recs, []) if i["check"] == 2]
        self.assertTrue(any("coarser" in m for m in messages))
        self.assertTrue(any("unknown" in m for m in messages))
        self.assertTrue(any("cycle" in m for m in messages))

    def test_reciprocal_parent_child_is_not_cycle(self):
        recs = records()
        recs["planet-feature.child"] = feature("child")
        recs["planet-feature.parent"]["children"] = ["planet-feature.child"]
        recs["planet-feature.child"]["parent_kinds"] = ["planet-feature.parent"]
        self.assertEqual(audit(recs, []), [])

    def test_built_and_landmark_evidence(self):
        recs = records()
        recs["planet-feature.parent"]["built"] = True
        issues = [i for i in audit(recs, []) if i["check"] == 3]
        self.assertEqual(len(issues), 3)
        recs["planet-feature.parent"].pop("built")
        recs["planet-feature.parent"]["register"] = "landmark"
        self.assertEqual(len([i for i in audit(recs, []) if i["check"] == 3]), 1)

    def test_emitted_manifest_coverage(self):
        self.assertEqual([i["key"] for i in audit(records(), ["planet-feature.unregistered"]) if i["check"] == 4],
                         ["planet-feature.unregistered"])

    def test_observed_run_gap_fails_only_in_strict_migration_mode(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "features.jsonl"
            path.write_text('{"kind":"planet-feature.unregistered"}\n')
            for strict, expected in ((False, 0), (True, 1)):
                args = ["planet_catalogue.py", "--json", "--features-jsonl", str(path)]
                if strict:
                    args.append("--strict")
                output = io.StringIO()
                with patch("planet_catalogue.load", return_value=records()), patch("planet_catalogue.manifest", return_value=[]), patch("sys.argv", args), contextlib.redirect_stdout(output):
                    with self.assertRaises(SystemExit) as raised:
                        main()
                self.assertEqual(raised.exception.code, expected)
                self.assertEqual(json.loads(output.getvalue())[0]["check"], 4)

    def test_blend_restrictions(self):
        for blend, register, flat, allowed in (("material", "cover", False, True), ("material", "feature", False, False),
                ("level", "water", False, True), ("level", "feature", True, True), ("level", "feature", False, False)):
            with self.subTest(blend=blend, register=register, flat=flat):
                recs = records(); rec = recs["planet-feature.parent"]
                rec["register"] = register; rec["shape"] = {"plan": "floor", "blend": blend, "flat_floor": flat}
                self.assertEqual(not any(i["check"] == 5 for i in audit(recs, [])), allowed)

    def test_line_pair_must_be_reciprocal(self):
        recs = records(); recs["planet-line.river"]["feature_kinds"] = []
        self.assertTrue(any(i["check"] == 6 for i in audit(recs, [])))

    def test_schema_ranges_lat_lon_and_cover_sources(self):
        rec = feature(); sch = schema(SCHEMA)
        self.assertEqual(validate(rec, sch, SCHEMA), [])
        for bounds in ([0, 1], [-1, 1], [1, float("inf")]):
            bad = copy.deepcopy(rec); bad["size"]["across_m"] = bounds
            self.assertTrue(validate(bad, sch, SCHEMA))
        rec["size"]["across_m"] = [2, 1]
        self.assertTrue(size_ranges(rec))
        example = {"name": "Example", "lat": 91, "lon": 181, "size": {"across_m": 100}, "source": "test",
                   "source_data": {"name": "WorldCover", "source": "test"}}
        rec["examples"] = [example]
        self.assertTrue(validate(rec, sch, SCHEMA))
        example["lat"], example["lon"] = 90, -180
        rec["size"]["across_m"] = [100, 1000]
        self.assertEqual(validate(rec, sch, SCHEMA), [])
        self.assertTrue(example_sources(rec))
        rec["register"] = "cover"
        self.assertEqual(example_sources(rec), [])


if __name__ == "__main__":
    unittest.main()
