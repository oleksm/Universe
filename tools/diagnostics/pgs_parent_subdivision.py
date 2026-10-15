"""Tiny optional-cell removal control; does not change the canonical field or production mesh."""
import json,sys
from pathlib import Path
import numpy as np
root=Path(sys.argv[1]);data=json.loads((root/'witnesses.json').read_text())
r=next(x for x in data['results'] if x['feature']=='parent_face_join');c=np.array(r['chart']['c']);n=np.array(r['chart']['n']);e=np.array(r['chart']['e']);radius=r['chart']['r']
def cross2(a,b): return a[0]*b[1]-a[1]*b[0]
def world(face,uv):
 d=c+(e*uv[0]+n*uv[1])/radius;d/=np.linalg.norm(d)
 a,b,z=np.array(face['positions']);normal=np.cross(b-a,z-a);p=d*(normal@a/(normal@d));v=np.cross(p-a,z-a)@normal/(normal@normal);w=np.cross(b-a,p-a)@normal/(normal@normal)
 h=np.array(face['bed'])@np.array([1-v-w,v,w]);return d*(radius+h)
origin=world(r['faces'][0],[0,0])
def clip(poly,bounds):
 for axis,bound,sign in [(0,bounds[0],1),(0,bounds[1],-1),(1,bounds[2],1),(1,bounds[3],-1)]:
  out=[]
  if len(poly):
   a=poly[-1];da=(a[axis]-bound)*sign
   for b in poly:
    db=(b[axis]-bound)*sign
    if (da>=0)!=(db>=0):
     p=a+(b-a)*(da/(da-db));p[axis]=bound;out.append(p)
    if db>=0:out.append(b)
    a=b;da=db
  poly=out
 return poly
out={'scope':'local16m square only, remove optional x/y=0 sampling lines together across all source faces; canonical faces retained; no global mesh correction','source_edges':[],'controls':[]}
f=r['faces'][0];p=np.array(f['uv'])
for i in range(3):
 a,b=p[i],p[(i+1)%3];d=b-a
 out['source_edges'].append(dict(edge=i,uv=[a.tolist(),b.tolist()],origin_line_distance_m=float(abs(cross2(d,-a))/np.linalg.norm(d)),x0_y=float(a[1]-a[0]*d[1]/d[0]),y0_x=float(a[0]-a[1]*d[0]/d[1])))
for shift in [0.,.125,128.]:
 for reverse in [False,True]:
  for cells in [True,False]:
   count=0;bad=0;area=0;owners=[];signature=[]
   bounds=[[-8,0,-8,0],[0,8,-8,0],[-8,0,0,8],[0,8,0,8]] if cells else [[-8,8,-8,8]]
   for f in r['faces']:
    for box in bounds:
     poly=clip([x.copy()+shift for x in np.array(f['uv'])[::(-1 if reverse else 1)]],np.array(box)+shift)
     poly=[x-shift for x in poly]
     if len(poly)<3:continue
     centre=np.mean(poly,axis=0)
     for a,b in zip(poly,poly[1:]+poly[:1]):
      ar=abs(float(cross2(a-centre,b-centre)))*.5
      if ar==0:continue
      area+=ar;count+=1;owners.append(f['owner']);q=np.array([world(f,x)-origin for x in [centre,a,b]],dtype=np.float32).astype(float)
      if np.linalg.norm(np.cross(q[1]-q[0],q[2]-q[0]))==0:bad+=1
     signature.extend(poly)
   out['controls'].append(dict(chart_translation_m=shift,reversed=reverse,optional_origin_lines=cells,triangles=count,collapsed_f32=bad,area_m2=area,source_owners=sorted(set(owners))))
a=np.array([100.,1.,2.],dtype=np.float64);b=a+np.array([1e-9,0,0]);out['distinct_mandatory_refusal']={'distinct':bool(np.any(a!=b)),'same_f32':bool(np.array_equal(a.astype(np.float32),b.astype(np.float32))),'action':'refuse both constraints; cannot remove a mandatory boundary'}
assert all(x['collapsed_f32']==0 and abs(x['area_m2']-256)<1e-4 for x in out['controls'] if not x['optional_origin_lines'])
(root/'parent.json').write_text(json.dumps(out,indent=2)+'\n')
# Shared mandatory-edge identity is the pair of original source vertices, not proximity.
f0=r['faces'][0];f1=next(f for f in r['faces'] if f['owner']==41014)
shared=sorted(set(map(tuple,f0['positions']))&set(map(tuple,f1['positions'])))
assert len(shared)==2
cache={}
for face in [f0,f1]:
 for reverse in [False,True]:
  edge=shared[::-1] if reverse else shared
  key=tuple(sorted(edge));cache.setdefault(key,len(cache))
assert len(cache)==1
out['mandatory_shared_edge']={'source_vertices':shared,'owners':[40805,41014],'identities_after_reverse':len(cache),'policy':'Coalesce optional four-cell sampling block in both owners; source boundary remains mandatory. Full shared polygon implementation not enabled.'}
old=json.loads(Path('out/review/v8-sparse-band/geometry/results.json').read_text())
old=next(x['witness'] for x in old['results'] if x['feature']=='parent_face_join')
out['original_collapse']={'uv':old['uv'],'positions':old['positions'],'reproduced':bool(np.array_equal(np.array(old['positions'][1],dtype=np.float32),np.array(old['positions'][2],dtype=np.float32))),'scope':'exact saved original rounded-cell output; new local control also changes triangulation and does not isolate grid-removal alone'}
(root/'parent.json').write_text(json.dumps(out,indent=2)+'\n')
