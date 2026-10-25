"""Diagram of the registry's data model from its schemas (Graphviz): kinds grouped by area, by the root they derive from.

    python3 tools/standards/schema_diagram.py            # writes docs/schema-map/roots.svg and references.svg

roots.svg       each record kind under the common definition it derives from (allOf $ref: physical_object,
                planet_context, ...); kinds that derive from none stand alone, grouped by area
references.svg  the same kinds and every x-ref between them (a field naming another kind's record by key)
index.html      the interactive map: search, filter by area, roots or references, click a kind for its fields and links
Generated: never edit the SVGs, rerun this.
"""
import collections, glob, json, os, subprocess
import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "docs", "schema-map")
COLORS = ["#4e79a7", "#f28e2b", "#59a14f", "#e15759", "#76b7b2", "#edc948", "#b07aa1", "#ff9da7", "#9c755f", "#bab0ac"]


def _load(path, cache={}):
    path = os.path.normpath(path)
    if path not in cache:
        cache[path] = yaml.safe_load(open(path))
    return cache[path]


def _resolve(ref, here):
    file, _, ptr = ref.partition("#")
    target = os.path.normpath(os.path.join(os.path.dirname(here), file)) if file else here
    node = _load(target) if os.path.isfile(target) else None
    for step in [x for x in ptr.split("/") if x]:
        node = node.get(step) if isinstance(node, dict) else None
    shared = (os.path.basename(target).replace(".schema.yaml", "") + "." if target != here else "") + ptr.split("/")[-1]
    return node, target, shared, (target, ptr)


def tree(sch, here, name="", req=False, stack=frozenset(), depth=0):
    """A schema as a property tree: {n, t, u, r, d, e, x, s, c}; $ref followed (s names the shared structure), allOf merged."""
    out = {"n": name}
    if req:
        out["r"] = 1
    if not isinstance(sch, dict) or depth > 9:
        return out
    if "$ref" in sch:
        node, target, shared, sig = _resolve(sch["$ref"], here)
        out["s"] = shared
        if sig in stack or node is None:
            out["d"] = "(see " + shared + " above)" if node is not None else "(unresolved " + sch["$ref"] + ")"
            return out
        inner = tree(node, target, name, req, stack | {sig}, depth + 1)
        inner["s"] = shared
        if sch.get("description"):
            inner["d"] = str(sch["description"])[:500]
        return inner
    t = sch.get("type")
    if t is not None:
        out["t"] = "|".join(t) if isinstance(t, list) else t
    for k, short in (("x-unit", "u"), ("const", "k")):
        if k in sch:
            out[short] = sch[k]
    if sch.get("description"):
        out["d"] = str(sch["description"])[:500]
    if sch.get("enum"):
        out["e"] = [str(v) for v in sch["enum"]][:60]
    if sch.get("x-ref"):
        out["x"] = sch["x-ref"] if isinstance(sch["x-ref"], list) else [sch["x-ref"]]
    for k in ("minimum", "maximum", "exclusiveMinimum", "pattern", "minItems", "maxItems"):
        if k in sch:
            out.setdefault("m", {})[k] = sch[k]
    props, required = {}, set(sch.get("required") or [])
    for part in sch.get("allOf") or []:                     # (allOf is derivation: what it derives from comes first)
        if isinstance(part, dict) and "$ref" in part:
            node, target, shared, sig = _resolve(part["$ref"], here)
            if isinstance(node, dict) and sig not in stack:
                out.setdefault("from", []).append(shared)
                for pk, pv in (node.get("properties") or {}).items():
                    props[pk] = (pv, target)
                required |= set(node.get("required") or [])
        elif isinstance(part, dict):
            for pk, pv in (part.get("properties") or {}).items():
                if pk not in props:
                    props[pk] = (pv, here)
            required |= set(part.get("required") or [])
    for pk, pv in (sch.get("properties") or {}).items():
        props[pk] = (pv, here)
    kids = [tree(pv, h, pk, pk in required, stack, depth + 1) for pk, (pv, h) in props.items()]
    if isinstance(sch.get("items"), dict):
        it = tree(sch["items"], here, "[ ]", False, stack, depth + 1)
        if it.get("c") or it.get("s") or it.get("x") or it.get("e"):
            kids.append(it)
        else:
            out["t"] = f"array of {it.get('t', 'any')}"
            for k in ("u", "x", "e"):
                if k in it:
                    out[k] = it[k]
    for key in ("oneOf", "anyOf"):
        for i, v in enumerate(sch.get(key) or []):
            label = ((v.get("properties") or {}).get("kind") or {}).get("const") if isinstance(v, dict) else None
            kids.append(tree(v, here, f"{key} {label or i + 1}", False, stack, depth + 1))
    if kids:
        out["c"] = kids
    return out


def model():
    """kinds; derives {root: kinds}; refs Counter {(from, to): fields}; fields {(from, to): [paths]}; uses {kind: {def: [paths]}}."""
    kinds, derives = {}, collections.defaultdict(set)
    fields, uses = collections.defaultdict(list), collections.defaultdict(lambda: collections.defaultdict(list))
    for p in sorted(glob.glob(os.path.join(ROOT, "standards", "**", "*.schema.yaml"), recursive=True)):
        s = _load(p)
        xk = s.get("x-kind") if isinstance(s, dict) else None
        if not xk:
            continue
        rel = os.path.relpath(p, os.path.join(ROOT, "standards"))
        area = rel.split(os.sep)[0] if os.sep in rel else "(root)"
        k = xk[0]
        desc = str(s.get("description") or "").strip()
        kinds[k] = {"tree": tree(s, p, k), "area": area, "file": rel, "title": s.get("title") or k, "also": xk[1:], "about": desc[:400],
                    "fields": sorted((s.get("properties") or {}).keys()), "required": s.get("required") or []}
        for al in s.get("allOf") or []:
            if isinstance(al, dict) and "$ref" in al and "#/definitions/" in al["$ref"]:
                derives[al["$ref"].split("/")[-1]].add(k)

        def walk(n, at, here, stack):
            if isinstance(n, dict):
                if "$ref" in n:                       # (a shared structure: follow it, so what it names counts too)
                    file, _, ptr = n["$ref"].partition("#")
                    target = os.path.normpath(os.path.join(os.path.dirname(here), file)) if file else here
                    name = ptr.split("/")[-1]
                    dname = (os.path.basename(target).replace(".schema.yaml", "") + "." + name) if target != here else name
                    if "/definitions/" in ptr:
                        uses[k][dname].append(at.strip(".") or "(record)")
                    if (target, ptr) not in stack and os.path.isfile(target):
                        node = _load(target)
                        for step in [x for x in ptr.split("/") if x]:
                            node = node.get(step) if isinstance(node, dict) else None
                        walk(node, at, target, stack | {(target, ptr)})
                for key, v in n.items():
                    if key == "x-ref":
                        for t in (v if isinstance(v, list) else [v]):
                            fields[(k, t)].append(at.strip(".") or "(record)")
                    elif key == "properties" and isinstance(v, dict):
                        for pk, pv in v.items():
                            walk(pv, f"{at}.{pk}", here, stack)
                    elif key == "items":
                        walk(v, at + "[]", here, stack)
                    elif key not in ("$ref", "definitions"):
                        walk(v, at, here, stack)
            elif isinstance(n, list):
                for x in n:
                    walk(x, at, here, stack)
        walk(s, "", p, frozenset())
    alias = {a: k for k, v in kinds.items() for a in v["also"]}
    merged = collections.defaultdict(list)
    for (a, b), fs in fields.items():
        b = alias.get(b, b)
        if b in kinds:
            merged[(a, b)] += [f for f in fs if f not in merged[(a, b)]]
    refs = collections.Counter({ab: len(fs) for ab, fs in merged.items() if ab[0] != ab[1]})
    return kinds, derives, refs, merged, uses


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
    kinds, derives, refs, merged, uses = model()
    os.makedirs(OUT, exist_ok=True)
    for name, with_refs in (("roots", False), ("references", True)):
        src = dot(kinds, derives, refs, with_refs)
        open(os.path.join(OUT, f"{name}.dot"), "w").write(src)
        subprocess.run(["dot", "-Tsvg", "-o", os.path.join(OUT, f"{name}.svg")], input=src, text=True, check=True)
    root_of = {k: r for r, ks in derives.items() for k in ks}
    data = {"kinds": [dict(v, key=k, root=root_of.get(k), uses={d: sorted(set(ps)) for d, ps in sorted(uses[k].items())}) for k, v in sorted(kinds.items())],
            "roots": {r: sorted(ks) for r, ks in sorted(derives.items())},
            "refs": [{"from": a, "to": b, "fields": fs} for (a, b), fs in sorted(merged.items())]}
    page = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "schema_map.html")).read()
    open(os.path.join(OUT, "index.html"), "w").write(page.replace("/*DATA*/null", json.dumps(data, separators=(",", ":"))))
    print(f"{len(kinds)} kinds in {len({v['area'] for v in kinds.values()})} areas; roots: "
          + ", ".join(f"{r} ({len(k)})" for r, k in sorted(derives.items())) + f"; {len(refs)} references -> {os.path.relpath(OUT, ROOT)}/")


if __name__ == "__main__":
    main()
