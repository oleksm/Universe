"""DC-DC converters for a ship's buses (ships, 2026-10-18: a 3 kV DC bus from the plants and batteries, a 270 V DC bus
for life support, pumps and cabins, a 28 V DC bus for avionics, sensors, comms and command). Each steps the 3 kV bus
down to one of the others: isolated silicon carbide stages, a high-frequency transformer, a cold plate on the ship's
coolant pair. Figures of the order of aerospace and traction converters, from memory; every record says so.

    python3 tools/standards/converters.py        # write them (idempotent)
"""
import os
import yaml

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
M = os.path.join(ROOT, "standards/SFO/metadata/equipment")
HEAD = "# yaml-language-server: $schema=../../schema/equipment.schema.yaml\n# Written by tools/standards/converters.py: change it there.\n"
KEEP = ("visual", "model_requests")
MEMORY = "From memory of aerospace and traction DC-DC converters (isolated SiC stages, liquid-cooled); to be sourced."

CONVERTERS = [  # (output V, kW, efficiency, kW/kg, kW/litre)
    (28, 1, 0.93, 0.5, 0.5), (28, 3, 0.93, 0.6, 0.5), (28, 5, 0.93, 0.6, 0.5),
    (270, 10, 0.97, 1.6, 2.0), (270, 40, 0.97, 2.0, 2.0), (270, 100, 0.97, 2.2, 2.0)]
SHARES = [("Power stages and controls", "good.electronics", 0.3, "module.electronics-works", "silicon carbide switches, gate drives, the controller"),
          ("Transformer and inductors", "stock.cu-wire-2", 0.35, "module.electronics-works", "the isolating high-frequency transformer and the filter chokes; ferrite cores counted with the copper"),
          ("Cold plate and case", "stock.al6061-pl-5", 0.35, "module.machining-centre", "a machined cold plate on the coolant pair, the sealed case and four feet")]


def r(x, nd=2):
    return round(float(x), nd)


def ports(v, kw, eff):
    """Its connectors (the harness's, counted with the routed runs): the 3 kV pair in, the low bus pair out by its
    current, a data connector, the coolant pair by its heat."""
    amps = kw * 1000.0 / v
    out = "circular-shell-25" if amps <= 55 else "circular-shell-33" if amps <= 120 else "hvdc-pole-400a"
    cool = "flat-face-dn13" if kw >= 10 else "flat-face-dn6"
    return amps, out, cool


def record(v, kw, eff, kwkg, kwl):
    p = kw * 1000.0
    amps, out, cool = ports(v, kw, eff)
    mass = r(kw / kwkg, 1)
    vol = kw / kwl / 1000.0                       # m3
    l = r((vol * 4) ** (1 / 3) * 1.0, 3)          # a flat box about 2 : 1 : 0.5
    w, h = r(l / 2, 3), r(vol / (l * l / 2), 3)
    loss = p * (1 / eff - 1)
    slug = f"power-converter-{v}v-{kw}kw"
    code = f"DC{v}K{kw}"
    parts = [{"code": f"{code}-{i + 1:02d}", "name": n, "description": d, "mass": r(mass * s, 2), "length": l, "width": w, "height": r(h * s * 2 if i < 2 else h, 3),
              "item": item, "quantity": r(mass * s * 1.15, 2), "module": mod} for i, (n, item, s, mod, d) in enumerate(SHARES)]
    return slug, {
        "identity": {"key": f"equipment.power.converter.{v}v-{kw}kw", "name": f"DC converter, 3 kV to {v} V, {kw} kW", "maker": "org.meridian", "revision": "draft", "slot": "power",
                     "description": f"Steps the ship's 3 kV DC bus down to its {v} V DC bus, up to {kw} kW out, isolated: a sealed box on four feet, the 3 kV pair in at its -x end (two connector.hvdc-pole-150a receptacles, {p / eff / 3000:.0f} A), the {v} V pair out ({amps:.0f} A: two connector.{out} receptacles) and data (connector.circular-shell-11) at its +x end, the coolant pair under it (connector.{cool}). The connectors' mass is the routed harness's (connector.schema physical), not the box's. {eff * 100:.0f}% efficient: at full load it draws {p / eff / 1000:.2f} kW and gives {loss / 1000:.2f} kW to the coolant loop."},
        "size_class": 1, "fits": "mount.power-s1",
        "physical": {"mass": mass, "volume": r(vol, 4), "length": l, "width": w, "height": h},
        "built_of": {"parts": slug, "list": parts},
        "making": {"module": "module.assembly-shop"},
        "function": {"kind": "converter", "from_voltage": 3000.0, "to_voltage": float(v), "carries": p, "efficiency": eff},
        "life": 473364000,
        "basis": [{"of": ["function", "physical", "built_of.list", "size_class"], "tier": "invented", "review": True, "note": f"{kwkg:g} kW/kg and {kwl:g} kW/litre with its cold plate; {eff * 100:.0f}% at full load; {MEMORY}"},
                  {"of": ["built_of", "making"], "rule": "rule.first-design-parts"},
                  {"of": ["life"], "rule": "rule.life-ship-system"}]}


def main():
    n = 0
    for c in CONVERTERS:
        slug, d = record(*c)
        path = os.path.join(M, slug + ".yaml")
        if os.path.exists(path):
            old = yaml.safe_load(open(path)) or {}
            d = dict(d, **{k: old[k] for k in KEEP if k in old and k not in d})
        text = HEAD + yaml.safe_dump(d, sort_keys=False, allow_unicode=True, width=230)
        if not os.path.exists(path) or open(path).read() != text:
            open(path, "w").write(text)
            n += 1
    print(f"{len(CONVERTERS)} converters; {n} written")


if __name__ == "__main__":
    main()
