"""Frozen neutral summit/lake captures, calibrated to matching absolute eyes."""
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path

root = Path('/home/alexm/git/universe')
out = root/'out/review/cape-fold-v4'
packages = {
    'control': Path('/home/alexm/git/planet-trees/out/benchmarks/scenery-mesh-control-s15-1250-v1'),
    'candidate': out/'package',
}
binary = root/'target/release/freefall'
records = []
for site, lat, lon, hours, altitude, down in [
    ('summit', -50.927589054482716, -19.166702846855568, 2, 1000, .25),
    ('lake', 59.01000627043136, -103.1714991841128, 9.3065, 10000, .6),
]:
    initial = {}
    def capture(variant, launch_alt, phase):
        package = packages[variant]
        sha = hashlib.sha256((package/'surface.pgs').read_bytes()).hexdigest()
        empty = out/(variant+'-empty-debug.json')
        empty.write_text(json.dumps({'version':'scenery-debug/1','surface_sha256':sha,'radius_m':6371000.,'layers':[]}))
        name = f'{site}-{variant}-{phase}'
        settings = dict(UNIVERSE_PGS1=str(package), UNIVERSE_PGS1_COLOURS='neutral', UNIVERSE_PGS1_SHADOWS='off', UNIVERSE_BODY='body.treistun.treistun-e', UNIVERSE_SCENARIO='lowflight', UNIVERSE_ENGINE_THREAD='0', UNIVERSE_LAT=str(lat), UNIVERSE_LON=str(lon), UNIVERSE_HOURS=str(hours), UNIVERSE_ALT=str(launch_alt), UNIVERSE_DOWN=str(down), UNIVERSE_HEADING='90', UNIVERSE_SPEED='0', UNIVERSE_SETTLERS='0', UNIVERSE_PAUSED='1', UNIVERSE_SCREENSHOT=str(out/(name+'.png')), UNIVERSE_SCREENSHOT_AT='480', UNIVERSE_PGS1_DEBUG=str(empty), UNIVERSE_PGS1_GEOMETRY='off')
        env = {k:v for k,v in os.environ.items() if not k.startswith('UNIVERSE_')}
        env.update(settings)
        log = out/(name+'.log')
        with log.open('w') as f:
            subprocess.run([str(binary)], env=env, stdout=f, stderr=subprocess.STDOUT, check=True, timeout=180)
        matches = re.findall(r'frame=480 .*eye_height_m=([\d.-]+)',log.read_text())
        assert len(matches)==1, log
        eye = float(matches[0])
        record = {'id':name,'surface_sha256':sha,'settings':settings,'eye_height_m':eye,'png_sha256':hashlib.sha256((out/(name+'.png')).read_bytes()).hexdigest()}
        records.append(record)
        (out/'captures.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'render_radius_m':6281370,'records':records},indent=2)+'\n')
        print(name,eye,flush=True)
        return eye
    for variant in packages:
        initial[variant] = capture(variant, altitude, 'calibration')
    # Higher initial eye retains at least the original launch clearance on both surfaces.
    target = max(initial.values())
    final = [capture(v, altitude+target-initial[v], 'final') for v in packages]
    assert abs(final[0]-final[1])<=.001, final
