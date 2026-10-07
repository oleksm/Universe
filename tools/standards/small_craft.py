"""Small-craft equipment (the ships session's request of 2026-10-07, for the user's personal commuter): ducted fans, a fuel
cell, a small battery, small tanks of propellant, gas, oxygen and water, a scrubber cartridge; and the schema words they need
(engine cycle ducted_fan with min_density, an open-loop pack's own air and water stores).

    python3 tools/standards/small_craft.py

Idempotent. Parts as a rule: each record carries its parts as built_of.list. Run from the repository root, then mounts.py.
"""
import math, os, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import edit

E = "standards/SFO/metadata/equipment/"
AL, TI, EL, MLI, ST, MOT, HP, PU, CFC = "stock.al6061-pl-5", "stock.ti64-pl-5", "good.electronics", "stock.mli-fl", "stock.st304-pl-3", "good.electric-motors", "good.heat-pumps", "good.pumps", "stock.cfc-pl-3"
WB, MC, AS = "module.welding-bay", "module.machining-centre", "module.assembly-shop"
Q = '"'


def q(s):
    return Q + str(s).replace(Q, "\\" + Q) + Q


def schemas():
    def dic(s):
        if "ducted_fan" in s: return s
        s = s.replace("    enum: [fusion_pulse, fusion_thermal, nuclear_thermal, chemical, cold_gas]\n", "    enum: [fusion_pulse, fusion_thermal, nuclear_thermal, chemical, cold_gas, ducted_fan]\n")
        return s.replace('      cold_gas: "Stored gas let out, about 0.7 km/s: newtons to kilonewtons, no flame, no heat; for attitude and docking."\n',
                         '      cold_gas: "Stored gas let out, about 0.7 km/s: newtons to kilonewtons, no flame, no heat; for attitude and docking."\n'
                         '      ducted_fan: "Air driven by an electric fan in a duct, 100 to 200 m/s: kilonewtons for megawatts and no propellant, where there is air (its thrust falls with the cube root of the air\'s density at the same power). The lift of a commuter on a world with air."\n')
    edit("standards/dictionary.schema.yaml", dic)

    def eq(s):
        if "min_density" in s: return s
        old = '        allOf: [{ $ref: "#/definitions/jet" }]\n        required: [kind, cycle, thrust, exhaust, efficiency, burns, throttle, gimbal, isotropic_loss, shield_pass, heat_to_hull]\n'
        assert s.count(old) == 1
        s = s.replace(old, '        allOf: [{ $ref: "#/definitions/jet" }]\n        required: [kind, cycle, thrust, exhaust, efficiency, throttle, gimbal, isotropic_loss, shield_pass, heat_to_hull]\n')
        anchor = '          reaction_offset: { type: number, x-unit: "m", description: "m from the mount\'s face, aft, to where the loss originates; the hull\'s share of the sphere is reckoned from there" }\n'
        assert s.count(anchor) == 1
        s = s.replace(anchor, anchor + '          min_density: { type: number, x-unit: "kg/m3", description: "kg/m3, the thinnest air a ducted fan is rated for; its thrust at full power falls with the cube root of the density (at 0.3 kg/m3, 63% of its thrust at 1.2). A fan burns nothing: `burns` is left out, and its power is `needs.power` at full thrust." }\n')
        anchor2 = '          cooling: { type: number, x-unit: "W", description: "W of the cabin\'s heat it carries away to the ship\'s heat path: the people\'s (need.food.heat a head) and their equipment\'s; it also dries the air" }\n'
        assert s.count(anchor2) == 1
        s = s.replace(anchor2, anchor2 + '          air_store: { type: number, x-unit: "kg", description: "kg of oxygen an open-loop pack carries in its own bottle; counted with the ship\'s air stores in the budgets" }\n          water_store: { type: number, x-unit: "kg", description: "kg of water an open-loop pack carries; counted with the ship\'s water stores" }\n')
        return s
    edit("standards/SFO/schema/equipment.schema.yaml", eq)

    def cyc(s):
        if "cycles:" in s: return s
        old = '          rate: { type: number, x-unit: "W", description: "W it gives or takes at most" }\n'
        assert s.count(old) == 1
        return s.replace(old, old + '          cycles: { type: integer, description: "full charge-and-discharge cycles before it is worn to 80% of its store; a high-rate pack fewer than a long-duration one" }\n')
    edit("standards/SFO/schema/equipment.schema.yaml", cyc)

    def build(s):
        old = '            if _fn.get("holds") == "element.o": _air += _fn["capacity"]'
        if "_fn.get(\"air_store\"" in s: return s
        assert s.count(old) == 1
        return s.replace(old, old + '\n        if _k == "life_support":\n            _air += _fn.get("air_store", 0); _water += _fn.get("water_store", 0)')
    edit("tools/standards/build.py", build)


def val(v):
    if isinstance(v, bool): return str(v).lower()
    if isinstance(v, str): return q(v) if (" " in v or ":" in v) else v
    return repr(v) if isinstance(v, float) else str(v)   # (repr: 30000000.0, never 3e+07, which YAML reads as text)


def rec(fn, key, name, slot, cls, mass, L, W, H, power, function, parts, code, desc, notes, maker="org.hadley"):
    f = E + fn + ".yaml"
    if os.path.exists(f): return
    lst = []
    for i, (pn, pd, share, item, mod) in enumerate(parts, 1):
        k = share ** (1 / 3); m = max(1, round(mass * share)); cut = 1.15 if mod in (WB, MC, "module.cutting-table") else 1.0
        desc_part = f", description: {q(pd)}" if pd else ""
        lst.append(f"    - {{ code: {code}-{i:02d}, name: {q(pn)}{desc_part}, mass: {m}, length: {L * k:.3g}, width: {W * k:.3g}, height: {H * k:.3g}, item: {item}, quantity: {m * cut:.4g}, module: {mod} }}")
    basis = []
    for of, tier, src, note in notes:
        src_part = f", source: {q(src)}" if src else ""
        basis.append(f"  - {{ of: {of}, tier: {tier}, review: true{src_part}, note: {q(note)} }}")
    needs = f"needs:\n  power: {power:g}\n" if power else ""
    text = (f"# yaml-language-server: $schema=../../schema/equipment.schema.yaml\nidentity:\n  key: {key}\n  name: {q(name)}\n  maker: {maker}\n  revision: draft\n  slot: {slot}\n  description: {q(desc)}\n"
            f"size_class: {cls}\nphysical:\n  mass: {mass:g}\n  volume: {L * W * H:.3g}\n  length: {L:g}\n  width: {W:g}\n  height: {H:g}\n{needs}built_of:\n  parts: {fn}\n  list:\n" + "\n".join(lst) + "\n"
            f"making:\n  module: module.assembly-shop\nfunction:\n" + "\n".join(f"  {k}: {val(v)}" for k, v in function.items()) + "\nlife: 473364000\nbasis:\n" + "\n".join(basis)
            + "\n  - { of: [built_of, making], rule: rule.first-design-parts }\n  - { of: [life], rule: rule.life-ship-system }\n")
    yaml.safe_load(text); open(f, "w").write(text)


def records():
    RHO, ETA = 1.2, 0.75
    for cls, T, d, mass, code in [(1, 10000.0, 1.0, 60, "FAN1"), (2, 25000.0, 1.6, 150, "FAN2"), (3, 60000.0, 2.5, 350, "FAN3")]:
        A = math.pi * d * d / 4; P = T ** 1.5 / math.sqrt(2 * RHO * A) / ETA; ve = math.sqrt(T / (RHO * A))
        rec(f"engine-fan-s{cls}", f"equipment.engine.fan.s{cls}", f"Ducted fan S{cls}", "engine", cls, mass, d + 0.2, d + 0.2, 0.6 + 0.2 * cls, round(P, -3),
            {"kind": "engine", "cycle": "ducted_fan", "thrust": T, "exhaust": float(round(ve)), "efficiency": ETA, "throttle": 0.05, "gimbal": 0.0, "isotropic_loss": 0.0, "shield_pass": 1.0, "reaction_offset": 0.0, "heat_to_hull": 0.0, "min_density": 0.3},
            [("Duct and stators", "", 0.3, CFC, MC), ("Fan and hub", "", 0.25, TI, MC), ("Electric motor", "", 0.35, MOT, AS), ("Inverter and controls", "", 0.1, EL, AS)], code,
            f"An electric fan in a {d:g} m duct: {T / 1000:g} kN in air of 1.2 kg/m3 for {P / 1e6:.2g} MW, nothing burnt. Its thrust falls with the cube root of the air's density at the same power; below 0.3 kg/m3 it is not rated.",
            [("[function, physical, needs]", "derived", None, f"Momentum theory for a ducted fan: thrust T from a disc of area A = {A:.2f} m2 in air of density 1.2 kg/m3 leaves at v = sqrt(T / (rho A)) = {ve:.0f} m/s and takes P = T^1.5 / sqrt(2 rho A) / eta of shaft power at eta = 0.75 for motor and duct: {P / 1e6:.2g} MW. Mass at about 6 kg a kilonewton (electric VTOL fans of the 2020s, from memory). Chosen; review.")])
    rec("power-fuelcell-s1", "equipment.power.fuelcell.s1", "Fuel cell", "power", 1, 90, 0.8, 0.5, 0.5, 0,
        {"kind": "power_plant", "output": 50000.0, "efficiency": 0.55, "burns": "material.hydrolox"},
        [("Stack", "", 0.5, EL, AS), ("Pumps, valves and plumbing", "", 0.3, PU, AS), ("Housing and radiator", "", 0.2, AL, MC)], "FCL1",
        "50 kW from hydrogen and oxygen at 55%, its water drinkable: a commuter's power with no reactor aboard. It burns the same hydrolox as the engines, 7 g/s at full.",
        [("[function, physical]", "invented", None, "An Apollo-type cell scaled to 50 kW: about 1.8 kg a kilowatt (the shuttle's three cells gave 21 kW for 118 kg each, from memory); 55% of the hydrolox's 13 MJ/kg to power, the rest heat and warm water. Review.")])
    rec("power-battery-s0", "equipment.power.battery.s0", "Battery S0", "power", 1, 200, 0.8, 0.5, 0.5, 0,
        {"kind": "battery", "stores": 100000000.0, "rate": 300000.0},
        [("Cells", "", 0.75, EL, AS), ("Housing and cooling", "", 0.15, AL, MC), ("Management", "", 0.1, EL, AS)], "BATS0",
        "100 MJ (28 kWh) at 300 kW in or out: a small craft's store, charged at the dock.",
        [("[function, physical]", "invented", None, "150 Wh/kg with its housing, as the S1; 300 kW in or out. From memory; review.")])
    for prop, dens, frac in [("hydrolox", 320, 0.14), ("methalox", 830, 0.08)]:
        for cap, tag in [(500, "0p5t"), (1000, "1t"), (2000, "2t")]:
            vol = cap / dens; d = (6 * vol / math.pi) ** (1 / 3); side = round(d + 0.1, 2)
            rec(f"tank-{prop}-{tag}", f"equipment.tank.{prop}.{tag}", f"{prop.capitalize()} tank {cap / 1000:g} t", "tank", 1, round(cap * frac), side, side, side, 0,
                {"kind": "tank", "capacity": float(cap), "holds": f"material.{prop}"},
                [("Shell", "", 0.7, AL, WB), ("Insulation", "", 0.2, MLI, AS), ("Fittings and gauging", "", 0.1, ST, MC)], f"TK{prop[:2].upper()}{tag.upper()}",
                f"A {vol:.1f} m3 ball holding {cap:,} kg of {prop}: a small craft's propellant.",
                [("[physical, function]", "invented", None, f"{prop} at {dens} kg/m3: {vol:.1f} m3 as a ball {d:.2f} m across; the tank {frac:.0%} of its contents, more than a big tank's share since a small one's wall and insulation do not shrink with it. Chosen; review.")])
    for cap, tag in [(50, "50kg"), (100, "100kg")]:
        rec(f"tank-nitrogen-{tag}", f"equipment.tank.nitrogen.{tag}", f"Nitrogen bottle {cap} kg", "tank", 1, round(cap * 0.6), round(0.5 + cap / 200, 2), 0.6, 0.6, 0,   # (its box holds the gas at 330 kg/m3: the game checks)
            {"kind": "tank", "capacity": float(cap), "holds": "material.nitrogen"},
            [("Composite pressure vessel", "", 0.8, CFC, WB), ("Valve and regulator", "", 0.2, ST, MC)], f"TKN{tag.upper()}",
            f"A composite bottle of {cap} kg of nitrogen at 300 bar for cold-gas thrusters: {cap / 0.714:.0f} s of a 0.5 kN block at full.",
            [("[physical, function]", "invented", None, "A composite overwrapped pressure vessel at about 0.6 kg a kilogram of gas at this size. From memory; review.")])
    for cap, tag in [(10, "10kg"), (20, "20kg")]:
        rec(f"tank-oxygen-{tag}", f"equipment.tank.oxygen.{tag}", f"Oxygen bottle {cap} kg", "tank", 1, round(cap * 1.5), round(0.4 + cap / 50, 2), 0.3, 0.3, 0,
            {"kind": "store", "capacity": float(cap), "holds": "element.o", "pressure": 30000000.0},
            [("Composite pressure vessel", "", 0.8, CFC, WB), ("Valve and regulator", "", 0.2, ST, MC)], f"TKO{tag.upper()}",
            f"A composite bottle of {cap} kg of oxygen at 300 bar: {cap / 0.895:.0f} person-days of breathing (need.air).",
            [("[physical, function]", "invented", None, "A composite overwrapped bottle at about 1.5 kg a kilogram of oxygen at this size, with its regulator. From memory; review.")])
    for cap, tag in [(50, "50kg"), (100, "100kg")]:
        rec(f"tank-water-{tag}", f"equipment.tank.water.{tag}", f"Water tank {cap} kg", "tank", 1, round(cap * 0.2), round(0.4 + cap / 250, 2), 0.4, 0.4, 0,
            {"kind": "store", "capacity": float(cap), "holds": "good.water"},
            [("Tank", "", 0.8, AL, WB), ("Fittings and pump", "", 0.2, PU, AS)], f"TKW{tag.upper()}",
            f"{cap} litres of water: {cap / 2.5:.0f} person-days to drink (need.water), a few days of washing.",
            [("[physical, function]", "invented", None, "A small aluminium tank at a fifth of its contents. Chosen; review.")])
    rec("life-support-cartridge", "equipment.life-support.cartridge", "Scrubber cartridge", "life_support", 1, 20, 0.4, 0.3, 0.3, 200,
        {"kind": "life_support", "water_recovery": 0.0, "air_recovery": 0.0, "persons": 2, "cooling": 0.0},
        [("Lithium hydroxide canister", "", 0.6, ST, MC), ("Fan and housing", "", 0.4, AL, MC)], "LSC1",
        "A lithium hydroxide canister with a fan: takes up two people's carbon dioxide for twelve hours, then is swapped. No oxygen of its own: fit a bottle (tank.oxygen) for the air they breathe.",
        [("[function, physical, needs]", "invented", None, "Two people give off 2.2 kg of carbon dioxide a day (need.air); 1.3 kg of lithium hydroxide takes up twelve hours of it; with its canister and fan, 20 kg. Apollo's canisters, from memory. Review.")])
    # VTOL packs: high-rate cells, 4 kW/kg at 0.5 MJ/kg (eVTOL packs of the 2020s, from memory: about 3 to 5 kW/kg, 140 to 170 Wh/kg at the pack)
    for cls, mass, code in [(0, 150, "BATV0"), (1, 500, "BATV1"), (2, 1500, "BATV2")]:
        rec(f"power-battery-vtol-s{cls}", f"equipment.power.battery-vtol.s{cls}", f"VTOL pack S{cls}", "power", 1 if cls < 2 else 2, mass, 0.6 + 0.3 * cls, 0.5 + 0.2 * cls, 0.4 + 0.1 * cls, 0,
            {"kind": "battery", "stores": float(mass * 500000), "rate": float(mass * 4000), "cycles": 1500},
            [("High-rate cells", "", 0.7, EL, AS), ("Cooling plates and housing", "", 0.2, AL, MC), ("Management and contactors", "", 0.1, EL, AS)], code,
            f"A pack built for a landing, not a voyage: {mass * 4 / 1000:g} MW out for {mass * 0.5 / (mass * 4) * 1000:.0f} s from {mass * 0.5:g} MJ; about 1,500 full cycles. Lighter per kilowatt than the S0 and S1, heavier per joule.",
            [("[function, physical]", "invented", None, "4 kW/kg and 0.5 MJ/kg (140 Wh/kg) at the pack, with cooling plates: the high-rate lithium packs of electric VTOL craft of the 2020s, from memory; 1,500 cycles to 80%. Review.")])
    # kilowatt thermal, for a craft that makes tens of kilowatts of heat, not megawatts
    rec("thermal-radiator-s0", "equipment.thermal.radiator.s0", "Radiator S0", "thermal", 1, 40, 2.0, 1.0, 0.05, 0,
        {"kind": "radiator", "rejects": 10000.0, "temperature": 330, "area": 2.0},
        [("Panel", "", 0.6, AL, MC), ("Tubes and manifold", "", 0.3, ST, WB), ("Fittings", "", 0.1, ST, MC)], "RAD0",
        "A 2 m2 panel at 330 K: 10 kW to space from both faces in vacuum; in air it also convects, more. For a cabin's and a pack's heat.",
        [("[function, physical]", "derived", None, "Two faces of 2 m2 at 330 K and emissivity 0.9 radiate 2 x 2 x 0.9 x 5.67e-8 x 330^4 = 2.4 kW to empty space; a 10 kW rating takes a pumped panel at 20 kg/m2 that sheds the rest by convection in air of 1 kg/m3 and more; in vacuum rate it at its radiated 2.4 kW. Chosen; review.")])
    rec("thermal-coolant-loop-s0", "equipment.thermal.coolant-loop.s0", "Coolant loop S0", "thermal", 1, 30, 0.5, 0.4, 0.3, 300,
        {"kind": "coolant_loop", "carries": 20000.0, "flow": 0.5},
        [("Pump and reservoir", "", 0.5, PU, AS), ("Lines and cold plates", "", 0.4, ST, MC), ("Controls", "", 0.1, EL, AS)], "CLP0",
        "A pumped water-glycol loop carrying 20 kW from cold plates to a radiator or an air cooler: 0.5 kg/s at a 10 K rise.",
        [("[function, physical, needs]", "derived", None, "20 kW at a 10 K rise in water-glycol (3.8 kJ/kg K) is 0.53 kg/s; a 300 W pump; 30 kg of pump, lines and plates. Chosen; review.")])
    rec("thermal-air-cooler-s0", "equipment.thermal.air-cooler.s0", "Air cooler S0", "thermal", 1, 15, 0.5, 0.4, 0.3, 400,
        {"kind": "heat_exchanger", "transfers": 20000.0},
        [("Finned core", "", 0.6, AL, MC), ("Fan", "", 0.3, MOT, AS), ("Housing", "", 0.1, AL, MC)], "ACL0",
        "A finned core with a fan: 20 kW from the coolant loop to the air, where there is air (of 0.3 kg/m3 or more); nothing in vacuum. A commuter on a world with air cools this way and carries a small radiator for the dock.",
        [("[function, physical, needs]", "invented", None, "A car's radiator and fan: about 20 kW at a 30 K difference from 15 kg of finned aluminium, 400 W of fan. From memory; review.")])
    rec("gear-strut-180kn", "equipment.gear.strut.180kn", "Landing leg 180 kN", "gear", 1, 190, 0.4, 0.4, 1.6, 800,
        {"kind": "landing_gear", "holds": 180000.0, "stroke": 0.4, "efficiency": 0.8, "sink_rate": 3.0, "extended": 1.6},
        [("Strut and oleo", "", 0.6, TI, MC), ("Pad", "", 0.2, AL, MC), ("Retraction actuator", "", 0.2, MOT, AS)], "GRS180",
        "A middle leg: 180 kN, a 0.4 m stroke, for craft of 20 to 40 t on four or six legs.",
        [("[physical, function, needs]", "invented", None, "At about 1 kg a kilonewton with titanium (the A320's 1.2 kg/kN, as the S1), 190 kg. Four hold a 24 t craft at 3 m/s on a 0.4 m stroke (1.1 g). Chosen; review.")])
    edit(E + "life-support-s0.yaml", lambda s: s if "air_store" in s else s.replace("  persons: 2\n  cooling: 400\n", "  persons: 2\n  cooling: 400\n  air_store: 2.2\n  water_store: 5\n"))
    if os.path.exists(E + "tank-hydrolox-s0.yaml"):
        s = open(E + "tank-hydrolox-s0.yaml").read().replace("key: equipment.tank.hydrolox.s0", "key: equipment.tank.hydrolox.3p5t").replace('name: "Hydrolox tank S0"', 'name: "Hydrolox tank 3.5 t"').replace("parts: tank-hydrolox-s0", "parts: tank-hydrolox-3p5t")
        open(E + "tank-hydrolox-3p5t.yaml", "w").write(s); os.remove(E + "tank-hydrolox-s0.yaml")


if __name__ == "__main__":
    schemas(); records()
    print("small-craft equipment written")
