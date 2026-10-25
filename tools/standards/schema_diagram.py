"""Diagram of the registry's data model from its schemas (Graphviz): kinds grouped by area, by the root they derive from.

    python3 tools/standards/schema_diagram.py            # writes docs/schema-map/roots.svg and references.svg

roots.svg       each record kind under the common definition it derives from (allOf $ref: physical_object,
                planet_context, ...); kinds that derive from none stand alone, grouped by area
references.svg  the same kinds and every x-ref between them (a field naming another kind's record by key)
Generated: never edit the SVGs, rerun this.
"""
import collections, glob, os, subprocess
import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "docs", "schema-map")
COLORS = ["#4e79a7", "#f28e2b", "#59a14f", "#e15759", "#76b7b2", "#edc948", "#b07aa1", "#ff9da7", "#9c755f", "#bab0ac"]


def model():
    kinds, derives, refs = {}, collections.defaultdict(set), collections.Counter()
    for p in sorted(glob.glob(os.path.join(ROOT, "standards", "**", "*.schema.yaml"), recursive=True)):
        s = yaml.safe_load(open(p))
        xk = s.get("x-kind") if isinstance(s, dict) else None
        if not xk:
            continue
        rel = os.path.relpath(p, os.path.join(ROOT, "standards"))
        area = rel.split(os.sep)[0] if os.sep in rel else "(root)"
        k = xk[0]
        kinds[k] = {"area": area, "file": rel, "title": s.get("title") or k, "also": xk[1:]}
        for al in s.get("allOf") or []:
            if isinstance(al, dict) and "$ref" in al and "#/definitions/" in al["$ref"]:
                derives[al["$ref"].split("/")[-1]].add(k)
        seen = set()

        def walk(n):
            if isinstance(n, dict):
                for key, v in n.items():
                    if key == "x-ref":
                        seen.update(v if isinstance(v, list) else [v])
                    walk(v)
            elif isinstance(n, list):
                for x in n:
                    walk(x)
        walk(s)
        for t in seen:
            refs[(k, t)] += 1
    alias = {a: k for k, v in kinds.items() for a in v["also"]}
    refs = collections.Counter({(a, alias.get(b, b)): n for (a, b), n in refs.items() if alias.get(b, b) in kinds and alias.get(b, b) != a})
    return kinds, derives, refs


def dot(kinds, derives, refs, with_refs):
    areas = sorted({v["area"] for v in kinds.values()})
    col = {a: COLORS[i % len(COLORS)] for i, a in enumerate(areas)}
    rooted = {k for ks in derives.values() for k in ks}
    lines = ["digraph registry {", '  graph [rankdir=LR, fontname="Helvetica", fontsize=11, overlap=false, splines=true, nodesep=0.15, ranksep=1.2];',
             '  node [shape=box, style="rounded,filled", fontname="Helvetica", fontsize=10, fillcolor="#ffffff"];',
             '  edge [fontname="Helvetica", fontsize=8];']
    for r in sorted(derives):
        lines.append(f'  "root:{r}" [label="{r}\\n(common definition, {len(derives[r])} kinds)", shape=box3d, fillcolor="#222222", fontcolor="#ffffff"];')
    for a in areas:
        lines.append(f'  subgraph "cluster_{a}" {{ label="{a}"; color="{col[a]}"; fontcolor="{col[a]}"; style="rounded";')
        for k, v in sorted(kinds.items()):
            if v["area"] == a:
                pen = "2" if k in rooted else "1"
                lines.append(f'    "{k}" [label="{k}", color="{col[a]}", penwidth={pen}, tooltip="{v["file"]}"];')
        lines.append("  }")
    for r, ks in sorted(derives.items()):
        for k in sorted(ks):
            lines.append(f'  "root:{r}" -> "{k}" [color="#222222", penwidth=1.6, arrowhead=onormal];')
    if with_refs:
        for (a, b), n in sorted(refs.items()):
            lines.append(f'  "{a}" -> "{b}" [color="{col[kinds[a]["area"]]}88", penwidth={min(0.6 + 0.3 * n, 2.4):.1f}, arrowsize=0.5];')
    lines.append("}")
    return "\n".join(lines)


def main():
    kinds, derives, refs = model()
    os.makedirs(OUT, exist_ok=True)
    for name, with_refs in (("roots", False), ("references", True)):
        src = dot(kinds, derives, refs, with_refs)
        open(os.path.join(OUT, f"{name}.dot"), "w").write(src)
        subprocess.run(["dot", "-Tsvg", "-o", os.path.join(OUT, f"{name}.svg")], input=src, text=True, check=True)
    print(f"{len(kinds)} kinds in {len({v['area'] for v in kinds.values()})} areas; roots: "
          + ", ".join(f"{r} ({len(k)})" for r, k in sorted(derives.items())) + f"; {len(refs)} references -> {os.path.relpath(OUT, ROOT)}/")


if __name__ == "__main__":
    main()
