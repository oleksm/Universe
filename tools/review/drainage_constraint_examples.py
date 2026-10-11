"""Small LP counterexamples for the basin pilot review; requires SciPy."""
import json
import numpy as np
from scipy.optimize import linprog


def solve(a, b, strict):
    a, b = np.asarray(a, float), np.asarray(b, float)
    r = linprog(np.zeros(a.shape[1]), A_ub=a, b_ub=b,
                bounds=[(None, None)] * a.shape[1], method='highs',
                options={'primal_feasibility_tolerance': 1e-9,
                         'dual_feasibility_tolerance': 1e-9} if strict else {})
    return dict(success=bool(r.success), status=int(r.status),
                maximum_violation=None if r.x is None else float(np.max(a @ r.x - b)))


# A fixed-only zero row may be discarded as feasible at the default tolerance.
# It is impossible at any x. Values match the pilot's engineering grade.
zero = {str(strict): solve([[0]], [-1e-7], strict) for strict in (False, True)}
assert zero['False']['success'] and not zero['True']['success']
# Synthetic single triangle, heights [A,B,C]. River A -> midpoint(B,C).
# Independently chosen receiver edges B->A and C->A form an acyclic tree.
# Their inequalities contradict the canonical river; no mesh defect needed.
river = [[-1, .5, .5]]
tree = [[1, -1, 0], [1, 0, -1]]
results = {name: solve(rows, [-1] * len(rows), True)
           for name, rows in [('river', river), ('tree', tree), ('joint', river + tree)]}
assert results['river']['success'] and results['tree']['success']
assert not results['joint']['success']
# Exact positive certificate: river + half of each tree row = 0 <= -2.
assert np.allclose(np.array([1, .5, .5]) @ np.array(river + tree), 0)
print(json.dumps(dict(zero_row=zero, synthetic_triangle=results,
                     certificate='river + 0.5*tree_B + 0.5*tree_C: 0 <= -2',
                     scope='Synthetic construction evidence, not identification of the v8 conflict'), indent=2))
