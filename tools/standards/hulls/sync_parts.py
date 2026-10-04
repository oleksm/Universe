#!/usr/bin/env python3
"""Bring a hull's parts in the registry up to date with its model.

    python3 tools/standards/hulls/sync_parts.py assets/models/mc07.glb mc-07            # show
    python3 tools/standards/hulls/sync_parts.py assets/models/mc07.glb mc-07 --write    # do it

The model owns a part's geometry: its size, its surface, where it sits. Those are measured from
the objects its record's `identity.drawing` names and refreshed. The registry owns everything else
(mass, what it is cut from, status...): never touched.

- An object of the model that no part names becomes a new part, at the top, named after it.
- A part whose objects are gone from the model is reported, not removed: it may have been renamed
  (point its `drawing` at the new objects and keep its code) or really gone (delete its file).

Not parts: collision boxes (COL_), decals, greebles, light and beam effects, and the empties.
"""
import fnmatch, json, os, re, sys

sys.path.insert(0, os.path.dirname(__file__))
from measure import measure

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
glb, hull = sys.argv[1], sys.argv[2]
write = "--write" in sys.argv
folder = os.path.join(ROOT, "standards", "SFO", "metadata", "parts", hull)
shown = os.path.relpath(os.path.abspath(glb), ROOT)
r = {k: v for k, v in measure(glb).items() if not (k.startswith(("COL_", "Decal_")) or "Greebles" in k or k.endswith("_Beam"))}
blocks = [v for k, v in r.items() if k.startswith("Hull_")] or list(r.values())
NOSE, KEEL = min(v["lo"][2] for v in blocks), min(v["lo"][1] for v in blocks)


def box(names):
    return [min(r[n]["lo"][i] for n in names) for i in range(3)], [max(r[n]["hi"][i] for n in names) for i in range(3)]


def facts(names, count):
    lo, hi = box(names)
    c = [(a + b) / 2 for a, b in zip(lo, hi)]
    side = "on the centre line" if abs(c[0]) < 0.05 else f"{abs(c[0]):.1f} m either side of the centre line" if count == 2 else f"{abs(c[0]):.1f} m to the {'left' if c[0] < 0 else 'right'} of the centre line"
    return {"length": round(hi[2] - lo[2], 2), "width": round(hi[0] - lo[0], 2), "height": round(hi[1] - lo[1], 2),
            "surface_area": round(sum(r[n]["area"] for n in names), 1),
            "position": f"Its centre is {c[2] - NOSE:.1f} m back from the nose and {c[1] - KEEL:.1f} m above the keel, {side}."}


files = sorted(os.path.join(d, f) for d, _, fs in os.walk(folder) for f in fs if f.endswith(".yaml"))
named, changes, gone = set(), [], []
for f in files:
    text = open(f).read()
    m = re.search(r'^  drawing: "?[^:"]+: ([^"\n]+)"?$', text, re.M)
    code = os.path.basename(f)[:-5]
    if not m:
        continue
    patterns = [p.strip() for p in m.group(1).split(",")]
    for p in patterns:
        named.update(fnmatch.filter(r, p))
    count = int(re.search(r"^  count: (\d+)", text, re.M).group(1)) if re.search(r"^  count: (\d+)", text, re.M) else 1
    # (Where there are several of a part, the first pattern names the one that is measured; where
    # there is one, all its patterns together are it.)
    mine = fnmatch.filter(r, patterns[0]) if count > 1 else sorted({n for p in patterns for n in fnmatch.filter(r, p)})
    if not mine:
        gone.append((code, m.group(1)))
        continue
    new = facts(mine, count)
    out = text
    for key in ("length", "width", "height", "surface_area"):
        old = re.search(rf"^  {key}: ([0-9.]+)$", out, re.M)
        if old and float(old.group(1)) != new[key]:
            changes.append((code, key, old.group(1), new[key]))
            out = re.sub(rf"^  {key}: [0-9.]+$", f"  {key}: {new[key]}", out, flags=re.M)
    old = re.search(r'^  position: "(Its centre is [^"]*)"$', out, re.M)
    if old and old.group(1) != new["position"]:
        changes.append((code, "position", old.group(1), new["position"]))
        out = out.replace(old.group(0), f"  position: {json.dumps(new['position'])}")
    if write and out != text:
        open(f, "w").write(out)

fresh = sorted(n for n in r if n not in named)
tops = [int(os.path.basename(f)[:-5].split("-")[1]) for f in files if os.path.dirname(f) == folder]
prefix = os.path.basename(files[0])[:-5].split("-")[0] if files else hull.replace("-", "").upper()
human = lambda n: re.sub(r"(?<=[a-z0-9])(?=[A-Z])", " ", n.replace("Hull_", "").replace("_L", ", left").replace("_R", ", right").replace("_", " ")).capitalize()
added = []
for k, n in enumerate(fresh, 1):
    code = f"{prefix}-{(max(tops) if tops else 0) + k:02d}"
    v = facts([n], 1)
    added.append((code, n))
    if write:
        open(os.path.join(folder, code + ".yaml"), "w").write("\n".join([
            "# yaml-language-server: $schema=../../../schema/part.schema.yaml", f"# Added from the model ({shown}): an object no part named. Measured there.",
            "identity:", f"  code: {code}", f"  name: {json.dumps(human(n))}", "  status: draft", f"  drawing: {json.dumps(shown + ': ' + n)}",
            "physical:", f"  length: {v['length']}", f"  width: {v['width']}", f"  height: {v['height']}", "shape:", f"  surface_area: {v['surface_area']}",
            "fit:", "  count: 1", f"  position: {json.dumps(v['position'])}"]) + "\n")

print(f"{len(files)} parts in the registry, {len(r)} objects in the model")
print(f"{len(changes)} measurements differ" + (":" if changes else ""))
for code, key, old, new in changes:
    print(f"  {code} {key}: {old} -> {new}")
print(f"{len(added)} objects no part names" + (" (added as parts):" if write and added else ":" if added else ""))
for code, n in added:
    print(f"  {n}" + (f" -> {code}" if write else f" (would be {code})"))
print(f"{len(gone)} parts whose objects are gone from the model" + (":" if gone else ""))
for code, d in gone:
    print(f"  {code}: {d}")
print("written" if write else "nothing written (add --write)")
