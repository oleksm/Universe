"""Crewing a ship (the ships session's request of 2026-10-06, docs/ships/crew-request.md on `ships`): the people places.

    python3 tools/standards/crew.py

Idempotent. Adds to the schemas (a `command` slot kind; equipment kinds command_station, berths, galley, head, fire_unit,
pressure_door, suit_locker, altimeter, camera; `persons` and `cooling` on life support; `heat` on a need; `leak_rate` on a
hull), writes the equipment records with their parts (shares of the whole, "Fitted within", as the other equipment's), puts
persons and cooling on the life support units and the people's heat on need.food, then runs mounts.py. Figures are from memory
of the space station's and airliners' practice, marked review, with the source named where there is one. Run from the
repository root.
"""
import glob, os, re, subprocess
import yaml

S = "standards/SFO/"; E = S + "metadata/equipment/"; P = S + "metadata/parts/"
q = lambda s: '"' + s.replace('"', '\\"') + '"'
LIFE = 473364000
LIFE_NOTE = "Fifteen years of service for a ship's system, as a ship's machinery is written off on Earth; then it is replaced, and its stock comes back as scrap. Chosen; the user's rule that everything made has a life."


def edit(path, fn):
    s = open(path, encoding="utf-8").read(); t = fn(s)
    if t != s:
        yaml.safe_load(t); open(path, "w", encoding="utf-8").write(t)


def schemas():
    # the slot kind
    def dic(s):
        if "command," in s or ", command" in s: return s
        s = s.replace("enum: [power, drive, thrusters, lift, tank, cargo, hyperdrive, capacitor, computer, transponder, sensors, comm, life_support, hardpoint, utility, avionics, access, handling, thermal, gear, gate]",
                      "enum: [power, drive, thrusters, lift, tank, cargo, hyperdrive, capacitor, computer, transponder, sensors, comm, life_support, hardpoint, utility, avionics, access, handling, thermal, gear, gate, command]")
        return s.replace('      gate: "A gate ring\'s, not a hull\'s."\n', '      gate: "A gate ring\'s, not a hull\'s."\n      command: "The command station\'s: where the ship is flown from; on the bridge, inside the pressurised space."\n')
    edit("standards/dictionary.schema.yaml", dic)
    # the equipment kinds
    def eq(s):
        if "const: command_station" in s: return s
        new = '''      - description: "Where the ship is flown from: seats, controls and displays for its pilots. On the bridge, inside the pressurised space. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, persons, g_rating, facing]
        properties:
          kind: { const: command_station, x-in-game: "not made" }
          persons: { type: integer, x-unit: "1", description: "how many it seats, at the controls" }
          g_rating: { type: number, x-unit: "m/s2", description: "m/s2, the most acceleration its seats are rated to hold a person through" }
          facing: { enum: [thrust, forward, any], description: "which way the seats face. thrust: backs to the main drive, so thrust pushes eyeballs-in (the usual). forward: along the ship's nose. any: they turn." }
      - description: "Where the crew sleep: bunks, lockers and the partition round them. In a cargo slot, as a cabin is. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, persons]
        properties:
          kind: { const: berths, x-in-game: "not made" }
          persons: { type: integer, x-unit: "1", description: "how many it sleeps" }
      - description: "Where food is kept cold, heated and eaten. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, persons]
        properties:
          kind: { const: galley, x-in-game: "not made" }
          persons: { type: integer, x-unit: "1", description: "how many it feeds, in turns" }
      - description: "Toilet and washing. Its water is the people's (need.washing); it sends the waste water to life support or the waste tank. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, persons]
        properties:
          kind: { const: head, x-in-game: "not made" }
          persons: { type: integer, x-unit: "1", description: "how many it serves" }
      - description: "Fire detection and suppression for a pressurised room: detectors and an agent bottle. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, protects]
        properties:
          kind: { const: fire_unit, x-in-game: "not made" }
          protects: { type: number, x-unit: "m3", description: "m3 of room one unit covers" }
      - description: "A hatch that holds cabin pressure across it, so a hole vents one compartment and not the ship. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, passage, pressure]
        properties:
          kind: { const: pressure_door, x-in-game: "not made" }
          passage: { type: number, x-unit: "m", description: "m, the clear width of its opening" }
          pressure: { type: number, x-unit: "Pa", description: "Pa it holds across it, either way" }
      - description: "Pressure suits and the locker they hang in, with their air. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, persons, hours]
        properties:
          kind: { const: suit_locker, x-in-game: "not made" }
          persons: { type: integer, x-unit: "1", description: "how many suits" }
          hours: { type: number, x-unit: "s", description: "s of air and power a suit carries" }
      - description: "A landing sensor: it measures the distance to the ground below. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, range, accuracy]
        properties:
          kind: { const: altimeter, x-in-game: "not made" }
          range: { type: number, x-unit: "m", description: "m, the farthest ground it reads" }
          accuracy: { type: number, x-unit: "m", description: "m, how closely" }
      - description: "A camera the pilot sees by where there is no window: under the ship for the pads, aft for docking. Not in the game yet."
        type: object
        additionalProperties: false
        required: [kind, field_of_view]
        properties:
          kind: { const: camera, x-in-game: "not made" }
          field_of_view: { type: number, x-unit: "rad", description: "rad across its picture" }
'''
        s = s.replace("          kind: { const: throat_coil, x-in-game: \"not made\" }\n", "          kind: { const: throat_coil, x-in-game: \"not made\" }\n" + new, 1)
        s = s.replace('          air_recovery: { type: number, x-unit: "1", description: "the share of the oxygen people breathe out as carbon dioxide it wins back; the rest is made up from the air store. The ISS recovers about 0.5." }\n',
                      '          air_recovery: { type: number, x-unit: "1", description: "the share of the oxygen people breathe out as carbon dioxide it wins back; the rest is made up from the air store. The ISS recovers about 0.5." }\n'
                      '          persons: { type: integer, x-unit: "1", description: "how many people it keeps: air, water and warmth for this many" }\n'
                      '          cooling: { type: number, x-unit: "W", description: "W of the cabin\'s heat it carries away to the ship\'s heat path: the people\'s (need.food.heat a head) and their equipment\'s; it also dries the air" }\n')
        s = s.replace('          holds: { type: string, description: "What it is built to hold, by its key (a good or an element); a cargo store takes any of its kind." }',
                      '          holds: { type: string, description: "What it is built to hold, by its key: a good, an element, or a market category (a food store holds market.food: any food stock)." }')
        return s
    edit(S + "schema/equipment.schema.yaml", eq)
    edit("standards/People/schema/need.schema.yaml", lambda s: s if "  heat:" in s else s.replace(
        '  power: { type: number, x-unit: "W", description: "W a person draws for it, on average" }\n',
        '  power: { type: number, x-unit: "W", description: "W a person draws for it, on average" }\n  heat: { type: number, x-unit: "W", description: "W a person gives off in meeting it: the food eaten leaves as heat. What a sealed place must carry away for each person." }\n'))
    edit(S + "schema/hull.schema.yaml", lambda s: s if "leak_rate" in s else s.replace(
        '      cabin_pressure: { type: number, x-unit: "Pa", description: "Pa, of the air its pressurised parts hold against vacuum" }\n',
        '      cabin_pressure: { type: number, x-unit: "Pa", description: "Pa, of the air its pressurised parts hold against vacuum" }\n'
        '      leak_rate: { type: number, x-unit: "kg/s/m3", description: "kg/s of air lost for each m3 of pressurised volume, through seals and the wall, by design. The space station leaks about 0.27 kg a day over its 916 m3: 3.4e-9 kg/s/m3 (from memory of NASA\'s figures; its 2019 onward Zvezda leak was several times that). Review." }\n'))


def part(folder, code, n, name, desc, mass, item, module, L, W, H, whole, share):
    os.makedirs(P + folder, exist_ok=True)
    f = P + folder + f"/{code}-{n:02d}.yaml"
    if os.path.exists(f): return
    cut = {"module.welding-bay": 1.15, "module.machining-centre": 1.15, "module.cutting-table": 1.15}.get(module, 1.0)
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
  - {{ of: [made_from, making], tier: invented, review: true, note: {q("Taken as cut from this stock with 15% lost." if cut > 1 else "Bought made, as the good; put together here.")} }}
  - {{ of: [physical.length, physical.width, physical.height], tier: derived, review: true, note: {q(f"Fitted within the {whole}: its share of the room by weight, in the same proportions, so that the parts fit the whole. Its own shape is not drawn.")} }}
''')


def equipment(slug, key, name, slot, cls, mass, L, W, H, power, function, parts, code, desc, note, module="module.assembly-shop"):
    """One piece with its parts: `parts` = [(name, description, share, item, making module)]."""
    f = E + slug + ".yaml"
    whole = name
    for n, (pn, pd, share, item, mod) in enumerate(parts, 1):
        k = (share) ** (1 / 3)
        part(slug, code, n, pn, pd, round(mass * share), item, mod, L * k, W * k, H * k, whole, share)
    if os.path.exists(f): return
    fn = "\n".join(f"  {k}: {v if not isinstance(v, str) else q(v) if ' ' in v else v}" for k, v in function.items())
    needs = f"needs:\n  power: {power:g}\n" if power else ""
    open(f, "w", encoding="utf-8").write(f'''# yaml-language-server: $schema=../../schema/equipment.schema.yaml
identity:
  key: {key}
  name: {q(name)}
  maker: org.hadley
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
  module: {module}
function:
{fn}
life: {LIFE}
basis:
  - {{ of: [physical, needs, function, size_class], tier: invented, review: true, note: {q(note)} }}
  - {{ of: [built_of, making], tier: invented, review: true, note: "A first design in a few parts, their shares chosen." }}
  - {{ of: [life], tier: invented, review: true, note: {q(LIFE_NOTE)} }}
''')


AL = "stock.al6061-pl-5"; SH = "stock.al6061-sh-2-panel"; EL = "good.electronics"; MLI = "stock.mli-fl"; HP = "good.heat-pumps"; MOT = "good.electric-motors"; PU = "good.pumps"; ST = "stock.st304-pl-3"
WB, MC, AS, CT = "module.welding-bay", "module.machining-centre", "module.assembly-shop", "module.cutting-table"


def records():
    G = 9.80665
    # 2. the command station
    for cls, slug, name, mass, L, W, H, pw, persons, g, code, note in [
        (1, "command-s1", "Pilot's station", 180, 1.2, 1.0, 1.5, 1500, 1, 9 * G, "CMD1", "One pilot: a seat rated to 9 g eyeballs-in (a fighter's seat and harness, 40 kg), a console of controls (60 kg) and its displays, computers and frame (80 kg); 1.5 kW of displays and computers. An airliner's flight deck seat is about 35 kg. From memory; chosen."),
        (2, "command-s2", "Pilot and co-pilot's station", 340, 2.2, 1.2, 1.5, 2500, 2, 9 * G, "CMD2", "Two seats side by side, each as the pilot's station, with a shared console between (an airliner's pedestal). From memory; chosen."),
        (3, "command-s3", "Bridge of four", 900, 3.5, 3.0, 2.2, 6000, 4, 6 * G, "CMD3", "Four stations in a room: pilot, co-pilot, engineer, navigator; seats rated to 6 g (a transport's, not a fighter's), consoles for each and a shared display wall. From memory; chosen.")]:
        equipment(slug, f"equipment.command.s{cls}", name, "command", cls, mass, L, W, H, pw, {"kind": "command_station", "persons": persons, "g_rating": round(g, 1), "facing": "thrust"},
                  [("Seats and harnesses", "Rated to the station's g.", 0.22, AL, MC), ("Consoles and frame", "Controls, pedestal and the structure they stand on.", 0.33, AL, MC), ("Displays and computers", "", 0.35, EL, AS), ("Wiring and linings", "", 0.10, MLI, AS)], code,
                  "Where the ship is flown from: seats, controls and displays for its pilots. On the bridge, inside the pressurised space.", note)
    # 3. berths, galley, head, food store
    for persons, mass, L, W, H in [(2, 260, 2.0, 1.0, 2.0), (4, 520, 2.0, 2.0, 2.0), (6, 780, 3.0, 2.0, 2.0)]:
        equipment(f"berths-{persons}", f"equipment.berths.{persons}", f"Crew berths {persons}", "cargo", 1 if persons < 6 else 2, mass, L, W, H, 50 * persons, {"kind": "berths", "persons": persons},
                  [("Bunks and lockers", "A bunk, a locker and a reading light a head.", 0.45, AL, MC), ("Partition and door", "The wall round them.", 0.40, SH, WB), ("Linings, bedding, lights", "", 0.15, MLI, AS)], f"BRT{persons}",
                  f"Where {persons} of the crew sleep: bunks, lockers and the partition round them. In a cargo slot, as a cabin is.",
                  f"130 kg a berth: a bunk and locker (40 kg, a ship's), its share of partition (60 kg) and fittings (30 kg); 1 m2 of deck a bunk at two high, 50 W a head of light. From memory of a ship's crew cabin; chosen.")
    equipment("galley", "equipment.galley.s1", "Galley", "cargo", 1, 250, 1.5, 0.9, 2.0, 3000, {"kind": "galley", "persons": 6},
              [("Cabinet and worktop", "", 0.40, AL, MC), ("Cold store and ovens", "A refrigerator and two heaters.", 0.45, HP, AS), ("Water fittings", "A tap, a drain to the waste water.", 0.15, ST, MC)], "GAL1",
              "Where food is kept cold, heated and eaten, for a crew of six in turns.", "An airliner's galley unit: about 250 kg fitted, 3 kW of ovens and a chiller (from memory of a galley insert's weights). Feeds six in turns; two for a larger crew. Chosen.")
    equipment("head", "equipment.head.s1", "Head", "cargo", 1, 200, 1.2, 1.0, 2.0, 500, {"kind": "head", "persons": 6},
              [("Compartment and door", "", 0.45, SH, WB), ("Toilet and basin", "A space toilet with its fans and separator; a basin.", 0.40, PU, AS), ("Fittings and linings", "", 0.15, ST, MC)], "HED1",
              "Toilet and washing for a crew of six. Its water is the people's need; its waste water goes to life support's recovery or the waste tank.",
              "A compartment of 1.2 by 1.0 m with a space toilet (the station's is about 100 kg with its fans and separator; from memory) and a basin; 500 W of fans and heating. Serves six; one more for every six. Chosen.")
    equipment("food-store", "equipment.store.food.s1", "Food store", "cargo", 1, 150, 1.5, 1.2, 2.0, 200, {"kind": "store", "capacity": 1000, "holds": "market.food"},
              [("Racks and bins", "Shelving and sealed bins.", 0.6, AL, MC), ("Chilled section", "A chiller for what must be kept cold.", 0.3, HP, AS), ("Fittings", "", 0.1, ST, MC)], "FST1",
              "A larder: racks and sealed bins holding a tonne of food of any kind sold as food (market.food), part of it chilled. 1,000 kg is 667 person-days at need.food's 1.5 kg a day.",
              "A tonne of packed food at about 350 kg/m3 (from memory of packaged provisions) in 3.6 m3 of racks, a tenth of it chilled; the racks and bins 150 kg. Chosen.")
    # 5. landing aids
    equipment("altimeter", "equipment.sensors.altimeter.s1", "Landing altimeter", "sensors", 1, 8, 0.3, 0.2, 0.15, 40, {"kind": "altimeter", "range": 5000, "accuracy": 0.1},
              [("Laser head and optics", "", 0.5, EL, AS), ("Housing", "", 0.3, AL, MC), ("Electronics", "", 0.2, EL, AS)], "ALT1",
              "A laser rangefinder looking down: the ground's distance from 5 km to touchdown, to a tenth of a metre. What the pilot lands by in the last hundred metres; the radar's 500 km is not for that.",
              "A lidar altimeter of a lander's class: 5 km range, 0.1 m accuracy, 8 kg, 40 W (from memory of planetary landers' lidars; a radar altimeter reads to about 0.5 m). Chosen.")
    equipment("camera", "equipment.sensors.camera.s1", "Camera", "sensors", 1, 2, 0.1, 0.1, 0.15, 10, {"kind": "camera", "field_of_view": 1.571},
              [("Lens and sensor", "", 0.6, EL, AS), ("Housing and heater", "", 0.4, AL, MC)], "CAM1",
              "A camera the pilot sees by where there is no window: 90 degrees across, under the ship for the pads or aft for docking. Fit one for each view the pilot needs.",
              "A ruggedised camera head of 2 kg with a heater, 90 degrees across, 10 W. From memory; chosen.")
    # 6. safety
    equipment("fire-unit", "equipment.utility.fire.s1", "Fire unit", "utility", 1, 25, 0.5, 0.3, 0.3, 20, {"kind": "fire_unit", "protects": 50},
              [("Agent bottle and nozzles", "", 0.6, ST, MC), ("Detectors and controller", "Smoke and heat detectors, the valve's electronics.", 0.4, EL, AS)], "FIR1",
              "Smoke and heat detectors and a bottle of agent for one pressurised room of up to 50 m3. One in each pressurised room.",
              "A 10 kg bottle of clean agent (as the station's portable extinguishers, from memory), detectors and the bottle's valve: 25 kg in all, covering 50 m3. Chosen.")
    equipment("pressure-door", "equipment.access.door.s1", "Pressure door", "access", 1, 90, 1.0, 0.1, 1.9, 0, {"kind": "pressure_door", "passage": 0.8, "pressure": 101325},
              [("Door leaf and frame", "", 0.7, AL, MC), ("Seal, latches and window", "", 0.3, ST, MC)], "PDR1",
              "A hatch that holds a cabin's pressure across it, either way, with a window: a hole vents one compartment, not the ship. The studio's doors between compartments are these.",
              "An 0.8 m clear opening in a 1.0 by 1.9 m leaf and frame, 90 kg: by the station's hatches (about 70 to 100 kg, from memory). It holds one atmosphere either way. Chosen.")
    equipment("suit-locker", "equipment.utility.suits.s1", "Suit locker, two suits", "utility", 1, 160, 1.0, 0.6, 2.0, 100, {"kind": "suit_locker", "persons": 2, "hours": 28800},
              [("Two suits with their packs", "Pressure suits with 8 hours of air and power each.", 0.75, MLI, AS), ("Locker", "", 0.25, SH, WB)], "SUT1",
              "Two pressure suits with eight hours of air and power each, and the locker they hang in with their charging. Suits for every one of the crew.",
              "A light EVA suit with its pack at 60 kg (between a 10 kg flight suit and the station's 145 kg suit; from memory) and 8 hours of air (the station's suit's 8.5); the locker 40 kg. Chosen.")


def life_and_needs():
    for slug, persons, note in [("life-s1", 3, "By the space station's weights (231 kg of water plant and 62 kg of oxygen plant a head) 800 kg keeps about three people; it carries their heat and their equipment's, 500 W a head (137 W of a person's heat plus displays, lights and fans), and dries the air."),
                                ("life-s2", None, None)]:
        p = E + slug + ".yaml"; d = yaml.safe_load(open(p)); s = open(p).read()
        if "  persons:" in s: continue
        if persons is None:
            persons = max(1, round(d["physical"]["mass"] / 270))
            note = f"By the space station's weights (about 270 kg of plant a head) {d['physical']['mass']:,} kg keeps about {persons} people; it carries their heat and their equipment's, 500 W a head, and dries the air."
        s = s.replace("  air_recovery: 0.5\n", f"  air_recovery: 0.5\n  persons: {persons}\n  cooling: {persons * 500}\n", 1)
        s = s.rstrip("\n") + f"\n  - {{ of: [function.persons, function.cooling], tier: derived, review: true, note: {q(note)} }}\n"
        yaml.safe_load(s); open(p, "w").write(s)
    p = "standards/People/metadata/needs/food.yaml"; s = open(p).read()
    if "heat:" not in s:
        s = s.replace("lasts: 1814400\n", "lasts: 1814400\nheat: 137\n")
        s = s.rstrip("\n") + '\n  - { of: [heat], tier: sourced, review: true, source: "standards/sources/research_people_needs.json", note: "A crew member\'s metabolic heat: 11.82 MJ a day, 137 W, NASA\'s design figure (Baseline Values and Assumptions). The food eaten leaves as heat; a sealed place carries it away. From memory of the document the file quotes; the figure itself is not in the file: review." }\n'
        yaml.safe_load(s); open(p, "w").write(s)


if __name__ == "__main__":
    schemas(); records(); life_and_needs()
    subprocess.run(["python3", "tools/standards/mounts.py"], check=True)
    print("crewing records written")
