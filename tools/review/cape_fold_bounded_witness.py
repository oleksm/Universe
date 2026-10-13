"""Verify the Cape Fold v2 collar certificate against the saved sparse system.
Usage: python cape_fold_bounded_witness.py OUTPUT_JSON
No fitting, rendering or production changes.
"""
import hashlib
import json
import sys
from collections import defaultdict
from fractions import Fraction
from pathlib import Path

import numpy as np

root = Path('/home/alexm/git/planet-trees/out/benchmarks')
trace_path = root / 'scenery-cape-fold-conflict-v2/trace.json'
dump_path = root / 'scenery-basin-s15-cape-fold-v2/constraint_system.npz'
trace = json.loads(trace_path.read_text())
d = np.load(dump_path)
coefficients = defaultdict(Fraction)
rhs = Fraction(0)
path_edges = []
weights = []
for row in trace['rows']:
    kind = row['provenance']['kind']
    if kind == 'collar_receiver':
        i = row['row']
        lo, hi = d['A_indptr'][i:i+2]
        actual = {int(d['variable'][j]): float(c) for j, c in
                  zip(d['A_indices'][lo:hi], d['A_data'][lo:hi])}
        assert actual == dict(zip(row['vertices'], row['coefficients']))
        assert float(d['rhs'][i]) == row['rhs']
        assert len(actual) == 2 and sum(actual.values()) == 0
        weight = 1 / Fraction(max(actual.values()))
        path_edges.append(row['provenance']['vertices'])
    else:
        assert kind in ('adjustment_upper_bound', 'adjustment_lower_bound')
        assert row['rhs'] == 150 and len(row['vertices']) == 1
        assert row['coefficients'] == ([1.] if kind.endswith('upper_bound') else [-1.])
        assert row['vertices'][0] in d['variable']
        weight = Fraction(1)
    for vertex, coefficient in zip(row['vertices'], row['coefficients']):
        coefficients[vertex] += weight * Fraction(coefficient)
        assert not d['fixed'][vertex]
    rhs += weight * Fraction(row['rhs'])
    weights.append({'row': row['row'], 'exact_positive_weight': str(weight)})
assert all(c == 0 for c in coefficients.values()) and rhs < 0
successors = dict(path_edges)
start, = set(successors) - set(successors.values())
path = [start]
while path[-1] in successors:
    path.append(successors[path[-1]])
assert len(path) == 8 and len(set(path)) == 8
rise = float(d['target'][path[-1]] - d['target'][path[0]])
report = {
    'trace_sha256': hashlib.sha256(trace_path.read_bytes()).hexdigest(),
    'constraint_dump_sha256': hashlib.sha256(dump_path.read_bytes()).hexdigest(),
    'path': path, 'target_endpoint_rise_m': rise,
    'exact_coefficient_sum': '0', 'exact_weighted_rhs': str(rhs),
    'contradiction_gap_m': float(-rhs),
    'path_only_minimum_symmetric_adjustment_m': (300 + float(-rhs))/2,
    'weights': weights,
    'scope': 'Seven stored collar rows plus the stated +/-150m adjustment bounds; not a global minimum or proof that alternate routing is feasible.'
}
Path(sys.argv[1]).write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({k: v for k, v in report.items() if k != 'weights'}, indent=2))
