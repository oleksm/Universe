#!/usr/bin/env python3
"""Exercise shared live diagnostics in an isolated Studio process (no user save writes)."""
import argparse, json, os, pathlib, subprocess, time, urllib.request
p=argparse.ArgumentParser();p.add_argument('binary');p.add_argument('design');p.add_argument('output');p.add_argument('--port',type=int,default=17879);p.add_argument('--mode',choices=['design','drive','balance','walk'],default='design');p.add_argument('--warmup',type=float,default=4);p.add_argument('--samples',type=int,default=1);p.add_argument('--debug',type=int,default=2);a=p.parse_args()
out=pathlib.Path(a.output).resolve();out.mkdir(parents=True,exist_ok=True)
env=dict(os.environ,UNIVERSE_OBSERVING='1',UNIVERSE_OBSERVE_PORT=str(a.port),XDG_DATA_HOME=str(out/'data'),UNIVERSE_DEBUG=str(a.debug),UNIVERSE_MAX_FPS='60')
for k in ['UNIVERSE_SCREENSHOT','UNIVERSE_SCREENSHOT_AT']:env.pop(k,None)
if a.mode=='drive':env['UNIVERSE_DRIVE']='0'
if a.mode=='balance':env['UNIVERSE_BALANCE']='trim'
if a.mode=='walk':env['UNIVERSE_STUDIO_WALK']=''
def get(path):
    with urllib.request.urlopen(f'http://127.0.0.1:{a.port}'+path,timeout=3) as r:return r.read().decode()
with (out/'run.log').open('w') as log:
    proc=subprocess.Popen([str(pathlib.Path(a.binary).resolve()),str(pathlib.Path(a.design).resolve())],env=env,stdout=log,stderr=subprocess.STDOUT)
    try:
        until=time.monotonic()+40
        while True:
            try:
                status=json.loads(get('/status'));break
            except Exception:
                assert proc.poll() is None, 'Studio exited; see run.log'
                if time.monotonic()>until:raise
                time.sleep(.2)
        assert status['mode']=='studio' and status['view']==a.mode,status
        time.sleep(a.warmup)
        samples=[]
        for n in range(a.samples):
            if n:time.sleep(.5)
            samples.append(json.loads(get('/perf')))
        (out/'perf-samples.json').write_text(json.dumps(samples,indent=2)+'\n')
        for route in ['status','perf','profile','resources','graphics']:
            body=get('/'+route);(out/(route+('.txt' if route=='profile' else '.json'))).write_text(body)
        shot=json.loads(get('/screenshot'))
        until=time.monotonic()+5
        while not pathlib.Path(shot['path']).exists():
            assert time.monotonic()<until,shot
            time.sleep(.05)
        # Let the asynchronous PNG writer finish before enabling recording's HUD.
        time.sleep(.3)
        recording=json.loads(get('/record/start'));assert recording['recording']
        time.sleep(2)
        stopped=json.loads(get('/record/stop'));assert stopped['written']==recording['recording']
        session=pathlib.Path(stopped['written'])
        trace=json.loads((session/'trace.json').read_text())
        assert any(e.get('name','').startswith('draw/studio') for e in trace['traceEvents']), 'missing Studio scopes'
        assert json.loads((session/'profile.json').read_text())['frames']
        time.sleep(.3)
        assert pathlib.Path(shot['path']).exists(),shot
        import shutil
        shutil.copyfile(shot['path'],out/'view.png')
        (out/'result.json').write_text(json.dumps({'status':status,'session':str(session),'trace_events':len(trace['traceEvents']),'passed':True},indent=2)+'\n')
    finally:
        proc.terminate()
        try:proc.wait(timeout=5)
        except subprocess.TimeoutExpired:proc.kill();proc.wait()
