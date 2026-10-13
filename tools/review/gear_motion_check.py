"""Independent source-driver deployment evaluator, never a fixture-pose interpolator.

Diagnostic only: installed runtime/collision remain held. Angles/lengths come
from source driver metadata. The explicit half-turn convention follows the author handoff
gear-runtime-handoff/gear-motion-contract.yaml (2026-10-10).
"""
import argparse, ast, hashlib, json, math
from pathlib import Path
import numpy as np


def expression(text, g):
    def visit(n):
        if isinstance(n, ast.Constant) and type(n.value) in (int,float): return float(n.value)
        if isinstance(n, ast.Name) and n.id=='g': return g
        if isinstance(n, ast.UnaryOp) and isinstance(n.op,(ast.USub,ast.UAdd)):return (-1 if isinstance(n.op,ast.USub) else 1)*visit(n.operand)
        if isinstance(n,ast.BinOp):
            a,b=visit(n.left),visit(n.right)
            if isinstance(n.op,ast.Add):return a+b
            if isinstance(n.op,ast.Sub):return a-b
            if isinstance(n.op,ast.Mult):return a*b
            if isinstance(n.op,ast.Div):return a/b
        if isinstance(n,ast.Call) and isinstance(n.func,ast.Name) and n.func.id in ('min','max') and not n.keywords:
            return (min if n.func.id=='min' else max)(visit(a) for a in n.args)
        raise ValueError('Unsupported driver syntax: '+text)
    result=visit(ast.parse(text,mode='eval').body)
    if not math.isfinite(result):raise ValueError('Nonfinite driver')
    return result


def rotation(axis,angle):
    axis=np.asarray(axis,dtype=float);axis/=np.linalg.norm(axis)
    x,y,z=axis;k=np.array([[0,-z,y],[z,0,-x],[-y,x,0.]])
    return np.eye(3)+math.sin(angle)*k+(1-math.cos(angle))*(k@k)


def evaluate(nodes,g,half_turn,precision="f64"):
    dtype=np.float32 if precision=="f32" else np.float64
    raw={};busy=set()
    def build(name):
        if name in raw:return raw[name]
        if name in busy:raise ValueError('Parent cycle')
        busy.add(name);n=nodes[name];loc=np.array(n['location_parent_blender_m'],dtype);angles=np.array(n['rotation_parent_blender_rad'],dtype)
        for d in n['drivers']:
            if d.get('variables')!={'g':[{'node':'MiningShip_Root','path':'["gear_deploy"]'}]}:raise ValueError('Unexpected driver dependency')
            if d['path'] not in ('location','rotation_euler') or d['index'] not in (0,1,2):raise ValueError('Unsupported channel')
            (loc if d['path']=='location' else angles)[d['index']]=expression(d['expression'],g)
        m=np.eye(4,dtype=dtype);m[:3,3]=loc;m[:3,:3]=rotation([0,0,1],angles[2])@rotation([0,1,0],angles[1])@rotation([1,0,0],angles[0])
        if n['parent'] is not None:m=build(n['parent'])@m
        raw[name]=m;busy.remove(name);return m
    for name in nodes:build(name)
    result={name:m.copy() for name,m in raw.items()};singular=[]
    for name,n in nodes.items():
        for c in n['constraints']:
            if c['type']!='DAMPED_TRACK' or c['track_axis']!='TRACK_Y' or c['influence']!=1:raise ValueError('Unsupported constraint')
            m=raw[name];delta=raw[c['target']][:3,3]-m[:3,3];length=np.linalg.norm(delta)
            if length<1e-9:raise ValueError('Coincident actuator anchors')
            a=m[:3,1].astype(float);delta=delta.astype(float);length=np.linalg.norm(delta);b=delta/length;dot=float(np.clip(a@b,-1,1));cross=np.cross(a,b);sine=float(np.linalg.norm(cross))
            if dot<0 and sine<1e-6:
                if half_turn!='preconstraint-x':raise ValueError(f'{name}: antiparallel tracking requires explicit convention')
                axis=m[:3,0].astype(float)
                q=rotation(axis,math.pi);singular.append(name)
            elif sine<1e-12:q=np.eye(3)
            else:q=rotation(cross,math.atan2(sine,dot))
            result[name][:3,:3]=q@m[:3,:3]
    # Fixtures currently have no tracked parent: fail rather than silently omit propagation.
    tracked={name for name,n in nodes.items() if n['constraints']}
    assert not any(n['parent'] in tracked for n in nodes.values())
    B=np.array([[1.,0,0,0],[0,0,1,0],[0,-1,0,0],[0,0,0,1]])
    return {n:B@m@B.T for n,m in result.items()},singular


def main():
    p=argparse.ArgumentParser();p.add_argument('source');p.add_argument('reference');p.add_argument('output');p.add_argument('--half-turn',choices=['refuse','preconstraint-x'],default='refuse');p.add_argument('--precision',choices=['f64','f32'],default='f64');a=p.parse_args()
    source=json.loads(Path(a.source).read_text());reference=json.loads(Path(a.reference).read_text());rename=reference.get('node_rename',{});mirror=bool(rename);S=np.diag([-1.,1,1,1]) if mirror else np.eye(4);worst={'error':0.};singular=0;per_node={};frames=[]
    for pose in reference['poses']:
        g=pose['control'];assert source['control']['min']<=g<=source['control']['max']
        values,half=evaluate(source['nodes'],g,a.half_turn,a.precision);singular+=len(half);values={rename.get(n,n):S@m@S for n,m in values.items()};assert set(values)==set(pose['nodes'])
        for name,m in values.items():
            error=float(np.abs(m-np.array(pose['nodes'][name]['model_gltf'])).max());per_node[name]=max(per_node.get(name,0),error)
            if error>worst['error']:worst={'error':error,'node':name,'control':g}
        frames.append({'control':g,'nodes':{n:m.tolist() for n,m in values.items()}})
    sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
    report={'source_sha256':sha(a.source),'reference_sha256':sha(a.reference),'poses':len(frames),'nodes':len(per_node),'half_turn_convention':a.half_turn,'precision':a.precision,'half_turn_evaluations':singular,'worst':worst,'per_node_component_error':per_node,'threshold':2e-6,'pass':worst['error']<2e-6,'scope':'Independent analytic deployment from authored drivers and unconstrained TRS; right conjugation; no reference transforms used to evaluate motion. Half-turn follows author handoff gear-motion-contract.yaml; no runtime enablement or collision/landing qualification.'}
    Path(a.output).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
    return 0 if report['pass'] else 1
if __name__=='__main__':raise SystemExit(main())
