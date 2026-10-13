#!/usr/bin/env python3
"""Compile the command harness against already-tested runtime rlibs; run under scenery_job.py.
Does not rebuild or mutate a shared Cargo target. The linked artifacts and source are hashed.
"""
import argparse, hashlib, json, pathlib, subprocess, time
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--deps',type=pathlib.Path,required=True)
p.add_argument('--handoff',type=pathlib.Path,required=True)
p.add_argument('--out',type=pathlib.Path,required=True)
p.add_argument('--mode',choices=['orbit','lowflight'],default='orbit')
p.add_argument('--hull',default='')
a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
source=pathlib.Path(__file__).with_suffix('.rs');binary=a.out/'journey'
def sha(f):return hashlib.file_digest(f.open('rb'),'sha256').hexdigest()
cmd=['rustc','--edition=2024',str(source),'-L',f'dependency={a.deps}','-C','opt-level=2','-o',str(binary)]
libs={}
for key in ['universe_world','universe_avionics','universe_sim','glam','serde_json']:
 found=max(a.deps.glob('lib'+key+'-*.rlib'),key=lambda f:f.stat().st_mtime)
 cmd+=['--extern',f'{key}={found}'];libs[key]={'path':str(found.resolve()),'sha256':sha(found)}
subprocess.run(cmd,check=True)
provenance={'base':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'source':str(source.resolve()),'source_sha256':sha(source),'binary_sha256':sha(binary),'linked':libs,'command':cmd,'mode':a.mode,'hull':a.hull,'handoff_sha256':sha(a.handoff),'scope':'headless actual shared runtime; no desktop/GPU/input-device validation; run this entire script under shared scenery launcher2CPU8GiB600s'}
(a.out/'provenance.json').write_text(json.dumps(provenance,indent=2))
t=time.monotonic();result=subprocess.run([str(binary.resolve()),str(a.handoff.resolve()),str(a.out.resolve()),a.mode,a.hull]);provenance|={'exit':result.returncode,'run_seconds':time.monotonic()-t};(a.out/'provenance.json').write_text(json.dumps(provenance,indent=2));raise SystemExit(result.returncode)
