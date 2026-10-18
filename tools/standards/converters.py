"""DC-DC converters for a ship's buses (ships, 2026-10-18: a 3 kV DC bus from the plants and batteries, a 270 V DC bus
for life support, pumps and cabins, a 28 V DC bus for avionics, sensors, comms and command). Each steps the 3 kV bus
down to one of the others: isolated silicon carbide stages, a high-frequency transformer, a cold plate on the ship's
coolant pair. Figures of the order of aerospace and traction converters, from memory; every record says so.

    python3 tools/standards/converters.py        # write them (idempotent)
"""
import os
import yaml

import equipment_ports as ep

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


def wiring(v, kw, eff, rec):
    """Its ports from the model's empties (input_0/1 the 3 kV pair, output_0/1 the low bus out, data, coolant_0/1),
    else where the description puts them, invented; and its internal: output only while the input is fed."""
    amps, out, cool = ports(v, kw, eff)
    p = kw * 1000.0
    em = ep.empties(rec)
    ph = rec["physical"]
    l, w, h = ph["length"], ph["width"], ph["height"]
    guess = {"input_0": ([-l / 2, 0.23 * w, 0.65 * h], [-1, 0, 0]), "input_1": ([-l / 2, -0.23 * w, 0.65 * h], [-1, 0, 0]),
             "output_0": ([l / 2, 0.23 * w, 0.65 * h], [1, 0, 0]), "output_1": ([l / 2, -0.23 * w, 0.65 * h], [1, 0, 0]),
             "data": ([l / 2, 0.0, 0.3 * h], [1, 0, 0]), "coolant_0": ([-0.12 * l, 0.0, 0.0], [0, 0, -1]), "coolant_1": ([0.12 * l, 0.0, 0.0], [0, 0, -1])}

    def at(name, port):
        a, f = em.get(name) or guess[name]
        return dict(port, at=[ep.r(x) for x in a], facing=[ep.r(x, 4) for x in f],
                    note=f"place from the model's empty {name}" if name in em else "place invented (no installed model with this empty), for review")
    draw = p / eff
    inp = ep.pole_pair("dc-in", "in", "male", ep.HV, "dc-3kv", draw / ep.HV, draw, size=150)
    if out.startswith("hvdc-pole"):
        outs = ep.pole_pair("dc-out", "out", "female", float(v), f"dc-{v}v", amps, p, size=int(out.split("-")[2][:-1]))
    else:
        sh = int(out.rsplit("-", 1)[1])
        outs = [ep.way(f"dc-out-{i}", "out", "female", float(v), f"dc-{v}v", amps / 2, p / 2, sh=sh) for i in (1, 2)]
    data = ep.data_ports(1)
    pts = [at("input_0", inp[0][0]), at("input_1", inp[1][0]), at("output_0", outs[0][0]), at("output_1", outs[1][0]), at("data", data[0][0])]
    for i, (fl, name) in enumerate((("in", "coolant_0"), ("out", "coolant_1"))):
        pts.append(at(name, {"id": f"coolant-{fl}", "flow": fl, "medium": "liquid", "interface": f"interface.{cool}", "half": "male", "make": "manual",
                               "connector": f"connector.{cool}", "power": ep.r(p * (1 / eff - 1), 1)}))
    oid = [q["id"] for q, _w in outs]
    internal = [{"kind": "converts", "from": ["dc-in+", "dc-in-"], "to": oid, "note": f"the {v} V side is live only while the 3 kV pair is fed, {kw} kW at most"}]
    if oid[0] == "dc-out-1":
        internal.append({"kind": "joined", "ports": oid, "note": "two ways off one output"})
    return pts, internal


BOARDS = [  # (V, ways of shell 17, of shell 25, of shell 33, kg, length, width, height)
    (28, 12, 4, 0, 6.0, 0.45, 0.12, 0.3), (270, 0, 8, 2, 11.0, 0.5, 0.15, 0.35)]
BOARD_SHARES = [("Enclosure and rails", "stock.al6061-pl-5", 0.45, "module.machining-centre", "a folded aluminium box, its DIN-style rails and the gland plates"),
                ("Bus bars", "stock.cu-wire-2", 0.2, "module.electronics-works", "copper bars, plus and minus, and their insulators"),
                ("Breakers and controls", "good.electronics", 0.35, "module.electronics-works", "a solid-state breaker on each way, the board's controller")]


def board(v, n17, n25, n33, kg, l, w, h):
    """A distribution board: one low bus's feed in (by whichever connector its converter gives: a pole pair, a shell 33
    or a shell 25), its ways out, each through its own breaker."""
    slug = f"power-distribution-{v}v-{n17 + n25 + n33}way"
    code = f"DB{v}W{n17 + n25 + n33}"
    ch = f"dc-{v}v"
    ins = ep.pole_pair("in", "in", "male", float(v), ch, 400, 400 * v, size=400) + [ep.way("in-33", "in", "male", float(v), ch, 120, 120 * v, sh=33), ep.way("in-25", "in", "male", float(v), ch, 55, 55 * v, sh=25)]
    outs, n = [], 0
    for sh, cnt in ((17, n17), (25, n25), (33, n33)):
        a = {17: 15, 25: 55, 33: 120}[sh]
        for _ in range(cnt):
            n += 1
            outs.append(ep.way(f"way-{n}", "out", "female", float(v), ch, a, a * v, sh=sh))
    total = 400 * v
    rec = {"physical": {"length": l, "width": w, "height": h}}
    ports = ep.place(rec, ins + outs + ep.data_ports(2))
    parts = [{"code": f"{code}-{i + 1:02d}", "name": nm, "description": d, "mass": r(kg * s, 2), "length": l, "width": w, "height": r(h * s, 3),
              "item": item, "quantity": r(kg * s * 1.15, 2), "module": mod} for i, (nm, item, s, mod, d) in enumerate(BOARD_SHARES)]
    return slug, {
        "identity": {"key": f"equipment.power.distribution.{v}v-{n17 + n25 + n33}way", "name": f"Distribution board, {v} V DC, {n17 + n25 + n33} ways", "maker": "org.meridian", "revision": "draft", "slot": "power",
                     "description": f"Where a ship's {v} V DC bus branches: fed from its converter by a pole pair or a circular shell 33 or 25 (whichever the converter has, all three on one bus bar), "
                                    + ", ".join(f"{c} ways of shell {sh} ({a} A)" for sh, c, a in ((17, n17, 15), (25, n25, 55), (33, n33, 120)) if c)
                                    + f" out, each through its own solid-state breaker, and the control bus through. Up to {total / 1000:g} kW on its bus bar. The connectors' mass is the routed harness's, not the board's."},
        "size_class": 1, "fits": "mount.power-s1",
        "physical": {"mass": kg, "volume": r(l * w * h, 4), "length": l, "width": w, "height": h},
        "built_of": {"parts": slug, "list": parts},
        "making": {"module": "module.assembly-shop"},
        "function": {"kind": "switchgear", "carries": float(total)},
        "life": 473364000,
        "ports": ports,
        "internal": [{"kind": "switched", "ports": [q["id"] for q, _w in ins + outs], "note": "one bus bar; each way through its own breaker, the feeds in joined"},
                     {"kind": "joined", "ports": ["data-1", "data-2"], "note": "the control bus passes through"}],
        "basis": [{"of": ["physical", "built_of.list", "function", "ports"], "tier": "invented", "review": True, "note": f"Of the order of aircraft DC power distribution panels, from memory; the ways (their count and sizes) chosen for a commuter's loads (ships, 2026-10-18); places invented until a model gives them."},
                  {"of": ["built_of", "making"], "rule": "rule.first-design-parts"},
                  {"of": ["life"], "rule": "rule.life-ship-system"}]}


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
    for c in CONVERTERS + BOARDS:
        slug, d = record(*c) if c in CONVERTERS else board(*c)
        path = os.path.join(M, slug + ".yaml")
        if os.path.exists(path):
            old = yaml.safe_load(open(path)) or {}
            d = dict(d, **{k: old[k] for k in KEEP if k in old and k not in d})
        if c in CONVERTERS:
            pts, internal = wiring(c[0], c[1], c[2], d)
            d["ports"], d["internal"] = pts, internal
            d["basis"].insert(1, {"of": ["ports"], "tier": "derived", "note": "Places from the installed model's empties where it has them (each port's note says); connectors by current (ports() here); docs/proposals/equipment-ports.md."})
        text = HEAD + yaml.safe_dump(d, sort_keys=False, allow_unicode=True, width=230)
        if not os.path.exists(path) or open(path).read() != text:
            open(path, "w").write(text)
            n += 1
    print(f"{len(CONVERTERS)} converters, {len(BOARDS)} distribution boards; {n} written")


if __name__ == "__main__":
    main()
