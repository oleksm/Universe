"""Vulkan depth regression plus matched dish and four shared Studio modes.
Run under scenery_job.py --memory-gib 8 --threads 2.
"""
import hashlib,json,os,subprocess
from pathlib import Path
from PIL import Image
root=Path(__file__).resolve().parents[2]
out=root/'out/review/studio-depth-v1';out.mkdir(parents=True,exist_ok=True)
data=out/'data/freefall/interiors';data.mkdir(parents=True,exist_ok=True)
fixtures={
'dish-review':[dict(id='equipment.comm.basic.s1#1',at=[0,0,0],size=[3,1.5,3])],
'installed-equipment-review':[
 dict(id='equipment.engine.ch.s2#1',at=[-2,1.15,0],size=[1.5,1.5,2.3],push=[0,1,0]),
 dict(id='equipment.engine.ch.s2#2',at=[2,1.15,0],size=[1.5,1.5,2.3],push=[0,0,-1])],
}
for name,blocks in fixtures.items():
    path=data/(name+'.json')
    if not path.exists(): path.write_text(json.dumps(dict(hull=name,on='',points=[],lines=[],groups=[],blocks=blocks),indent=2))
records=[]
def capture(name,binary,args=(),**settings):
    env={k:v for k,v in os.environ.items() if not k.startswith('UNIVERSE_')}
    env.update(UNIVERSE_SCREENSHOT=str(out/(name+'.png')),UNIVERSE_SCREENSHOT_AT='60',**settings)
    with (out/(name+'.log')).open('w') as log:
        subprocess.run([str(root/binary),*args],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=180)
    records.append({'name':name,'settings':settings,'sha256':hashlib.sha256((out/(name+'.png')).read_bytes()).hexdigest()})
    print(name,flush=True)
capture('depth-regression','target/release/examples/canvas_depth')
im=Image.open(out/'depth-regression.png').convert('RGB');w,h=im.size
samples={}
for name,x,y,expected in [('left_near',.12,.3,(255,0,0)),('right_near',.37,.3,(0,255,0)),('reverse_left',.62,.3,(255,0,0)),('reverse_right',.87,.3,(0,255,0)),('ui',.25,.47,(255,255,255)),('new_camera',.9,.86,(0,0,255))]:
    actual=im.getpixel((int(x*w),int(y*h)));assert actual==expected,(name,actual,expected);samples[name]=actual
# Entire crossing-triangle region agrees in opposite submission orders; allow
# only the one-pixel rasterization edge differences from translated float inputs.
mismatch=0
for y in range(int(.2*h),int(.79*h)):
    for x in range(w//2):
        mismatch+=im.getpixel((x,y))!=im.getpixel((x+w//2,y))
assert mismatch<30,mismatch
capture('dish-after','target/release/freefall-studio',['dish-review'],XDG_DATA_HOME=str(out/'data'))
for mode,opts in [('design',{'UNIVERSE_TOOL':'modules'}),('walk',{'UNIVERSE_STUDIO_WALK':'0,0.45,8,0'}),('balance',{'UNIVERSE_BALANCE':'trim'}),('drive',{'UNIVERSE_DRIVE':'0'})]:
    capture(mode,'target/release/freefall-studio',['installed-equipment-review'],XDG_DATA_HOME=str(out/'data'),**opts)
report={'samples':samples,'opposite_order_mismatched_pixels':mismatch,'captures':records,'binary_sha256':hashlib.sha256((root/'target/release/freefall-studio').read_bytes()).hexdigest()}
(out/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
