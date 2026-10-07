"""Parts as a rule (registry audit, step 3b-i): a generated part folder becomes a list on its equipment record
(`built_of.list`), and the parts are derived from it by the build and by the registry reader (lib.part_yaml).

    python3 tools/standards/parts_inline.py --check   # every generated folder: derive from the files' own figures, compare field for field
    python3 tools/standards/parts_inline.py --write   # write the lists into the equipment records and delete the folders

A folder with a measured part (identity.drawing) or a sub-folder keeps its files. Run from the repository root.
"""
import glob, os, shutil, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import part_yaml, part_basis, effective_basis

E = "standards/SFO/metadata/equipment/"; P = "standards/SFO/metadata/parts/"
FIELDS = [("identity", "key"), ("identity", "code"), ("identity", "name"), ("identity", "description"), ("identity", "revision"), ("physical", "mass"), ("physical", "length"), ("physical", "width"), ("physical", "height"), ("making", "module"), ("fit", "count")]


def generated(folder):
    d = P + folder + "/"
    if not os.path.isdir(d): return []
    if any(os.path.isdir(d + x) for x in os.listdir(d)): return []
    files = sorted(glob.glob(d + "*.yaml"))
    if any("drawing:" in open(f).read() for f in files): return []
    return files


def norm(b):
    return [tuple(sorted((k, tuple(v) if isinstance(v, list) else v) for k, v in en.items())) for en in b]


def item_of(pt, eq=None):
    mf = (pt.get("made_from") or [{}])[0]
    it = {"code": pt["identity"]["code"], "name": pt["identity"]["name"], "mass": pt["physical"]["mass"], "length": pt["physical"]["length"], "width": pt["physical"]["width"], "height": pt["physical"]["height"]}
    if pt["identity"].get("description"): it["description"] = pt["identity"]["description"]
    if mf.get("item"): it["item"] = mf["item"]
    if mf.get("quantity") is not None: it["quantity"] = mf["quantity"]
    if (pt.get("making") or {}).get("module"): it["module"] = pt["making"]["module"]
    if (pt.get("fit") or {}).get("count", 1) != 1: it["count"] = pt["fit"]["count"]
    if eq is not None:
        defaults = {tuple(b["of"]): b for b in part_basis(eq, it)}
        own = [b for b in pt.get("basis") or [] if norm([b]) != norm([defaults.get(tuple(b["of"]), {})])]
        if own: it["basis"] = own
    return it


def get(d, path):
    for p in path:
        d = (d or {}).get(p) if isinstance(d, dict) else None
    return d


def folder_defaults(eq, files):
    """`shares` and `whole` for the equipment's built_of, from what most of its parts say; set on a copy of the record."""
    import collections, copy
    eq = copy.deepcopy(eq); bo = eq.setdefault("built_of", {})
    pts = [yaml.safe_load(open(f)) for f in files]
    shares = collections.Counter(b.get("rule") for pt in pts for b in pt.get("basis") or [] if b.get("of") == ["physical.mass", "fit"] and b.get("rule"))
    if shares and shares.most_common(1)[0][1] * 2 >= len(pts) and shares.most_common(1)[0][0] != (next((b.get("rule") for b in eq.get("basis") or [] if "built_of" in (b.get("of") or [])), None)):
        bo["shares"] = shares.most_common(1)[0][0]
    cut = [pt for pt in pts if (pt.get("making") or {}).get("module") in ("module.welding-bay", "module.machining-centre", "module.cutting-table", "module.electronics-works") and str((pt.get("made_from") or [{}])[0].get("item", "")).startswith("stock.")]
    whole = sum(1 for pt in cut for b in pt.get("basis") or [] if b.get("of") == ["made_from", "making"] and b.get("rule") == "rule.built-in-whole")
    if cut and whole * 2 > len(cut): bo["whole"] = True
    return eq


def compare(eq, folder, files):
    eq = folder_defaults(eq, files)
    diffs = []
    for f in files:
        pt = yaml.safe_load(open(f)); it = item_of(pt, eq); der = yaml.safe_load(part_yaml(eq, folder, it))
        for path in FIELDS:
            a, b = get(pt, path), get(der, path)
            if (a or None) != (b or None) and not (path == ("fit", "count") and (a or 1) == (b or 1)): diffs.append((f, ".".join(path), a, b))
        a = [(m.get("item"), m.get("quantity")) for m in pt.get("made_from") or []]; b = [(m.get("item"), m.get("quantity")) for m in der.get("made_from") or []]
        if a != b: diffs.append((f, "made_from", a, b))
        a, b = sorted(norm(pt.get("basis") or [])), sorted(norm(der.get("basis") or []))
        if a != b: diffs.append((f, "basis", a, b))
    return diffs


def flow(it):
    q = lambda s: '"' + str(s).replace('"', '\\"') + '"'
    order = ["code", "name", "description", "mass", "length", "width", "height", "item", "quantity", "module", "count"]
    head = "    - { " + ", ".join(f"{k}: {q(it[k]) if k in ('name', 'description') else it[k]}" for k in order if k in it)
    if it.get("basis"):
        from lib import _basis_flow
        return head + ",\n        basis: [" + ", ".join(_basis_flow(en) for en in it["basis"]) + "] }"
    return head + " }"


def main():
    check, write = "--check" in sys.argv, "--write" in sys.argv
    n_f = n_p = 0; alld = []
    for ef in sorted(glob.glob(E + "*.yaml")):
        eq = yaml.safe_load(open(ef)); folder = (eq.get("built_of") or {}).get("parts")
        if not folder or (eq.get("built_of") or {}).get("list"): continue
        files = generated(folder)
        if not files: continue
        alld += compare(eq, folder, files); n_f += 1; n_p += len(files)
        if write and not alld:
            eqd = folder_defaults(eq, files); bo = eqd["built_of"]
            items = [item_of(yaml.safe_load(open(f)), eqd) for f in files]
            s = open(ef, encoding="utf-8").read()
            head = f"built_of:\n  parts: {folder}\n" + (f"  shares: {bo['shares']}\n" if bo.get("shares") else "") + ("  whole: true\n" if bo.get("whole") else "")
            s = s.replace(f"built_of:\n  parts: {folder}\n", head + "  list:\n" + "\n".join(flow(it) for it in items) + "\n", 1)
            yaml.safe_load(s); open(ef, "w", encoding="utf-8").write(s)
            shutil.rmtree(P + folder)
    print(f"{n_f} folders, {n_p} parts; {len(alld)} field differences")
    for d in alld[:20]: print("  ", d)
    if write and not alld: print("lists written, folders removed")


if __name__ == "__main__":
    main()
