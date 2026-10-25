"""The works that make the kit (the owner, 2026-10-18: "Wires manufactured in world?"; factories make factories). Every
KIT- stock item (a drum of cable, a length of pipe, a flange pair, a valve, a panel) is made by one of seven works
modules, one recipe a stock item. What goes in is what the supplied record is built of, as fractions of a kg of it: each
material taken as the mill stock or good the other works use for it, a little lost as scrap where metal is cut.

    python3 tools/standards/kit_works.py           # write the works (idempotent)
    python3 tools/standards/kit_works.py --check   # list any KIT- stock no module makes (reg.py check runs it)

Sizes, rates, power and staff are chosen (invented); the inputs follow the records' parts.
"""
import glob, os, sys
import yaml
import lib  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
M = os.path.join(ROOT, "standards/SFO/metadata")
KEEP = ("visual", "model_requests")

FEED = {"material.structural-steel-a36": "stock.sta36-pl-8", "material.stainless-steel-304": "stock.st304-pl-3",
        "material.aluminium-alloy-6061": "stock.al6061-pl-5", "material.high-strength-steel-4340": "stock.st4340-bar-120",
        "material.polyethylene-uhmw": "good.polyethylene", "material.multi-layer-insulation": "stock.mli-fl",
        "material.fused-silica": "stock.fs-pl-6", "material.soda-lime-glass": "good.glass",
        "good.concrete": "good.concrete", "good.water": "good.water", "good.refractory-brick": "good.refractory-brick"}
FEED.update({"material." + slug: "stock.chem-" + slug for slug in
             ("ptfe", "epdm-rubber", "silicone-rubber", "pir-foam", "polyimide-film", "silica-aerogel")})
COPPER = "material.copper"
SCRAP = {"stock.sta36-pl-8": "stock.sta36-scrap", "stock.st304-pl-3": "stock.st304-scrap", "stock.al6061-pl-5": "stock.al6061-scrap",
         "stock.st4340-bar-120": "stock.st4340-scrap", "stock.cu-ingot": "stock.cu-scrap", "stock.cu-wire-2": "stock.cu-scrap"}
CUT = 0.03

STAFF = lambda *p: [{"profession": f"profession.{n}", "count": c} for n, c in p]
WORKS = {
    "cable-works": ("Cable works", "Make cable and wire", "Strands drawn copper wire into cores, extrudes their insulation and sheaths, lays the cores up and winds the cable onto drums: power cables, flexible cables, installation cables and hook-up wire.",
                    (240, 40, 15), 1.5, 4.5e6, STAFF(("plant-operator", 6), ("machinist", 2), ("technician", 2)),
                    "Copper wire is stranded into cores, insulated and sheathed by extrusion, laid up and wound onto drums."),
    "pipe-shop": ("Pipe and fittings shop", "Make pipe and fittings", "Rolls and welds plate into pipe and ducts, lines and lags them, and machines flanges, chutes, hoods, couplings and bellows joints.",
                  (300, 60, 18), 3.0, 6.0e6, STAFF(("welder", 6), ("metalworker", 4), ("machinist", 2), ("technician", 1)),
                  "Plate is rolled and welded, or cut and machined; linings and lagging go on; each piece is pressure tested."),
    "hose-works": ("Hose works", "Make hose", "Extrudes rubber tube, braids steel wire or corrugates stainless tube over it, covers it and cuts it to lengths.",
                   (150, 30, 12), 0.5, 2.0e6, STAFF(("plant-operator", 4), ("technician", 1)),
                   "A tube is extruded or corrugated, braided, covered, cured and cut to lengths."),
    "valve-shop": ("Valve and coupling shop", "Make valves and couplings", "Casts and machines bodies and trim, assembles and tests valves, quick couplings, unions and container locks.",
                   (150, 50, 12), 0.5, 2.5e6, STAFF(("machinist", 4), ("fitter", 2), ("metalworker", 2), ("technician", 1)),
                   "Bodies and trim are machined from stock, fitted with their seats and seals, assembled and tested."),
    "electrical-fittings-works": ("Electrical fittings works", "Make electrical fittings", "Presses and machines copper, moulds insulators and housings, and assembles terminations, plugs and sockets, connectors, wiring boxes, collectors, conductor rails, bus joints and cable reels.",
                                  (150, 40, 12), 0.3, 1.5e6, STAFF(("electrician", 2), ("machinist", 2), ("plant-operator", 2), ("technician", 1)),
                                  "Copper is machined into contacts and bars, housings and insulators are moulded, and the whole assembled and tested."),
    "building-panel-works": ("Building panel works", "Make building panels", "Forms skins, foams or welds cores and stiffeners, glazes, hangs doors and frames each building panel and stair for the kit's 3 m grid.",
                             (250, 50, 15), 2.0, 3.0e6, STAFF(("metalworker", 4), ("welder", 2), ("plant-operator", 2), ("technician", 1)),
                             "Skins are formed and joined to their cores or stiffeners, openings cut and fitted, the panel framed and sealed."),
    "cradle-shop": ("Cradle shop", "Make equipment cradles", "Cuts kit tube and plate, welds them into the cradles that hold a ship's equipment on its frame, and sets their threaded inserts and clamp lugs.",
                    (120, 40, 12), 0.5, 1.5e6, STAFF(("welder", 3), ("machinist", 1), ("metalworker", 2), ("technician", 1)),
                    "Tube and plate are cut, jigged and welded; inserts are set and lugs bored; each cradle is proof loaded."),
    "conveyor-shop": ("Conveyor shop", "Make conveyors", "Builds belt conveyor sections and their idlers, drag conveyor casings and chains, and the transfer chutes between them.",
                      (200, 40, 15), 1.5, 3.0e6, STAFF(("metalworker", 4), ("fitter", 2), ("technician", 1)),
                      "Stringers, casings and chutes are cut and welded, idlers and chains assembled, belting cut and spliced."),
}
RUN_WORKS = {"cable": "cable-works", "hose": "hose-works", "pipe": "pipe-shop", "lined-duct": "pipe-shop", "jacketed-duct": "pipe-shop",
             "belt": "conveyor-shop", "drag-conveyor": "conveyor-shop", "busbar": "electrical-fittings-works",
             "conductor-rail": "electrical-fittings-works", "festoon": "electrical-fittings-works"}
CONN_WORKS = {"flange": "pipe-shop", "lined-flange": "pipe-shop", "chute": "pipe-shop", "hood": "pipe-shop", "clamp-coupling": "pipe-shop",
              "expansion-joint": "pipe-shop", "valve": "valve-shop", "quick-coupling": "valve-shop", "union": "valve-shop", "twistlock": "valve-shop", "strut-clamp": "valve-shop", "run-clip": "valve-shop", "support-shoe": "cradle-shop",
              "termination": "electrical-fittings-works", "bus-joint": "electrical-fittings-works", "plug-socket": "electrical-fittings-works",
              "collector": "electrical-fittings-works", "cable-reel": "electrical-fittings-works", "belt-transfer": "conveyor-shop"}


def load_all():
    recs = {}
    for f in glob.glob(os.path.join(M, "*", "*.yaml")):
        try:
            d = yaml.safe_load(open(f, encoding="utf-8")) or {}
        except Exception:
            continue
        k = (d.get("identity") or {}).get("key") if isinstance(d, dict) else None
        if k:
            recs[k] = d
    return recs


def kit_stock(recs):
    return sorted(k for k in recs if k.startswith("stock.kit-"))


def works_for(t):
    folder = t["identity"]["key"].split(".")[0]
    if folder == "panel":
        return "building-panel-works"
    if folder == "adapter":
        return "cradle-shop"
    table = RUN_WORKS if folder == "run" else CONN_WORKS
    if t.get("kind") not in table:
        raise SystemExit(f"kit_works: no works for {t['identity']['key']} (kind {t.get('kind')})")
    return table[t["kind"]]


def recipe(key, s, t, works, unmade):
    parts = t.get("built_of") or (t.get("per_m") or {}).get("built_of") or []
    total = sum(p["mass"] for p in parts)
    by = {}
    for p in parts:
        m = p["item"]
        if m == COPPER:
            feed = "stock.cu-wire-2" if works == "cable-works" else "stock.cu-ingot"
        else:
            feed = FEED.get(m, m)
        by[feed] = by.get(feed, 0.0) + p["mass"] / total
    inputs, outputs = [], []
    for feed, f in sorted(by.items(), key=lambda x: -x[1]):
        q = f * (1 + CUT) if feed in SCRAP else f
        inputs.append({"item": feed, "quantity": round(q, 4)})
        if feed in SCRAP and round(f * CUT, 4) > 0:
            outputs.append({"item": SCRAP[feed], "quantity": round(f * CUT, 4)})
    inputs = [x for x in inputs if x["quantity"] > 0] or inputs[:1]
    merged = {}
    for o in outputs:
        merged[o["item"]] = round(merged.get(o["item"], 0.0) + o["quantity"], 4)
    name, step, desc, size, rate, power, staff, does = WORKS[works]
    r = {"makes": key, "does": does, "inputs": inputs}
    if merged:
        r["outputs"] = [{"item": k, "quantity": v} for k, v in merged.items()]
    r.update({"rate": rate, "power": power})
    return r


def build(recs):
    unmade, recipes = set(), {w: [] for w in WORKS}
    for key in kit_stock(recs):
        s = recs[key]
        t = recs[s["supplies"]["item"]]
        w = works_for(t)
        recipes[w].append(recipe(key, s, t, w, unmade))
    out = {}
    for w, (name, step, desc, size, rate, power, staff, does) in WORKS.items():
        out[w] = {"identity": {"key": f"module.{w}", "name": name, "step": step, "description": desc},
                  "physical": {"length": size[0], "width": size[1], "height": size[2]},
                  "staff": staff, "recipes": recipes[w], "life": 788940000,
                  "basis": [{"of": ["recipes.inputs", "recipes.outputs"], "tier": "derived", "review": True, "note": f"Each recipe's inputs are the parts of the record its stock supplies, as kg a kg; each material as the mill stock or good other works make of it; {CUT * 100:g}% of cut metal lost as scrap (invented). A material no works makes yet is taken as the material itself."},
                            {"of": ["physical", "recipes.rate", "recipes.power"], "tier": "invented", "review": True, "note": "Its size, rate and power chosen for the kind of plant; to be reviewed."},
                            {"of": ["staff"], "tier": "invented", "review": True, "note": "Coarse: by the kind of plant, not this one; to be reviewed."},
                            {"of": ["life"], "rule": "rule.life-plant"}]}
        components = [yaml.safe_load(open(f, encoding="utf-8")) for f in sorted(glob.glob(os.path.join(M, "parts", w, "*.yaml")))]
        if components and all((p.get("physical") or {}).get("mass") is not None for p in components):
            out[w]["physical"]["mass"] = sum(p["physical"]["mass"] * (p.get("fit") or {}).get("count", 1) for p in components)
            out[w]["basis"].append({"of": ["physical.mass"], "tier": "derived", "review": True, "note": "Sum of its construction parts, at their recorded counts. The coarse construction budgets remain invented and under review."})
    return out, unmade


def write(slug, d):
    path = os.path.join(M, "modules", slug + ".yaml")
    if os.path.exists(path):
        old = yaml.safe_load(open(path, encoding="utf-8")) or {}
        d = dict(d, **{k: old[k] for k in KEEP if k in old and k not in d})
    d = lib.ruled(d)
    text = "# yaml-language-server: $schema=../../schema/module.schema.yaml\n" + yaml.safe_dump(d, sort_keys=False, allow_unicode=True, width=230)
    lib.own(path, "tools/standards/kit_works.py")
    if not os.path.exists(path) or open(path, encoding="utf-8").read() != text:
        open(path, "w", encoding="utf-8").write(text)
        return 1
    return 0


def check(recs):
    made = {r["makes"] for k, d in recs.items() if k.startswith("module.") for r in d.get("recipes") or []}
    missing = [k for k in kit_stock(recs) if k not in made]
    for k in missing:
        print(f"problem: {k} is made by no module (tools/standards/kit_works.py)")
    print(f"{len(kit_stock(recs)) - len(missing)} of {len(kit_stock(recs))} kit stock made; {len(missing)} fault(s)")
    return 1 if missing else 0


def main(argv):
    recs = load_all()
    if "--check" in argv:
        sys.exit(check(recs))
    out, _ = build(recs)
    n = sum(write(w, d) for w, d in out.items())
    print(f"{len(out)} works, {sum(len(d['recipes']) for d in out.values())} recipes; {n} written")
    made = {r["makes"] for k, d in recs.items() if k.startswith("module.") for r in d.get("recipes") or []}
    unmade = {i["item"] for d in out.values() for r in d["recipes"] for i in r["inputs"]} - made
    if unmade:
        print("inputs no works makes yet: " + ", ".join(sorted(unmade)))


if __name__ == "__main__":
    main(sys.argv[1:])
