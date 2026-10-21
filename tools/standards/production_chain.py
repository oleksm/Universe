"""Audit every stocked item, its ingredients, and its manufactured packaging.

Recipe coverage is not ingredient coverage. Solve the production graph from
declared natural-resource and elemental supply boundaries by a fixed point;
cycles cannot prove themselves. A recipe's by-products require all its inputs.
Shop recipes come from each part's making.module and made_from, as in the game.
No assertion is made about installed facilities, power, capacity or qualification.
"""
from pathlib import Path
from collections import defaultdict
import sys
import yaml

ROOT = Path(__file__).resolve().parents[2]


def load_records():
    records = {}
    for path in (ROOT / "standards/SFO/metadata").rglob("*.yaml"):
        d = yaml.safe_load(path.read_text())
        if isinstance(d, dict) and (key := (d.get("identity") or {}).get("key")):
            records[key] = d
    return records


def audit(records):
    recipes, aliases = [], []
    sources = defaultdict(set)
    supplied = defaultdict(list)
    for key, d in records.items():
        if key.startswith("module."):
            for r in d.get("recipes") or []:
                ins = {i["item"] for i in r.get("inputs") or [] if i.get("from", "stock") != "place" and i.get("quantity", 0) > 0}
                # Empty or zero-input recipes are faults, not sources of matter.
                if not any(i.get("quantity", 0) > 0 for i in r.get("inputs") or []):
                    continue
                outs = {r["makes"]} | {o["item"] for o in r.get("outputs") or [] if o.get("quantity", 0) > 0}
                recipes.append((key, ins, outs))
                for out in outs:
                    sources[out].add(key)
        elif key.startswith("part.") and (maker := (d.get("making") or {}).get("module")):
            ins = {i["item"] for i in d.get("made_from") or [] if i.get("quantity", 0) > 0}
            if ins and maker in records:
                recipes.append((maker, ins, {key}))
                sources[key].add(maker)
        if key.startswith("stock."):
            target = (d.get("supplies") or {}).get("item")
            if target:
                supplied[target].append(key)
            parts = d.get("made_from") or []
            # A homogeneous raw/bulk stock wraps its good. A compound kit that
            # contains PTFE does not manufacture PTFE or any of its other parts.
            if len(parts) == 1 and not target:
                item = parts[0]["item"]
                if (d.get("identity") or {}).get("form") in ("bulk", "fluid"):
                    aliases.append((item, key))
                if item.startswith("material."):
                    aliases.append((key, item))

    # Opening/disposal is an inventory transformation, not a factory recipe:
    # it turns manufactured tare into the same mass of mixed waste. It cannot
    # bootstrap manufacturing, and does not recover virgin materials for free.
    operation_sources = defaultdict(set)
    for key, d in records.items():
        if not key.startswith("package.") or not (waste := d.get("waste_stock")):
            continue
        empty = supplied.get(key, [])
        if len(empty) != 1 or waste not in records:
            continue
        outs = {waste}
        composition = records[waste].get("made_from") or []
        if len(composition) == 1:
            outs.add(composition[0]["item"])
        recipes.append(("Opening/disposal of " + key, {empty[0]}, outs))
        for out in outs:
            sources[out].add("Opening/disposal of " + key)
            operation_sources[out].add(key)

    # Element supply is the existing registry boundary pending world composition.
    # Natural sources likewise identify a resource, not a placed, operating mine.
    boundary = {k for k, d in records.items() if (k.startswith("element.") and k not in sources) or (k.startswith("good.") and (d.get("source") or {}).get("occurs"))}
    available = set(boundary)
    while True:
        before = len(available)
        for _, ins, outs in recipes:
            if ins <= available:
                available.update(outs)
        for source, target in aliases:
            if source in available:
                available.add(target)
        if len(available) == before:
            break

    issues, rows = [], []
    for key, d in sorted(records.items()):
        if not key.startswith(("stock.", "package.")):
            continue
        empty = (d.get("supplies") or {}).get("item", "").startswith("package.")
        pkg = key if key.startswith("package.") else d.get("package")
        packers = supplied.get(pkg, []) if pkg else []
        missing = []
        if key.startswith("stock.") and key not in available:
            missing.append("no ingredient-complete manufacturing route")
        if not empty:
            if not pkg or pkg not in records:
                missing.append("no handling package")
            elif len(packers) != 1:
                missing.append(f"package has {len(packers)} empty-stock definitions (one required)")
            elif packers[0] not in available:
                missing.append("empty package has no ingredient-complete manufacturing route")
            if pkg in records and not isinstance(records[pkg].get("returns_empty"), bool):
                missing.append("normal-opening return policy not specified")
            if pkg in records and records[pkg].get("waste_stock") not in records:
                missing.append("spent-tare waste stock not specified")
        if empty:
            target = records.get(d["supplies"]["item"], {})
            if d.get("package"):
                missing.append("empty package is packed in another package")
            if (d.get("physical") or {}).get("mass") != (target.get("physical") or {}).get("mass"):
                missing.append("empty-stock mass differs from package tare")
        if missing:
            issues.extend(f"{key}: {m}" for m in missing)
        rows.append({"key": key, "name": d["identity"]["name"], "mass": (d.get("physical") or {}).get("mass"), "sources": sorted(sources.get(key, [])), "package": pkg, "empty_stock": packers[0] if len(packers) == 1 else None, "issues": missing})

    # Keep the factories-make-factories check scoped to the works used by kit
    # production and the newly introduced supply chain, not unrelated old gaps.
    kit_makers = {owner for owner, _, outs in recipes if any(k.startswith(("stock.kit-", "stock.chem-", "stock.pkg-")) for k in outs)}
    kit_makers |= {"module.factory-works"} & records.keys()
    for maker in sorted(kit_makers):
        if maker not in available:
            issues.append(f"{maker}: no ingredient-complete construction recipe")
    blocked_inputs = sorted({i for _, ins, _ in recipes for i in ins if i not in available})
    return {"rows": rows, "issues": issues, "available": available, "boundary": boundary, "blocked_inputs": blocked_inputs, "works": sorted(kit_makers), "operation_sources": operation_sources}


def main():
    result = audit(load_records())
    for issue in result["issues"]:
        print("problem: " + issue)
    if result["blocked_inputs"]:
        print("Unresolved recipe inputs: " + ", ".join(result["blocked_inputs"]))
    stock = [r for r in result["rows"] if r["key"].startswith("stock.")]
    packages = [r for r in result["rows"] if r["key"].startswith("package.")]
    print("Conditional on natural-resource and elemental supply; draft recipes do not establish qualified plant performance or installed facilities.")
    print(f"{sum(not r['issues'] for r in stock)}/{len(stock)} stock chains; {sum(not r['issues'] for r in packages)}/{len(packages)} package chains; {len(result['works'])} kit/supply works; {len(result['issues'])} fault(s)")
    return bool(result["issues"])


if __name__ == "__main__":
    sys.exit(main())
