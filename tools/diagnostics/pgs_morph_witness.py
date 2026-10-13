"""Compare frozen CPU morph with shader-like f32 patch-size/distance arithmetic."""
import json,sys
from pathlib import Path
import numpy as np
root=Path(sys.argv[1]);out=[]
for row in json.loads((root/'witnesses.json').read_text())['results']:
 if 'witness' not in row:continue
 sides=[]
 for name in ['first','second']:
  s=row['witness'][name];origin=np.array(s['origin']);eye=np.array(s['eye']);own=(np.array(s['raw'])-origin).astype(np.float32);target=(np.array(s['target'])-origin).astype(np.float32)
  far=np.float32(np.float32(np.float32(2.4)*np.float32(2))*np.float32(s['size']))*np.float32(.95)
  # place(v) includes camera-relative origin converted to f32; rotation may add rounding.
  relative=(origin-eye).astype(np.float32);distance=np.linalg.norm(own+relative,axis=1).astype(np.float32)
  t=np.clip((distance-far*np.float32(.6))/(far-far*np.float32(.6)),np.float32(0),np.float32(1));m=t*t*(np.float32(3)-np.float32(2)*t)
  world=origin+(own+(target-own)*m[:,None]).astype(np.float32).astype(float)
  sides.append(dict(patch=s['patch'],old_morph=s['morph'],shader_like_morph=m.tolist(),max_endpoint_change_m=float(np.max(np.linalg.norm(world-np.array(s['world']),axis=1)))))
 out.append(dict(feature=row['feature'],sides=sides,scope='shader-like f32 arithmetic without GPU camera rotation; top surface only, skirts excluded; not an actual GPU replay'))
(root/'morph.json').write_text(json.dumps(out,indent=2)+'\n')
