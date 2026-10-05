#!/usr/bin/env python3
"""Writes what the seed makes of the settled systems into the celestial registry
(standards/Celestial/metadata/systems). Runs the game's exporter
(`cargo run -p universe-world --example celestial_export`), or reads its JSON from a file.

A record whose provenance is `seeded` is written again; one that is `curated` or `frozen` is left
alone, and so is anything in it. Nothing is ever deleted: a body the seed no longer makes is
listed, for a person to decide.

    python3 tools/standards/celestial_export.py [exported.json] [--dry]
"""
import json, os, re, subprocess, sys
import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "standards", "Celestial", "metadata", "systems")
args = [a for a in sys.argv[1:] if not a.startswith("--")]
dry = "--dry" in sys.argv
raw = open(args[0]).read() if args else subprocess.run(["cargo", "run", "-q", "-p", "universe-world", "--example", "celestial_export"], cwd=ROOT, capture_output=True, text=True, check=True).stdout
data = json.loads(raw)
slug = lambda name: re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
# (A field's rocks: the game names their class by its label; the record names the rock class by its key.)
_RC = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "standards", "Celestial", "metadata", "rock-classes")
ROCK = {i["label"].lower(): i["key"] for i in (yaml.safe_load(open(os.path.join(_RC, f)))["identity"] for f in sorted(os.listdir(_RC))) if i.get("label")}
r = lambda v, n=4: float(f"{v:.{n}g}")
NATURAL = {"star", "rocky planet", "gas giant", "ice giant", "moon", "asteroid"}
wrote = kept = 0
seen = set()
# (Records are in SI. Each value is rounded as it reads (km, days, hours, light years), then written in SI.)
def law(name):
    """A law's value, from the Dogma registry (standards/Dogma), by its file name."""
    import glob as _g
    return float(yaml.safe_load(open(_g.glob(os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "standards", "Dogma", "metadata", "*", name + ".yaml"))[0]))["value"])


LY, SUN_W = law("light-year"), law("sun-luminosity")
si = lambda v: float(f"{v:.15g}")


def put(path, schema, rec):
    """Write a record unless a person has taken it over."""
    global wrote, kept
    seen.add(path)
    was = (yaml.safe_load(open(path)) or {}) if os.path.exists(path) else {}
    if was.get("provenance") in ("curated", "frozen"):
        kept += 1
        return
    # (What a person wrote about it stays with it: the seed has nothing to say there.)
    for k in ("about", "story"):
        if k in (was.get("identity") or {}):
            rec["identity"][k] = was["identity"][k]
    text = f"# yaml-language-server: $schema={schema}\n# As the seed makes it. Change its provenance to curated to take it over: it is then never written again.\n" + yaml.safe_dump(rec, sort_keys=False, allow_unicode=True, default_flow_style=None, width=120)
    if not os.path.exists(path) or open(path).read() != text:
        wrote += 1
        if not dry:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            open(path, "w").write(text)


for s in data["systems"]:
    d = sum(v * v for v in s["from_home_ly"]) ** 0.5
    put(os.path.join(OUT, slug(s["name"]) + ".yaml"), "../../schema/system.schema.yaml", {
        "provenance": "seeded",
        "identity": {"key": "system." + slug(s["name"]), "name": s["name"], "index": s["index"]},
        "position": {"from_home": [si(round(v, 3) * LY) for v in s["from_home_ly"]], "distance": si(round(d, 3) * LY)},
    })
    for b in s["bodies"]:
        # (Small bodies, marked by the export, go to small-bodies/: not written here yet.)
        if b["kind"] not in NATURAL or b.get("small"):
            continue
        rec = {"provenance": "seeded", "identity": {"key": f"body.{slug(s['name'])}.{slug(b['name'])}", "name": b["name"], "kind": b["kind"]}}
        if "parent" in b:
            rec["identity"]["parent"] = f"body.{slug(s['name'])}.{slug(b['parent'])}"
        if b["kind"] == "star":
            rec["star"] = {"class": s["class"], "luminosity": si(r(s["luminosity_suns"]) * SUN_W)}
        if "orbit" in b:
            o = b["orbit"]
            rec["orbit"] = {"semi_major_axis": si(r(o["semi_major_axis"] / 1000, 6) * 1000), "eccentricity": r(o["eccentricity"]), "period": si(r(o["period"] / 86400, 6) * 86400), "inclination": round(o["inclination"], 3)}
        ph = {"mass": r(b["mass"], 5), "radius": si(r(b["radius"] / 1000, 6) * 1000)}
        if "gravity" in b:
            ph["gravity"] = round(b["gravity"], 3)
        ph["day"] = si(r(b["day"] / 3600, 5) * 3600)
        ph["tilt"] = round(b["tilt"], 2)
        if "albedo" in b:
            ph["albedo"] = b["albedo"]
        if "rings" in b:
            ph["rings"] = [si(r(v / 1000, 6) * 1000) for v in b["rings"]]
        rec["physical"] = ph
        sf = {}
        if "terrain" in b:
            sf["terrain"], sf["relief"] = b["terrain"], round(b["relief"])
        if "mean_temperature" in b:
            sf["mean_temperature"] = b["mean_temperature"]
        sf["colour"] = b["colour"]
        rec["surface"] = sf
        if "atmosphere" in b:
            a = b["atmosphere"]
            rec["atmosphere"] = {"surface_density": a["surface_density"], "scale_height": si(r(a["scale_height"] / 1000) * 1000), "top": si(r(a["top"] / 1000) * 1000)}
        if "rock" in b:
            k = b["rock"]
            rec["rock"] = {"class": ROCK[k["class"].lower()], "structure": k["structure"].lower(), "density": round(k["density"])}
        put(os.path.join(OUT, slug(s["name"]), "bodies", slug(b["name"]) + ".yaml"), "../../../../schema/body.schema.yaml", rec)
    for f in s["fields"]:
        kind = "trojan" if f["kind"].startswith("Trojan") else f["kind"].lower()
        put(os.path.join(OUT, slug(s["name"]), "fields", slug(f["name"]) + ".yaml"), "../../../../schema/population.schema.yaml", {
            "provenance": "seeded",
            "identity": {"key": f"population.{slug(s['name'])}.{slug(f['name'])}", "name": f["name"], "kind": kind, "anchor": f"body.{slug(s['name'])}.{slug(f['anchor'])}"},
            "rocks": {"class": ROCK[f["class"].lower()], "count": f["count"], "extent": si(r(f["extent"] / 1000) * 1000)},
        })
gone = [os.path.relpath(os.path.join(dp, fn), ROOT) for dp, _, fns in os.walk(OUT) for fn in fns if fn.endswith(".yaml") and os.path.join(dp, fn) not in seen and os.path.basename(dp) in ("systems", "bodies", "fields")]
print(f"{len(data['systems'])} systems from seed {data['seed']}: {wrote} records {'would be ' if dry else ''}written, {kept} kept as a person left them")
for g in gone:
    print(f"  the seed no longer makes {g}")
