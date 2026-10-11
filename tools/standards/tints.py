"""The colour a stock's package contents are tinted (docs/asset-contract.md, Packages): `tint: [r, g, b]`, sRGB 0 to 1,
written into every stock record. Artistic: the look of the stuff in its package, not a measured reflectance (the
records' own `colour` is words). Rerun after adding stock; idempotent.

    python3 tools/standards/tints.py            # counts by where the colour came from
    python3 tools/standards/tints.py --write    # write `tint:` into the records

First match wins: a word in the stock's item or material (KEYWORDS, in order), then its market (MARKETS).
"""
import glob, os, re, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import edit  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
S = os.path.join(ROOT, "standards/SFO/metadata")

KEYWORDS = [   # (a word, or words, in the item or material key, sRGB)
    ("carbon-dioxide", (0.85, 0.87, 0.90)), ("natural-gas", (0.80, 0.84, 0.88)), ("sandwich", (0.70, 0.70, 0.68)), ("honeycomb", (0.80, 0.80, 0.70)),
    ("stainless", (0.72, 0.73, 0.75)), ("st304", (0.72, 0.73, 0.75)), ("304", (0.72, 0.73, 0.75)),
    ("titanium", (0.56, 0.56, 0.59)), ("ti64", (0.56, 0.56, 0.59)), ("ti-6al", (0.56, 0.56, 0.59)),
    ("aluminium", (0.80, 0.81, 0.83)), ("6061", (0.80, 0.81, 0.83)), ("al1350", (0.80, 0.81, 0.83)),
    ("bronze", (0.66, 0.49, 0.26)), ("brz", (0.66, 0.49, 0.26)), ("copper-concentrate", (0.42, 0.40, 0.30)),
    ("copper-ore", (0.38, 0.44, 0.36)), ("copper", (0.73, 0.45, 0.20)), ("cu", (0.73, 0.45, 0.20)),
    ("gold", (0.52, 0.46, 0.36)), ("4340", (0.44, 0.45, 0.47)), ("a36", (0.44, 0.45, 0.47)), ("steel", (0.44, 0.45, 0.47)),
    ("iron-ore", (0.47, 0.26, 0.18)), ("pellets", (0.35, 0.22, 0.18)), ("sponge-iron", (0.35, 0.33, 0.32)),
    ("meteoritic", (0.40, 0.38, 0.36)), ("nickel", (0.45, 0.43, 0.38)), ("chromite", (0.20, 0.20, 0.21)),
    ("bauxite", (0.62, 0.36, 0.24)), ("laterite", (0.60, 0.35, 0.22)), ("red-mud", (0.58, 0.25, 0.18)),
    ("manganese", (0.22, 0.20, 0.20)), ("ilmenite", (0.18, 0.18, 0.19)), ("titanomagnetite", (0.20, 0.20, 0.21)),
    ("uranium", (0.40, 0.42, 0.30)), ("sulphur", (0.86, 0.80, 0.30)), ("phosphate", (0.70, 0.66, 0.58)),
    ("potash", (0.80, 0.62, 0.55)), ("lithium", (0.78, 0.76, 0.72)), ("tungsten", (0.30, 0.30, 0.32)), ("tin", (0.45, 0.44, 0.42)),
    ("zinc", (0.50, 0.50, 0.50)), ("lead", (0.40, 0.41, 0.44)), ("molybdenum", (0.32, 0.32, 0.34)), ("kimberlite", (0.33, 0.38, 0.33)),
    ("diamond", (0.88, 0.90, 0.92)), ("graphite", (0.18, 0.18, 0.19)), ("coal", (0.10, 0.10, 0.10)), ("carbon", (0.12, 0.12, 0.13)),
    ("cfc", (0.12, 0.12, 0.13)), ("sic", (0.24, 0.24, 0.26)), ("silicon", (0.36, 0.38, 0.42)), ("wafer", (0.30, 0.32, 0.40)),
    ("aramid", (0.86, 0.75, 0.25)), ("polyimide", (0.80, 0.55, 0.15)), ("ptfe", (0.93, 0.93, 0.92)), ("polyethylene", (0.93, 0.93, 0.92)),
    ("pvc", (0.90, 0.90, 0.90)), ("silica", (0.88, 0.86, 0.80)), ("glass", (0.80, 0.86, 0.86)), ("salt", (0.94, 0.94, 0.94)),
    ("lime", (0.92, 0.91, 0.88)), ("cement", (0.68, 0.68, 0.66)), ("concrete", (0.66, 0.66, 0.64)), ("gypsum", (0.92, 0.91, 0.88)),
    ("clay", (0.70, 0.52, 0.38)), ("brick", (0.66, 0.32, 0.24)), ("limestone", (0.82, 0.80, 0.74)), ("aggregate", (0.58, 0.57, 0.55)),
    ("sand", (0.82, 0.74, 0.56)), ("slag", (0.30, 0.30, 0.31)), ("tailings", (0.55, 0.52, 0.48)), ("waste-rock", (0.50, 0.48, 0.45)),
    ("ice", (0.86, 0.92, 0.97)), ("water", (0.55, 0.72, 0.86)), ("crude-oil", (0.08, 0.07, 0.06)), ("methanol", (0.90, 0.92, 0.94)),
    ("acid", (0.92, 0.92, 0.85)), ("caustic", (0.92, 0.92, 0.92)), ("gas", (0.80, 0.84, 0.88)), ("methane", (0.80, 0.84, 0.88)),
    ("carbon-dioxide", (0.85, 0.87, 0.90)), ("wheat", (0.85, 0.70, 0.40)), ("barley", (0.86, 0.75, 0.45)), ("oats", (0.84, 0.77, 0.55)),
    ("rice", (0.95, 0.94, 0.90)), ("maize", (0.94, 0.78, 0.25)), ("flour", (0.96, 0.95, 0.92)), ("sugar", (0.97, 0.97, 0.97)),
    ("beans", (0.60, 0.35, 0.25)), ("soybeans", (0.88, 0.80, 0.55)), ("lentils", (0.70, 0.45, 0.30)), ("peas", (0.55, 0.65, 0.30)),
    ("chickpeas", (0.88, 0.76, 0.52)), ("coffee", (0.35, 0.22, 0.14)), ("cocoa", (0.40, 0.25, 0.18)), ("tea", (0.30, 0.32, 0.18)),
    ("apples", (0.75, 0.18, 0.15)), ("oranges", (0.95, 0.55, 0.10)), ("lemons", (0.95, 0.88, 0.25)), ("bananas", (0.95, 0.85, 0.30)),
    ("grapes", (0.40, 0.18, 0.35)), ("strawberries", (0.85, 0.15, 0.18)), ("tomatoes", (0.85, 0.20, 0.12)), ("potatoes", (0.75, 0.62, 0.42)),
    ("carrots", (0.92, 0.50, 0.12)), ("cabbage", (0.55, 0.72, 0.40)), ("lettuce", (0.55, 0.78, 0.35)), ("spinach", (0.25, 0.45, 0.20)),
    ("onions", (0.85, 0.70, 0.50)), ("garlic", (0.92, 0.90, 0.84)), ("peppers", (0.80, 0.18, 0.12)), ("cucumbers", (0.30, 0.50, 0.22)),
    ("pumpkins", (0.92, 0.55, 0.15)), ("watermelons", (0.25, 0.48, 0.22)), ("mangoes", (0.95, 0.65, 0.20)), ("olive", (0.45, 0.48, 0.20)),
    ("milk", (0.97, 0.97, 0.95)), ("whey", (0.93, 0.92, 0.80)), ("butter", (0.98, 0.90, 0.60)), ("cheese", (0.97, 0.84, 0.45)),
    ("eggs", (0.94, 0.88, 0.76)), ("bread", (0.80, 0.58, 0.32)), ("beer", (0.85, 0.60, 0.15)), ("wine", (0.45, 0.08, 0.15)),
    ("oil", (0.88, 0.78, 0.30)), ("beef", (0.62, 0.18, 0.18)), ("pork", (0.88, 0.62, 0.60)), ("mutton", (0.66, 0.24, 0.22)),
    ("chicken", (0.95, 0.80, 0.70)), ("fish", (0.75, 0.78, 0.80)), ("offal", (0.50, 0.15, 0.15)), ("hay", (0.80, 0.72, 0.42)),
    ("straw", (0.86, 0.78, 0.50)), ("bagasse", (0.82, 0.74, 0.55)), ("wool", (0.92, 0.89, 0.82)), ("cotton", (0.95, 0.95, 0.93)),
    ("flax", (0.78, 0.70, 0.52)), ("manure", (0.30, 0.22, 0.14)), ("compost", (0.28, 0.22, 0.16)), ("feed", (0.70, 0.58, 0.35)),
]
MARKETS = {"market.ores": (0.42, 0.38, 0.34), "market.minerals": (0.70, 0.68, 0.64), "market.food": (0.78, 0.66, 0.42),
           "market.biologics": (0.52, 0.44, 0.30), "market.chemicals": (0.86, 0.86, 0.84), "market.water": (0.55, 0.72, 0.86),
           "market.fuel": (0.15, 0.15, 0.15), "market.textiles": (0.86, 0.83, 0.76), "market.electronics": (0.20, 0.32, 0.26),
           "market.computers": (0.22, 0.24, 0.26), "market.machinery": (0.40, 0.44, 0.48), "market.medicine": (0.94, 0.94, 0.94),
           "market.tools": (0.45, 0.46, 0.48), "market.metals": (0.56, 0.57, 0.59)}


def tint_for(d):
    words = " ".join([d["identity"]["key"]] + [x.get("item", "") for x in d.get("made_from") or []])
    for w, c in KEYWORDS:     # (whole words: a key's words are split by dots and hyphens)
        if re.search(r"(^|[.\- ])" + re.escape(w) + r"($|[.\- ])", words):
            return c, "word " + w
    return MARKETS.get(d["identity"].get("traded_as"), (0.6, 0.6, 0.6)), "market"


def main(argv):
    counts = {}
    for f in sorted(glob.glob(os.path.join(S, "stock/*.yaml")) + glob.glob(os.path.join(S, "mill-stock/*.yaml"))):
        d = yaml.safe_load(open(f))
        c, why = tint_for(d)
        counts[why.split(" ")[0]] = counts.get(why.split(" ")[0], 0) + 1
        if "--write" in argv:
            line = f"tint: [{c[0]}, {c[1]}, {c[2]}]"
            def put(s, line=line):
                if re.search(r"^tint: .*$", s, flags=re.M):
                    return re.sub(r"^tint: .*$", line, s, count=1, flags=re.M)
                return s.replace("\nmade_from:", f"\n{line}\nmade_from:", 1)
            edit(f, put)
    print(counts)


if __name__ == "__main__":
    main(sys.argv)
