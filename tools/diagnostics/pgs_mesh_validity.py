"""Independent saved indexed-mesh graph, directed-edge and f32 validity screen."""
import collections,json,pathlib,struct,sys,hashlib,math
out=[]
for path in sorted(pathlib.Path(sys.argv[1]).glob('*.json')):
 g=json.load(path.open())
 if not isinstance(g,dict) or 'tiles' not in g:continue
 for tile in g['tiles']:
  uv=tile['uv'];p=[struct.unpack('<fff',struct.pack('<fff',*v)) for v in tile['vertices']];tris=tile['triangles'];edges=collections.defaultdict(list);adj=collections.defaultdict(set);used=set();collapsed=[];areas=[];boundary=[]
  for i,(a,b,c) in enumerate(tris):
   used.update((a,b,c));x,y,z=uv[a],uv[b],uv[c];areas.append(((y[0]-x[0])*(z[1]-x[1])-(y[1]-x[1])*(z[0]-x[0]))*.5)
   ab=[p[b][j]-p[a][j] for j in range(3)];ac=[p[c][j]-p[a][j] for j in range(3)];cross=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]]
   if sum(v*v for v in cross)==0:collapsed.append(i)
   for x,y in [(a,b),(b,c),(c,a)]:edges[tuple(sorted((x,y)))].append((x,y));adj[x].add(y);adj[y].add(x)
  bad=[];winding=[];degree=collections.Counter()
  for (a,b),directions in edges.items():
   outer=any(uv[a][k]==uv[b][k] and abs(uv[a][k])==64 for k in [0,1]);expected=1 if outer else 2
   if len(directions)!=expected:bad.append([a,b,len(directions),expected])
   if len(directions)==2 and directions[0]!=directions[1][::-1]:winding.append([a,b])
   if outer:boundary.append([a,b]);degree[a]+=1;degree[b]+=1
  seen=set();components=0
  for seed in used:
   if seed in seen:continue
   components+=1;todo=[seed]
   while todo:
    v=todo.pop()
    if v in seen:continue
    seen.add(v);todo.extend(adj[v]-seen)
  bvalid=all(d==2 for d in degree.values());area=sum(areas);euler=len(used)-len(edges)+len(tris)
  result=dict(feature=g['feature'],geometry_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),vertices=len(p),used_vertices=len(used),triangles=len(tris),unmatched_edges=len(bad),directed_edge_failures=len(winding),degenerate_f32_triangles=len(collapsed),negative_chart_triangles=sum(a<=0 for a in areas),components=components,euler=euler,chart_area_m2=area,boundary_vertices=len(degree),boundary_degree2=bvalid,examples=dict(edges=bad[:3],degenerate=collapsed[:3]))
  result['pass']=not bad and not winding and not collapsed and all(a>0 for a in areas) and components==1 and euler==1 and bvalid and abs(area-16384)<1e-4
  out.append(result)
print(json.dumps(out,indent=2))
