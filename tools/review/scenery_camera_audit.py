"""Read-only footprint/frustum audit from the exact PGS point section and saved masks."""
import argparse, hashlib, json, struct
from pathlib import Path
import numpy as np
p=argparse.ArgumentParser();p.add_argument('package');p.add_argument('output');p.add_argument('--render-radius',type=float);a=p.parse_args()
folder=Path(a.package);blob=(folder/'surface.pgs').read_bytes();source_radius=struct.unpack_from('<d',blob,24)[0];radius=a.render_radius or source_radius
pos=36
for _ in range(7):n=struct.unpack_from('<I',blob,pos)[0];pos+=4+n
pos=(pos+7)//8*8;count=struct.unpack_from('<I',blob,pos)[0];pos+=8
for k in range(count):
 kind,ver,flags,stride,offset,length,rows,digest=struct.unpack_from('<IIIIQQQ32s',blob,pos+72*k)
 assert hashlib.sha256(blob[offset:offset+length]).digest()==digest
 if kind==1:
  vertices=np.frombuffer(blob,dtype=[('p','<f8',(3,)),('height','<f8'),('source','<u4'),('zero','<u4')],count=rows,offset=offset)
model=np.load(folder/'model.npz');points=vertices['p'];points=points/np.linalg.norm(points,axis=1)[:,None];heights=vertices['height']
def direction(lat,lon):
 lat,lon=np.deg2rad([lat,lon]);return np.array([np.cos(lat)*np.cos(lon),np.sin(lat),-np.cos(lat)*np.sin(lon)])
def ll(p):
 p=p/np.linalg.norm(p);return {'lat':float(np.rad2deg(np.arcsin(p[1]))),'lon':float(np.rad2deg(np.arctan2(-p[2],p[0])))}
up=direction(-50.927589054482716,-19.166702846855568);north=np.array([0.,1.,0.])-up*up[1];north/=np.linalg.norm(north);east=np.cross(north,up);eye=up*(radius+6456.784228772857);forward=east-up*.25;forward/=np.linalg.norm(forward);right=np.cross(forward,up);right/=np.linalg.norm(right);camup=np.cross(right,forward)
xyz=points*(radius+heights[:,None])-eye;z=xyz@forward
frustum=(z>0)&(np.abs(xyz@right)<=z*np.tan(np.pi/6)*16/9)&(np.abs(xyz@camup)<=z*np.tan(np.pi/6))
result={'render_radius_m':radius,'source_radius_m':source_radius,'surface_sha256':hashlib.sha256(blob).hexdigest(),'scope':'frozen 60deg vertical FOV,16:9; point frustum only, no terrain occlusion; counts are vertices, not area','masks':{}}
for name in ['changed','intended_basin','mapped_source_basin']:
 mask=model[name].astype(bool);angular=np.arccos(np.clip(points[mask]@up,-1,1))*radius
 result['masks'][name]={'vertices':int(mask.sum()),'in_frustum':int((mask&frustum).sum()),'behind_camera':int((mask&(z<=0)).sum()),'surface_range_m':[float(angular.min()),float(angular.max())],'height_range_m':[float(heights[mask].min()),float(heights[mask].max())]}
ids=np.flatnonzero(model['intended_basin']);hi=int(ids[np.argmax(heights[ids])]);lo=int(ids[np.argmin(heights[ids])]);connector=model['connector'].astype(int)
result['targets']={name:{'point_id':int(i),**ll(points[i]),'height_m':float(heights[i])}for name,i in [('basin_high_vertex',hi),('basin_low_vertex',lo),('connector_start',connector[0]),('connector_end',connector[-1])]}
# Reproducible views aimed across real high/low basin support; these are diagnostic
# targets, not a claim that an unsurveyed maximum is a geomorphic ridge crest.
local_ids=ids[(np.arccos(np.clip(points[ids]@up,-1,1))*radius)<10000.]
hi=int(local_ids[np.argmax(heights[local_ids])]);lo=int(local_ids[np.argmin(heights[local_ids])])
result['local_camera_targets']={name:{'point_id':i,**ll(points[i]),'height_m':float(heights[i])}for name,i in [('local_high',hi),('local_low',lo)]}
result['proposed_cameras']=[]
for name,start,target in [('high_to_low',hi,lo),('low_to_high',lo,hi)]:
 u=points[start];absolute=float(max(heights[start],heights[target])+1000.)
 n=np.array([0.,1.,0.])-u*u[1];n/=np.linalg.norm(n);e=np.cross(n,u)
 delta=points[target]*(radius+heights[target])-u*(radius+absolute);vertical=float(delta@u);flat=delta-u*vertical
 result['proposed_cameras'].append({'id':name,'diagnostic_only':True,'camera_point_id':start,'target_point_id':target,**ll(u),'absolute_height_m':absolute,'candidate_alt_m':absolute-float(heights[start]),'heading_deg':float(np.rad2deg(np.arctan2(flat@e,flat@n)))%360,'down':-vertical/float(np.linalg.norm(flat)),'target_range_m':float(np.linalg.norm(delta)),'time_s':7200,'colours':'neutral','shadows':'off'})
Path(a.output).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
