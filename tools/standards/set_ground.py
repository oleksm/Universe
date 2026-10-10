"""Point a body's `ground` at a planet-unfold snapshot in the worlds store, after checking it.

    python3 tools/standards/set_ground.py body.treistun.treistun-e worlds/TRE1/ground/earth_s3-20261009b [note]

Checks the snapshot's manifest: format planet-unfold-tiles/1, its world_id and body the record's, every listed file
present at its size; then writes {format, path, manifest_sha256, radius, note} into the record's `ground` block (replacing
one that is there). Run from the repository root.
"""
import glob, hashlib, json, os, re, sys
import yaml

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


def store():
    p = os.environ.get("UNIVERSE_WORLDS") or os.path.expanduser("~/git/planet-sim/out")
    return p if os.path.isdir(p) else sys.exit("no worlds store (UNIVERSE_WORLDS)")


def main(key, path, note=None):
    rec = next((f for f in glob.glob(os.path.join(ROOT, "standards/Celestial/metadata/systems/*/bodies/*.yaml")) if (yaml.safe_load(open(f)) or {}).get("identity", {}).get("key") == key), None) or sys.exit(f"no body {key}")
    body = yaml.safe_load(open(rec)); folder = os.path.join(store(), path); mpath = os.path.join(folder, "manifest.json")
    m = json.load(open(mpath)); sha = hashlib.sha256(open(mpath, "rb").read()).hexdigest()
    wid = (body.get("survey") or {}).get("world_id")
    if m.get("format") != "planet-unfold-tiles/1": sys.exit(f"format {m.get('format')}")
    if m.get("body") not in (None, key) or m.get("world_id") not in (None, wid): sys.exit(f"manifest names {m.get('body')} / {m.get('world_id')}, not {key} / {wid}")
    bad = [n for n, f in m["files"].items() if not os.path.isfile(os.path.join(folder, n)) or ("size" in f and os.path.getsize(os.path.join(folder, n)) != f["size"])]
    if bad: sys.exit(f"{len(bad)} files missing or of the wrong size: {bad[:5]}")
    total = sum(os.path.getsize(os.path.join(folder, n)) for n in m["files"])
    note = note or f"{m.get('name', '')}: {len(m['files'])} files, {total / 1e6:.0f} MB in the worlds store" + (f"; terraform {m['source']['terraform']}" if (m.get("source") or {}).get("terraform") else "") + "."
    block = (f'ground:\n  format: "planet-unfold-tiles/1"\n  path: {json.dumps(path)}\n  manifest_sha256: "{sha}"\n  radius: {m.get("radius_m")}\n  note: {json.dumps(note)}\n')
    s = open(rec).read()
    s2 = re.sub(r"^ground:\n(?:  .*\n)+", block, s, count=1, flags=re.M) if re.search(r"^ground:\n", s, flags=re.M) else s.rstrip("\n") + "\n" + block
    yaml.safe_load(s2); open(rec, "w").write(s2)
    print(f"{key}: ground -> {path} ({len(m['files'])} files, {total / 1e6:.0f} MB, sha {sha[:12]})")


if __name__ == "__main__":
    if len(sys.argv) < 3: sys.exit(__doc__)
    main(*sys.argv[1:4])
