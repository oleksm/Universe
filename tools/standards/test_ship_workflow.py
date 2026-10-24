"""Workflow definitions cannot prove a build possible with circular prerequisites."""
import unittest
import re
import hashlib
import yaml
from validate import workflow_dependencies, validate, schema, key_of
from pathlib import Path

HERE = Path(__file__).resolve().parents[2] / "standards/SpaceshipBuilding/schema"


def records(edges):
    return [(key, key + ".yaml", {"depends_on": deps}, {}) for key, deps in edges.items()]


class ShipWorkflow(unittest.TestCase):
    def test_dag_and_unrelated_links(self):
        self.assertEqual(workflow_dependencies(records({
            "ship-stage.brief": [], "ship-stage.release": ["ship-stage.brief"],
            "ship-artifact.design": [], "ship-artifact.export": ["ship-artifact.design"]})), [])

    def test_cycles_and_duplicate_edges(self):
        for edges in ({"ship-stage.a": ["ship-stage.a"]},
                      {"ship-artifact.a": ["ship-artifact.b"], "ship-artifact.b": ["ship-artifact.a"]},
                      {"ship-stage.a": ["ship-stage.b", "ship-stage.b"], "ship-stage.b": []}):
            with self.subTest(edges=edges):
                self.assertTrue(workflow_dependencies(records(edges)))

    def test_malformed_prerequisites_are_left_to_schema(self):
        for deps in (None, 42, [{"key": "ship-stage.a"}]):
            self.assertEqual(workflow_dependencies(records({"ship-stage.a": deps})), [])

    def test_definition_rejects_instance_and_runtime_fields(self):
        p = str(HERE / "ship-artifact.schema.yaml")
        rec = {"identity": {"key": "ship-artifact.design", "name": "Design", "about": "Intent"},
               "status": "draft", "implementation": "proposed", "owner": "ships",
               "sources": [{"document": "spec.md", "section": "2.4"}],
               "purpose": "Keep intent", "contents": ["Requirements"], "boundary": "Authoring only"}
        self.assertEqual(validate(rec, schema(p), p), [])
        for field in ("instances", "placements", "geometry_ready", "runtime_verified"):
            with self.subTest(field=field):
                self.assertTrue(validate(dict(rec, **{field: []}), schema(p), p))

    def test_source_sections_and_pilot_definitions_remain_covered(self):
        root = HERE.parents[2]
        coverage = yaml.safe_load((root / "docs/spaceship-building-coverage.yaml").read_text())
        source = root / coverage["source_document_in_registry"]
        self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), coverage["normalized_document_sha256"])
        proposal = source.read_text()
        sections = set(re.findall(r"^### (\d+\.\d+) ", proposal, re.M))
        self.assertEqual(sections, {item["source_section"] for item in coverage["coverage"]})
        definitions = {rec["identity"]["key"]: rec
                       for path in (HERE.parent / "metadata").glob("*/*.yaml")
                       for rec in [yaml.safe_load(path.read_text())]}
        mapped = {key for item in coverage["coverage"] for key in item["records"]}
        self.assertEqual(set(definitions), mapped)
        self.assertTrue({f"ship-check.pilot-{i:02}" for i in range(1, 17)} <= mapped)
        self.assertEqual({d["order"] for k, d in definitions.items() if k.startswith("ship-stage.")}, set(range(9)))
        for rec in definitions.values():
            for source in rec["sources"]:
                if source["document"].startswith("docs/"):
                    self.assertTrue((root / source["document"]).is_file(), source["document"])

    def test_hull_and_sockets_delivery_stop_stays_defined(self):
        meta = HERE.parent / "metadata"
        load = lambda p: yaml.safe_load((meta / p).read_text())
        scope = load("rules/scope.yaml")["statement"]
        for phrase in ("hull and sockets only", "empty installed fit", "players in the shipyard",
                       "never widen it", "not a hull defect", "cannot be laundered into generic sockets"):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, scope)
        for word in ("'Full auto'", "'continue'", "'finish'", "'ready'", "'install'"):
            self.assertIn(word, scope)
        self.assertIn("installed fit stays empty", load("rules/module-reuse.yaml")["statement"])
        for stage in ("stages/brief.yaml", "stages/architecture.yaml", "stages/integration.yaml"):
            with self.subTest(stage=stage):
                self.assertTrue(any("1.1.1" in line for line in load(stage)["exit"]))

    def test_explorer_is_a_hull_and_sockets_delivery(self):
        """Owner 2026-10-24: hull + sockets only. No full-auto request may install a loadout on it."""
        rec = yaml.safe_load((HERE.parents[2] / "standards/SFO/metadata/hulls/explorer-study.yaml").read_text())
        self.assertEqual(rec.get("fit", []), [])
        self.assertEqual(rec.get("thrusters", []), [])

    def test_path_defines_key(self):
        self.assertEqual(key_of("SpaceshipBuilding/metadata/checks/raw-frame.yaml", {}),
                         "ship-check.raw-frame")


if __name__ == "__main__":
    unittest.main()
