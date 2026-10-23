#!/usr/bin/env python3
"""Open a pinned Explorer assembly in the game's existing static showcase."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pose', choices=['open', 'closed'], default='open')
    parser.add_argument('--binary', type=Path, help='Built freefall executable (defaults to target/release/freefall)')
    parser.add_argument('--check', action='store_true', help='Verify exact review bytes and print the launch command without starting the game')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    packet = root / 'tools/review/explorer-study'
    binding = json.loads((packet / 'current-export-bindings.json').read_bytes())
    entry = next(e for e in binding['exports'] if e['pose'] == args.pose)
    model = packet / Path(entry['path']).name
    data = model.read_bytes()
    if hashlib.sha256(data).hexdigest() != entry['sha256'] or len(data) != entry['bytes']:
        parser.error(f'{model}: review bytes differ from the Blender binding')
    binary = (args.binary or root / 'target/release/freefall').expanduser().resolve()
    # The showcase still initializes the normal world. Use a separate save area.
    scratch = Path(os.environ.get('XDG_CACHE_HOME', str(Path.home() / '.cache'))) / 'freefall/explorer-study-review'
    env = os.environ.copy()
    for name in ['UNIVERSE_HULL', 'UNIVERSE_TOOL', 'UNIVERSE_EQUIPMENT_MOUNTS']:
        env.pop(name, None)
    env.update(UNIVERSE_TEST='1', UNIVERSE_SCENARIO='showcase',
               UNIVERSE_MODEL=str(model), UNIVERSE_MODEL_CLEAN='1',
               XDG_DATA_HOME=str(scratch / 'data'))
    print(json.dumps({'scope': 'Static assembled review; not a commissioned Explorer ship',
                      'pose': args.pose, 'model_sha256': entry['sha256'],
                      'binary': str(binary), 'cwd': str(root),
                      'environment': {k: env[k] for k in ['UNIVERSE_TEST', 'UNIVERSE_SCENARIO', 'UNIVERSE_MODEL', 'UNIVERSE_MODEL_CLEAN', 'XDG_DATA_HOME']}}, indent=2), flush=True)
    if args.check:
        return
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error(f'freefall executable unavailable: {binary}; build it or pass --binary')
    scratch.mkdir(parents=True, exist_ok=True)
    raise SystemExit(subprocess.call([str(binary)], cwd=root, env=env))


if __name__ == '__main__':
    main()
