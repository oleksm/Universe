"""Read-only outer-interface gate against the existing settled/morphed cube grid.
No runtime/GPU acceptance. Caller must wrap the whole script in the shared guard.
"""
import argparse,collections,hashlib,json,math,os,pathlib,subprocess
import numpy as np

def main():
 a=argparse.ArgumentParser();a.add_argument('source');a.add_argument('features');a.add_argument('geometry');a.add_argument('output');args=a.parse_args()
 out=pathlib.Path(args.output);out.mkdir(parents=True,exist_ok=True);results=[]
 for feature in json.load(open(args.features))['features']:
  name=feature['id'];g=json.load(open(pathlib.Path(args.geometry)/(name+'.json')))
  edges=collections.Counter()
  for t in g['tiles']:
   for tri in t['triangles']:
    uv=[tuple(t['uv'][i]) for i in tri]
    for u,v in zip(uv,uv[1:]+uv[:1]):edges[tuple(sorted((u,v)))]+=1
  outer=[(u,v) for (u,v),count in edges.items() if count==1 and any(u[k]==v[k] and abs(u[k])==64 for k in [0,1])]
  samples=sorted(set(tuple((np.array(u)*(1-w)+np.array(v)*w).tolist()) for u,v in outer for w in [0,.1,.25,.5,.75,.9,1]))
  q=feature['queries'][0];c=np.array(q['unit_direction']);c/=np.linalg.norm(c);n=np.array([0.,1.,0.])-c*c[1];n/=np.linalg.norm(n);e=np.cross(n,c);r=6281370
  probes=[]
  for uv in samples:
   d=c+(e*uv[0]+n*uv[1])/r;d/=np.linalg.norm(d);probes.append(dict(local_xy_m=list(uv),unit_direction=d.tolist()))
  handoff=out/(name+'-probes.json');handoff.write_text(json.dumps(dict(corridor=probes)))
  h=q['water'].get('level_m',q['terrain_height_m'])+1.7
  result=out/(name+'-grid.json');env=dict(os.environ,PGS_WALK_HANDOFF=str(handoff),PGS_WALK_TERRAIN_DISTANCE='1',PGS_WALK_MAX_LEVEL='19',PGS_WALK_INCLUDE_WATER='1');env.pop('PGS_WALK_AT_EACH',None)
  cmd=['target/release/examples/pgs_walk_audit',args.source,str(q['latitude_degrees']),str(q['longitude_degrees']),str(h),'90','1',str(result),str(r)]
  with (out/(name+'.log')).open('w') as log:subprocess.run(cmd,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
  data=json.load(result.open());worst=max(data['samples'],key=lambda x:abs(x['error_m']));bad=[x for x in data['samples'] if abs(x['error_m'])>.1]
  results.append(dict(feature=name,outer_edges=len(outer),samples=len(samples),failures=len(bad),max_error_m=abs(worst['error_m']),worst=worst,levels=data['levels'],camera=data['camera'],report_sha256=hashlib.sha256(result.read_bytes()).hexdigest()))
 (out/'results.json').write_text(json.dumps(dict(scope='Frozen128m outer boundary against existing settled grid/morph at centre eye; boundary-conforming strip cannot change its outer endpoint. Finite CPU preflight, not actual submitted/GPU geometry.',results=results),indent=2)+'\n')
if __name__=='__main__':main()
