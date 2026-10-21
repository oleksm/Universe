"""PlanetSim ground catalogue: six cross-checks, with explicit draft migration gaps.

Use --strict to fail on any gap. --emitted-kinds accepts the forge's JSON/YAML
manifest; --features-jsonl additionally checks kinds observed in a simulator run.
The default manifest is a versioned YAML snapshot, never an implicit sibling repo.
"""
from pathlib import Path
import argparse
import json
import math
import yaml

ROOT = Path(__file__).resolve().parents[2]
TREE = ROOT / "standards"
STRICT = False  # Switch only after Planets completes the catalogue migration.
NAMES = ("planet", "continent", "region", "landscape", "site", "slope", "ground", "surface")
LEVELS = tuple("planet-level." + name for name in NAMES)
VIEWS = tuple(10.0 ** power for power in range(7, -1, -1))
CHECKS = ("size and ladder", "expansion graph", "examples and built geometry",
          "emitted kinds", "surface blend", "line/feature reciprocity")


def derived_ladder(across):
    """Inclusive thresholds: largest >= 10 pixels; smallest fits the finest view.

    Ranges below 1 cm or wholly larger than the planet view have no valid ladder.
    Oversized largest members are allowed when smaller members fit a view.
    """
    low, high = across
    if not all(isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x) and x > 0 for x in across) or low > high:
        raise ValueError("size range must be finite, positive and ordered")
    coarse = next((i for i, view in enumerate(VIEWS) if high >= view / 100), None)
    fine = next((i for i in range(7, -1, -1) if VIEWS[i] >= low), None)
    if coarse is None or fine is None or coarse > fine:
        raise ValueError("size has no representable view interval in the eight-decade ladder")
    return [LEVELS[coarse], LEVELS[fine]]


def size_ranges(rec):
    """Schema-adjacent ordering checks for provided ranges; no invented defaults."""
    if not (rec.get("identity") or {}).get("key", "").startswith("planet-feature."):
        return []
    return [f"size.{name}: minimum exceeds maximum" for name, bounds in (rec.get("size") or {}).items()
            if name in ("across_m", "length_m", "relief_m") and isinstance(bounds, list)
            and len(bounds) == 2 and all(isinstance(x, (int, float)) for x in bounds) and bounds[0] > bounds[1]]


def example_sources(rec):
    if not (rec.get("identity") or {}).get("key", "").startswith("planet-feature."):
        return []
    errors = []
    for i, example in enumerate(rec.get("examples", [])):
        if not example.get("source_dem") and not (rec.get("register") == "cover" and example.get("source_data")):
            errors.append(f"examples[{i}]: source_dem required (cover may use source_data)")
    return errors


def load(tree=TREE):
    result = {}
    for folder in ("features", "lines", "levels"):
        for path in (Path(tree) / "PlanetSim/metadata" / folder).glob("*.yaml"):
            rec = yaml.safe_load(path.read_text())
            result[rec["identity"]["key"]] = rec
    return result


def manifest(path=None):
    path = Path(path) if path else TREE / "sources/planet-feature-emitted-kinds.yaml"
    data = yaml.safe_load(path.read_text())
    if not isinstance(data, dict) or not isinstance(data.get("kinds"), list) or not all(isinstance(k, str) for k in data["kinds"]):
        raise ValueError("emitted-kind manifest needs a kinds array of strings")
    return data["kinds"]


def audit(records, emitted):
    issues = []
    features = {k: r for k, r in records.items() if k.startswith("planet-feature.")}
    graph = {k: set() for k in features}

    def gap(key, check, message):
        issues.append({"key": key, "check": check, "message": message})

    for key, view in zip(LEVELS, VIEWS):
        rec = records.get(key)
        if not rec or rec.get("span") != view or rec.get("spacing") != [view / 1000, view / 1000]:
            gap(key, 1, "level must match its decade view and view/1000 ground pixel")
    for key, rec in sorted(features.items()):
        # 1. Sizes define the draw interval; context `levels` is not a substitute.
        for field in ("size", "ladder", "description", "shape"):
            if not rec.get(field):
                gap(key, 1, f"missing {field}")
        if rec.get("size") and not rec["size"].get("basis"):
            gap(key, 1, "size needs nonempty basis")
        across = (rec.get("size") or {}).get("across_m")
        if across:
            try:
                want = derived_ladder(across)
                if rec.get("ladder") != want:
                    gap(key, 1, f"ladder must be {want}")
            except (ValueError, TypeError):
                gap(key, 1, "size has no valid derived ladder")
        for message in size_ranges(rec):
            gap(key, 1, message)
        if rec["identity"].get("about", "").startswith(("A relief feature of the ", "A relief landmark of the ")):
            gap(key, 1, "identity.about is the old placeholder")

        # 2. Both directions describe the same parent -> child graph. Growth edges
        # participate too; reciprocal declarations do not create a false cycle.
        edges = [(key, child) for child in rec.get("children", [])]
        edges += [(parent, key) for parent in rec.get("parent_kinds", [])]
        edges += [(key, item["kind"]) for item in rec.get("grows_into", [])]
        for parent, child in edges:
            if parent not in features or child not in features:
                gap(key, 2, f"unknown feature edge {parent} -> {child}")
                continue
            graph[parent].add(child)
            pl, cl = features[parent].get("ladder"), features[child].get("ladder")
            if pl and cl and pl[0] in LEVELS and cl[0] in LEVELS and LEVELS.index(cl[0]) < LEVELS.index(pl[0]):
                gap(key, 2, f"child {child} starts coarser than {parent}")

        # 3. A point still needs its declared dimensions/spans, not an empty list.
        if (rec.get("built") or rec.get("register") == "landmark" or rec.get("landmark")) and not rec.get("examples"):
            gap(key, 3, "built/landmark kind needs a measured example")
        if rec.get("built"):
            for field in ("dimensions", "spans"):
                if not rec.get(field):
                    gap(key, 3, f"built kind needs nonempty {field}")

        for message in example_sources(rec):
            gap(key, 3, message)

        # 5. A non-water level blend must explicitly declare its flat floor.
        shape = rec.get("shape") or {}
        if shape.get("blend") == "material" and rec.get("register") != "cover":
            gap(key, 5, "material blend is only for cover")
        if shape.get("blend") == "level" and rec.get("register") != "water" and not shape.get("flat_floor"):
            gap(key, 5, "level blend requires water or an explicit flat_floor")

        # 6. Check all declared geometry/meaning pairs, not only the two mandatory pairs.
        for line in rec.get("line_kinds", []):
            if line not in records or key not in records[line].get("feature_kinds", []):
                gap(key, 6, f"line {line} needs reciprocal feature_kinds")

    active, done = [], set()

    def visit(key):
        if key in active:
            gap(key, 2, "cycle " + " -> ".join(active[active.index(key):] + [key]))
            return
        if key in done:
            return
        active.append(key)
        for child in sorted(graph[key]):
            visit(child)
        active.pop()
        done.add(key)

    for key in sorted(graph):
        visit(key)
    # 4. Simulator manifest and/or observed run kinds must be registered.
    for key in sorted(set(emitted)):
        if key not in features:
            gap(key, 4, "simulator emits an unregistered kind")
    for line, rec in sorted(records.items()):
        if not line.startswith("planet-line."):
            continue
        for key in rec.get("feature_kinds", []):
            if key not in features or line not in features[key].get("line_kinds", []):
                gap(line, 6, f"feature {key} needs reciprocal line_kinds")
    for line, key in (("planet-line.river", "planet-feature.trunk-river"),
                      ("planet-line.divide", "planet-feature.drainage-divide")):
        if line not in records or key not in features or key not in records[line].get("feature_kinds", []) or line not in features[key].get("line_kinds", []):
            gap(key, 6, f"required geometry/meaning pair missing: {line}")
    return issues


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--strict", action="store_true")
    parser.add_argument("--emitted-kinds")
    parser.add_argument("--features-jsonl")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    emitted = manifest(args.emitted_kinds)
    if args.features_jsonl:
        for line in Path(args.features_jsonl).read_text().splitlines():
            if line.strip():
                item = json.loads(line)
                emitted.append(item["kind"])
    records = load()
    issues = audit(records, emitted)
    if args.json:
        print(json.dumps(issues, indent=2))
    else:
        for issue in issues:
            print(f"{issue['key']}: check {issue['check']}: {issue['message']}")
        print(f"planet catalogue: {sum(k.startswith('planet-feature.') for k in records)} kinds; {len(issues)} gap(s); "
              + ("strict" if args.strict or STRICT else "migration report (use --strict to fail on gaps)"))
    raise SystemExit(1 if issues and (args.strict or STRICT) else 0)


if __name__ == "__main__":
    main()
