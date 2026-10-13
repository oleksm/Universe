"""Independent stored-row certificate and extended-precision endpoint event audit.
Usage: python cape_fold_witness.py OUTPUT_JSON (fixed Cape Fold v1 inputs).
Run under the shared2CPU/4GiB scenery launcher.
"""
import sys,json,hashlib
from pathlib import Path
import numpy as np
from scipy import sparse
from fractions import Fraction
sys.path.insert(0,'/home/alexm/git/planet-trees/src')
from planet_trees import pgs1 as P
root=Path('/home/alexm/git/planet-trees/out/benchmarks/scenery-basin-s15-cape-fold-v1');d=np.load(root/'constraint_system.npz');meta=json.loads((root/'constraint_rows.json').read_text());base=P.read('/home/alexm/git/planet-trees/out/benchmarks/scenery-mesh-control-s15-1250-v1/surface.pgs');sampler=P.Sampler(base)
A=sparse.csr_matrix((d['A_data'],d['A_indices'],d['A_indptr']),shape=d['A_shape']);G=sparse.csr_matrix((d['grade_data'],d['grade_indices'],d['grade_indptr']),shape=d['grade_shape']);rows=[152,301];c0=A[rows[0]].toarray().ravel();c1=A[rows[1]].toarray().ravel();j=np.flatnonzero(c0)[0];weights=np.array([-c1[j],c0[j]]);weights/=weights.sum();print('certificate',weights,'coef',abs(weights@A[rows].toarray()).max(),'rhs',weights@d['rhs'][rows])
assert np.array_equal(np.flatnonzero(c0),np.flatnonzero(c1)) and len(np.flatnonzero(c0))==1
f0,f1=Fraction(float(c0[j])),Fraction(float(c1[j]));w0,w1=-f1/(f0-f1),f0/(f0-f1)
assert w0>0 and w1>0 and w0*f0+w1*f1==0
exact_rhs=w0*Fraction(float(d['rhs'][152]))+w1*Fraction(float(d['rhs'][301]));assert exact_rhs<0
result={'exact_binary_float_certificate':{'weights':[str(w0),str(w1)],'rhs':str(exact_rhs),'coefficient_sum':'0','adjustable_vertex':int(d['variable'][j])},'longdouble_epsilon':float(np.finfo(np.longdouble).eps),'constraint_dump_sha256':hashlib.sha256((root/'constraint_system.npz').read_bytes()).hexdigest(),'rows':[],'weights':weights.tolist(),'max_adjustable_coefficient':float(abs(weights@A[rows].toarray()).max()),'weighted_rhs':float(weights@d['rhs'][rows])}
for row in rows:
 m=meta[row];queryids=m['queries'];q=d['river_queries'][queryids];u=q/np.linalg.norm(q,axis=1)[:,None];mid=u.mean(0);mid/=np.linalg.norm(mid);face=int(sampler.locate([mid])[0][0]);ids=base.triangles[face];edgeids=[[int(ids[i]),int(ids[(i+1)%3])] for i in range(3)];triangle=base.p[ids].astype(np.longdouble);norm=np.cross(triangle,np.roll(triangle,-1,axis=0));norm/=np.sqrt(np.sum(norm*norm,axis=1))[:,None];signed=norm@mid.astype(np.longdouble)
 route=[v for v in meta if v.get('source_vertices')==m['source_vertices'] and v['kind']=='reference_river'];first=min(v['queries'][0] for v in route);last=max(v['queries'][1] for v in route);a,b=d['river_queries'][[first,last]].astype(np.longdouble);delta=b-a
 t=-np.sum(norm*a,axis=1)/np.sum(norm*delta,axis=1)
 enddist=float(np.linalg.norm(u[1]-np.array(b,float)/np.linalg.norm(b))*base.metadata['radius_m'])
 r={'row':row,'provenance':m,'owner_face':face,'face_vertices':ids.tolist(),'interval_length_m':float(np.linalg.norm(u[1]-u[0])*base.metadata['radius_m']),'end_to_junction_m':enddist,'signed_edge_distance_m':(signed*base.metadata['radius_m']).astype(float).tolist(),'edge_vertices':edgeids,'longdouble_boundary_parameters':t.astype(float).tolist(),'route_query_range':[first,last],'query_ids':queryids,'last_endpoint':np.array(b,float).tolist(),'midpoint_owners':sampler._owners(mid,face)}
 r['endpoint_vertex_43264_direction_error']=float(np.linalg.norm(np.array(b,float)/np.linalg.norm(b)-base.p[43264]/np.linalg.norm(base.p[43264])))
 r['endpoint_incident_edges']=[43264 in e for e in edgeids]
 result['rows'].append(r);print(json.dumps(r))
Path(sys.argv[1]).write_text(json.dumps(result,indent=2)+'\n')
