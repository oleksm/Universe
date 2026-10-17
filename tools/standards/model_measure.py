"""A plant unit's envelope boxes, measured from its model: the boxes the fit check (fit_layout.py) holds runs and
other units out of, in the record's frame (x along, y across, z up, metres; glTF's y up becomes z, its z becomes -y).

    python3 tools/standards/model_measure.py <model.glb> --as=unit.<slug>                # the boxes it would write
    python3 tools/standards/model_measure.py <model.glb> --as=unit.<slug> --apply        # write them, close the request
    python3 tools/standards/model_measure.py <model.glb> --as=unit.<slug> --list         # every named node's box, to pick --nodes from

One box per top-level group of the model (the scene root's children, or --nodes=a,b,c to name them); with more than
--max boxes (default 10) the two whose joined box adds least volume are joined until it fits. Name the groups so that
open space stays open: a belt's way under a hopper, a duct's way past a loading end. --apply writes the boxes into
tools/standards/unit_envelopes.yaml (plants.py puts them on the record), drops the unit's open `measure` request from
tools/standards/unit_requests.yaml, and regenerates. Docs: docs/asset-contract.md, "Rework".
"""
import datetime, json, os, re, struct, subprocess, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from assets import ROOT, inspect_glb, records  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))


def tops(path):
    """The names of the scene root's children (or the roots themselves when there are several)."""
    b = open(path, "rb").read()
    clen = struct.unpack_from("<I", b, 12)[0]
    g = json.loads(b[20:20 + clen])
    nodes = g.get("nodes", [])
    scene = g["scenes"][g.get("scene", 0)]["nodes"] if g.get("scenes") else list(range(len(nodes)))
    if len(scene) == 1 and nodes[scene[0]].get("children"):
        scene = nodes[scene[0]]["children"]
    return [nodes[i].get("name") for i in scene if nodes[i].get("name")]


def to_record(lo, hi):
    """glTF (y up) to the record's frame (z up): x stays, y is -z, z is y."""
    return [round(lo[0], 3), round(-hi[2], 3), round(lo[1], 3), round(hi[0], 3), round(-lo[2], 3), round(hi[1], 3)]


def vol(b):
    return max(0.0, b[3] - b[0]) * max(0.0, b[4] - b[1]) * max(0.0, b[5] - b[2])


def join(a, b):
    return [min(a[0], b[0]), min(a[1], b[1]), min(a[2], b[2]), max(a[3], b[3]), max(a[4], b[4]), max(a[5], b[5])]


def measure(path, names=None, most=10):
    info = inspect_glb(path)
    names = names or tops(path)
    boxes = [to_record(*info["node_box"][n]) for n in names if n in info["node_box"]]
    while len(boxes) > most:
        best = min(((i, j) for i in range(len(boxes)) for j in range(i + 1, len(boxes))),
                   key=lambda ij: vol(join(boxes[ij[0]], boxes[ij[1]])) - vol(boxes[ij[0]]) - vol(boxes[ij[1]]))
        a, b = boxes[best[0]], boxes[best[1]]
        boxes = [x for k, x in enumerate(boxes) if k not in best] + [join(a, b)]
    return boxes, [n for n in names if n in info["node_box"]]


def main(argv):
    opt = {a.split("=", 1)[0].lstrip("-"): (a.split("=", 1)[1] if "=" in a else True) for a in argv[1:] if a.startswith("--")}
    glb = next((a for a in argv[1:] if not a.startswith("--")), None)
    key = opt.get("as")
    if not glb or not isinstance(key, str) or not key.startswith("unit."):
        sys.exit(__doc__)
    hit = [r for k, kk, p, r in records() if kk == key]
    if not hit:
        sys.exit(f"no record {key}")
    if opt.get("list"):
        for n, b in inspect_glb(glb)["node_box"].items():
            print(f"{n:<40} {to_record(*b)}")
        return
    names = opt["nodes"].split(",") if isinstance(opt.get("nodes"), str) else None
    boxes, used = measure(glb, names, int(opt.get("max", 10)))
    version = (hit[0].get("visual") or {}).get("version")
    print(json.dumps({"key": key, "from_version": version, "groups": used, "envelope": boxes}, indent=1))
    if not opt.get("apply"):
        print("dry run: --apply writes them")
        return
    f = os.path.join(HERE, "unit_envelopes.yaml")
    text = open(f).read()
    entry = f"{key}:     # (measured from v{version} by model_measure.py, {datetime.date.today().isoformat()}: {', '.join(used)})\n" + "".join(f"- {json.dumps(b)}\n" for b in boxes)
    m = re.search(rf"^{re.escape(key)}:.*\n((?:- .*\n)*)", text, re.M)
    text = text[:m.start()] + entry + text[m.end():] if m else text.rstrip("\n") + "\n" + entry
    open(f, "w").write(text)
    rq = os.path.join(HERE, "unit_requests.yaml")
    reqs = yaml.safe_load(open(rq)) or {}
    if reqs.get(key):
        left = [q for q in reqs[key] if q["do"] != "measure"]
        lines = open(rq).read()
        block = re.search(rf"^{re.escape(key)}:\n((?:- .*\n)*)", lines, re.M)
        lines = lines[:block.start()] + ((f"{key}:\n" + "".join(l + "\n" for l in block.group(1).splitlines() if "do: measure" not in l)) if left else "") + lines[block.end():]
        open(rq, "w").write(lines)
    out = subprocess.run([sys.executable, os.path.join(HERE, "plants.py")], capture_output=True, text=True, cwd=ROOT)
    print((out.stdout.strip().splitlines() or [""])[-1], out.stderr[-500:])
    print("written; run tools/standards/fit_layout.py and validate.py, then commit on fso")


if __name__ == "__main__":
    main(sys.argv)
