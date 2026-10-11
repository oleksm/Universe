"""Capture one saved hull-free design through every shared studio visual path."""
import os,json,subprocess,hashlib
from pathlib import Path
root=Path('/home/alexm/git/universe');out=root/'out/review/studio-equipment-v2';records=[]
for mode,options in [('design',{'UNIVERSE_TOOL':'modules'}),('walk',{'UNIVERSE_STUDIO_WALK':'0,0.45,8,0'}),('balance',{'UNIVERSE_BALANCE':'trim'}),('drive',{'UNIVERSE_DRIVE':'0'})]:
 env={k:v for k,v in os.environ.items() if not k.startswith('UNIVERSE_')}
 settings=dict(XDG_DATA_HOME=str(root/'out/review/studio-equipment-v1/data'),UNIVERSE_SCREENSHOT=str(out/(mode+'.png')),UNIVERSE_SCREENSHOT_AT='120',**options);env.update(settings)
 with (out/(mode+'.log')).open('w') as log:subprocess.run([str(root/'target/release/freefall-studio'),'installed-equipment-review'],env=env,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=180)
 records.append({'mode':mode,'settings':settings,'png_sha256':hashlib.sha256((out/(mode+'.png')).read_bytes()).hexdigest()})
 (out/'captures.json').write_text(json.dumps({'binary_sha256':hashlib.sha256((root/'target/release/freefall-studio').read_bytes()).hexdigest(),'saved_design_sha256':hashlib.sha256((root/'out/review/studio-equipment-v1/data/freefall/interiors/installed-equipment-review.json').read_bytes()).hexdigest(),'records':records},indent=2)+'\n');print(mode,flush=True)
