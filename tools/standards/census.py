"""Census and traffic (docs/registry-people.md): who lives at each settlement by trade, and the ships its supply takes.

    python3 tools/standards/census.py

Census: each settlement's people by profession, summed from its facilities' modules (module.staff) and the needs' service
ratios (need.served_by: one doctor to 290 and so on); the rest are dependants. Traffic: for each settlement supplied from
another (settlement.resupply.from), what its people take a day of stocked goods, the flight between the two bodies at a
torchship's cruise, and the haulers that run needs; the supplier's freight fleet is their sum, the resupply interval the
fleet's cadence. Writes `census` on settlements, `resupply.interval`, and `fleet` on the carrying organisations. Idempotent.
Run from the repository root, then stocking.py and build.py.
"""
import glob, math, os, re
import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
LA = os.path.join(ROOT, "standards", "LocalAdministration", "metadata", "administrations")
SFO = os.path.join(ROOT, "standards", "SFO", "metadata")
CEL = os.path.join(ROOT, "standards", "Celestial", "metadata", "systems")
DAY = 86400.0
CRUISE = 1.0            # m/s2, a hauler's cruise acceleration, chosen: a tenth of a g, fuel in mind (docs/registry-people.md)
TURNAROUND = 1.0 * DAY  # at each end: dock, unload, load
HAULER = "hull.hauler"
FREIGHT_ORG = "org.treistun-freight"

load = lambda p: yaml.safe_load(open(p, encoding="utf-8")) or {}
modules = {d["identity"]["key"]: d for d in map(load, glob.glob(os.path.join(SFO, "modules", "*.yaml")))}
needs = {d["identity"]["key"]: d for d in map(load, glob.glob(os.path.join(ROOT, "standards", "People", "metadata", "needs", "*.yaml")))}
hulls = {d["identity"]["key"]: d for d in map(load, glob.glob(os.path.join(SFO, "hulls", "*.yaml")))}
bodies = {d["identity"]["key"]: d for d in map(load, glob.glob(os.path.join(CEL, "*", "bodies", "*.yaml")))}
STOCKED = ("need.food", "need.medicine", "need.clothes", "need.tools", "need.company")


def write_block(text, name, lines, comment=""):
    """Replace or append a top-level block `name:` in a record's text."""
    text = re.sub(rf"^{name}:\n(?:  .*\n)*", "", text, flags=re.M)
    text = re.sub(rf"^# {name}: .*\n", "", text, flags=re.M)
    block = (f"# {name}: {comment}\n" if comment else "") + f"{name}:\n" + lines
    m = re.search(r"^about: ", text, re.M)
    return (text[:m.start()] + block + text[m.start():]) if m else text.rstrip("\n") + "\n" + block


def star_orbit(key):
    """A body's orbit round its star: a moon's is its planet's. (m)"""
    b = bodies[key]
    while b["identity"].get("kind") in ("moon",) or (b.get("orbit") is None and b["identity"].get("parent")):
        b = bodies[b["identity"]["parent"]]
    return (b.get("orbit") or {}).get("semi_major_axis"), b


def flight(a, b):
    """s one way between bodies a and b: a torchship's brachistochrone at CRUISE over their typical separation."""
    ra, pa = star_orbit(a); rb, pb = star_orbit(b)
    if pa["identity"]["key"] == pb["identity"]["key"]:    # the same planet (a moon and its world): the moon's orbit
        d = (bodies[a].get("orbit") or bodies[b].get("orbit") or {}).get("semi_major_axis", 1e8)
    else:
        d = math.sqrt(ra * ra + rb * rb)                     # the root-mean-square separation over their phases
    return 2.0 * math.sqrt(d / CRUISE)


def main():
    setts = {}
    for f in glob.glob(os.path.join(LA, "*", "*.yaml")):
        d = load(f)
        if d.get("population"):
            setts[d["identity"]["key"]] = (f, d)
    # ---- census
    for key, (f, d) in setts.items():
        pop = d["population"]; count = {}
        folder = f[:-5]
        for ff in glob.glob(os.path.join(folder, "facilities", "*.yaml")):
            fac = load(ff)
            mods = [(m["module"], m.get("count", 1)) for ln in fac.get("lines", []) for m in ln.get("modules", [])] + [(m["module"], m.get("count", 1)) for m in fac.get("modules", [])]
            for mk, n in mods:
                for st in modules.get(mk, {}).get("staff", []):
                    count[st["profession"]] = count.get(st["profession"], 0) + st["count"] * n
        for nd in needs.values():
            for sv in nd.get("served_by", []):
                if sv["profession"] == "profession.pilot":
                    continue                                   # pilots come from the fleets
                count[sv["profession"]] = count.get(sv["profession"], 0) + math.ceil(pop / sv["serves"])
        d["_works"] = sum(count.values()); d["_census"] = count
    # ---- traffic: routes to each supplied settlement
    routes = []
    for key, (f, d) in setts.items():
        rs = d.get("resupply")
        if not rs:
            continue
        frm = setts[rs["from"]][1]
        tonnes = d["population"] * sum(t["rate"] for n in STOCKED for t in needs[n].get("takes", [])) * DAY
        one_way = flight(d["at"], frm["at"])
        round_trip = 2 * one_way + 2 * TURNAROUND
        hold = hulls[HAULER]["identity"].get("hold") or 150000
        ships = max(1, math.ceil(tonnes * (round_trip / DAY) / hold))
        interval = max(DAY, round_trip / ships)
        routes.append({"to": key, "from": rs["from"], "tonnes_day": tonnes, "one_way": one_way, "round_trip": round_trip, "ships": ships, "interval": interval})
    # ---- write: census, intervals
    freight = 0
    for key, (f, d) in setts.items():
        s = open(f, encoding="utf-8").read()
        pop = d["population"]; count = dict(d["_census"])
        r_ = next((r for r in routes if r["to"] == key), None)
        if r_:
            s = re.sub(r"^resupply: \{ from: ([^,]+), interval: [0-9.e+]+ \}", lambda m: f"resupply: {{ from: {m.group(1)}, interval: {r_['interval']:.0f} }}", s, flags=re.M)
            s = re.sub(r"^# Supplied from .*\n", f"# Supplied from {r_['from'].split('.')[-1].replace('-', ' ').title()}: {r_['tonnes_day'] / 1000:.1f} t a day of what its people take; {r_['one_way'] / DAY:.1f} days each way at a torchship's cruise ({CRUISE:g} m/s2) over the two orbits' typical separation; {r_['ships']} hauler(s) on the run, so a delivery every {r_['interval'] / DAY:.1f} days (tools/standards/census.py).\n", s, flags=re.M)
        # pilots: two to a ship of every fleet based here (org.*.fleet), the freight line's counted from the routes
        pilots = sum(2 * r["ships"] for r in routes if r["from"] == key)
        for of in glob.glob(os.path.join(ROOT, "standards", "**", "*.yaml"), recursive=True):
            if os.sep + "makers" + os.sep in of or of.endswith(os.sep + "treistun.yaml"):
                org = load(of)
                if str(org.get("identity", {}).get("key", "")).startswith("org.") and org.get("identity", {}).get("key") != FREIGHT_ORG:
                    pilots += sum(2 * fl["count"] for fl in org.get("fleet", []) if fl.get("home") == key)
        if pilots:
            count["profession.pilot"] = pilots
        works = sum(count.values()); dependants = pop - works
        lines = "".join(f"  - {{ profession: {k}, count: {v} }}\n" for k, v in sorted(count.items(), key=lambda kv: -kv[1]))
        lines += f"  - {{ profession: profession.dependant, count: {max(dependants, 0)} }}\n"
        s = write_block(s, "census", lines, f"its {pop:,} people by trade: the staff of its works (module.staff) and the trades its needs are served by (need.served_by), the rest dependants (children, the old, the unassigned)" + ("; MORE PEOPLE WORK HERE THAN LIVE HERE" if dependants < 0 else ""))
        yaml.safe_load(s); open(f, "w", encoding="utf-8").write(s)
        if dependants < 0:
            print(f"{key}: {works:,} at work for {pop:,} people")
        freight += sum(r["ships"] for r in routes if r["from"] == key)
    # ---- fleets: the freight company's haulers at the granary; the administration's patrol; the band's raiders
    for r in routes:
        print(f"{r['from'].split('.')[-1]:>14} -> {r['to'].split('.')[-1]:<20} {r['tonnes_day'] / 1000:6.1f} t/day  {r['one_way'] / DAY:5.1f} d one way  {r['ships']:3d} hauler(s)  every {r['interval'] / DAY:4.1f} d")
    print(f"freight fleet: {freight} haulers")
    return routes, freight


if __name__ == "__main__":
    main()
