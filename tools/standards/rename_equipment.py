"""One naming rule for equipment (registry audit, step 3b-iv): equipment.<slot>[.<family>].<variant>, the family left out when it
is the slot's own word; the file and the parts folder named after the key's tail with hyphens for dots (tank-deuterium-s1). The variant is the
record's own trailing token (s1, 5t, crew, mc07) or s<size class>. Every old key goes into content/base/aliases.ron, and every
reference in standards/, docs/, content/base/prices.ron and the crates' literals follows.

    python3 tools/standards/rename_equipment.py          # show the map
    python3 tools/standards/rename_equipment.py --write  # do it

Run from the repository root; then mounts.py, the build and the tests.
"""
import glob, os, re, subprocess, sys
import yaml

E = "standards/SFO/metadata/equipment/"; P = "standards/SFO/metadata/parts/"
SIZEISH = re.compile(r"^(s\d|k\d|\d+t|\d+)$")
FAMILY = {"tank": "deuterium", "water-tank": "water", "gas-cargo-tank": "gas-cargo", "liquid-cargo-tank": "liquid-cargo", "waste-tank": "waste", "air-store": "air",
          "tank-methalox": "methalox", "tank-hydrolox": "hydrolox", "tank-hydrogen": "hydrogen", "tank-nitrogen": "nitrogen", "life": None, "landing-gear": "strut", "ore-bay": "ore-bay",
          "gun-mass-driver": "mass-driver", "laser-pulse": "pulse-laser", "suit-locker": "suits", "fire-unit": "fire", "pressure-door": "door", "food-store": "food-store"}


def plan():
    recs = {}
    for f in sorted(glob.glob(E + "*.yaml")):
        d = yaml.safe_load(open(f)); recs[os.path.basename(f)[:-5]] = d
    prefixes = {}
    for fn in recs:
        t = fn.split("-")
        if len(t) >= 2: prefixes.setdefault("-".join(t[:-1]), []).append(fn)
    out = {}
    for fn, d in recs.items():
        slot = d["identity"]["slot"].replace("_", "-"); cls = d.get("size_class", 1); t = fn.split("-")
        if t[0] == "mc07":
            family, variant = "-".join(t[1:]), "mc07"
        elif SIZEISH.match(t[-1]) and len(t) >= 2:
            family, variant = "-".join(t[:-1]), t[-1]
        elif len(t) >= 2 and len(prefixes.get("-".join(t[:-1]), [])) >= 2:
            family, variant = "-".join(t[:-1]), t[-1]
        else:
            family, variant = fn, f"s{cls}"
        family = FAMILY.get(family, family)
        if family and family.startswith(slot + "-"): family = family[len(slot) + 1:]
        if family == slot or family is None: family = ""
        key = f"equipment.{slot}." + (f"{family}." if family else "") + variant
        new_fn = key[len("equipment."):].replace(".", "-")   # (file name = the key's tail)
        out[fn] = (d["identity"]["key"], key, new_fn, (d.get("built_of") or {}).get("parts"))
    return out


def main(write):
    m = plan()
    changes = {old: new for fn, (old, new, nf, folder) in m.items() if old != new}
    files = {fn: nf for fn, (old, new, nf, folder) in m.items() if fn != nf}
    print(f"{len(changes)} keys change, {len(files)} files move")
    for fn, (old, new, nf, folder) in sorted(m.items()):
        if old != new or fn != nf: print(f"  {old:<40} -> {new:<44} {fn} -> {nf}")
    if not write: return
    # 1. the references, whole keys only, longest first
    rx = re.compile("|".join(re.escape(k) for k in sorted(changes, key=len, reverse=True)) + r"(?![a-z0-9-]|\.[a-z])")
    paths = [f for f in glob.glob("standards/**/*.yaml", recursive=True) if not f.startswith("standards/changes.yaml")] + glob.glob("docs/**/*.md", recursive=True) + ["content/base/prices.ron", "standards/game-keys.yaml", "crates/sim/src/operator.rs", "crates/world/src/belts.rs", "crates/game/src/dev.rs", "tools/standards/build.py", "tools/standards/crew.py", "tools/standards/propulsion.py", "tools/standards/mounts.py"]
    touched = 0
    for f in paths:
        if not os.path.exists(f): continue
        s = open(f, encoding="utf-8").read(); t = rx.sub(lambda mm: changes[mm.group(0)], s)
        if t != s: open(f, "w", encoding="utf-8").write(t); touched += 1
    # 2. the files and the parts folders
    for fn, (old, new, nf, folder) in m.items():
        if fn != nf:
            os.rename(E + fn + ".yaml", E + nf + ".yaml")
        if folder and folder != nf and os.path.isdir(P + folder) and not os.path.isdir(P + nf):
            os.rename(P + folder, P + nf)
            s = open(E + nf + ".yaml", encoding="utf-8").read().replace(f"  parts: {folder}\n", f"  parts: {nf}\n", 1); open(E + nf + ".yaml", "w", encoding="utf-8").write(s)
    # 3. the aliases, for saves and peers
    p = "content/base/aliases.ron"; s = open(p).read()
    priced = set(re.findall(r'"(equipment\.[a-z0-9.-]+)"', open("content/base/prices.ron").read()))   # (an alias must point at an entry the game has: the priced, made kinds)
    body = "".join(f'    "{old}": "{new}",\n' for old, new in sorted(changes.items()) if new in priced)
    s = re.sub(r"\{\}\s*$", "{\n    // Equipment keys to one rule, equipment.<slot>[.<family>].<variant> (registry audit, 2026-10-07).\n" + body + "}\n", s) if s.rstrip().endswith("{}") else s.replace("}\n", body + "}\n", 1)
    open(p, "w").write(s)
    # 4. the registry's own list of every rename, for anything that saved a key and is not the game
    p = "standards/renames.yaml"; s = open(p).read()
    for old, new in sorted(changes.items()):
        if f"was: {old}," not in s:
            s = s.rstrip("\n") + f"\n  - {{ was: {old}, is: {new}, date: \"{__import__('datetime').date.today().isoformat()}\", why: \"one naming rule for equipment: equipment.<slot>[.<family>].<variant>\" }}\n"
    open(p, "w").write(s)
    print(f"{touched} files with references rewritten; aliases and standards/renames.yaml written")


if __name__ == "__main__":
    main("--write" in sys.argv)
