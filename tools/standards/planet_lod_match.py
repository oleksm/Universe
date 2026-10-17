"""Landform patterns for a place: its signature (fields in SI, layer classes, nearby landmarks, factor strengths) against
the PlanetLOD patterns' matching rules (tools/standards/planet_lod.py match).

    python3 tools/standards/planet_lod_match.py <signature.json> [--top=10] [--family=glacial] [--json]
    python3 tools/standards/planet_lod_match.py --example           # a signature to start from
"""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planet_lod  # noqa: E402

TREE = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "standards")
EXAMPLE = {"fields": {"aridity": 0.8, "temperature": 270.0, "ice-lgm": 1200.0, "ice-cycles": 30, "local-relief": 1600.0, "discharge": 120.0, "rain": 3.2e-8},
           "classes": {"zones": 7, "climate": 3}, "landmarks": [], "factors": {"TECT_UPLIFT": 0.6}}


def main(argv):
    opt = {a.split("=", 1)[0].lstrip("-"): (a.split("=", 1)[1] if "=" in a else True) for a in argv[1:] if a.startswith("--")}
    if opt.get("example"):
        print(json.dumps(EXAMPLE, indent=1)); return
    pos = [a for a in argv[1:] if not a.startswith("--")]
    if not pos:
        sys.exit(__doc__)
    sig = json.load(open(pos[0]))
    R = planet_lod.load(TREE)
    rows = planet_lod.match(R, sig, int(opt.get("top", 10)), opt.get("family") if isinstance(opt.get("family"), str) else None)
    if opt.get("json"):
        print(json.dumps(rows, indent=1)); return
    for r in rows:
        print(f"{r['score']:.3f}  {r['key']}  ({r['name']})")
        print("       " + ", ".join(f"{c} {w:g}x{s:.2f}" for c, w, s in r["fired"][:6]))


if __name__ == "__main__":
    main(sys.argv)
