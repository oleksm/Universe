"""Bounded CPU-only canonical-face/grid overlay. No runtime or source mutation."""
import argparse, collections, hashlib, json, math, pathlib, struct, time, tracemalloc, resource
import numpy as np

NONE=2**32-1
POINT=np.dtype([('p','<f8',(3,)),('height','<f8'),('source','<u4'),('zero','<u4')])
FACE=np.dtype([('vertices','<u4',(3,)),('neighbours','<u4',(3,)),('body','<u4'),('rock','<u2'),('pattern','<u2')])
ENTRY=struct.Struct('<IIIIQQQ32s')

def read_source(path):
    data=path.read_bytes();pos=36
    if data[:4]!=b'PGS1': raise ValueError('not PGS1')
    for _ in range(7):
        n=struct.unpack_from('<I',data,pos)[0];pos+=4+n
    pos=(pos+7)//8*8; count=struct.unpack_from('<I',data,pos)[0];pos+=8;sections={}
    for i in range(count):
        kind,ver,flags,stride,off,size,n,digest=ENTRY.unpack_from(data,pos+i*ENTRY.size)
        payload=memoryview(data)[off:off+size]
        if hashlib.sha256(payload).digest()!=digest:raise ValueError('section hash')
        if kind in (1,2):
            dtype=POINT if kind==1 else FACE
            if stride!=dtype.itemsize or n*stride!=size:raise ValueError('stride mismatch')
            sections[kind]=np.frombuffer(payload,dtype)
        if kind==4:sections[4]={int(i):float(h) for i,_,h in struct.iter_unpack('<IId',payload)}
    return data,sections[1],sections[2],sections.get(4,{})

def clip(poly,axis,bound,greater):
    if not len(poly):return []
    out=[]; a=poly[-1]; da=(a[axis]-bound)*(1 if greater else -1)
    for b in poly:
        db=(b[axis]-bound)*(1 if greater else -1)
        if (da>=0)!=(db>=0):
            t=da/(da-db);v=a+(b-a)*t;v[axis]=bound;out.append(v)
        if db>=0:out.append(b.copy())
        a,da=b,db
    return out

def rect(poly,x0,x1,y0,y1):
    for a,b,g in [(0,x0,True),(0,x1,False),(1,y0,True),(1,y1,False)]:poly=clip(poly,a,b,g)
    return poly

def area(poly):
    if len(poly)<3:return 0.
    p=np.array(poly);return abs(float(np.sum(p[:,0]*np.roll(p[:,1],-1)-p[:,1]*np.roll(p[:,0],-1))))*.5

def build(points,faces,bodies,feature,plan):
    start=time.perf_counter();r=plan['radius_m'];c=np.array(feature['queries'][0]['unit_direction']);c/=np.linalg.norm(c)
    n=np.array([0.,1.,0.]);n-=c*np.dot(c,n);n/=np.linalg.norm(n);e=np.cross(n,c)
    def direction(uv):
        d=c+e*uv[0]/r+n*uv[1]/r;return d/np.linalg.norm(d)
    def project(p):
        den=p@c
        if np.any(den<=0):raise ValueError('face outside projection hemisphere')
        return np.stack((p@e/den*r,p@n/den*r),axis=-1)
    half=plan['tile_half_width_m'];step=plan['grid_cell_m'];bounds=(-half,half,-half,half)
    # Two independently emitted neighbouring tiles, separated at x=0.
    queue=collections.deque(feature['faces']);seen=set();hit={};original={}
    while queue:
        f=int(queue.popleft())
        if f in seen:continue
        if len(seen)>=plan['max_search_faces_per_pair']:raise ValueError('search face cap')
        seen.add(f);poly=project(points['p'][faces['vertices'][f]])
        clipped=rect(list(poly),*bounds)
        if area(clipped)<1e-12:continue
        hit[f]=clipped;original[f]=poly
        queue.extend(int(v) for v in faces['neighbours'][f] if int(v) not in seen)
    if not hit:raise ValueError('empty cover')
    if abs(sum(area(v) for v in hit.values())-(2*half)**2)>1e-4:raise ValueError('coverage area mismatch')
    positions={};vertex_owner={};tiles=[];generated=0
    def position(uv,f):
        key=tuple(np.round(uv,8))
        if key not in positions:
            d=direction(key); ids=faces['vertices'][f];a,b,z=points['p'][ids];normal=np.cross(b-a,z-a)
            p=d*np.dot(normal,a)/np.dot(normal,d);den=np.dot(normal,normal)
            v=np.dot(np.cross(p-a,z-a),normal)/den;w=np.dot(np.cross(b-a,p-a),normal)/den
            h=(1-v-w)*points['height'][ids[0]]+v*points['height'][ids[1]]+w*points['height'][ids[2]]
            # Preserve the visible water level separately from dry canonical height.
            if int(faces['body'][f])!=NONE:h=bodies[int(faces['body'][f])]
            positions[key]=d*(r+h);vertex_owner[key]=f
        return key
    ref_height=float(np.mean(points['height'][faces['vertices'][feature['faces'][0]]]))
    for tile,(xmin,xmax) in enumerate([(-half,0.),(0.,half)]):
        origin=direction(((xmin+xmax)/2,0))*(r+ref_height)
        # No shared vertex/height cache across tiles: seam agreement is measured.
        positions.clear();vertex_owner.clear()
        keys=[];indices={};triangles=[];provenance=[]
        def index(uv,f):
            key=position(uv,f)
            if key not in indices:indices[key]=len(keys);keys.append(key)
            return indices[key]
        for f in sorted(hit):
            poly=original[f];lo=poly.min(axis=0);hi=poly.max(axis=0)
            for ix in range(max(round(xmin/step),math.floor(lo[0]/step)),min(round(xmax/step),math.ceil(hi[0]/step))):
                for iy in range(max(round(-half/step),math.floor(lo[1]/step)),min(round(half/step),math.ceil(hi[1]/step))):
                    cut=rect(list(poly),ix*step,(ix+1)*step,iy*step,(iy+1)*step)
                    if area(cut)<1e-12:continue
                    ids=[index(uv,f) for uv in cut]
                    # Remove consecutive duplicate clipping intersections.
                    ids=[v for j,v in enumerate(ids) if v!=ids[j-1]]
                    if len(ids)<3:continue
                    for j in range(1,len(ids)-1):
                        if area([keys[ids[0]],keys[ids[j]],keys[ids[j+1]]])<1e-12:continue
                        triangles.append([ids[0],ids[j],ids[j+1]]);provenance.append([f,int(faces['body'][f])]);generated+=1
                    if generated>plan['max_output_triangles_per_pair'] or len(positions)>plan['max_output_vertices_per_pair']:raise ValueError('geometry cap')
        tri_area=sum(area([keys[i] for i in tri]) for tri in triangles)
        if abs(tri_area-(xmax-xmin)*2*half)>1e-4:raise ValueError('emitted area mismatch')
        vertices=np.array([positions[k]-origin for k in keys],dtype=np.float32)
        tiles.append(dict(origin=origin.tolist(),vertices=vertices.tolist(),uv=[list(k) for k in keys],triangles=triangles,provenance=provenance))
    count=sum(len(t['vertices']) for t in tiles)
    if count>plan['max_output_vertices_per_pair']:raise ValueError('duplicate vertex cap')
    packed=sum(len(t['vertices'])*12+len(t['triangles'])*20 for t in tiles)
    working=packed+len(positions)*160+len(seen)*128
    if working>plan['max_working_bytes_excluding_resident_source']:raise ValueError('working estimate cap')
    seams=[]
    for t in tiles:
        seams.append({tuple(uv):np.array(t['origin'])+np.array(v) for uv,v in zip(t['uv'],t['vertices']) if uv[0]==0})
    if seams[0].keys()!=seams[1].keys():raise ValueError('shared edge splits differ')
    seam=max((float(np.linalg.norm(seams[0][k]-seams[1][k])) for k in seams[0]),default=0.)
    source_extent=max(float(np.linalg.norm(points['p'][faces['vertices'][f]][a]/np.linalg.norm(points['p'][faces['vertices'][f]][a])-points['p'][faces['vertices'][f]][b]/np.linalg.norm(points['p'][faces['vertices'][f]][b]))*r) for f in hit for a,b in [(0,1),(1,2),(2,0)])
    result=dict(max_source_edge_chord_m=source_extent,feature=feature['id'],status='built',source_faces=len(hit),search_faces=len(seen),vertices=count,triangles=generated,packed_bytes=packed,working_bytes_estimate=working,seam_max_m=seam,seam_points=len(seams[0]),cpu_ms=(time.perf_counter()-start)*1000,tiles=tiles)
    if len(json.dumps(result,separators=(',',':')).encode())>plan['max_serialized_geometry_bytes_per_pair']:raise ValueError('serialized byte cap')
    return result

def main():
    a=argparse.ArgumentParser();a.add_argument('source');a.add_argument('features');a.add_argument('plan');a.add_argument('out');args=a.parse_args()
    out=pathlib.Path(args.out);out.mkdir(parents=True,exist_ok=True);plan=json.load(open(args.plan));t=time.perf_counter();blob,points,faces,bodies=read_source(pathlib.Path(args.source));read_ms=(time.perf_counter()-t)*1000
    digest=hashlib.sha256(blob).hexdigest()
    if digest!=plan['source_sha256']:raise ValueError('source binding')
    features=json.load(open(args.features))['features'];results=[]
    # Existing frozen features include three crests, shore and native joins.
    # A declared large-native-face control uses the largest adjacent shore face's centroid.
    shore=features[0];f=max(shore['faces'],key=lambda f:np.linalg.norm(np.cross(*(points['p'][faces['vertices'][f]][1:]-points['p'][faces['vertices'][f]][0]))))
    d=points['p'][faces['vertices'][f]].mean(axis=0);d/=np.linalg.norm(d)
    features=features+[dict(id='large_native_face_centroid',faces=[f],queries=[dict(unit_direction=d.tolist())])]
    for feature in features:
        try:
            tracemalloc.start();r=build(points,faces,bodies,feature,plan);_,peak=tracemalloc.get_traced_memory();tracemalloc.stop();r['traced_working_peak_bytes']=peak
            if peak>plan['max_working_bytes_excluding_resident_source']:raise ValueError('measured working cap')
        except ValueError as err:
            tracemalloc.stop();r=dict(feature=feature['id'],status='refused',reason=str(err))
        (out/(feature['id']+'.json')).write_text(json.dumps(r,separators=(',',':'))+'\n')
        results.append({k:v for k,v in r.items() if k!='tiles'})
    # Adversarial cap controls must fail, not silently truncate geometry.
    refusals=[]
    for field,value in [('max_search_faces_per_pair',1),('max_output_vertices_per_pair',1),('max_serialized_geometry_bytes_per_pair',32)]:
        try:build(points,faces,bodies,features[0],dict(plan,**{field:value}));refusals.append([field,'UNEXPECTED PASS'])
        except ValueError as err:refusals.append([field,str(err)])
    (out/'results.json').write_text(json.dumps(dict(source_sha256=digest,read_ms=read_ms,source_bytes=len(blob),process_peak_rss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,results=results,refusal_controls=refusals,script_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()),indent=2)+'\n')
if __name__=='__main__':main()
