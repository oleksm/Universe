"""Independent saved-array arithmetic check; does not rerun source extraction."""
import argparse, hashlib, json
from pathlib import Path
import numpy as np
p=argparse.ArgumentParser();p.add_argument('artifact');p.add_argument('output');a=p.parse_args()
root=Path(a.artifact);manifest=json.loads((root/'manifest.json').read_text())
for name,entry in manifest['files'].items():
 assert hashlib.sha256((root/name).read_bytes()).hexdigest()==entry['sha256'],name
report=json.loads((root/'audit.json').read_text());stages=np.load(root/'stages.npz');cross=np.load(root/'cross-sections.npz');mask=stages['mask'].astype(bool)
# Explicit finite interior indexing: no wrapped neighbours, no shared helper.
y,x=np.where(mask);ok=(y>0)&(x>0)&(y<mask.shape[0]-1)&(x<mask.shape[1]-1);y,x=y[ok],x[ok];ok=mask[y-1,x]&mask[y+1,x]&mask[y,x-1]&mask[y,x+1];y,x=y[ok],x[ok]
def departure(z):return z[y,x]-(z[y-1,x]+z[y+1,x]+z[y,x-1]+z[y,x+1])/4
native=departure(stages['native_source']);metrics={}
for name in ['native_source','source_2500','vector','mesh_vector','target_delta','candidate_delta']:
 z=stages[name];v=departure(z);rms=float(np.sqrt(np.dot(v,v)/len(v)));aa=native-native.mean();vv=v.astype(float);bb=vv-vv.mean();corr=float(np.dot(aa,bb)/np.sqrt(np.dot(aa,aa)*np.dot(bb,bb)))
 assert abs(rms-report['metrics'][name]['neighbour_departure_rms_m'])<1e-10
 assert abs(corr-report['detail_agreement_vs_native'][name]['correlation'])<1e-10,(name,corr,report['detail_agreement_vs_native'][name]['correlation'],z.dtype)
 metrics[name]={'departure_rms_m':rms,'correlation':corr}
assert np.max(abs(cross['baseline_plus_vector']-cross['baseline_total']-cross['vector']))<1e-9
assert np.max(abs(cross['candidate_total']-cross['baseline_total']-cross['candidate_delta']))<1e-9
counts={}
for name,reported in report['channel_cross_sections']['fields'].items():
 z=cross[name];assert z.shape==(114,5) and np.isfinite(z).all()
 for key,indices in [('2500',(1,3)),('5000',(0,4))]:
  left,right=indices;n=sum(bool(row[left]>row[2]+.01 and row[right]>row[2]+.01) for row in z)
  contrast=float(np.median((z[:,left]+z[:,right])/2-z[:,2]));assert n==reported[key]['both_flanks_above_count'];assert abs(contrast-reported[key]['median_flank_contrast_m'])<1e-10
  counts.setdefault(name,{})[key]={'both_flanks_above':n,'median_contrast_m':contrast}
r={'artifact':str(root),'manifest_sha256':hashlib.sha256((root/'manifest.json').read_bytes()).hexdigest(),'samples':int(mask.sum()),'interior_samples':len(y),'metrics':metrics,'cross_sections':counts,'scope':'Independent saved-array arithmetic and construction code review; no independent native dataset extraction or source/target replay. Coarse correlated probes, not bank surveys or continuous drainage. 0.01m is diagnostic comparison tolerance, not an accepted physical threshold. Source planar frame, not host-radius slopes.'}
Path(a.output).write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({'samples':r['samples'],'interior':len(y),'metrics':metrics,'counts_2500':{k:v['2500']['both_flanks_above'] for k,v in counts.items()}}))
