"""ship-check.socket-qualification: what passes and what each criterion catches."""
import hashlib, json, os, struct, tempfile, unittest
import ship_qualify
from ship_qualify import qualify

MOUNTS = {
    "mount.small": {"identity": {"key": "mount.small", "slot": "engine", "size_class": 1},
                    "envelope": {"length": 0.5, "width": 0.5, "height": 0.5}, "bears": {"mass": 100, "thrust": 1000}},
    "mount.huge": {"identity": {"key": "mount.huge", "slot": "engine", "size_class": 1},
                   "envelope": {"length": 7, "width": 3, "height": 3}, "bears": {"mass": 40000}},
}
EQ = {"equipment.block": {"identity": {"key": "equipment.block"}, "fits": "mount.small", "size_class": 1,
                          "physical": {"mass": 15, "length": 0.4, "width": 0.4, "height": 0.4},
                          "function": {"kind": "engine", "thrust": 500}}}


def hull(**over):
    h = {"identity": {"key": "hull.t"}, "physical": {"mass": 1000, "length": 4, "width": 2, "height": 2},
         "slots": [{"name": "rcs_1", "kind": "engine", "size": 1, "mount": "mount.small"}],
         "fit": [{"slot": "rcs_1", "item": "equipment.block"}],
         "thrusters": [{"nozzle": "n1", "slot": "rcs_1", "share": 0.5}, {"nozzle": "n2", "slot": "rcs_1", "share": 0.5}]}
    h.update(over)
    return h


def crit(h):
    return {c for c, _ in qualify(h, MOUNTS, EQ)[0]}


class SocketQualification(unittest.TestCase):
    def test_a_small_hull_with_fitting_sockets_passes(self):
        self.assertEqual(crit(hull()), set())

    def test_a_mount_bigger_than_the_hull_is_caught(self):
        self.assertIn("S3", crit(hull(slots=[{"name": "rcs_1", "kind": "engine", "size": 1, "mount": "mount.huge"}], fit=[], thrusters=[])))

    def test_mount_envelopes_together_past_the_hull_box(self):
        slots = [{"name": f"s{i}", "kind": "engine", "size": 1, "mount": "mount.small"} for i in range(200)]
        self.assertIn("S4", crit(hull(slots=slots, fit=[], thrusters=[])))

    def test_the_games_fit_rule(self):
        heavy = dict(EQ["equipment.block"], physical={"mass": 500, "length": 0.4, "width": 0.4, "height": 0.4})
        self.assertIn("F3", {c for c, _ in qualify(hull(), MOUNTS, {"equipment.block": heavy})[0]})
        unmounted = dict(EQ["equipment.block"], fits=None)
        self.assertIn("F3", {c for c, _ in qualify(hull(), MOUNTS, {"equipment.block": unmounted})[0]})
        self.assertIn("F2", crit(hull(fit=[{"slot": "rcs_1", "item": "equipment.none"}])))

    def test_engine_shares(self):
        self.assertIn("T3", crit(hull(thrusters=[{"nozzle": "n1", "slot": "rcs_1", "share": 1}, {"nozzle": "n2", "slot": "rcs_1", "share": 1}])))
        self.assertIn("T1", crit(hull(thrusters=[{"nozzle": "n1", "slot": "nowhere", "share": 1}])))

    def test_no_mass_and_duplicate_slots(self):
        self.assertIn("H1", crit(hull(physical={"length": 4, "width": 2, "height": 2})))
        s = {"name": "rcs_1", "kind": "engine", "size": 1, "mount": "mount.small"}
        self.assertIn("S1", crit(hull(slots=[s, s])))

    def test_the_model_read_is_the_copy_the_record_pins(self):
        def glb(d, names):
            j = json.dumps({"nodes": [{"name": n} for n in names]}).encode()
            os.makedirs(os.path.join(d, "m"), exist_ok=True)
            b = b"glTF" + struct.pack("<II", 2, 20 + len(j)) + struct.pack("<I", len(j)) + b"JSON" + j
            open(os.path.join(d, "m/h.glb"), "wb").write(b)
            return hashlib.sha256(b).hexdigest()
        with tempfile.TemporaryDirectory() as old, tempfile.TemporaryDirectory() as new:
            glb(old, [])
            pin = glb(new, ["mount_rcs_1", "n1", "n2"])
            saved = ship_qualify.ROOT, ship_qualify.GAME
            ship_qualify.ROOT, ship_qualify.GAME = old, new
            try:
                self.assertNotIn("S5", crit(hull(model="m/h.glb", model_sha256=pin)))
                faults, unchecked = qualify(hull(model="m/h.glb", model_sha256="0" * 64), MOUNTS, EQ)
                self.assertNotIn("S5", {c for c, _ in faults})
                self.assertTrue(unchecked)
            finally:
                ship_qualify.ROOT, ship_qualify.GAME = saved


if __name__ == "__main__":
    unittest.main()
