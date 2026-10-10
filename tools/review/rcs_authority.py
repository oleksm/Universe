#!/usr/bin/env python3
"""Independent bounded positive-thrust LP from mc07_rcs_review's acceleration columns.
Usage: rcs_authority.py COLUMNS.json OUTPUT.json [CONTROLS.json]
Requires numpy/scipy. Pure single-axis limits impose zero acceleration in the other
five axes. No operating targets, ratings, mass or physics are changed.
"""
import json
import sys
import numpy as np
from scipy.optimize import linprog


def review(data):
    cases = []
    for case in data['cases']:
        a = np.asarray(case['acceleration_columns'], dtype=float).T
        scale = np.maximum(np.max(np.abs(a), axis=1), 1e-15)
        normalized = a / scale[:, None]
        limits = []
        weak = []
        max_residual = 0.0
        for axis in range(6):
            capacities = []
            for sign in (1, -1):
                rows = [r for r in range(6) if r != axis]
                result = linprog(-sign * normalized[axis], A_eq=normalized[rows],
                                 b_eq=np.zeros(5), bounds=[(0, 1)] * a.shape[1], method='highs')
                if not result.success:
                    raise RuntimeError((case['id'], axis, sign, result.message))
                residual = float(np.max(np.abs(normalized[rows] @ result.x)))
                if residual > 1e-7:  # numerical equality residual, not a flight tolerance
                    raise RuntimeError(('LP equality residual', residual))
                max_residual = max(max_residual, residual)
                capacity = max(0.0, float(sign * (a @ result.x)[axis]))
                capacities.append(capacity)
                limits.append({'axis': axis, 'sign': sign, 'capacity': capacity,
                               'duty': result.x.tolist(), 'equality_residual': residual})
            weak.append(min(capacities))
        # Review commands at 25% of each weakest signed pure-axis limit. The 25%
        # is an invented diagnostic test amplitude, not a device operating rule.
        targets = []
        for axis in range(6):
            for sign in (1, -1):
                acceleration = [0.0] * 6
                acceleration[axis] = sign * 0.25 * weak[axis]
                targets.append({'name': f'axis{axis}:{sign:+}', 'acceleration': acceleration})
        # Convex combinations of the feasible axial requests remain feasible.
        for signs in [(1, 1, 1, 1, 1, 1), (-1, 1, -1, 1, -1, 1)]:
            targets.append({'name': f'mixed:{signs}',
                            'acceleration': [s * w / 12 for s, w in zip(signs, weak)]})
        cases.append({'id': case['id'], 'mass_kg': case['mass_kg'],
                      'rank': int(np.linalg.matrix_rank(normalized)),
                      'positive_authority_all_axes': all(v > 1e-10 for v in weak),
                      'weak_signed_limits': weak, 'max_equality_residual': max_residual,
                      'limits': limits, 'targets': targets})
    return {'provenance': {k:v for k,v in data.items() if k != 'cases'}, 'method': 'bounded positive-thrust LP; RCS only; zero unwanted accelerations',
            'units': ['m/s2'] * 3 + ['rad/s2'] * 3, 'cases': cases}


if __name__ == '__main__':
    with open(sys.argv[1]) as f:
        result = review(json.load(f))
    if len(sys.argv) > 3:
        with open(sys.argv[3]) as f:
            controls = json.load(f)
        for key in ['model_sha256', 'sockets_json_sha256', 'sockets_glb_sha256']:
            assert controls[key] == result['provenance'][key], key
        for case, raw in zip(result['cases'], controls['cases']):
            assert case['id'] == raw['id']
            caps = np.asarray(case['weak_signed_limits'])
            summaries = []
            for mode in ['rcs_only', 'flight_main_off']:
                for step in [0, 1]:
                    worst = None
                    for target in raw['controls']:
                        if target['mode'] != mode:
                            continue
                        wanted = np.asarray(target['wanted'])
                        sample = target['samples'][step]
                        actual = np.asarray(sample['acceleration'])
                        error = float(np.max(np.abs((actual-wanted)/caps)) / np.max(np.abs(wanted/caps)))
                        if worst is None or error > worst['normalized_error']:
                            worst = {'mode': mode, 'call': sample['step'], 'target': target['name'],
                                     'normalized_error': error, 'wanted': wanted.tolist(), 'actual': actual.tolist()}
                    summaries.append(worst)
            case['allocator_review'] = summaries
            case['com_shape_m'] = raw['com_shape_m']
            case['inertia_column_major'] = raw['inertia_column_major']
            case['runtime_turn_envelope'] = raw['runtime_turn_envelope']
    with open(sys.argv[2], 'w') as f:
        json.dump(result, f, indent=2)
        f.write('\n')
    for case in result['cases']:
        print(case['id'], 'mass', case['mass_kg'], 'limits', case['weak_signed_limits'])
