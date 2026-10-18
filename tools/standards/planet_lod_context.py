"""The context of a level or a Planet Simulator record (tools/standards/planet_lod.py context): for a level, what is
meaningful there, by kind, and what its parent hands down frozen; for a record, what it names and what names it.

    python3 tools/standards/planet_lod_context.py pattern            # a level, by slug or key
    python3 tools/standards/planet_lod_context.py water-gap          # a record, by key or its last part
    python3 tools/standards/planet_lod_context.py <target> --json
"""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planet_lod  # noqa: E402

TREE = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "standards")


def main(argv):
    pos = [a for a in argv[1:] if not a.startswith("--")]
    if not pos:
        sys.exit(__doc__)
    c = planet_lod.context(planet_lod.load(TREE), pos[0])
    if "--json" in argv or "error" in c:
        print(json.dumps(c, indent=1)); return
    if "level" in c:
        print(f"{c['level']}: span {c['span_m']:,.0f} m, spacing {c['spacing_m'][0]:g}-{c['spacing_m'][1]:g} m; parent {c['parent']}, child {c['child']}")
        print(f"  exists: {', '.join(c['exists'] or [])}")
        if c["from_parent"]:
            print(f"  from the parent, frozen: {', '.join(c['from_parent'])}")
        for k, v in c["records"].items():
            print(f"  {k} ({len(v)}): {', '.join(x.split('.', 1)[1] for x in v[:14])}{' ...' if len(v) > 14 else ''}")
    else:
        print(f"{c['record']} ({c['name']}): {c['about']}")
        print(f"  the sim's word: {c['source']}")
        print(f"  names: {', '.join(c['names'])}")
        print(f"  named by ({len(c['named_by'])}): {', '.join(c['named_by'][:20])}{' ...' if len(c['named_by']) > 20 else ''}")


if __name__ == "__main__":
    main(sys.argv)
