#!/usr/bin/env python3
"""Seeds what the game does not make yet into the celestial registry: the scattered disc, the far
cloud and each returning comet's meteoroid stream (regions/), for each system written out. The small
bodies themselves (comets, centaurs, crossing asteroids, captured moons, dwarf planets, each belt's
largest) are the engine's: celestial_export.py writes them from its export.

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
def law(name):
    """A law's value, from the Dogma registry (standards/Dogma), by its file name."""
    import glob as _g
    return float(yaml.safe_load(open(_g.glob(os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "standards", "Dogma", "metadata", "*", name + ".yaml"))[0]))["value"])


AU, G, SUN = law("astronomical-unit") / 1000, law("gravitation"), law("sun-mass")          # km, SI, kg
SUN_W = law("sun-luminosity")
si = lambda v: float(f"{v:.15g}")       # (records are in SI; this works in km, days and AU, and writes each value in SI)
galaxy = yaml.safe_load(open(os.path.join(CEL, "seeding", "galaxy.yaml")))["galaxy"]
laws = yaml.safe_load(open(os.path.join(CEL, "seeding", "asteroids.yaml")))
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


def own_density(cls, nm, radius_km):
    """A body's own density: each its own, between its class's as a rubble pile and as one solid piece
    (drawn from the galaxy's seed and its name); one over 200 km in radius has pulled itself solid."""
    ph = (classes.get(cls) or {}).get("physical") or {}
    lo, hi = ph.get("density_rubble"), ph.get("density_monolith")
    if not hi or not lo or radius_km >= 200:
        return hi
    return round(random.Random(f"{galaxy['seed']}:density:{nm}").uniform(lo, hi), -1)


def body(kind, nm, parent, a_km, e, incl, radius_km, cls, about, mu, d=None):
    rec = {"provenance": "seeded", "in_game": "not made", "identity": {"key": f"body.{SYS}.{slug(nm)}", "name": nm, "kind": kind, "parent": f"body.{SYS}.{slug(parent)}", "about": about},
           "orbit": {"semi_major_axis": si(r3(a_km, 6) * 1000), "eccentricity": r3(e, 8), "inclination": round(incl, 2), "period": si(r3(2 * math.pi * math.sqrt((a_km * 1000) ** 3 / mu) / 86400, 5) * 86400)},
           "physical": {"radius": si(r3(radius_km) * 1000)}, "rock": {"class": "rock-class." + cls}}
    d = d or (laws["sizes"].get("comet_density") if kind == "comet" else own_density(cls, nm, radius_km))
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

    # (The small bodies themselves are the engine's now: celestial_export.py writes them from its export.
    # Here, only the regions the game does not make. A returning comet is one of the records, on an orbit
    # that does not reach the far cloud.)
    small = os.path.join(sysdir, "small-bodies")
    recs = [yaml.safe_load(open(os.path.join(small, f))) for f in sorted(os.listdir(small))] if os.path.isdir(small) else []
    comets = [(c["identity"]["name"], c["orbit"]["semi_major_axis"] / 1000 / AU * (1 - c["orbit"]["eccentricity"]), c["orbit"]["semi_major_axis"] / 1000 / AU * (1 + c["orbit"]["eccentricity"]))
              for c in recs if c["identity"]["kind"] == "comet" and c["orbit"]["semi_major_axis"] / 1000 / AU < 1000]
    # (A stream whose comet is gone goes with it, unless a person has taken it over.)
    regions = os.path.join(sysdir, "regions")
    for f in sorted(os.listdir(regions)) if os.path.isdir(regions) else []:
        was = yaml.safe_load(open(os.path.join(regions, f))) or {}
        if was.get("identity", {}).get("kind") == "meteoroid stream" and was.get("provenance") == "seeded" and was["identity"].get("parent") not in {c["identity"]["key"] for c in recs}:
            print(f"  gone: {f}")
            if not dry:
                os.remove(os.path.join(regions, f))

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
