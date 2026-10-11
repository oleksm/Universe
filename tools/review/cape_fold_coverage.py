import sys,json,numpy as np,hashlib
from pathlib import Path
sys.path.insert(0,'/home/alexm/git/planet-trees/src')
from planet_trees import pgs1 as P
r=Path('/home/alexm/git/planet-trees/out/benchmarks/scenery-basin-s15-cape-fold-v4');m=np.load(r/'model.npz');s=P.read(r/'surface.pgs');d=json.loads(Path('docs/diagnostics/cape-fold-light-v1/candidate.json').read_text());ids=np.array([q['face'] for q in d['samples']]);result={}
for key in ['changed','intended_basin','mapped_source_basin']:
 hit=m[key][s.triangles[ids]].any(axis=1);result[key]={'faces_touching_mask':int(hit.sum()),'total_sampled_faces':len(hit),'canonical_slope_degrees':{'median':float(np.median([q['canonical_slope_deg'] for q,h in zip(d['samples'],hit) if h])),'max':float(np.max([q['canonical_slope_deg'] for q,h in zip(d['samples'],hit) if h]))}}
result['source_mask_sha256']=hashlib.sha256((r/'model.npz').read_bytes()).hexdigest()
result['scope']='Any vertex of canonical owner face in producer mask, not exact mask-area fraction or visibility integral.'
Path('docs/diagnostics/cape-fold-light-v1/coverage.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
