"""visual.axes: every perpendicular pair is a proper rotation into the design frame (right +X, forward +Y, up +Z);
parallel pairs are refused; absent is the identity.

    python3 tools/standards/test_axes.py
"""
import itertools, os, sys, unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from assets import AXES, axes_problem, axes_rotation, to_design  # noqa: E402


def det(m):
    (a, b, c), (d, e, f), (g, h, i) = m
    return a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)


class AxesTest(unittest.TestCase):
    def test_absent_is_identity(self):
        self.assertEqual(axes_rotation(None), ((1, 0, 0), (0, 1, 0), (0, 0, 1)))
        self.assertEqual(axes_rotation({"forward": "+Y", "up": "+Z"}), axes_rotation(None))

    def test_every_perpendicular_pair_is_a_proper_rotation(self):
        pairs = [(f, u) for f, u in itertools.product(AXES, AXES) if axes_problem({"forward": f, "up": u}) is None]
        self.assertEqual(len(pairs), 24)         # the cube's rotation group
        seen = set()
        for f, u in pairs:
            m = axes_rotation({"forward": f, "up": u})
            for i, j in itertools.product(range(3), range(3)):
                self.assertEqual(sum(a * b for a, b in zip(m[i], m[j])), 1 if i == j else 0, (f, u))
            self.assertEqual(det(m), 1, (f, u))
            self.assertEqual(to_design({"forward": f, "up": u}, AXES[f]), (0, 1, 0))
            self.assertEqual(to_design({"forward": f, "up": u}, AXES[u]), (0, 0, 1))
            seen.add(m)
        self.assertEqual(len(seen), 24)

    def test_parallel_and_unknown_refused(self):
        for f, u in itertools.product(AXES, AXES):
            if f[1] == u[1]:
                self.assertIn("parallel", axes_problem({"forward": f, "up": u}))
                self.assertRaises(ValueError, axes_rotation, {"forward": f, "up": u})
        self.assertIsNotNone(axes_problem({"forward": "+W", "up": "+Z"}))
        self.assertIsNotNone(axes_problem({"forward": "+Y"}))

    def test_engine_models(self):
        # The CH engines' models thrust along -Z with +Y up (engine's Studio mismatch, f6e2f130 on main).
        ax = {"forward": "-Z", "up": "+Y"}
        self.assertEqual(to_design(ax, (0, 0, -1)), (0, 1, 0))
        self.assertEqual(to_design(ax, (0, 1, 0)), (0, 0, 1))
        self.assertEqual(to_design(ax, (1, 0, 0)), (1, 0, 0))


if __name__ == "__main__":
    unittest.main()
