"""Summarize frozen-camera ray samples and unchanged PNG display contrast."""
import json
from pathlib import Path
import numpy as np
from PIL import Image

root = Path('docs/diagnostics/cape-fold-light-v1')
result = {}
def stats(values):
    a = np.asarray(values)
    return dict(min=float(a.min()), p05=float(np.quantile(a,.05)), median=float(np.median(a)), p95=float(np.quantile(a,.95)), max=float(a.max()), mean=float(a.mean()))
for name in ['control','candidate']:
    d = json.loads((root/(name+'.json')).read_text())
    samples = d['samples']
    img = np.asarray(Image.open('out/review/cape-fold-v4/summit-'+name+'-final.png').convert('RGB'), dtype=float)
    pixels = []
    for s in samples:
        x,y = s['pixel']
        patch=img[y-2:y+3,x-2:x+3]
        pixels.append(float(np.median(patch@np.array([.2126,.7152,.0722]))))
    m = {k:stats([s[k] for s in samples]) for k in ['range_m','height_m','canonical_slope_deg','render_slope_deg','normal_error_deg','canonical_sun_cos','render_sun_cos','cell_m','render_height_error_m']}
    m['unit_sun_lambert_ambient_proxy'] = stats([.06+.94*max(0,s['render_sun_cos']) for s in samples])
    m['display_luma_8bit_5x5_median'] = stats(pixels)
    m.update(samples=len(samples),rays=d['rays'],sun_elevation_deg=d['sun_elevation_deg'],sun_azimuth_deg=d['sun_azimuth_deg'],sun_view_angle_deg=d['sun_view_angle_deg'])
    m['most_sloped_samples'] = sorted(samples,key=lambda s:s['canonical_slope_deg'],reverse=True)[:5]
    result[name]=m
result['limits']='Bounded canonical first-hit rays; ideal settled LOD reproduction, not GPU normals readback. Unit-sun Lambert+ambient proxy excludes actual irradiance, atmosphere, fill and tone mapping. PNG luma is display-referred, not physical radiance.'
(root/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:{a:b for a,b in v.items() if a!='most_sloped_samples'} if isinstance(v,dict) else v for k,v in result.items()},indent=2))
