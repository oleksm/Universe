"""Mounts (SFO 19): the standard between a hull's slot and what is fitted to it, derived from the equipment list.

    python3 tools/standards/mounts.py

For every (slot kind, size class) in use by equipment or a hull's slots: an envelope a tenth larger than the largest piece,
the weight, thrust, recoil (a breaker's push-back too), landing load and hold (a clamp's or docking gear's grip on the ship) a quarter more than the most, the feeds (power, cooling as a millionth of the
jet's loss, fuel) likewise, a nozzle opening four fifths of the width, and the attachment (pattern by kind; each point the
weight at 3 g plus the thrust or landing or recoil load, with the margin; shear half). A class with no equipment is scaled
from the nearest, twice the volume a class. Writes standards/SFO/metadata/mounts/<slot>-s<class>.yaml, `fits` on each piece
of equipment and `mount` on each hull slot. Idempotent: run it after adding or resizing equipment. Run from the repository root.
"""
import glob, os, re, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import device_heat

S = "standards/SFO/"; E = S + "metadata/equipment/"; MT = S + "metadata/mounts/"
q = lambda s: '"' + s.replace('"', '\\"') + '"'
def r(v, n=3): return float(f"{v:.{n}g}")
NAMES = {"power": "power plant", "drive": "main drive", "thrusters": "thruster", "lift": "lift", "tank": "tank", "cargo": "cargo", "hyperdrive": "hyperdrive", "capacitor": "capacitor",
         "computer": "flight computer", "transponder": "transponder", "sensors": "sensor", "comm": "comm", "life_support": "life support", "hardpoint": "hardpoint", "utility": "utility",
         "avionics": "avionics", "gear": "landing gear", "access": "access", "handling": "handling", "thermal": "thermal", "command": "command station", "engine": "engine"}
PATTERN = {"drive": ("ring", 8), "thrusters": ("ring", 4), "lift": ("ring", 6), "tank": ("saddles", 4), "gear": ("trunnion", 3), "hardpoint": ("ring", 4), "access": ("ring", 8), "thermal": ("corners", 4), "engine": ("ring", 8)}
M, DESIGN_G = 1.25, 3 * 9.80665


def main():
    os.makedirs(MT, exist_ok=True)
    eq = {}
    for f in sorted(glob.glob(E + "*.yaml")):
        d = yaml.safe_load(open(f)); i = d["identity"]
        if i.get("slot") and d.get("size_class"):
            eq.setdefault((i["slot"], d["size_class"]), []).append(d)
    slots = set()
    for f in glob.glob(S + "metadata/hulls/*.yaml"):
        for sl in yaml.safe_load(open(f)).get("slots") or []:
            slots.add((sl["kind"], sl["size"]))

    def figures(kind, cls):
        here = eq.get((kind, cls))
        near = next(((c, eq[(kind, c)]) for c in sorted(range(1, 5), key=lambda c: abs(c - cls)) if (kind, c) in eq), None)
        base, k = (here, 1.0) if here else (near[1], 2.0 ** (cls - near[0])) if near else ([], 1.0)
        if not base:
            return None, 1.0, None
        g = lambda fn: max(fn(d) for d in base)
        fnk = lambda d: d["function"]
        heat = lambda d: device_heat(fnk(d))   # (one rule: lib.device_heat, SFO 22)
        burn = lambda d: fnk(d)["output"] / fnk(d)["efficiency"] / 3.45e14 if fnk(d)["kind"] == "power_plant" else (fnk(d)["thrust"] / fnk(d)["exhaust"] if "thrust" in fnk(d) else 0)
        return {"L": g(lambda d: d["physical"]["length"]) * k ** (1 / 3), "W": g(lambda d: d["physical"]["width"]) * k ** (1 / 3), "H": g(lambda d: d["physical"]["height"]) * k ** (1 / 3),
                "mass": g(lambda d: d["physical"]["mass"]) * k, "power": g(lambda d: (d.get("needs") or {}).get("power", 0)) * k,
                "thrust": g(lambda d: fnk(d).get("thrust", 0)) * k, "heat": g(heat) * k, "burn": g(burn) * k,
                "recoil": g(lambda d: fnk(d).get("slug_mass", 0) * fnk(d).get("muzzle_speed", 0) * fnk(d).get("rate", 0) + (fnk(d).get("feed_force", 0) if fnk(d)["kind"] == "breaker" else 0)) * k,
                "hold": g(lambda d: fnk(d).get("holds", 0) if fnk(d)["kind"] == "docking" else 0) * k,
                "landing": g(lambda d: fnk(d).get("holds", 0) if fnk(d)["kind"] == "landing_gear" else 0) * k}, k, here is not None

    n = 0
    for kind, cls in sorted(set(eq) | slots):
        fig, k, exact = figures(kind, cls)
        if fig is None:
            continue
        slug = f"{kind.replace('_', '-')}-s{cls}"; key = f"mount.{slug}"
        name = f"{NAMES.get(kind, kind).capitalize()} mount, class {cls}"
        s = (f"# yaml-language-server: $schema=../../schema/mount.schema.yaml\n# SFO 19. {'Set round the equipment of this slot and class on the list today, with a quarter to spare.' if exact else 'No equipment of this class is on the list: scaled from the nearest class, twice the volume a class.'}\n"
             f"identity:\n  key: {key}\n  name: {q(name)}\n  slot: {kind}\n  size_class: {cls}\n  description: {q('What a class ' + str(cls) + ' ' + NAMES.get(kind, kind) + ' slot offers, and what fits it.')}\n")
        s += f"envelope:\n  length: {r(fig['L'] * 1.1)!r}\n  width: {r(fig['W'] * 1.1)!r}\n  height: {r(fig['H'] * 1.1)!r}\nbears:\n  mass: {r(fig['mass'] * M)!r}\n"
        if fig["thrust"]: s += f"  thrust: {r(fig['thrust'] * M)!r}\n"
        if fig["recoil"]: s += f"  recoil: {r(fig['recoil'] * M)!r}\n"
        if fig["landing"]: s += f"  landing: {r(fig['landing'] * M)!r}\n"
        if fig.get("hold"): s += f"  hold: {r(fig['hold'] * M)!r}\n"
        pattern, pts = PATTERN.get(kind, ("corners", 4 if fig["mass"] < 2000 else 8))
        per = (fig["mass"] * DESIGN_G + fig["thrust"] + fig["landing"] + fig["recoil"] + fig.get("hold", 0)) * M / pts
        s += f"attachment:\n  points: {pts}\n  pattern: {pattern}\n  each:\n    tension: {r(per)!r}\n    compression: {r(per)!r}\n    shear: {r(per / 2)!r}\n"
        feeds = []
        if fig["power"]: feeds.append(f"  power: {r(fig['power'] * M)!r}\n")
        if fig["heat"]: feeds.append(f"  cooling: {r(fig['heat'] * M)!r}\n")
        if fig["burn"]: feeds.append(f"  fuel: {r(fig['burn'] * M)!r}\n")
        if feeds: s += "feeds:\n" + "".join(feeds)
        if fig["thrust"]: s += f"nozzle:\n  diameter: {r(fig['W'] * 0.8)!r}\n"
        s += "basis:\n" + f"  - {{ of: [envelope, bears, attachment{', feeds' if feeds else ''}{', nozzle' if fig['thrust'] else ''}], rule: rule.mounts-margin }}\n"   # (the rule, stated once)
        yaml.safe_load(s)
        open(MT + slug + ".yaml", "w").write(s); n += 1
    # equipment fits a mount; a hull's slots offer one
    for f in sorted(glob.glob(E + "*.yaml")):
        s = open(f).read(); d = yaml.safe_load(s); i = d["identity"]
        if not i.get("slot") or not d.get("size_class") or "\nfits:" in s:
            continue
        key = f"mount.{i['slot'].replace('_', '-')}-s{d['size_class']}"
        s, k = re.subn(r"^size_class: (\d)\n", lambda m: m.group(0) + f"fits: {key}\n", s, count=1, flags=re.M)
        yaml.safe_load(s); open(f, "w").write(s)
    for f in glob.glob(S + "metadata/hulls/*.yaml"):
        s = open(f).read()
        s2 = re.sub(r"^(  - \{ name: ([a-z_0-9]+), kind: ([a-z_]+), size: (\d) \})$", lambda m: f"  - {{ name: {m.group(2)}, kind: {m.group(3)}, size: {m.group(4)}, mount: mount.{m.group(3).replace('_', '-')}-s{m.group(4)} }}", s, flags=re.M)
        if s2 != s: open(f, "w").write(s2)
    print(n, "mounts")


if __name__ == "__main__":
    main()
