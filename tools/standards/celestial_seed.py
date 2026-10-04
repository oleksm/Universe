#!/usr/bin/env python3
"""Seeds what the game does not make yet into the celestial registry: comets, centaurs, crossing
asteroids, captured moons, dwarf planets and each belt's largest body (small-bodies/), and the
scattered disc, the far cloud and comets' meteoroid streams (regions/), for each system written out.

Made from the galaxy's seed and the system's own records, by the rules below (each follows what is
known of the Sun's: see the vocabulary). The same seed gives the same bodies. A record that is
there already is never written over: delete one to have it made again.

    python3 tools/standards/celestial_seed.py [--dry]
"""
import math, os, random, re, sys
import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CEL = os.path.join(ROOT, "standards", "Celestial", "metadata")
dry = "--dry" in sys.argv
AU, G, SUN = 1.495978707e8, 6.6743e-11, 1.98847e30          # km, SI, kg
SUN_W = 3.828e26
si = lambda v: float(f"{v:.15g}")       # (records are in SI; this works in km, days and AU, and writes each value in SI)
galaxy = yaml.safe_load(open(os.path.join(CEL, "galaxy.yaml")))
laws = yaml.safe_load(open(os.path.join(CEL, "asteroids.yaml")))
for _b in ("main_belt", "outer_belt"):
    for _e in ("inner_edge", "outer_edge"):
        laws[_b][_e] = float(f"{laws[_b][_e] / (AU * 1000):.12g}")          # (m, to AU)
classes = {f[:-5]: yaml.safe_load(open(os.path.join(CEL, "rock-classes", f))) for f in os.listdir(os.path.join(CEL, "rock-classes"))}
slug = lambda name: re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
r3 = lambda v, n=4: float(f"{v:.{n}g}")

# (Names: the game's own syllables, crates/world/src/names.rs.)
ONSETS = ["", "", "", "b", "br", "c", "d", "dr", "f", "g", "h", "k", "kr", "l", "m", "n", "p", "r", "s", "sh", "st", "t", "th", "tr", "v", "w", "y", "z"]
VOWELS = ["a", "a", "e", "e", "i", "i", "o", "o", "u", "ai", "au", "ei"]
CODAS = ["", "", "", "", "", "l", "m", "n", "n", "r", "r", "s", "th", "k", "nd", "rn"]
ENDINGS = ["", "", "", "", "a", "is", "on", "um", "ia", "ar"]
used = set()


def name(rng):
    while True:
        n = rng.randint(2, 3)
        s = ""
        for k in range(n):
            s += rng.choice(ONSETS) + rng.choice(VOWELS)
            if k + 1 == n or rng.random() < 0.3:
                s += rng.choice(CODAS)
        if s[-1] not in "aeiouy":
            s += rng.choice(ENDINGS)
        v = sum(c in "aeiouy" for c in s)
        triple = any(all(c in "aeiouy" for c in s[i:i + 3]) or all(c not in "aeiouy" for c in s[i:i + 3]) for i in range(len(s) - 2))
        if 4 <= len(s) <= 9 and not triple and v * 3 <= len(s) * 2 and s not in used:
            used.add(s)
            return s.capitalize()


wrote = kept = 0


def put(sysdir, folder, schema, rec, why):
    global wrote, kept
    path = os.path.join(sysdir, folder, slug(rec["identity"]["name"]) + ".yaml")
    if os.path.exists(path):
        kept += 1
        return
    wrote += 1
    if not dry:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        open(path, "w").write(f"# yaml-language-server: $schema=../../../../schema/{schema}.schema.yaml\n# Seeded in the registry (tools/standards/celestial_seed.py), from the galaxy's seed: {why}\n# The game does not make it yet. Change its provenance to curated to take it over.\n"
                              + yaml.safe_dump(rec, sort_keys=False, allow_unicode=True, default_flow_style=None, width=120))


def density(cls):
    return ((classes.get(cls) or {}).get("physical") or {}).get("density_monolith")


def body(kind, nm, parent, a_km, e, incl, radius_km, cls, about, mu):
    rec = {"provenance": "seeded", "in_game": "not made", "identity": {"key": f"body.{SYS}.{slug(nm)}", "name": nm, "kind": kind, "parent": f"body.{SYS}.{slug(parent)}", "about": about},
           "orbit": {"semi_major_axis": si(r3(a_km, 6) * 1000), "eccentricity": r3(e, 8), "inclination": round(incl, 2), "period": si(r3(2 * math.pi * math.sqrt((a_km * 1000) ** 3 / mu) / 86400, 5) * 86400)},
           "physical": {"radius": si(r3(radius_km) * 1000)}, "rock": {"class": "rock-class." + cls}}
    d = laws["sizes"].get("comet_density") if kind == "comet" else density(cls)
    if d:
        rec["physical"]["density"] = d
        rec["physical"]["mass"] = r3(d * 4 / 3 * math.pi * (radius_km * 1000) ** 3)
    return rec


for fn in sorted(os.listdir(os.path.join(CEL, "systems"))):
    if not fn.endswith(".yaml"):
        continue
    sysm = yaml.safe_load(open(os.path.join(CEL, "systems", fn)))
    sysdir = os.path.join(CEL, "systems", fn[:-5])
    SYS = fn[:-5]
    sname = sysm["identity"]["name"]
    bodies = [yaml.safe_load(open(os.path.join(sysdir, "bodies", b))) for b in sorted(os.listdir(os.path.join(sysdir, "bodies")))]
    sun = next(b for b in bodies if b["identity"]["kind"] == "star")
    star = {"mass": sun["physical"]["mass"] / SUN, "luminosity": float(f"{sun['star']['luminosity'] / SUN_W:.12g}")}
    bodies = [b for b in bodies if b["identity"]["kind"] != "star"]
    for b in bodies:
        b["orbit"]["semi_major_axis"] = float(f"{b['orbit']['semi_major_axis'] / 1000:.12g}")       # (m, to km)
    mu = G * star["mass"] * SUN
    used.update(b["identity"]["name"] for b in bodies)
    planets = sorted((b for b in bodies if b["identity"]["parent"] == f"body.{SYS}.{slug(sname)}" and b["identity"]["kind"] != "asteroid"), key=lambda b: b["orbit"]["semi_major_axis"])
    au = lambda b: b["orbit"]["semi_major_axis"] / AU
    giants = [b for b in planets if b["identity"]["kind"] in ("gas giant", "ice giant")]
    rocky = [b for b in planets if b["identity"]["kind"] == "rocky planet"]
    frost, warm = 2.7 * math.sqrt(star["luminosity"]), math.sqrt(star["luminosity"])
    rng = lambda what: random.Random(f"{galaxy['seed']}:{sysm['identity'].get('index')}:{what}")
    res = lambda r_: (lambda p, q: (q / p) ** (2 / 3))(*[float(v) for v in r_.split(":")])
    mb, ob = laws["main_belt"], laws["outer_belt"]
    gas = next((b for b in giants if b["identity"]["kind"] == "gas giant" and au(b) > 0.8 * frost), giants[0] if giants else None)
    belt = (au(gas) * res(mb["inner_resonance"]), au(gas) * res(mb["outer_resonance"])) if gas else (mb["no_giant_inner"] * frost, mb["no_giant_outer"] * frost)
    ring = lambda lo, hi: hi ** 2 - lo ** 2
    share = ring(*belt) / ring(mb["inner_edge"], mb["outer_edge"])
    pick = lambda r, zone: r.choices(list(classes), weights=[(c.get("found") or {}).get(zone, 0) for c in classes.values()])[0]

    # The main belt's largest body: 39% of the belt's mass (the Sun's largest is), the belt's mass by the ground it covers.
    r = rng("largest")
    # (By the belt's own mix: the ground it has in each zone, as the build works it out.)
    zn = laws.get("zones") or {}
    cuts = [belt[0], min(max(zn.get("warm_to", 0.93) * frost, belt[0]), belt[1]), min(max(zn.get("frost_to", 1.04) * frost, belt[0]), belt[1]), belt[1]]
    cls = pick(r, r.choices(["warm", "frost_line", "cold"], weights=[cuts[i + 1] ** 2 - cuts[i] ** 2 for i in range(3)])[0])
    d = density(cls) or 2500
    rad = (3 * 0.39 * mb["mass"] * share / (4 * math.pi * d)) ** (1 / 3) / 1000
    a = r.uniform(belt[0] + 0.25 * (belt[1] - belt[0]), belt[1] - 0.25 * (belt[1] - belt[0]))
    put(sysdir, "small-bodies", "body", body("dwarf planet" if rad >= 400 else "asteroid", name(r), sname, a * AU, r.uniform(0.03, 0.12), r.uniform(1, 11), rad, cls,
        "The largest body of the main belt: over a third of all the belt's mass." + ("" if rad >= 400 else " Too small to have pulled itself round."), mu),
        "a belt's largest body holds 39% of its mass; the belt's mass goes by the ground it covers.")

    # Crossing asteroids: knocked out of the belt onto orbits that come in among the rocky planets.
    r = rng("crossing")
    inner = [p for p in rocky if au(p) < belt[1]]
    for _ in range(5 if inner else 0):
        target = r.choice(inner)
        q, far = au(target) * r.uniform(0.6, 1.25), r.uniform(*belt)
        if far <= q:
            far = q * r.uniform(1.3, 2.2)
        a, e = (q + far) / 2, (far - q) / (far + q)
        put(sysdir, "small-bodies", "body", body("crossing asteroid", name(r), sname, a * AU, e, r.uniform(1, 25), math.exp(r.uniform(math.log(0.1), math.log(1.5))), pick(r, "warm"),
            f"Knocked out of the belt. At its closest it comes in to {q:.2f} AU, near the orbit of {target['identity']['name']}.", mu),
            "about one for every 1,200 of the belt's, on orbits from the belt in to a rocky planet's.")

    # Captured moons: far out round each giant, tilted, stretched, most going round backward.
    for g in giants:
        r = rng("captured:" + g["identity"]["name"])
        reach = g["orbit"]["semi_major_axis"] * (g["physical"]["mass"] / (3 * star["mass"] * SUN)) ** (1 / 3)
        # (Outside its own moons: no closer than half again the farthest of them.)
        least = max([0.05 * reach] + [1.5 * m["orbit"]["semi_major_axis"] for m in bodies if m["identity"]["parent"] == g["identity"]["key"]])
        for _ in range(r.randint(2, 4) if g["identity"]["kind"] == "gas giant" else r.randint(1, 2)):
            back, e = r.random() < 0.6, r.uniform(0.1, 0.5)
            near = least / (1 - e)                       # (so that even at its closest it stays outside them)
            if near >= 0.47 * reach:                     # (no room between its own moons and the limit of what it can hold)
                continue
            put(sysdir, "small-bodies", "body", body("captured moon", name(r), g["identity"]["name"], r.uniform(near, 0.47 * reach), e, r.uniform(140, 175) if back else r.uniform(25, 55),
                math.exp(r.uniform(math.log(1), math.log(60))), r.choices(["primitive", "carbonaceous"], [0.7, 0.3])[0],
                f"Once it went round the star; {g['identity']['name']} caught it. It goes round {'backward' if back else 'the same way as the planet turns'}, far out.", G * g["physical"]["mass"]),
                "between 0.05 and 0.47 of the giant's reach, as the Sun's giants' are.")

    # Centaurs: ice bodies wandering among the giants.
    r = rng("centaurs")
    for _ in range(3 if len(giants) >= 2 else 0):
        a = r.uniform(au(giants[0]) * 1.15, au(giants[-1]) * 0.9)
        put(sysdir, "small-bodies", "body", body("centaur", name(r), sname, a * AU, r.uniform(0.1, 0.5), r.uniform(2, 25), math.exp(r.uniform(math.log(10), math.log(120))), "icy",
            "An ice body among the giants, on an orbit that will last a few million years. One day a giant will throw it inward as a comet, or out.", mu),
            "between the first giant and the last.")

    # Dwarf planets of the outer belt: the Sun's has perhaps 200; this one's by the ground it covers.
    if giants:
        lo, hi = au(giants[-1]) / res(ob["inner_resonance"]), au(giants[-1]) / res(ob["outer_resonance"])
        expect = 200 * ring(lo, hi) / ring(ob["inner_edge"], ob["outer_edge"])
        r = rng("dwarfs")
        for _ in range(min(3, round(expect))):
            put(sysdir, "small-bodies", "body", body("dwarf planet", name(r), sname, r.uniform(lo, hi) * AU, r.uniform(0.03, 0.25), r.uniform(1, 28), r.uniform(450, 1200), "icy",
                f"A world of ice in the outer belt, heavy enough to have pulled itself round. One of perhaps {round(expect)}.", mu),
                "the outer belt's largest: round above about 400 km in radius.")

    # Comets: returning ones thrown in by the giants (under 200 years); and one from the far cloud.
    r = rng("comets")
    comets = []
    for _ in range(4 if giants else 0):
        q, far = warm * r.uniform(0.3, 2.5), r.uniform(au(giants[-1]) * 0.9, au(giants[-1]) * 2.2)
        a, e = (q + far) / 2, (far - q) / (far + q)
        nm = name(r)
        comets.append((nm, q, far))
        put(sysdir, "small-bodies", "body", body("comet", nm, sname, a * AU, e, r.uniform(2, 35), r.uniform(0.75, 2.5), "icy",
            f"A returning comet. It comes in to {q:.2f} AU, where it boils and grows a tail, and goes out to {far:.1f} AU.", mu),
            "returning comets come round in under 200 years, thrown in from the outer belt and scattered disc.")
    q, a = warm * r.uniform(0.3, 2.5), r.uniform(2000, 20000) * (star["mass"]) ** (1 / 3)
    put(sysdir, "small-bodies", "body", body("comet", name(r), sname, a * AU, 1 - q / a, r.uniform(0, 180), r.uniform(2, 10), "icy",
        f"A comet from the far cloud. It comes in to {q:.2f} AU once in a very long time.", mu),
        "the others come from the far cloud, once in thousands to millions of years.")

    # Regions.
    def region(kind, nm, lo, hi, about, why, parent=None):
        rec = {"provenance": "seeded", "in_game": "not made", "identity": {"key": f"population.{SYS}.{slug(nm)}", "name": nm, "kind": kind, "about": about}, "extent": {"inner": si(r3(lo) * AU * 1000), "outer": si(r3(hi) * AU * 1000)}}
        if parent:
            rec["identity"]["parent"] = f"body.{SYS}.{slug(parent)}"
        put(sysdir, "regions", "population", rec, why)
    if giants:
        region("scattered disc", "Scattered disc", au(giants[-1]), au(giants[-1]) * 100 / 30, "Ice bodies the giants threw outward, on long, tilted, stretched orbits. They come no closer than the last giant.",
               "the Sun's: no closer than its last giant at 30 AU, reaching past 100.")
    scale = star["mass"] ** (1 / 3)
    region("far cloud", "Far cloud", 2000 * scale, 200000 * scale, "A vast thin shell of ice bodies at the edge of the star's hold. Past it is the next star's.",
           "the Sun's is 2,000 to 200,000 AU; this star's by the cube root of its mass, as the reach of a pull goes.")
    for nm, q, far in comets:
        region("meteoroid stream", nm + " stream", q, far, f"Grains and pebbles shed by the comet {nm}, strung along its orbit. A world whose orbit it crosses passes through it once each time round.",
               "a returning comet leaves one along its orbit.", parent=nm)

print(f"{wrote} records {'would be ' if dry else ''}written, {kept} already there and left alone")
