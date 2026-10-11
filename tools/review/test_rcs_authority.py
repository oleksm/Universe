"""Analytical LP controls: rank six alone must not imply positive controllability."""
import unittest
import numpy as np
from rcs_authority import review

class AuthorityTest(unittest.TestCase):
    def check_columns(self, columns, expected):
        case = review({'cases':[{'id':'analytic','mass_kg':1,'acceleration_columns':columns.tolist()}]})['cases'][0]
        self.assertEqual(case['rank'], 6)
        self.assertEqual(case['positive_authority_all_axes'], expected)
        np.testing.assert_allclose(case['weak_signed_limits'], [1 if expected else 0]*6)
    def test_bidirectional_bounded_controls(self):
        self.check_columns(np.concatenate([np.eye(6),-np.eye(6)],axis=1).T, True)
    def test_rank_six_one_sided_is_not_controllable(self):
        self.check_columns(np.eye(6), False)

if __name__ == '__main__':
    unittest.main()
