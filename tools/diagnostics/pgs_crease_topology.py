"""Independent saved-mesh edge incidence and inter-tile seam check."""
import collections,hashlib,json,math,pathlib,sys
root=pathlib.Path(sys.argv[1]);half=json.load(open(sys.argv[2]))['tile_half_width_m'];out=[]
for p in sorted(root.glob('*.json')):
 g=json.load(p.open())
 if 'tiles' not in g:continue
 edges=collections.Counter();points=collections.defaultdict(list)
 for t in g['tiles']:
  for uv,v in zip(t['uv'],t['vertices']):points[tuple(uv)].append([a+b for a,b in zip(t['origin'],v)])
  for tri in t['triangles']:
   uv=[tuple(t['uv'][i]) for i in tri]
   for a,b in zip(uv,uv[1:]+uv[:1]):edges[tuple(sorted((a,b)))]+=1
 bad=[]
 for (a,b),count in edges.items():
  boundary=any(a[k]==b[k] and abs(a[k])==half for k in [0,1])
  if count!=(1 if boundary else 2):bad.append([a,b,count,boundary])
 seam=max((math.dist(v[0],v[1]) for v in points.values() if len(v)==2),default=0.)
 out.append(dict(feature=g['feature'],geometry_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),edges=len(edges),unexpected_edges=bad,independent_seam_max_m=seam))
print(json.dumps(out,indent=2))
