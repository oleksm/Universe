#!/usr/bin/env python3
"""Run the real Walker on owner collision exports; no installed asset changes."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

p = argparse.ArgumentParser(__doc__)
p.add_argument('fixture', type=Path)
p.add_argument('output', type=Path)
p.add_argument('--binary', type=Path, default=Path('target/debug/examples/ramp_threshold'))
a = p.parse_args()
manifest = json.loads((a.fixture / 'manifest.json').read_text())
paths = json.loads((a.fixture / 'walking-path.json').read_text())
reports = []
for route in paths['paths']:
    entry = next(e for e in manifest['outputs'] if e['pose'] == route['pose'])
    glb = a.fixture / (route['pose'] + '.glb')
    assert hashlib.sha256(glb.read_bytes()).hexdigest() == entry['glb_sha256']
    points = [w['surface']['surface_point_gltf_m'] for w in route['waypoints'] if w['surface']]
    # Diagnostic centreline window includes the actual cover and chamfer.
    heel = min(points, key=lambda v: abs(v[2] - 2.0))
    for name, end in [('heel', heel), ('full', points[-1])]:
        result = subprocess.run([str(a.binary.resolve()), str(glb),
                                 ','.join(map(str, points[0])), ','.join(map(str, end))],
                                text=True, capture_output=True)
        if not result.stdout:
            raise RuntimeError(result.stderr)
        report = json.loads(result.stdout)
        assert report['sha256'] == entry['glb_sha256']
        assert report['triangles'] == entry['triangles']
        assert result.returncode in (0, 1), result.stderr
        reports.append(dict(pose=route['pose'], route=name, exit=result.returncode,
                            stderr=result.stderr, **report))
a.output.write_text(json.dumps(dict(manifest=manifest, routes=reports,
    scope='Isolated static snapshots, diagnostic centreline, both directions, 30/60/120 Hz; no combined hull or live motion acceptance.',
    cargo='UNSUPPORTED: aggregate inventory only; registry confirms no carrier/container traversal contract.'), indent=2) + '\n')
for r in reports:
    print(r['pose'], r['route'], 'reached', sum(x['reached_end'] for x in r['results']),
          'continuously supported', sum(x['continuous_support'] for x in r['results']), '/ 6')
