"""Independent GLB bind/rigid-frame/parent-reference audit, not a motion evaluator."""
import argparse,hashlib,json,struct
from pathlib import Path
import numpy as np
p=argparse.ArgumentParser();p.add_argument('glb');p.add_argument('fixture');p.add_argument('output');a=p.parse_args()
b=Path(a.glb).read_bytes();j=json.loads(b[20:20+struct.unpack_from('<I',b,12)[0]]);f=json.loads(Path(a.fixture).read_text());assert f['export_sha256']==hashlib.sha256(b).hexdigest()
def mat(n):
 if 'matrix'in n:return np.array(n['matrix']).reshape(4,4).T
 x,y,z,w=n.get('rotation',[0,0,0,1]);r=np.array([[1-2*(y*y+z*z),2*(x*y-z*w),2*(x*z+y*w)],[2*(x*y+z*w),1-2*(x*x+z*z),2*(y*z-x*w)],[2*(x*z-y*w),2*(y*z+x*w),1-2*(x*x+y*y)]])
 m=np.eye(4);m[:3,:3]=r@np.diag(n.get('scale',[1,1,1]));m[:3,3]=n.get('translation',[0,0,0]);return m
world={};parents={}
def walk(i,parent,m):
 n=j['nodes'][i];name=n['name'];assert name not in world;world[name]=m@mat(n);parents[name]=parent
 for c in n.get('children',[]):walk(c,name,world[name])
for i in j['scenes'][j.get('scene',0)]['nodes']:walk(i,None,np.eye(4))
rest=f['control']['rest'];pose=min(f['poses'],key=lambda p:abs(p['control']-rest));assert abs(pose['control']-rest)<1e-12
err=0.;ortho=0.;hier=0.;det=1.
for name,m in world.items():
 assert name in pose['nodes'],name;err=max(err,float(np.abs(m-np.array(pose['nodes'][name]['model_gltf'])).max()))
for p in f['poses']:
 for name,n in p['nodes'].items():
  m=np.array(n['model_gltf']);local=np.array(n['parent_gltf']);assert np.isfinite(m).all();parent=n['parent'];expected=local if parent is None else np.array(p['nodes'][parent]['model_gltf'])@local
  hier=max(hier,float(np.abs(m-expected).max()));det=min(det,float(np.linalg.det(m[:3,:3])));ortho=max(ortho,float(np.abs(m[:3,:3].T@m[:3,:3]-np.eye(3)).max()))
assert err<2e-6 and hier<2e-6 and ortho<2e-5 and det>0.99998,(err,hier,ortho,det)
r={'glb_sha256':hashlib.sha256(b).hexdigest(),'fixture_sha256':hashlib.sha256(Path(a.fixture).read_bytes()).hexdigest(),'glb_nodes':len(world),'reference_poses':len(f['poses']),'rest_control':rest,'max_bind_matrix_component_error':err,'max_parent_composition_component_error':hier,'max_orthogonality_error':ortho,'min_determinant':det,'scope':'Independent static binds and all-reference hierarchy/rigidity only; source vertex equivalence and mechanism dynamics not independently re-evaluated; moving runtime held'}
Path(a.output).write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
