"""Manufactured packaging, kit feedstocks, and the works that make the kit.

The owner's stock-chain review, 2026-10-20. All new plant sizes, construction
budgets, rates and power are drafts, explicitly marked invented/review. Chemical
balances are ideal net balances across integrated works, not qualified yields.
Run this, then kit_works.py. --check checks only this generator's owned files.
"""
from pathlib import Path
import sys
import yaml
import lib  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
META = ROOT / "standards/SFO/metadata"
SOURCE = "standards/sources/production-chains.yaml"
OUT = {}


def put(folder, slug, kind, d):
    path = META / folder / (slug + ".yaml")
    rel = "../" * (len(Path(folder).parts) + 1)
    lib.own(path, "tools/standards/supply_chains.py")
    OUT[path] = f"# yaml-language-server: $schema={rel}schema/{kind}.schema.yaml\n" + yaml.safe_dump(d, sort_keys=False, width=150)


def basis(of, note, tier="invented"):
    return {"of": of, "tier": tier, "review": True, "source": SOURCE, "note": note}


def amounts(d):
    return [{"item": k, "quantity": round(v, 8)} for k, v in d.items() if v > 0]


def recipe(key, does, ins, outs=None, rate=0.3, power=3000000):
    d = {"makes": key, "does": does, "inputs": amounts(ins), "rate": rate, "power": power}
    if outs:
        d["outputs"] = amounts(outs)
    return d


def module(slug, name, description, size, recipes):
    return {"identity": {"key": "module." + slug, "name": name, "step": description.split(".")[0], "description": description},
            "physical": dict(zip(("length", "width", "height"), size)),
            "staff": [{"profession": "profession.plant-operator", "count": 4}, {"profession": "profession.technician", "count": 1}],
            "recipes": recipes, "life": 788940000,
            "revision": {"status": "draft", "owner": "registry", "on_conflict": "propose", "because": "First production-chain budget. Chemistry sublines, plant construction and operating figures remain to qualify; no model is commissioned."},
            "basis": [basis(["recipes"], "Draft integrated recipes. Routes from the listed sources; ideal net balances or chosen recoveries, not measured industrial yields. Catalysts and circulating solvents are internal inventory; their makeup and detailed process equipment remain to qualify."),
                      basis(["physical", "recipes.rate", "recipes.power", "staff"], "Chosen plant allocation, throughput, electric draw and watch. Construction is a coarse budget, not an engineered plant or an authorization to model one."),
                      {"of": ["life"], "rule": "rule.life-plant"}]}


def stock(slug, name, item, package, mass=None, supplies=None, components=None, physical=None):
    d = {"identity": {"key": "stock." + slug, "code": slug.upper(), "name": name, "form": "piece" if supplies else "bulk", "traded_as": "market.machinery" if supplies else "market.chemicals"},
         "made_from": [{"item": item}]}
    if package:
        d["package"] = "package." + package
    if mass is not None:
        d["physical"] = dict(physical or {}, mass=mass)
    if components:
        d["made_from"] = [{"item": k, "quantity": round(mass*v, 8)} for k, v in components.items()]
    if supplies:
        d["supplies"] = {"item": supplies}
    d["basis"] = [basis(["identity", "made_from", "physical"] if mass is not None else ["identity", "made_from"], "A stocked feedstock, or one empty handling unit. Empty-unit mass is the existing package's tare; its contents are separate.", "derived")]
    put("stock", slug.upper(), "mill-stock", d)


def good(slug, name, kind, density, description, source=None):
    d = {"identity": {"key": "good." + slug, "name": name, "kind": kind, "description": description},
         "physical": {"bulk_density": density}, "basis": [basis(["physical.bulk_density"], "Chosen handling density; to be reviewed.")]}
    if source:
        d["source"] = source
    put("goods", slug, "good", d)


def build():
    recs = {}
    for p in META.glob("*/*.yaml"):
        d = yaml.safe_load(p.read_text())
        if isinstance(d, dict) and (key := (d.get("identity") or {}).get("key")):
            recs[key] = d
    atoms = {k.split(".")[1]: d["identity"]["atomic_mass"] for k, d in recs.items() if k.startswith("element.")}
    mw = lambda **f: sum(atoms[k] * n for k, n in f.items())
    methanol, water, hydrogen = mw(c=1, h=4, o=1), mw(h=2, o=1), mw(h=2)

    # Ideal overall balance from the registry's established methanol carbon chain.
    # Internal monomer synthesis is deliberately stated, not hidden as a claim
    # that methanol is directly polymerised into every one of these materials.
    def organic(c, h, n=0, o=0, share=1):
        mass = mw(c=c, h=h, n=n, o=o)
        ins = {"good.methanol": c * methanol / mass * share}
        if n:
            ins["element.n"] = n * atoms["n"] / mass * share
        outs = {"good.water": (c - o) * water / mass * share,
                "element.h": (2 * c + 2 * o - h) / 2 * hydrogen / mass * share}
        return ins, outs

    def combine(*ds):
        out = {}
        for d in ds:
            for k, v in d.items():
                out[k] = out.get(k, 0) + v
        return out

    chem = []
    # HF from fluorspar; calcium sulphate is explicitly hydrated to the existing
    # gypsum good, so its two waters are not mistaken for anhydrous CaSO4.
    hf = mw(h=1, f=1)
    chem.append(recipe("good.hydrogen-fluoride", "Digest fluorspar with sulphuric acid, distil HF, and hydrate the calcium sulphate to gypsum. Ideal pure-feed balance; ore beneficiation is internal and not yet sized.",
                        {"good.fluorspar": mw(ca=1, f=2)/(2*hf), "good.sulphuric-acid": mw(h=2,s=1,o=4)/(2*hf), "good.water": 2*water/(2*hf)},
                        {"good.gypsum": mw(ca=1,s=1,o=6,h=4)/(2*hf)}))
    cf2 = mw(c=1,f=2)
    chem.append(recipe("stock.chem-ptfe", "Chlorinate methane to chloroform; exchange chlorine for fluorine with HF; crack R22 to TFE and polymerise it. Intermediates stay inside this integrated works. Ideal net balance.",
                        {"good.methane": mw(c=1,h=4)/cf2, "good.hydrogen-fluoride": 2*hf/cf2, "element.cl": 6*atoms["cl"]/cf2},
                        {"good.hydrogen-chloride": 6*mw(h=1,cl=1)/cf2}))
    chem.append(recipe("element.cl", "Electrolyse recovered hydrogen chloride; return chlorine to chlorination and collect hydrogen.",
                        {"good.hydrogen-chloride": mw(h=1,cl=1)/atoms["cl"]}, {"element.h": atoms["h"]/atoms["cl"]}))
    epdm_i, epdm_o = [], []
    for f, comp in ((0.48, (2,4)), (0.28, (3,6)), (0.04, (9,12))):
        i, o = organic(*comp, share=f); epdm_i.append(i); epdm_o.append(o)
    chem.append(recipe("stock.chem-epdm-rubber", "Draft integrated olefin and ENB synthesis from methanol, terpolymerisation and compounding: 80% EPDM (60/35/5 ethylene/propylene/ENB), 19% carbon filler and 1% sulphur. Ideal net balance; formulation and catalyst makeup unqualified.",
                        combine(*epdm_i, {"good.carbon": .19, "element.s": .01}), combine(*epdm_o)))
    pdms = mw(c=2,h=6,o=1,si=1)
    chem.append(recipe("stock.chem-silicone-rubber", "Make methylchlorosilanes from silicon and methanol via recycled HCl, hydrolyse to siloxane, polymerise and cure with silica filler. Draft 80% PDMS / 20% silica; ideal net balance, cure package unqualified.",
                        {"good.silicon": .8*atoms["si"]/pdms, "good.methanol": .8*2*methanol/pdms, "good.silica-sand": .2},
                        {"good.water": .8*water/pdms}))
    i, o = organic(15, 10, 2, 2, share=.9)
    chem.append(recipe("stock.chem-pir-foam", "Draft integrated methanol-to-aromatics and isocyanate synthesis, then foam and cure an MDI-rich system with glycerine polyol. 90% ideal MDI / 10% glycerine mass model; proprietary formulation, catalysts and blowing-agent makeup unqualified.", combine(i, {"good.glycerine": .1}), o))
    i, o = organic(22, 10, 2, 5)
    chem.append(recipe("stock.chem-polyimide-film", "Draft integrated aromatic monomer synthesis from methanol and nitrogen, PMDA/ODA polyamic-acid formation, casting and thermal imidisation. Ideal C22H10N2O5 film net balance; monomer sublines and solvent makeup unqualified.", i, o, rate=.05, power=1500000))
    silica = mw(si=1,o=2)
    chem.append(recipe("stock.chem-silica-aerogel", "Dissolve silica in caustic, acidify sodium silicate with HCl, wash, exchange solvent and dry the gel. Ideal silica/salt/water balance plus chosen 0.05 kg solvent makeup lost to chemical waste per kg; recovery and hydrophobisation unqualified.",
                        {"good.silica-sand": 1, "good.caustic-soda": 2*mw(na=1,o=1,h=1)/silica, "good.hydrogen-chloride": 2*mw(h=1,cl=1)/silica, "good.methanol": .05},
                        {"good.salt": 2*mw(na=1,cl=1)/silica, "good.water": 2*water/silica, "good.chemical-waste": .05}))
    # Use the magnesite record's actual Mg grade, not pure MgCO3 ore.
    mg_share = recs["good.magnesite-ore"]["composition"][0]["share"]
    ore = .9*atoms["mg"]/mw(mg=1,o=1)/mg_share
    carbonate = .9*mw(mg=1,c=1,o=3)/mw(mg=1,o=1)
    chem.append(recipe("good.refractory-brick", "Beneficiate and calcine magnesite, press dead-burned magnesia with carbon and cure its bond. Draft 90% MgO / 10% carbon budget; binder chemistry and firing losses unqualified.",
                        {"good.magnesite-ore": ore, "good.carbon": .1}, {"good.carbon-dioxide": carbonate-.9, "good.tailings": ore-carbonate}))
    chem.append(recipe("good.paper", "Pulp bagasse, wash and press sheet, and dry it. Draft recovery and process-water budget; spent liquor goes to chemical waste.",
                        {"good.bagasse": 2, "good.water": 4, "good.caustic-soda": .05}, {"good.chemical-waste": 5.05}))
    chem.append(recipe("good.fibreboard", "Dry and press crop fibre with polyethylene binder into pallet board and spacers. Draft moisture and binder budget.",
                        {"good.crop-residue": 1.2, "good.polyethylene": .1}, {"good.waste-water": .3}))
    specialty = module("specialty-chemical-works", "Specialty chemical works", "Make insulation polymers, refractory and packaging feedstocks. Integrated reactors, separation, polymerisation, drying and recovery sections; all are draft net budgets.", (180,80,25), chem)
    put("modules", "specialty-chemical-works", "module", specialty)
    for slug, name, density, desc, kind in (
        ("hydrogen-fluoride", "Hydrogen fluoride", 1000, "Anhydrous HF, the fluorine feed for fluoropolymers.", "intermediate"),
        ("hydrogen-chloride", "Hydrogen chloride", 1000, "Dry HCl returned from fluorination or taken for silicate gel production; recovered by electrolysis. Stocked under pressure; its handling density is a draft liquefied-gas budget.", "intermediate"),
        ("paper", "Packaging paper", 500, "Dry bagasse pulp sheet for cartons.", "intermediate"),
        ("fibreboard", "Pallet fibreboard", 700, "Pressed crop fibre with polyethylene binder for pallets and spacers.", "intermediate")):
        good(slug, name, kind, density, desc)
        stock(slug+"-bulk", name+", in bulk", "good."+slug, "gas-rack" if slug == "hydrogen-chloride" else "tote" if slug == "hydrogen-fluoride" else "plate-stack")
    good("fluorspar", "Fluorspar", "rock", 1800, "Calcium fluoride feed; acid-grade concentrate is the idealisation used by this draft chain.", {"occurs": "rocky planets and moons", "note": "A resource class, not a claimed deposit or a chosen supplier. Beneficiation and actual deposit grades remain to qualify."})
    stock("fluorspar-bulk", "Fluorspar, in bulk", "good.fluorspar", "bulk-bag")
    for slug, name, pkg in (("ptfe","PTFE resin","bulk-bag"), ("epdm-rubber","EPDM compound","crate-heavy"), ("silicone-rubber","Silicone rubber compound","crate-heavy"), ("pir-foam","PIR foam","plate-stack"), ("polyimide-film","Polyimide film","coil"), ("silica-aerogel","Silica aerogel","bulk-bag")):
        stock("chem-"+slug, name, "material."+slug, pkg)
    stock("refractory-brick-bulk", "Refractory brick, in bulk", "good.refractory-brick", "crate-heavy")
    good("packaging-waste", "Spent packaging", "by-product", 300, "Mixed spent bindings, wrapping or discarded containers, awaiting sorting or disposal. No automatic recovery of their original materials is claimed.")
    wp = META / "goods/packaging-waste.yaml"
    wd = yaml.safe_load(OUT[wp]); wd["waste"] = True
    put("goods", "packaging-waste", "good", wd)
    stock("packaging-waste-bulk", "Spent packaging, in bulk", "good.packaging-waste", "skip")

    # Shares of each package's existing empty mass, not its payload.
    steel, ss, al, pe, board, paper = "stock.sta36-pl-8", "stock.st304-pl-3", "stock.al6061-pl-5", "good.polyethylene", "good.fibreboard", "good.paper"
    compositions = {
        "bale": {"good.cotton-cloth": 1}, "bar-bundle": {board:.6, steel:.4},
        "bulk-bag": {pe:.4,board:.6}, "carton-pallet": {paper:.35,board:.6,pe:.05},
        "coil": {steel:.2,board:.8}, "cold-crate": {pe:.3,al:.65,"stock.chem-pir-foam":.05},
        "crate-heavy": {board:.85,steel:.15}, "crate-steel": {steel:1},
        "cryo-dewar": {ss:.95,"stock.mli-fl":.03,"stock.chem-ptfe":.02},
        "fish-tank": {pe:.9,steel:.1}, "gas-rack": {steel:.98,"stock.chem-epdm-rubber":.02},
        "ladle": {steel:.7,"good.refractory-brick":.3}, "livestock-crate": {steel:1},
        "plate-stack": {board:.9,steel:.1}, "produce-crate": {pe:1},
        "skip": {steel:1}, "tote": {pe:.3,steel:.5,board:.2}}
    pack_recipes = []
    for slug, shares in compositions.items():
        package = recs["package."+slug]; mass = package["physical"]["mass"]
        key = "stock.pkg-"+slug
        # Made_from is a material identity for the stock view; the complete
        # manufacturing inputs are the module recipe, never inferred from it.
        stock("pkg-"+slug, package["identity"]["name"]+", empty" if slug != "bale" else "Bale bindings, one set", next(iter(shares)), None, mass, "package."+slug, shares, package["physical"])
        pack_recipes.append(recipe(key, "Form, join and inspect one empty handling unit (bindings only for a bale). Draft material shares of its existing tare; no payload is manufactured here.", shares, rate=.5, power=1000000))
    packaging = module("packaging-works", "Packaging works", "Make empty handling units. Sheet fabrication, tank and crate assembly, plastics moulding and bag/carton conversion; draft material budgets.", (120,60,15), pack_recipes)
    put("modules", "packaging-works", "module", packaging)

    # Coarse construction parts for each works, with the final assembly itself
    # a recipe. No settlement, maker allocation or plant layout is invented.
    import kit_works
    works = {slug: (data[0], data[3]) for slug, data in kit_works.WORKS.items()}
    works.update({"specialty-chemical-works": ("Specialty chemical works", (180,80,25)), "packaging-works": ("Packaging works", (120,60,15)), "factory-works": ("Factory works", (120,60,20))})
    factory_recipes, construction_masses = [], {}
    codes = {"cable-works": 1, "pipe-shop": 2, "hose-works": 3, "valve-shop": 4, "electrical-fittings-works": 5,
             "building-panel-works": 6, "cradle-shop": 7, "conveyor-shop": 8, "specialty-chemical-works": 9,
             "packaging-works": 10, "factory-works": 11}
    for slug, (name, size) in works.items():
        idx = codes[slug]           # stable component codes if the works dictionary is reordered
        total = size[0]*size[1]*40       # chosen machinery kg / allocated m2, not a measured engineering estimate
        construction_masses[slug] = total
        inputs = {}
        for j, (pn, feed, share, maker) in enumerate((("Frames and guards", "stock.sta36-pl-20-panel", .4, "module.welding-bay"), ("Process machinery", "stock.st4340-fg", .45, "module.machining-centre"), ("Drives", "good.electric-motors", .1, "module.assembly-shop"), ("Controls", "good.electronics", .05, "module.assembly-shop")), 1):
            code = f"WK{idx:02d}-{j:02d}"
            mass = total*share
            part = {"identity": {"key":"part."+code.lower(), "code":code, "name":name+": "+pn},
                    "physical": {"mass":mass, "length":size[0]*share, "width":size[1]*.5, "height":size[2]*.5},
                    "made_from":[{"item":feed,"quantity":mass}], "making":{"module":maker}, "fit":{"count":1},
                    "basis":[basis(["physical", "made_from", "fit"], "Coarse construction budget: 40 kg of machinery per allocated square metre, split 40% frames, 45% process machinery, 10% drives, 5% controls. Allocated component envelopes are placeholders, not model geometry or a load-qualified design. No cutting loss assumed in this first budget.")]}
            put("parts/"+slug, code, "part", part)
            inputs["part."+code.lower()] = share
        factory_recipes.append(recipe("module."+slug, "Assemble the works' frames, process machinery, drives and controls, then commission it. Draft construction budget; foundations and buildings are separate.", inputs, rate=.25, power=1000000))
    put("modules", "factory-works", "module", module("factory-works", "Factory works", "Build industrial works. Assemble prepared machinery parts, connect drives and controls, test and commission; can build another factory works from its parts.", (120,60,20), factory_recipes))
    for slug in ("specialty-chemical-works", "packaging-works", "factory-works"):
        path = META / "modules" / (slug + ".yaml")
        d = yaml.safe_load(OUT[path])
        d["physical"]["mass"] = construction_masses[slug]
        d["basis"].append(basis(["physical.mass"], "Sum of the four construction parts, one of each. Their budgets remain invented and under review.", "derived"))
        put("modules", slug, "module", d)


def main():
    build()
    changed = [p for p, content in OUT.items() if not p.exists() or p.read_text() != content]
    if "--check" in sys.argv:
        for p in changed:
            print("problem: generated supply chain stale: " + str(p.relative_to(ROOT)))
        print(f"{len(OUT)} supply-chain records; {len(changed)} fault(s)")
        return bool(changed)
    for p in changed:
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(OUT[p])
    print(f"{len(OUT)} supply-chain records; {len(changed)} written")
    return 0


if __name__ == "__main__":
    sys.exit(main())
