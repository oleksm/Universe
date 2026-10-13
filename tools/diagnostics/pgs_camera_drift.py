"""Read saved PGS_CONTACT logs; do not equate launch settings with view identity."""
import pathlib,json,sys,hashlib
root=pathlib.Path(sys.argv[1]);result=[]
for path in sorted(root.glob('present-settled-*.log')):
 rows=[json.loads(line.split('PGS_CONTACT ',1)[1]) for line in path.read_text().splitlines() if 'PGS_CONTACT {' in line]
 rows=[r for r in rows if 'eye_height_m' in r];end=rows[-1]['frame'];window=[r for r in rows if r['frame']>=end-300]
 changes=[];previous=None
 for row in rows:
  pair=(row['eye_height_m'],row['canonical_height_m'])
  if pair!=previous:changes.append(dict(frame=row['frame'],eye_height_m=pair[0],canonical_height_m=pair[1]));previous=pair
 result.append(dict(run=path.stem,log_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),changes=changes,last_terrain_frame=end,conservative_final_window=[end-300,end+29],window_sample_count=len(window),window_eye_range_m=[min(r['eye_height_m'] for r in window),max(r['eye_height_m'] for r in window)],window_canonical_range_m=[min(r['canonical_height_m'] for r in window),max(r['canonical_height_m'] for r in window)]))
print(json.dumps(dict(scope='Height samples only, every30 terrain draws; one visible terrain draw per GUI frame assumed. No full position/orientation/time captured. Equal scalar samples do not prove fixed transform. Baseline-repeat jump falls inside final300 window even allowing29-frame logging phase.',results=result),indent=2))
