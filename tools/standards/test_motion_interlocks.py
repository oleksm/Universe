"""Semantic failures which JSON Schema alone cannot reject."""
import copy
import math
import unittest
from validate import motion_interlocks


class MotionInterlocks(unittest.TestCase):
    def setUp(self):
        self.record = {"motion": [
            {"id": "slew", "kind": "rotate", "angle": [-math.radians(95), math.radians(95)]},
            {"id": "luff", "kind": "tilt", "angle": [-0.2094, 0.16],
             "interlock": {"axis": "slew", "when_abs_below": math.radians(12), "angle": [-0.2094, 0.08]}}]}

    def test_valid_and_absent(self):
        self.assertEqual(motion_interlocks(self.record), [])
        self.assertEqual(motion_interlocks({}), [])

    def test_missing_or_self_axis(self):
        for axis in ("missing", "luff"):
            with self.subTest(axis=axis):
                self.record["motion"][1]["interlock"]["axis"] = axis
                self.assertTrue(motion_interlocks(self.record))

    def test_translating_axis(self):
        self.record["motion"][0]["kind"] = "travel"
        self.assertTrue(motion_interlocks(self.record))

    def test_invalid_intervals(self):
        for interval in ([0.08, -0.2094], [-0.3, 0.08], [-0.2094, 0.2]):
            with self.subTest(interval=interval):
                self.record["motion"][1]["interlock"]["angle"] = interval
                self.assertTrue(motion_interlocks(self.record))

    def test_unbounded_or_duplicate(self):
        record = copy.deepcopy(self.record)
        del record["motion"][1]["angle"]
        self.assertTrue(motion_interlocks(record))
        self.record["motion"].append(copy.deepcopy(self.record["motion"][0]))
        self.assertTrue(motion_interlocks(self.record))


if __name__ == "__main__":
    unittest.main()
