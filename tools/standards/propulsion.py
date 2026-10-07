"""Propulsion as a family of single engine units (the ships session's request of 2026-10-06, docs/ships/propulsion-request.md on
`ships`, from the user's "do we even need belly lift... create request to Registry for options").

    python3 tools/standards/propulsion.py

Idempotent. Adds to the schemas (slot kind `engine`; equipment kinds `engine` and `swivel`; dictionary `engine_cycle`), writes
the engines (fusion thermal, nuclear thermal, chemical, cold gas, attitude chemical), the swivels, the propellants as materials
and stock with their tanks and a propellant plant, the standards SFO 22 (Engines) and SFO 23 (Landing pads), then runs
mounts.py. Anchors are sourced where the registry has the source (Discovery II, RL10; standards/sources/research_ship_equipment.json);
the rest is from memory of flown engines and marked review. The old drive, lift and thruster families stay as the game's
stand-ins until the engine takes the new kind. Run from the repository root.
"""
import glob, os, sys, subprocess
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import q, edit

S = "standards/SFO/"; E = S + "metadata/equipment/"; P = S + "metadata/parts/"; MS = S + "metadata/mill-stock/"; MAT = S + "metadata/materials/"
LIFE = 473364000
LIFE_NOTE = "Fifteen years of service for a ship's system, as a ship's machinery is written off on Earth; then it is replaced, and its stock comes back as scrap. Chosen; the user's rule that everything made has a life."
SRC = "standards/sources/research_ship_equipment.json"
OMEGA = 0.05   # the hull's share of the sphere round the reaction, for the default heat_to_hull (a hull-mounted engine; the studio computes the design's own)


def schemas():
    def dic(s):
        if "  engine_cycle:" in s: return s
        s = s.replace("thermal, gear, gate, command]", "thermal, gear, gate, command, engine]")
        s = s.replace('      command: "The command station\'s: where the ship is flown from; on the bridge, inside the pressurised space."\n',
                      '      command: "The command station\'s: where the ship is flown from; on the bridge, inside the pressurised space."\n      engine: "An engine unit\'s (SFO 22): one engine, pointed where the design points it, on a gimbal or a swivel or fixed. Lift, drive and thrusters are what a design does with engines, not kinds of slot."\n')
        return s.rstrip("\n") + '''
  engine_cycle:
    description: "How an engine makes its jet (SFO 22). The cycle sets the exhaust speed's range, and so the jet power a thrust costs and the propellant it burns."
    enum: [fusion_pulse, fusion_thermal, nuclear_thermal, chemical, cold_gas]
    x-values:
      fusion_pulse: "The fusion products themselves are the exhaust, about 10,000 km/s: a torch. Terawatts of jet for a meganewton; deep space only; at ship scale not offered (Daedalus-class stages run to a thousand tonnes)."
      fusion_thermal: "Fusion heats hydrogen propellant thrown through a magnetic nozzle, about 350 km/s (Discovery II): gigawatts for tens of kilonewtons, hundreds of tonnes of engine. The cruise engine of a large ship."
      nuclear_thermal: "A fission reactor heats hydrogen, about 9 km/s: hundreds of kilonewtons from tens of tonnes; gigawatt jets a pad must stand."
      chemical: "Fuel burnt with its oxidiser, 3.5 to 4.5 km/s: meganewtons from a tonne of engine; the lander's engine, paying in propellant (a tonne a second for a heavy ship)."
      cold_gas: "Stored gas let out, about 0.7 km/s: newtons to kilonewtons, no flame, no heat; for attitude and docking."
'''
    edit("standards/dictionary.schema.yaml", dic)
    def eq(s):
        if "const: engine, x-in-game" in s: return s
        new = '''      - description: "One engine unit (SFO 22): a jet of a given cycle, pointed where the design points it. Replaces the drive, lift and thruster families as roles a design gives engines. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, cycle, thrust, exhaust, efficiency, burns, throttle, gimbal, isotropic_loss, shield_pass, heat_to_hull]
        properties:
          kind: { const: engine, x-in-game: "not made" }
          cycle: { $ref: "../../dictionary.schema.yaml#/definitions/engine_cycle" }
          thrust: { type: number, x-unit: "N", description: "N at full throttle" }
          exhaust: { type: number, x-unit: "m/s", description: "m/s, how fast the jet leaves; propellant flow is thrust / exhaust, jet power half their product" }
          efficiency: { type: number, x-unit: "1", description: "the share of the energy released (the fuel's) that goes into the jet; the rest leaves as heat and radiation" }
          burns: { type: string, x-ref: [material], description: "What releases the energy, by its key: the fusion fuel, the fission fuel's stand-in, the chemical propellant blend, or the gas itself." }
          propellant: { type: string, x-ref: [material], description: "The reaction mass thrown, where it is not the fuel: hydrogen for a fusion thermal or nuclear thermal engine." }
          throttle: { type: number, x-unit: "1", minimum: 0, maximum: 1, description: "the least thrust it holds steadily, as a share of full; 0: on or off only (pulsed)" }
          gimbal: { type: number, x-unit: "rad", description: "rad either way it can point its jet from its axis on its own gimbal; 0: fixed" }
          isotropic_loss: { type: number, x-unit: "1", description: "the share of the energy released that leaves the reaction in every direction (neutrons, X-rays, gamma), not with the jet: what a hull in its way takes" }
          shield_pass: { type: number, x-unit: "1", description: "the share of that loss its own shadow shield lets through toward the ship" }
          reaction_offset: { type: number, x-unit: "m", description: "m from the mount's face, aft, to where the loss originates; the hull's share of the sphere is reckoned from there" }
          heat_to_hull: { type: number, x-unit: "1", description: "the share of the jet's power that comes aboard as heat, for the budgets: isotropic_loss / efficiency x the hull's share of the sphere x shield_pass, plus the engine's own soak-back. The record's figure takes the hull's share as 0.05 (a hull-mounted engine); a design computes its own from its geometry and uses that instead." }
      - description: "A swivel that turns an engine unit through a large angle (vectored thrust): aft in flight, down to land. Far heavier than a gimbal; that is the trade. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, turns, bears, slew]
        properties:
          kind: { const: swivel, x-in-game: "not made" }
          turns: { type: number, x-unit: "rad", description: "rad it turns through" }
          bears: { type: number, x-unit: "N", description: "N of thrust it carries through its bearing" }
          slew: { type: number, x-unit: "rad/s", description: "rad/s it turns at under load" }
'''
        return s.replace("          kind: { const: throat_coil, x-in-game: \"not made\" }\n", "          kind: { const: throat_coil, x-in-game: \"not made\" }\n" + new, 1)
    edit(S + "schema/equipment.schema.yaml", eq)


def part(folder, code, n, name, desc, mass, item, module, L, W, H, whole, share):
    os.makedirs(P + folder, exist_ok=True)
    f = P + folder + f"/{code}-{n:02d}.yaml"
    if os.path.exists(f): return
    cut = 1.15 if module in ("module.welding-bay", "module.machining-centre", "module.cutting-table") else 1.0
    open(f, "w", encoding="utf-8").write(f'''# yaml-language-server: $schema=../../../schema/part.schema.yaml
identity:
  key: part.{code.lower()}-{n:02d}
  code: {code}-{n:02d}
  name: {q(name)}
  revision: draft
  description: {q(desc)}
physical:
  mass: {mass:g}
  length: {L:.3g}
  width: {W:.3g}
  height: {H:.3g}
made_from:
  - item: {item}
    quantity: {mass * cut:.4g}
making:
  module: {module}
fit:
  count: 1
basis:
  - {{ of: [physical.mass, fit], tier: invented, review: true, note: {q(f"Its share of the {whole} ({share:.0%}), chosen.")} }}
  - {{ of: [made_from, making], rule: {"rule.cut-loss" if cut > 1 else "rule.built-in-whole"} }}
  - {{ of: [physical.length, physical.width, physical.height], rule: rule.fitted-within }}
''')


def equipment(slug, key, name, slot, cls, mass, L, W, H, power, function, parts, code, desc, notes, maker="org.kestrel"):
    f = E + slug + ".yaml"
    for n, (pn, pd, share, item, mod) in enumerate(parts, 1):
        k = share ** (1 / 3)
        part(slug, code, n, pn, pd, max(1, round(mass * share)), item, mod, L * k, W * k, H * k, name, share)
    if os.path.exists(f): return
    def val(v):
        if isinstance(v, bool): return str(v).lower()
        if isinstance(v, str): return q(v) if (" " in v or ":" in v) else v
        return f"{v:g}" if isinstance(v, float) else str(v)
    fn = "\n".join(f"  {k}: {val(v)}" for k, v in function.items())
    needs = f"needs:\n  power: {power:g}\n" if power else ""
    basis = "\n".join(f"  - {{ of: {of}, tier: {tier}, review: true{', source: ' + q(src) if src else ''}, note: {q(note)} }}" for of, tier, src, note in notes)
    open(f, "w", encoding="utf-8").write(f'''# yaml-language-server: $schema=../../schema/equipment.schema.yaml
identity:
  key: {key}
  name: {q(name)}
  maker: {maker}
  revision: draft
  slot: {slot}
  description: {q(desc)}
size_class: {cls}
physical:
  mass: {mass:g}
  volume: {L * W * H:.3g}
  length: {L:g}
  width: {W:g}
  height: {H:g}
{needs}built_of:
  parts: {slug}
making:
  module: module.assembly-shop
function:
{fn}
life: {LIFE}
basis:
{basis}
  - {{ of: [built_of, making], rule: rule.first-design-parts }}
  - {{ of: [life], rule: rule.life-ship-system }}
''')


AL = "stock.al6061-pl-5"; TI = "stock.ti64-pl-5"; EL = "good.electronics"; MLI = "stock.mli-fl"; PU = "good.pumps"; ST = "stock.st304-pl-3"; CU = "stock.cu-wire-2"; SIC = "stock.sic-tl-10"; C = "good.carbon"; MOT = "good.electric-motors"; HP = "good.heat-pumps"
WB, MC, AS = "module.welding-bay", "module.machining-centre", "module.assembly-shop"


def h2h(iso, eff, shield, soak=1e-4):
    return float(f"{iso / eff * OMEGA * shield + soak:.3g}")


def engines():
    # Fusion thermal: Discovery II anchors the S3; the smaller two scale down at its specific power (optimistic: the one small design, DFD, is ten times heavier for its jet).
    FT = [(1, 1000.0, 30000, 6.0, 3.0, 3.0, "FTE1", "a tenth of a Discovery II's jet in a thirtieth of its mass: optimistic scaling (the Princeton Direct Fusion Drive, 2 MW, is 2.8 t: 0.7 kW/kg); review"),
          (2, 5000.0, 90000, 10.0, 4.0, 4.0, "FTE2", "scaled from Discovery II at a little under its specific power; review"),
          (3, 27800.0, 361000, 24.0, 8.0, 8.0, "FTE3", "Discovery II itself: reactor 310 t (first wall 14, coils 206, shield 90), nozzle 3, power conversion 30, coolant 11, heating 5, refrigeration 2")]
    for cls, thrust, mass, L, W, H, code, scale in FT:
        jet = 0.5 * thrust * 347000
        equipment(f"engine-ft-s{cls}", f"equipment.engine.ft.s{cls}", f"Fusion thermal engine S{cls}", "engine", cls, mass, L, W, H, 0,
                  {"kind": "engine", "cycle": "fusion_thermal", "thrust": thrust, "exhaust": 347000, "efficiency": 0.61, "burns": "material.deuterium", "propellant": "material.hydrogen", "throttle": 0.2, "gimbal": 0.05, "isotropic_loss": 0.5, "shield_pass": 0.01, "reaction_offset": L * 0.3, "heat_to_hull": h2h(0.5, 0.61, 0.01)},
                  [("Reactor coils and cases", "The toroidal and poloidal coils in their titanium strengtheners.", 0.55, TI, WB), ("Neutron and radiation shield", "Carbon-graphite round the coils.", 0.25, C, AS), ("First wall and nozzle", "Silicon carbide first wall; the magnetic nozzle.", 0.05, SIC, MC), ("Power conversion and heating", "", 0.09, EL, AS), ("Coolant and refrigeration", "", 0.04, PU, AS), ("Propellant feed", "", 0.02, ST, MC)], code,
                  f"A D-He3 fusion reactor heating hydrogen through a magnetic nozzle: {thrust / 1000:g} kN at 347 km/s, {jet / 1e9:.2g} GW of jet. A cruise engine: {thrust / 1000:g} kN moves a 156 t ship at {thrust / 156000:.2g} m/s2. Hydrogen is its propellant, D-He3 its fuel.",
                  [("[function.exhaust, function.efficiency, function.thrust]", "sourced", SRC, "Discovery II (NASA/TM-2005-213559): exhaust 347 km/s at the nozzle exit, 4.83 GW of jet from 7.9 GW of fusion (0.61), 6,250 lbf = 27.8 kN, 0.08 kg/s. The S3 is that engine; the others scale."),
                   ("[physical]", "derived", SRC, scale),
                   ("[function.isotropic_loss, function.shield_pass, function.heat_to_hull]", "invented", None, f"D-He3 with side D-D reactions: about 5% of the fusion power in neutrons and about 20% in X-rays leave the plasma in every direction; a half-metre carbon-graphite shadow shield (Discovery II's 90 t) passes about a hundredth toward the ship. heat_to_hull = 0.25 / 0.61 x 0.05 x 0.01 + 1e-4 soak-back = {h2h(0.25, 0.61, 0.01):g} of the jet, for a hull taking a twentieth of the sphere: {jet * h2h(0.25, 0.61, 0.01) / 1e6:.2g} MW aboard at full thrust. From memory of the physics; the figures want a source. Review."),
                   ("[function.throttle, function.gimbal]", "invented", None, "Deep throttling by fuel rate (a fifth); a magnetic nozzle steers the jet a few degrees. Chosen.")])
    # Nuclear thermal: NERVA-class, from memory.
    NT = [(1, 25000.0, 3500, 3.0, 1.2, 1.2, "NTE1"), (2, 110000.0, 9000, 5.0, 1.8, 1.8, "NTE2"), (3, 330000.0, 18000, 7.0, 2.5, 2.5, "NTE3")]
    for cls, thrust, mass, L, W, H, code in NT:
        jet = 0.5 * thrust * 9000
        equipment(f"engine-nt-s{cls}", f"equipment.engine.nt.s{cls}", f"Nuclear thermal engine S{cls}", "engine", cls, mass, L, W, H, 0,
                  {"kind": "engine", "cycle": "nuclear_thermal", "thrust": thrust, "exhaust": 9000, "efficiency": 0.95, "burns": "material.hydrogen", "propellant": "material.hydrogen", "throttle": 0.3, "gimbal": 0.1, "isotropic_loss": 0.05, "shield_pass": 0.01, "reaction_offset": L * 0.4, "heat_to_hull": h2h(0.05, 0.95, 0.01)},
                  [("Reactor core and vessel", "Fuel elements in their pressure vessel.", 0.4, TI, WB), ("Shadow shield", "", 0.25, C, AS), ("Nozzle", "Regeneratively cooled.", 0.15, ST, WB), ("Turbopump and feed", "", 0.12, PU, AS), ("Controls", "", 0.08, EL, AS)], code,
                  f"A fission reactor heating hydrogen: {thrust / 1000:g} kN at 9 km/s, {jet / 1e9:.2g} GW of jet, {thrust / 9000:.3g} kg/s of hydrogen. Hovering a 156 t ship at 1 g takes {1.53e6 / thrust:.1f} of these. The fuel is the reactor's; the game draws the hydrogen.",
                  [("[function.exhaust, function.thrust, physical.mass]", "invented", None, "NERVA-class, from memory: the NERVA XE gave about 250 kN at 7 km/s from about 18 t with its shield; 9 km/s is the advanced solid-core figure (Isp about 900 s). The classes scale by thrust. No source looked up. Review."),
                   ("[function.isotropic_loss, function.shield_pass, function.heat_to_hull]", "invented", None, f"About a twentieth of the reactor's power leaks as neutrons and gamma in every direction; a shadow shield passes a hundredth toward the ship: {h2h(0.05, 0.95, 0.01):g} of the jet aboard at a twentieth of the sphere. From memory; review."),
                   ("[function.throttle, function.gimbal]", "invented", None, "A solid core throttles to about a third; a gimballed nozzle gives about six degrees. Chosen.")])
    # Chemical: RL10 (sourced) and Raptor-class (from memory).
    CH = [(1, "hydrolox", 25000.0, 4400, 80, 1.2, 0.6, 0.6, 0.2, 0.07, "CHE1", "scaled from the RL10 by thrust; review"),
          (2, "hydrolox", 110000.0, 4400, 300, 2.3, 1.5, 1.5, 0.2, 0.07, "CHE2", "the RL10: about 110 kN, 300 kg, hydrogen and oxygen; its extendable nozzle is carbon-carbon"),
          (3, "methalox", 500000.0, 3600, 600, 2.5, 1.2, 1.2, 0.4, 0.26, "CHE3", "between the RL10 and a Raptor by thrust, at a Raptor's weight per thrust; review"),
          (4, "methalox", 2300000.0, 3600, 1630, 3.1, 1.3, 1.3, 0.4, 0.26, "CHE4", "a Raptor 2 class engine, from memory: about 230 tf, 1,630 kg, methane and oxygen, throttling to about 40%, gimbal about 15 degrees. No source looked up. Review")]
    for cls, prop, thrust, ve, mass, L, W, H, thr, gim, code, scale in CH:
        jet = 0.5 * thrust * ve; energy = {"hydrolox": 13e6, "methalox": 10e6}[prop]; eff = float(f"{0.5 * ve * ve / energy:.2g}")
        equipment(f"engine-ch-s{cls}", f"equipment.engine.ch.s{cls}", f"Chemical engine S{cls} ({prop})", "engine", cls, mass, L, W, H, 0,
                  {"kind": "engine", "cycle": "chemical", "thrust": thrust, "exhaust": ve, "efficiency": eff, "burns": f"material.{prop}", "throttle": thr, "gimbal": gim, "isotropic_loss": 0.0, "shield_pass": 1.0, "reaction_offset": L * 0.5, "heat_to_hull": 1e-4},
                  [("Thrust chamber and nozzle", "Regeneratively cooled.", 0.45, ST, WB), ("Turbopumps", "", 0.3, PU, AS), ("Valves, lines and injector", "", 0.15, ST, MC), ("Controller and gimbal actuators", "", 0.1, MOT, AS)], code,
                  f"{prop.capitalize()} burnt and expanded: {thrust / 1000:g} kN at {ve / 1000:g} km/s, {jet / 1e9:.2g} GW of jet, {thrust / ve:,.0f} kg/s of propellant. Hovering a 156 t ship at 1 g takes {1.53e6 / thrust:.1f} of these and {1.53e6 / ve:,.0f} kg/s: a 60-second landing burns {1.53e6 / ve * 60 / 1000:,.0f} t. The lander's engine.",
                  [("[function.exhaust, function.thrust, physical.mass, function.throttle, function.gimbal]", "sourced" if cls == 2 else "invented", SRC if cls == 2 else None, scale),
                   ("[function.efficiency]", "derived", None, f"Half the exhaust speed squared ({0.5 * ve * ve / 1e6:.1f} MJ/kg) over the blend's {energy / 1e6:g} MJ/kg: {eff}."),
                   ("[function.heat_to_hull]", "invented", None, "A chemical engine's heat is its own, carried off by its propellant; a ten-thousandth of the jet soaks back into the mount. Chosen.")])
    # Cold gas and small chemical attitude blocks.
    equipment("engine-cg-s1", "equipment.engine.cg.s1", "Cold gas block, 0.5 kN", "engine", 1, 15, 0.4, 0.4, 0.4, 50,
              {"kind": "engine", "cycle": "cold_gas", "thrust": 500.0, "exhaust": 700, "efficiency": 1.0, "burns": "material.nitrogen", "throttle": 0.0, "gimbal": 0.0, "isotropic_loss": 0.0, "shield_pass": 1.0, "reaction_offset": 0.0, "heat_to_hull": 0.0},
              [("Nozzles and valves", "Four nozzles facing out, fast valves.", 0.6, ST, MC), ("Regulator and manifold", "", 0.3, ST, MC), ("Controller", "", 0.1, EL, AS)], "CGE1",
              "Four cold nitrogen nozzles, 0.5 kN in all: attitude and docking with no flame and no heat, at 0.7 km/s. Pulsed, not throttled.",
              [("[function, physical]", "invented", None, "A cold gas thruster block as flown on spacecraft: nitrogen at about 0.7 km/s, pulsed; 15 kg for half a kilonewton of nozzles and valves. From memory; review.")])
    equipment("engine-cg-s2", "equipment.engine.cg.s2", "Cold gas block, 5 kN", "engine", 1, 80, 0.7, 0.7, 0.7, 100,
              {"kind": "engine", "cycle": "cold_gas", "thrust": 5000.0, "exhaust": 700, "efficiency": 1.0, "burns": "material.nitrogen", "throttle": 0.0, "gimbal": 0.0, "isotropic_loss": 0.0, "shield_pass": 1.0, "reaction_offset": 0.0, "heat_to_hull": 0.0},
              [("Nozzles and valves", "", 0.6, ST, MC), ("Regulator and manifold", "", 0.3, ST, MC), ("Controller", "", 0.1, EL, AS)], "CGE2",
              "Four cold nitrogen nozzles, 5 kN in all, for a heavy ship's attitude and docking. 7 kg/s of nitrogen at full.",
              [("[function, physical]", "invented", None, "Scaled from the half-kilonewton block. From memory; review.")])
    equipment("engine-ca-s1", "equipment.engine.ca.s1", "Attitude engine block, 5 kN (methalox)", "engine", 1, 45, 0.6, 0.6, 0.6, 200,
              {"kind": "engine", "cycle": "chemical", "thrust": 5000.0, "exhaust": 3000, "efficiency": 0.45, "burns": "material.methalox", "throttle": 0.0, "gimbal": 0.0, "isotropic_loss": 0.0, "shield_pass": 1.0, "reaction_offset": 0.0, "heat_to_hull": 1e-4},
              [("Thrusters", "Four pressure-fed chambers facing out.", 0.6, ST, MC), ("Valves and manifold", "", 0.3, ST, MC), ("Controller", "", 0.1, EL, AS)], "CAE1",
              "Four small pressure-fed methalox thrusters, 5 kN in all at 3 km/s: ten times the impulse of cold gas for the same propellant, with a flame. Pulsed.",
              [("[function, physical]", "invented", None, "Small pressure-fed bipropellant thrusters, as a spacecraft's reaction control: about 3 km/s, kilonewtons, tens of kilograms. From memory; review.")])
    # Swivels by thrust class.
    for cls, bears, mass, L, W, H, pw, code in [(1, 30000.0, 70, 0.8, 0.8, 0.4, 2000, "SWV1"), (2, 120000.0, 160, 1.2, 1.2, 0.5, 4000, "SWV2"), (3, 500000.0, 450, 1.8, 1.8, 0.7, 12000, "SWV3"), (4, 2500000.0, 1800, 3.0, 3.0, 1.0, 50000, "SWV4")]:
        equipment(f"swivel-s{cls}", f"equipment.swivel.s{cls}", f"Engine swivel S{cls}", "engine", cls, mass, L, W, H, pw,
                  {"kind": "swivel", "turns": 1.571, "bears": bears, "slew": 0.1},
                  [("Trunnion and bearing", "", 0.5, TI, MC), ("Actuators", "", 0.3, MOT, AS), ("Frame", "", 0.2, AL, WB)], code,
                  f"A trunnion that turns an engine of up to {bears / 1000:g} kN through 90 degrees in about 16 s: aft in flight, down to land. Vectored thrust; the price is {mass} kg a unit against a gimbal's nothing.",
                  [("[function, physical, needs]", "invented", None, "A trunnion bearing and two actuators carrying the engine's thrust: about 0.7 kg a kilonewton plus a frame, turning a quarter turn in sixteen seconds. Chosen; no flown example at this scale. Review.")])


def propellants():
    if not os.path.exists(MAT + "nitrogen.yaml"):
        open(MAT + "nitrogen.yaml", "w").write('''# yaml-language-server: $schema=../../schema/material.schema.yaml
identity:
  key: material.nitrogen
  name: "Nitrogen"
  class: fluid
  form:
  - fluid
  description: "Cold gas for attitude thrusters (about 0.7 km/s), kept at 300 bar."
fuel:
  release: none
mass:
  density: 330
basis:
  - { of: [mass.density], tier: invented, review: true, note: "Nitrogen at 300 bar and room temperature is about 330 kg/m3 (from memory of its compressibility); as stored. Review." }
''')
    for code, key, name, mat, note in [("METHALOX-LIQ", "stock.methalox-liq", "Methalox, liquid", "material.methalox", "Methane and oxygen as one stock at 1:3.6 by mass, the engine's ratio; in truth two tanks."),
                                       ("HYDROLOX-LIQ", "stock.hydrolox-liq", "Hydrolox, liquid", "material.hydrolox", "Hydrogen and oxygen as one stock at 1:6 by mass, the engine's ratio; in truth two tanks."),
                                       ("HYDROGEN-LIQ", "stock.hydrogen-liq", "Hydrogen, liquid", "material.hydrogen", "Reaction mass for nuclear thermal and fusion thermal engines, at 20 K."),
                                       ("NITROGEN-GAS", "stock.nitrogen-gas", "Nitrogen, compressed", "material.nitrogen", "Cold gas at 300 bar.")]:
        if os.path.exists(MS + code + ".yaml"): continue
        open(MS + code + ".yaml", "w").write(f'''# yaml-language-server: $schema=../../schema/mill-stock.schema.yaml
identity:
  key: {key}
  traded_as: market.fuel
  code: {code}
  name: {q(name)}
  form: fluid
made_from:
  - item: {mat}
basis:
  - {{ of: [identity], tier: invented, review: true, note: {q(note + " Added 2026-10-06 for the engine family (SFO 22).")} }}
''')
    # tanks: the deuterium tanks' envelopes, filled with each propellant
    for prop, dens, frac, note in [("methalox", 830, 0.06, "a low-pressure liquid tank at 6% of its contents"), ("hydrolox", 320, 0.12, "insulated for liquid hydrogen and oxygen, 12% of its contents"), ("hydrogen", 71, 0.4, "a cryogenic tank at 20 K with its insulation and boil-off plant: 40% of its light contents")]:
        for cls, vol, L in [(1, 49.0, 4.6), (2, 92.0, 5.6)]:
            slug = f"tank-{prop}-s{cls}"; cap = round(vol * dens, -2); mass = round(cap * frac, -1)
            equipment(slug, f"equipment.tank.{prop}.s{cls}", f"{prop.capitalize()} tank S{cls}", "tank", cls, mass, L, L, L, 0,
                      {"kind": "tank", "capacity": cap, "holds": f"material.{prop}"},
                      [("Shell", "", 0.7, AL, WB), ("Insulation", "", 0.2, MLI, AS), ("Fittings and gauging", "", 0.1, ST, MC)], f"TK{ {'methalox': 'ML', 'hydrolox': 'HL', 'hydrogen': 'H2'}[prop] }{cls}",
                      f"A {vol:g} m3 tank holding {cap:,.0f} kg of {prop}.", [("[physical, function]", "invented", None, f"The deuterium tank S{cls}'s volume filled with {prop} at {dens} kg/m3; {note}. Chosen; review.")], maker="org.hadley")
    equipment("tank-nitrogen-s1", "equipment.tank.nitrogen.s1", "Nitrogen bottle S1", "tank", 1, 150, 1.2, 1.0, 1.0, 0,
              {"kind": "tank", "capacity": 300, "holds": "material.nitrogen"},
              [("Composite pressure vessel", "", 0.8, "stock.cfc-pl-3", WB), ("Valves and regulator", "", 0.2, ST, MC)], "TKN1",
              "A composite bottle holding 300 kg of nitrogen at 300 bar: ten minutes of a 0.5 kN block at full.", [("[physical, function]", "invented", None, "A composite overwrapped pressure vessel at about half a kilogram a kilogram of gas. From memory; review.")], maker="org.hadley")
    # the plant
    p = S + "metadata/modules/propellant-plant.yaml"
    if not os.path.exists(p):
        open(p, "w").write('''# yaml-language-server: $schema=../../schema/module.schema.yaml
identity:
  key: module.propellant-plant
  name: "Propellant plant"
  step: "Make propellants"
  description: "Electrolysers, an air separator and liquefiers: water split to hydrogen and oxygen and chilled to liquids, methane and oxygen blended, nitrogen compressed. Wherever there is water, air and power, there is propellant."
physical:
  length: 80
  width: 40
  height: 20
staff:
  - { profession: profession.chemist, count: 1 }
  - { profession: profession.plant-operator, count: 3 }
  - { profession: profession.technician, count: 1 }
recipes:
  - makes: stock.hydrolox-liq
    does: "Water split by electrolysis, the hydrogen and oxygen chilled to liquids and kept at the engine's ratio."
    inputs:
      - { item: good.water, quantity: 1.0, from: place }
    rate: 0.1
    power: 2000000
  - makes: stock.hydrogen-liq
    does: "Water split; the hydrogen chilled to 20 K; the oxygen set aside."
    inputs:
      - { item: good.water, quantity: 9.0, from: place }
    outputs:
      - { item: element.o, quantity: 8.0 }
    rate: 0.02
    power: 4000000
  - makes: stock.methalox-liq
    does: "Methane and oxygen chilled and blended at the engine's ratio."
    inputs:
      - { item: good.methane, quantity: 0.22 }
      - { item: element.o, quantity: 0.78 }
    rate: 0.2
    power: 200000
  - makes: stock.nitrogen-gas
    does: "Nitrogen compressed to 300 bar."
    inputs:
      - { item: element.n, quantity: 1.0 }
    rate: 0.1
    power: 50000
life: 788940000
basis:
  - { of: [recipes.power], tier: invented, review: true, note: "Electrolysis at about 50 kWh a kilogram of hydrogen (180 MJ) plus liquefaction at about 10 kWh: 20 MJ a kilogram of hydrolox (a ninth hydrogen), 200 MJ a kilogram of liquid hydrogen; a megajoule a kilogram to chill methalox; half as much to compress nitrogen. From memory of the industry's figures; review." }
  - { of: [recipes.inputs, recipes.outputs], tier: derived, review: true, note: "By mass: water is a ninth hydrogen; hydrolox is kept at 1:6 (so a kilogram of water's 8 of oxygen outruns the 6 the engine wants, and the surplus is counted in the blend here, a simplification); methalox at 1:3.6; nitrogen as the element." }
  - { of: [physical, recipes.rate, staff], tier: invented, review: true, note: "A port's propellant plant: a tonne of hydrolox in three hours. Chosen." }
  - { of: [life], tier: invented, review: true, note: "Twenty-five years for industrial plant, as plant is depreciated on Earth; then it is rebuilt. Chosen; the user's rule that everything made has a life." }
''')
    # a line wherever the deuterium plant stands, else at the Trethi power station
    hosts = [f for f in glob.glob("standards/LocalAdministration/metadata/administrations/**/facilities/*.yaml", recursive=True) if "module.deuterium-plant" in open(f).read()]
    hosts = hosts or ["standards/LocalAdministration/metadata/administrations/treistun/port-trethi/facilities/trethi-power-station.yaml"]
    for f in hosts:
        s = open(f).read()
        if "module.propellant-plant" in s: continue
        line = "  - makes: stock.hydrolox-liq\n    also: [stock.hydrogen-liq, stock.methalox-liq, stock.nitrogen-gas]\n    modules:\n      - { module: module.propellant-plant, count: 1 }\n"
        if "\nlines:\n" in s:
            i = s.index("\nlines:\n") + len("\nlines:\n"); s = s[:i] + line + s[i:]
        else:
            i = s.index("\nmodules:\n"); s = s[:i] + "\nlines:\n" + line + s[i + 1:]
        if "module.tank-farm" not in s and "\nmodules:\n" in s:
            i = s.index("\nmodules:\n") + len("\nmodules:\n"); s = s[:i] + "  - { module: module.tank-farm, count: 1 }\n" + s[i:]
        yaml.safe_load(s); open(f, "w").write(s)
    return hosts


def standards():
    if not os.path.exists(S + "metadata/0022-engines.yaml"):
        open(S + "metadata/0022-engines.yaml", "w").write('''# yaml-language-server: $schema=../schema/standard.schema.yaml
identity:
  key: standard.sfo.22
title: Engines
parent: standard.sfo.16
purpose: An engine is one unit that makes a jet; a design points it, fixes it, gimbals it or swivels it, and gives it a role (main drive, lift, attitude) that is the design's, not the engine's. Each engine says its cycle, its thrust, its exhaust speed and so its jet power and propellant flow, what it burns and throws, how deep it throttles, how far it steers, and what share of its power comes aboard as heat. These are the figures that decide whether a ship can land, where, and at what price in propellant, and whether a port can stand beside it.
details:
  text:
    - "Jet power is half the thrust times the exhaust speed; propellant flow is thrust over exhaust speed. The same thrust costs a hundred times the power at a hundred times the exhaust speed, and a hundredth of the propellant. That is the whole trade: fusion cruises, chemistry lands."
    - "Cycles (dictionary engine_cycle): fusion pulse (products as exhaust, about 10,000 km/s: terawatts a meganewton, Daedalus-class stages of a thousand tonnes; not offered at ship scale, the old torch records are the game's stand-ins), fusion thermal (hydrogen heated by a D-He3 reactor through a magnetic nozzle, 347 km/s as Discovery II: gigawatts for tens of kilonewtons from hundreds of tonnes of engine), nuclear thermal (hydrogen through a fission core, 9 km/s: hundreds of kilonewtons from tens of tonnes), chemical (3.6 to 4.4 km/s: meganewtons from a tonne), cold gas (0.7 km/s, no flame)."
    - "Heat aboard. What leaves the reaction in every direction (isotropic_loss: neutrons, X-rays, gamma) reaches the hull by the share of the sphere the hull fills from the reaction point, through the engine's own shadow shield (shield_pass). heat_to_hull = isotropic_loss / efficiency x hull share x shield_pass, plus soak-back. The record's figure takes the hull share as 0.05; a design computes its own from the reaction_offset and its geometry, and that is the figure its budget uses. Nothing is let out with the plume that physics does not let out: a chemical engine's loss is zero, a fusion engine's a quarter of its power."
    - "Landing. Hovering a mass m at gravity g takes thrust m g; at exhaust speed v that is m g / v of propellant a second and m g v / 2 of jet power on the pad. A 156 t ship on a 1 g world at 3.6 km/s: 425 kg/s and 2.8 GW; at 9 km/s: 170 kg/s and 6.9 GW; at 347 km/s: 4.4 kg/s and 265 GW, and nothing near it survives. A lander lands on chemistry or a fission core and pays in propellant; a fusion ship stays in orbit and sends a shuttle down (SFO 23 says what a pad stands)."
    - "Steering. A gimbal turns the jet a few degrees on the engine's own mount at no weight; a swivel (kind swivel) turns the engine through a quarter turn and carries its thrust through a bearing, at hundreds of kilograms to tonnes. Which a design uses, and where its engines point in each phase of flight, is the design's; the balance of thrust through the centre of mass, with one engine out, is the design's check."
status: draft
''')
    if not os.path.exists(S + "metadata/0023-landing-pads.yaml"):
        open(S + "metadata/0023-landing-pads.yaml", "w").write('''# yaml-language-server: $schema=../schema/standard.schema.yaml
identity:
  key: standard.sfo.23
title: Landing pads
parent: standard.sfo.3
purpose: A pad is rated by the jet it stands. A ship may set down where the pad's class is at least its landing jet's power, and nowhere else without breaking the port's law. The classes let a port exist beside the ships that land at it, and tell a design what it may land on.
details:
  text:
    - "Class 1, 100 MW: a camp's pad, bare ground or plates; cold gas and small chemical engines; a light lander on a low-gravity world."
    - "Class 2, 1 GW: a port's pad, refractory concrete with a flame trench; a heavy lander on chemistry at a fraction of a g, or a light one at 1 g."
    - "Class 3, 10 GW: a major port's pad, a water deluge and a trench the size of a launch pad's (a Saturn V's first stage put about 45 GW on its pad for a minute); heavy ships on chemistry or a fission core at 1 g."
    - "Beyond class 3 nothing lands: a fusion jet at hundreds of gigawatts stays in orbit. A pad also says the stand-off round it, within which nothing else may be, by its class; and a nuclear thermal exhaust, slightly active, is for class 3 pads that take it, by the port's law."
    - "The classes are chosen (2026-10-06) to give a design something to be sized against; the figures on each port's pads, and what the law does about a breach, are the administration's. Review."
status: draft
''')


if __name__ == "__main__":
    schemas(); engines(); hosts = propellants(); standards()
    subprocess.run(["python3", "tools/standards/mounts.py"], check=True)
    print("propulsion written; propellant plant at", [h.split("/")[-1] for h in hosts])
