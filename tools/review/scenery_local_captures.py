import os,json,subprocess,hashlib
from pathlib import Path
root=Path('/home/alexm/git/universe');out=root/'out/review/pgs-local-views-v1';records=[]
packages={'control':'/home/alexm/git/planet-trees/out/benchmarks/scenery-mesh-control-s15-1250-v1','candidate':'/home/alexm/git/planet-trees/out/planet/earth_s15/basin_review_001'}
side='/home/alexm/git/planet-trees/out/planet/earth_s15/debug_review_002/scenery-debug.json'
for variant,package in packages.items():
 data=json.loads((root/f'docs/diagnostics/scenery-local-views-v1/{variant}.json').read_text())
 for v in data['views']:
  c=v['camera']
  for mode in (['neutral'] if variant=='control' else ['channels','neutral']):
   name=f'{c["id"]}-{variant}-{mode}'; png=out/(name+'.png');log=out/(name+'.log')
   env={k:v for k,v in os.environ.items() if not k.startswith('UNIVERSE_')}
   settings={'UNIVERSE_PGS1':package,'UNIVERSE_PGS1_COLOURS':'neutral','UNIVERSE_PGS1_SHADOWS':'off','UNIVERSE_BODY':'body.treistun.treistun-e','UNIVERSE_SCENARIO':'lowflight','UNIVERSE_ENGINE_THREAD':'0','UNIVERSE_LAT':str(c['lat']),'UNIVERSE_LON':str(c['lon']),'UNIVERSE_HOURS':'2','UNIVERSE_ALT':str(v['launch_alt_m']),'UNIVERSE_DOWN':str(c['down']),'UNIVERSE_HEADING':str(c['heading_deg']),'UNIVERSE_SPEED':'0','UNIVERSE_SETTLERS':'0','UNIVERSE_PAUSED':'1','UNIVERSE_SCREENSHOT':str(png),'UNIVERSE_SCREENSHOT_AT':'240'}
   if mode=='channels':settings.update(UNIVERSE_PGS1_DEBUG=side,UNIVERSE_PGS1_DEBUG_LAYERS='reference_rivers,native_rivers,connector',UNIVERSE_PGS1_DEBUG_LIFT='2',UNIVERSE_PGS1_GEOMETRY='off')
   if mode=='neutral':
    empty=out/(variant+'-empty-debug.json')
    empty.write_text(json.dumps({'version':'scenery-debug/1','surface_sha256':data['surface_sha256'],'radius_m':6371000.,'layers':[]}))
    settings.update(UNIVERSE_PGS1_DEBUG=str(empty),UNIVERSE_PGS1_GEOMETRY='off')
   # Calibrated from the first run's actual frame240 eye heights; compensate
   # surface lookup/frame rounding, preserving requested absolute camera height.
   observed={('control','high_to_low'):6751.398120,('control','low_to_high'):6751.439128,('candidate','high_to_low'):6751.318028,('candidate','low_to_high'):6751.364972}
   settings['UNIVERSE_ALT']=str(v['launch_alt_m']+c['absolute_height_m']-observed[(variant,c['id'])])
   env.update(settings)
   with log.open('w') as f:subprocess.run([str(root/'target/debug/freefall')],env=env,stdout=f,stderr=subprocess.STDOUT,check=True,timeout=180)
   records.append({'id':name,'settings':settings,'png_sha256':hashlib.sha256(png.read_bytes()).hexdigest(),'log':str(log)})
   print(name,flush=True)
   (out/'captures.json').write_text(json.dumps({'binary_sha256':hashlib.sha256((root/'target/debug/freefall').read_bytes()).hexdigest(),'captures':records},indent=2)+'\n')
