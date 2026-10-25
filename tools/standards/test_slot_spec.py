"""A slot is a spec: its limits, a product judged against them, and the ship judged from its fitted products."""
import unittest
import numpy as np
import slot_spec

MOUNTS = {"mount.e": {"identity": {"key": "mount.e", "slot": "engine", "size_class": 1},
                      "envelope": {"length": 1, "width": 1, "height": 1}, "bears": {"mass": 100, "thrust": 1000}}}
SLOT = {"name": "main", "kind": "engine", "size": 1, "mount": "mount.e", "role": "main",
        "spec": [{"of": "nozzles", "min": 1}, {"of": "push", "axis": [0, 0, -1], "within": 30}]}


def engine(**over):
    q = {"identity": {"key": "e", "slot": "engine"}, "size_class": 1, "fits": "mount.e",
         "physical": {"mass": 50, "length": 0.5, "width": 0.5, "height": 0.5},
         "function": {"kind": "engine", "thrust": 800, "burns": "material.x"}, "nozzles": [{"node": "nozzle_a", "share": 1}]}
    q.update(over)
    return q


class SlotSpec(unittest.TestCase):
    def setUp(self):
        self.saved = slot_spec.product_frames
        slot_spec.product_frames = lambda q: {"nozzle_a": np.eye(4)}   # exhaust along the product's -Z

    def tearDown(self):
        slot_spec.product_frames = self.saved

    def test_the_spec_is_the_mounts_limits_and_the_slots(self):
        froms = {(L["of"], L["from"].split(" ")[0]) for L in slot_spec.limits(SLOT, MOUNTS)}
        self.assertTrue({("physical.mass", "mount.e"), ("function.thrust", "mount.e"), ("push", "slot")} <= froms)

    def test_a_product_is_judged_on_every_limit(self):
        bad = lambda q, f: {L["of"] for L, _, ok, _ in slot_spec.judge(q, slot_spec.limits(SLOT, MOUNTS), f) if ok is False}
        flip = np.diag([1.0, 1, -1, 1])                      # the slot turned round: the exhaust goes forward
        self.assertEqual(bad(engine(), flip), set())          # pushes along -Z, the nose
        self.assertIn("push", bad(engine(), np.eye(4)))
        self.assertIn("physical.mass", bad(engine(physical={"mass": 500, "length": 0.5, "width": 0.5, "height": 0.5}), flip))
        self.assertIn("nozzles", bad(engine(nozzles=None), flip))

    def test_the_ship_is_judged_from_its_fitted_products(self):
        hull = {"physical": {"mass": 50}, "spec": [{"of": "ship.main_to_weight", "min": 1, "gravity": 1.0},
                                                    {"of": "ship.unfed_engines", "max": 0}]}
        hf = {"mount_main": np.diag([1.0, 1, -1, 1])}
        vals = slot_spec.ship_values(hull, {"main": SLOT}, [{"slot": "main", "item": "e"}], {"e": engine()}, hf)
        res = {L["of"]: ok for L, _, ok, _ in slot_spec.judge_ship(hull, vals)}
        self.assertEqual(res, {"ship.main_to_weight": True, "ship.unfed_engines": False})


class Conformance(unittest.TestCase):
    """ship-rule.mass-properties cases: the reference must reproduce them, as the game must."""
    def test_every_case(self):
        import yaml, ship
        from ship_qualify import load
        rule = yaml.safe_load(open(slot_spec.RULE))
        hulls, eq = load("hulls"), load("equipment")
        for case in rule.get("cases") or []:
            h = ship.record(case["ship"], hulls)
            if slot_spec.hull_model(h) is None:
                self.skipTest("no model matching model_sha256")
            v = slot_spec.ship_values(h, {s["name"]: s for s in h["slots"]}, h.get("fit") or [], eq, slot_spec.hull_frames(h))
            for e in case["expect"]:
                name, _, ax = e["of"].rpartition(".") if e["of"].count(".") > 1 else (e["of"], "", "")
                got = v[name] if not ax else (v[name]["xyz".index(ax[0])])
                self.assertLessEqual(abs(got - e["value"]), e["tolerance"], f"{case['ship']} {e['of']}: {got} vs {e['value']}")


if __name__ == "__main__":
    unittest.main()
