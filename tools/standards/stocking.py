"""Stocking in three layers (docs/registry-stocking.md): working stock, distribution stock (a settlement's resupply cycle) and the
administration's compulsory reserve; derives every warehouse's day-0 stock from them. Idempotent. Run from the repository root."""
import yaml, glob, os, re
LA = "standards/LocalAdministration/metadata/administrations/treistun/"; DAY = 86400.0
# 1. schema: the law on an administration; a settlement's resupply
p = "standards/organisation.schema.yaml"; s = open(p).read()
if "compulsory_stock:" not in s:
    old = "  zoning:\n    description: Its zoning code, what each kind of zone permits.\n"
    assert s.count(old) == 1
    s = s.replace(old, '''  compulsory_stock:
    description: "Its law on reserves: for each need people die without, how long the whole administration's people must be able to live on what is held in store, and where that reserve lies. Like zoning, it can be broken; the record of a breach is the administration's. Beside it, the registry's convention for the other two layers: a works keeps three days of its inputs, a store a week, a farm a season; a settlement's warehouse keeps `warehouse_cover` resupply cycles of what its people take."
    type: object
    additionalProperties: false
    required: [reserves]
    properties:
      warehouse_cover: { type: number, x-unit: "1", description: "how many of its resupply cycles (settlement.resupply.interval) a settlement's warehouse holds of what its people take: one delivery late must not empty it", default: 1.5 }
      reserves:
        type: array
        items:
          type: object
          additionalProperties: false
          required: [need, lasts, held_at]
          properties:
            need: { type: string, x-ref: [need], description: "The need the reserve is for, by its key." }
            lasts: { type: number, x-unit: "s", description: "s the whole administration's people could live on the reserve alone" }
            held_at:
              description: "Where it lies, and each place's share; the shares add to 1."
              type: array
              items:
                type: object
                additionalProperties: false
                required: [settlement, share]
                properties:
                  settlement: { type: string, x-ref: [settlement] }
                  share: { type: number, x-unit: "1" }
            note: { type: string }
''' + old)
    open(p, "w").write(s)
p = "standards/LocalAdministration/schema/settlement.schema.yaml"; s = open(p).read()
if "  resupply:" not in s:
    old = "  population: {"
    s = s.replace(old, '''  resupply:
    description: "How it is kept supplied with what it does not make: where its ships come from and how often. Sets its warehouse's distribution stock (the administration's compulsory_stock.warehouse_cover)."
    type: object
    additionalProperties: false
    required: [from, interval]
    properties:
      from: { type: string, x-ref: [settlement], description: "The settlement its supplies are hauled from, by its key." }
      interval: { type: number, x-unit: "s", description: "s between deliveries, on the usual run" }
''' + old, 1)
    open(p, "w").write(s)
# 2. the law, on Treistun
p = LA[:-1] + ".yaml"; s = open(p).read()
if "compulsory_stock:" not in s:
    s = s.replace("zoning:\n", '''# The reserve law (2026-10-06): the needs people die without, as a cautious state keeps them. Four months of food and three of
# medicine are Switzerland's compulsory stocks (from memory; review); held at the two big ports under open sky, half each.
compulsory_stock:
  warehouse_cover: 1.5
  reserves:
    - { need: need.food, lasts: 10368000, held_at: [{ settlement: settlement.treistun.port-eikir, share: 0.5 }, { settlement: settlement.treistun.port-nacaubun, share: 0.5 }], note: "120 days of food for everyone in Treistun." }
    - { need: need.medicine, lasts: 7776000, held_at: [{ settlement: settlement.treistun.port-eikir, share: 0.5 }, { settlement: settlement.treistun.port-nacaubun, share: 0.5 }], note: "90 days of medicine for everyone in Treistun." }
zoning:
''', 1)
    yaml.safe_load(s); open(p, "w").write(s)
# 3. each settlement's resupply: from Port Eikir (the granary) or Port Trethi (the mill), at an interval by how far it is (chosen)
RESUPPLY = {"port-eikir": None, "port-sirnendis": ("port-eikir", 3), "port-nacaubun": ("port-eikir", 7), "treistun-e-station": ("port-eikir", 7),
            "port-zaudalein": ("port-eikir", 20), "port-lisaur": ("port-eikir", 20), "port-fabindum": ("port-eikir", 20), "port-seiwiti": ("port-eikir", 20),
            "port-trethi": ("port-eikir", 30), "port-aipika": ("port-eikir", 60), "port-weisonum": ("port-eikir", 60)}
for slug, rs in RESUPPLY.items():
    p = LA + slug + ".yaml"; s = open(p).read()
    if "resupply:" in s or rs is None: continue
    frm, days = rs
    s = re.sub(r"^(population: .*\n)", lambda m: m.group(1) + f"# Supplied from {frm.replace('-', ' ').title()} every {days} days: chosen by how far it lies (the inner worlds three weeks, Rime a month, the giants' moons two); a haul reckoned from the orbits is owed.\nresupply: {{ from: settlement.treistun.{frm}, interval: {days * DAY:g} }}\n", s, count=1, flags=re.M)
    assert "resupply:" in s, slug; yaml.safe_load(s); open(p, "w").write(s)
# 4. day-0 warehouse stock from the layers
needs = {yaml.safe_load(open(f))["identity"]["key"]: yaml.safe_load(open(f)) for f in glob.glob("standards/People/metadata/needs/*.yaml")}
BASKET = {"stock.flour-bulk": .25, "stock.potatoes-bulk": .20, "stock.cabbage-bulk": .05, "stock.carrots-bulk": .05, "stock.onions-bulk": .05, "stock.apples-bulk": .10, "stock.cheese-bulk": .05, "stock.beef-bulk": .05, "stock.pork-bulk": .05, "stock.chicken-bulk": .05, "stock.sugar-bulk": .05, "stock.vegetable-oil-bulk": .05}
ITEMS = {"need.food": BASKET, "need.medicine": {"stock.medicines-bulk": 1}, "need.clothes": {"stock.clothes-bulk": 1}, "need.tools": {"stock.tools-bulk": 1}, "need.company": None}
def takes(need):  # item -> kg/s a person
    return {t["item"]: t["rate"] for t in needs[need].get("takes", [])}
adm = yaml.safe_load(open(LA[:-1] + ".yaml")); law = adm["compulsory_stock"]; cover = law.get("warehouse_cover", 1.5)
setts = {}
for f in glob.glob(LA + "*.yaml"):
    d = yaml.safe_load(open(f))
    if d.get("population"): setts[d["identity"]["key"]] = d
total = sum(d["population"] for d in setts.values())
WORKING = 7 * DAY   # the granary's own warehouse: a store's week
for key, d in setts.items():
    slug = key.split(".")[-1]; wh = glob.glob(LA + f"{slug}/facilities/*warehouse*.yaml")
    if not wh: print("no warehouse:", slug); continue
    pop = d["population"]; interval = (d.get("resupply") or {}).get("interval", WORKING)
    stock = {}
    for need, items in ITEMS.items():
        per = takes(need)
        if items is None:   # company: beer and wine by their own items
            for it, rate in per.items(): stock[f"stock.{it.split('.')[1]}-bulk"] = pop * rate * interval * cover
            continue
        rate = sum(per.values())
        for it, share in items.items(): stock[it] = pop * rate * interval * cover * share
    for rsv in law["reserves"]:
        for h in rsv["held_at"]:
            if h["settlement"] != key: continue
            rate = sum(takes(rsv["need"]).values())
            for it, share in ITEMS[rsv["need"]].items(): stock[it] = stock.get(it, 0) + total * rate * rsv["lasts"] * h["share"] * share
    path = wh[0]; s = open(path).read()
    # strip the old food/medicine/clothes/tools/drink lines and their comments; keep fuel, oxygen, water and the rest
    s = re.sub(r"^# Thirty days of food.*\n", "", s, flags=re.M)
    s = re.sub(r"^# Ninety days of medicine.*\n", "", s, flags=re.M)
    s = re.sub(r"^# (Thirty|Ninety) days.*\n", "", s, flags=re.M)
    s = "".join(l for l in s.splitlines(True) if not any(it in l for it in stock))
    held = [r_["need"].split(".")[1] for r_ in law["reserves"] if any(h["settlement"] == key for h in r_["held_at"])]
    comment = (f"# Distribution stock: {cover:g} resupply cycles ({interval / DAY:g} days) of what its {pop:,} people take: food in a plain basket (flour .25, potatoes .20, apples .10, the rest .05 each), medicine, clothes, tools, beer and wine."
               + (f" Plus the administration's compulsory reserve of {' and '.join(held)} for all {total:,} people of Treistun, its share (org.treistun compulsory_stock)." if held else "") + "\n")
    lines = "".join(f"  - {{ item: {k}, quantity: {float(f'{v:.3g}')} }}\n" for k, v in stock.items() if v > 0)
    if "\nstock:\n" in s: s = s.rstrip("\n") + "\n" + comment + lines
    else: s = s.rstrip("\n") + "\nstock:\n" + comment + lines
    yaml.safe_load(s); open(path, "w").write(s)
    print(slug, pop, f"{interval / DAY:g} d", {k.split('.')[1]: round(v / 1000) for k, v in stock.items() if k in ("stock.flour-bulk", "stock.medicines-bulk", "stock.clothes-bulk")})
