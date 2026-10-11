"""Which package each stock travels in (standards/SFO/metadata/packages): one rule, written into every stock record as
`package:`. Rerun after adding stock; idempotent.

    python3 tools/standards/packages.py            # show the counts and any stock no rule covers
    python3 tools/standards/packages.py --write    # write `package:` into the records

Rules, first match wins: by form for mill stock (plate, sheet, panel, sandwich, honeycomb, tile -> plate stack; bar,
tube -> bar bundle; wire, fibre, film, and rolled strip -> coil; ingot, blank, forging, box -> heavy crate; scrap ->
skip; fluid -> tote, or gas rack for a gas), then for bulk stock by what it is: gases, liquids, molten or hot metal,
live animals, chilled food, fresh produce, bales, made goods by the box, heavy bulk (dense, or ores, minerals, waste),
and the rest (light granular) in bulk bags.
"""
import glob, os, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import edit  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
S = os.path.join(ROOT, "standards/SFO/metadata")
GASES = {"carbon-dioxide", "methane", "natural-gas"}
LIQUIDS = {"beer", "milk", "wine", "whey", "olive-oil", "vegetable-oil", "glycerine", "methanol", "nitric-acid",
           "sulphuric-acid", "crude-oil", "water", "waste-water", "lithium-brine", "sewage-sludge", "caustic-soda", "chemical-waste"}
MOLTEN = ("liquid-", "hot-ingot-", "refined-steel-")
BALES = {"hay", "bagasse", "crop-residue", "flax", "wool", "seed-cotton", "textile-waste", "spent-medium", "growing-medium"}
BOXED = {"market.electronics", "market.computers", "market.medicine", "market.tools", "market.textiles", "market.machinery"}
HEAVY_MARKETS = {"market.ores", "market.minerals"}
FORMS = {"plate": "plate-stack", "sheet": "plate-stack", "panel": "plate-stack", "sandwich": "plate-stack",
         "honeycomb": "plate-stack", "tile": "plate-stack", "bar": "bar-bundle", "tube": "bar-bundle", "wire": "coil",
         "fibre": "coil", "film": "coil", "ingot": "crate-heavy", "blank": "crate-heavy", "forging": "crate-heavy",
         "box": "crate-heavy", "scrap": "skip"}


def goods():
    out = {}
    for f in glob.glob(os.path.join(S, "goods/*.yaml")):
        d = yaml.safe_load(open(f)) or {}
        out[d["identity"]["key"]] = d.get("physical") or {}
    return out


# Where the rules would pick wrong: powders that go in bags whatever their density, things made by the piece, valuables.
OVERRIDES = {"cement": "bulk-bag", "lime": "bulk-bag", "alumina": "bulk-bag", "gypsum": "bulk-bag", "salt": "bulk-bag",
             "silica-sand": "bulk-bag", "dust": "bulk-bag", "soda-ash": "bulk-bag", "potash": "bulk-bag",
             "phosphate-fertiliser": "bulk-bag", "fertiliser": "bulk-bag", "brick": "crate-heavy",
             "carbon-anode": "crate-heavy", "graphite-electrode": "crate-heavy", "diamonds": "carton-pallet",
             "polysilicon": "crate-heavy", "silicon": "crate-heavy", "meteoritic-iron": "crate-heavy", "sponge-iron": "skip",
             "iron-ore-pellets": "skip", "asteroid-water-ice": "cold-crate", "polyethylene": "bulk-bag", "pvc": "bulk-bag"}


def package_for(d, g):
    i = d["identity"]; form = i.get("form"); market = i.get("traded_as", "")
    item = ((d.get("made_from") or [{}])[0]).get("item", "")
    name = item.split(".", 1)[-1]
    p = g.get(item, {})
    if form == "bulk" and name in OVERRIDES:
        return OVERRIDES[name]
    if form in FORMS:
        return FORMS[form]
    if form == "fluid":
        return "gas-rack" if name in GASES else "tote"
    if name in GASES:
        return "gas-rack"
    if name.startswith(MOLTEN):
        return "ladle"
    if name.endswith("-strip-6061"):
        return "coil"
    if name in LIQUIDS:
        return "tote"
    if name.startswith("live-"):
        return "livestock-crate"
    t = p.get("storage_max_temperature")
    if t is not None and t <= 278 and market in ("market.food", "market.biologics"):
        return "cold-crate"
    if t is not None and market == "market.food":
        return "produce-crate"
    if name in BALES:
        return "bale"
    if market in BOXED:
        return "carton-pallet" if (p.get("bulk_density") or 0) < 1200 else "crate-heavy"
    if market in HEAVY_MARKETS or market == "market.fuel" or (p.get("bulk_density") or 0) >= 1400:
        return "skip"
    return "bulk-bag"


def main(argv):
    g = goods(); counts = {}; missing = []
    files = sorted(glob.glob(os.path.join(S, "stock/*.yaml")) + glob.glob(os.path.join(S, "mill-stock/*.yaml")))
    known = {os.path.basename(f)[:-5] for f in glob.glob(os.path.join(S, "packages/*.yaml"))}
    for f in files:
        d = yaml.safe_load(open(f))
        k = package_for(d, g)
        if k not in known:
            missing.append((d["identity"]["key"], k)); continue
        counts[k] = counts.get(k, 0) + 1
        if "--write" in argv:
            def put(s, k=k):
                import re
                if re.search(r"^package: .*$", s, flags=re.M):
                    return re.sub(r"^package: .*$", f"package: package.{k}", s, count=1, flags=re.M)
                return s.replace("\nmade_from:", f"\npackage: package.{k}\nmade_from:", 1)
            edit(f, put)
    for k, n in sorted(counts.items(), key=lambda x: -x[1]):
        print(f"{n:>4}  package.{k}")
    print(f"{sum(counts.values())} stock records, {len(counts)} packages")
    for key, k in missing:
        print(f"  no package {k!r} for {key}")


if __name__ == "__main__":
    main(sys.argv)
