import numpy as np,json,hashlib
from pathlib import Path
p=Path('/home/alexm/git/planet-trees/out/benchmarks/scenery-basin-s15-cape-fold-review-v2/route_termini.npz');d=np.load(p);wet=d['wet_vertices'].astype(bool);ids=np.flatnonzero(d['affected']);terms={}
for mode in ['baseline','candidate']:
 recv=d[mode+'_receiver'];cache={}
 for start in ids:
  path=[];seen=set();i=int(start)
  while i not in cache and not wet[i] and recv[i]!=i:
   assert i not in seen,'cycle';seen.add(i);path.append(i);i=int(recv[i])
  terminal=cache.get(i,i);cache[i]=terminal
  for j in path:cache[j]=terminal
 terms[mode]=np.array([cache[int(i)] for i in ids]);assert np.array_equal(terms[mode],d[mode+'_terminal'][ids]);assert wet[terms[mode]].all()
r={'scope':'Independent traversal of producer receiver arrays; not independent reconstruction of slopes/receivers or water-domain intersections.','affected':len(ids),'baseline_reaching_wet':int(wet[terms['baseline']].sum()),'candidate_reaching_wet':int(wet[terms['candidate']].sum()),'changed_first_wet_vertex':int((terms['baseline']!=terms['candidate']).sum()),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()};Path('docs/diagnostics/cape-fold-v4/terminal-check.json').write_text(json.dumps(r,indent=2)+'\n');print(r)
