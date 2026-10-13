"""Read-only exact stored-value support predicates and finite-edge attribution."""
import json, sys
from decimal import Decimal as D, localcontext
from pathlib import Path
from fractions import Fraction
import numpy as np

def vec(v): return [D.from_float(float(x)) for x in v]
def sub(a,b): return [x-y for x,y in zip(a,b)]
def dot(a,b): return sum(x*y for x,y in zip(a,b))
def cross(a,b): return [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
def det(a,b,c): return dot(a,cross(b,c))
def exact_support(point, triangle):
    p=vec(point);a,b,c=map(vec,triangle)
    orientation=det(a,b,c)
    fp=[Fraction.from_float(float(x)) for x in point];fa,fb,fc=[[Fraction.from_float(float(x)) for x in row] for row in triangle]
    fo=det(fa,fb,fc);exact=[det(fa,fb,fp)*fo,det(fb,fc,fp)*fo,det(fc,fa,fp)*fo]
    signs=[det(a,b,p)*orientation,det(b,c,p)*orientation,det(c,a,p)*orientation]
    assert all((x>0)-(x<0)==(y>0)-(y<0) for x,y in zip(signs,exact))
    n=cross(sub(b,a),sub(c,a));plane=abs(dot(n,sub(p,a)))/dot(n,n).sqrt()
    distances=[]
    for x,y in [(a,b),(b,c),(c,a)]:
        e=sub(y,x);v=sub(p,x);t=max(D(0),min(D(1),dot(v,e)/dot(e,e)))
        q=sub(v,[t*z for z in e]);distances.append(dot(q,q).sqrt())
    return dict(ray_inside=all(x>=0 for x in exact),exact_rational_signs=[(x>0)-(x<0) for x in exact],signed_determinants=[str(x) for x in signs],plane_distance_m=float(plane),nearest_finite_edge_m=float(min(distances)))

root=Path(sys.argv[1]);out={}
with localcontext() as ctx:
    ctx.prec=80
    seams=json.loads((root/'seams.json').read_text());out['support']=[]
    for feature in seams['results']:
        misses=[]
        for m in feature['misses']:
            checks=[exact_support(m['point'],s['world']) for s in m['supports']]
            misses.append(dict(edge=m['edge'],fraction=m['fraction'],any_robust_ray_support=any(c['ray_inside'] for c in checks),checks=checks))
        out['support'].append(dict(feature=feature['feature'],strict_misses=len(misses),robust_inside=sum(m['any_robust_ray_support'] for m in misses),misses=misses))

witness=json.loads((root/'witnesses.json').read_text());out['retained']=[]
for row in witness['results']:
    if 'witness' not in row: continue
    w=row['witness'];a,b=w['first'],w['second']
    at=np.array(a['edge_tags']);bt=np.array(b['edge_tags'])
    axis=int(np.argmax(np.abs(at[1]-at[0])))
    ia=sorted(at[:,axis].tolist());ib=sorted(bt[:,axis].tolist())
    aw,bw=np.array(a['world']),np.array(b['world'])
    comparisons=[]
    for tag in sorted(set(at[:,axis])|set(bt[:,axis])):
        if max(ia[0],ib[0])<=tag<=min(ia[1],ib[1]):
            t=(tag-at[0,axis])/(at[1,axis]-at[0,axis]);u=(tag-bt[0,axis])/(bt[1,axis]-bt[0,axis])
            comparisons.append(dict(lattice_coordinate=int(tag),morphed_distance_m=float(np.linalg.norm(aw[0]*(1-t)+aw[1]*t-bw[0]*(1-u)-bw[1]*u)),raw_distance_m=float(np.linalg.norm(np.array(a['raw'])[0]*(1-t)+np.array(a['raw'])[1]*t-np.array(b['raw'])[0]*(1-u)-np.array(b['raw'])[1]*u))))
    out['retained'].append(dict(feature=row['feature'],patches=[a['patch'],b['patch']],intervals=[ia,ib],same_finite_segment=ia==ib,overlap= [max(ia[0],ib[0]),min(ia[1],ib[1])],morph=[a['morph'],b['morph']],comparisons=comparisons,intersection_disagreement_m=w['distance_m']))
(root/'checks.json').write_text(json.dumps(out,indent=2)+'\n')
