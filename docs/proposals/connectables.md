# Everything that connects (proposal)

2026-10-17. The owner: "we need to literally design pipes, cables, wires, tubes, and everything that can connect something ... plugs, receivers, whatever." This is a survey of what joins one thing to another, and how the registry should hold it. It follows the lesson of 2026-10-16: families and preferred sizes, never one model per size.

## The one shape every connection has

Every connection in the world, a flange, a mains plug, a docking collar, a rail coupler, is the same five things:

| Part | What it is | Example |
|---|---|---|
| **Interface** (a standard) | the mating geometry and its rating: size, pressure, temperature, voltage, current, flow, load; its sealing (air, vacuum, cryogenic); how it is made and broken (welded, bolted, quick, automatic) | DN 200 PN 16 flange; 400 V 125 A 3P+E socket; IDSS docking ring |
| **Halves** | the two mating parts: male and female (plug and receiver), or androgynous (both the same) | flange pair; plug and socket; nozzle and receptacle; twistlock and corner casting |
| **Conduit** | what carries the flow between two interfaces | pipe, hose, tube, cable, bus, belt, rail, road |
| **Fittings** | what changes the conduit along its way | bend, branch, reducer, valve, expansion joint, adapter, splice |
| **Support** | what holds the conduit up | pipe stand, cable tray, idler, rail bed, hanger |

SFO 25 already has four of these: connectors, runs, fittings counted by the generator, and supports. It lacks the **interface as its own record**, with gender, make-and-break mode and adapters. That is what lets plugs and receivers mate across vendors.

## The inventory, by what flows

What the registry has today is marked **(have)**. Standards are named where one fixes the interface.

### 1. Liquids and gases
- **Conduits:**
  - rigid pipe: carbon steel, stainless, plastic for water, refractory-lined (have), water-jacketed (have), lagged (have), vacuum-jacketed for cryogens;
  - **tube**: small bore, 6 to 50 mm, for hydraulics, instruments and sample lines;
  - **hose**: flexible, rubber or metal braided, for things that move or are coupled by hand;
  - **duct**: large and low pressure, for gas and air;
  - double-walled pipe for hydrogen on airless ground (have, as double-sealed joints).
- **Permanent joints:** butt weld, socket weld, flanges by DN and PN (EN 1092, ASME B16.5) (have), threaded, grooved couplings, compression fittings for tube.
- **Plugs and receivers (quick, by hand or machine):**
  - cam-lock hose couplings (EN 14420-7);
  - hydraulic quick couplings (ISO 7241 A and B, ISO 16028 flat-face);
  - dry-break couplings, which spill nothing;
  - bayonet couplings for vacuum-jacketed cryogenic lines (the male slides into the female, with a long heat path);
  - pressure refuelling nozzles and receptacles (ISO 45, SAE AS5877): **ships and vehicles refuel through these**;
  - clamp couplings (have) and catch hoods (have).
- **Fittings:** elbow, tee, reducer, cap (have), and these, not yet in the registry:
  - **valves:** gate, ball, butterfly, check, control, relief and safety, slide gate (have, inside the sealed furnace);
  - **expansion joints and bellows**;
  - strainers, meters, steam traps;
  - heat tracing.

### 2. Vacuum
- KF quick flanges (small, clamped), ISO-K (large, clamped), CF metal-sealed (ultra-high vacuum).
- On airless worlds vacuum is the outside: what matters is pressure-to-vacuum joints, double seals and their leak paths (have, in part).

### 3. Bulk solids
- **Conduits:**
  - troughed belts, open (have) and sealed in galleries (have);
  - pipe conveyors: a belt rolled into a tube, closed, which can turn;
  - cleated and sidewall belts for steep climbs;
  - drag and en-masse chain (have);
  - screw conveyors and bucket elevators;
  - **pneumatic conveying**: powder blown or sucked through pipe. Closed and dust-free, but on airless ground the gas must be carried with it;
  - chutes.
- **Joints:** transfer chute and skirt (have), chute flange (have), rotary valve and lock (have, inside units), slide gate.

### 4. Molten metal and slag
- Runners and launders, ladle slide gates and nozzles, tundish shrouds.
- Carried by **ladles, pots and torpedo cars** rather than conduits: these are batch carriers (see 8).

### 5. Power
- **Conduits:**
  - cables, LV, MV and HV, by voltage and area (have; voltages from the IEC 60038 tables);
  - busbars, AC and DC (have);
  - **enclosed bus ducts**;
  - **overhead lines** on poles and pylons, for a works' site and between works;
  - earthing conductors.
- **Feeds to moving things:** **festoons, cable reels and conductor rails**. These are the cranes' and cars' missing feeds: a reel suits long outdoor runs, a conductor rail suits tight spaces.
- **Joints:**
  - terminations (have) and cable joints, or splices;
  - bus joints (have);
  - **industrial plugs and sockets (IEC 60309): 16, 32, 63, 125, 250, 400, 630 and 800 A, up to 1,000 V.** This is the "plug and receiver" a crew or a player uses to hook up a machine;
  - large mining couplers above that.
- Switchgear and transformer bushings are the units' end of it (have, as ports).

### 6. Signals and data
- Instrument and control cable, fibre optic, fieldbus, radio.
- Small connectors: circular industrial ones and data jacks.
- Light in mass, but every unit's controls need them. Coarse first: carry them as control cores bundled with each power cable, and add a separate data network later.

### 7. Heat
- Steam and condensate (have), cooling water loops (have), radiator loops (have).
- Thermal oil, refrigerant lines.
- **Heat pipes**, which carry heat with no pump. Natural for airless and space plant.

### 8. Batch carriers and mechanical couplings
- **Rails:** gauge, rail section, switches, buffers (have, as rails). **Rail couplers:** buffer-and-chain; Janney (US freight, Type E, F and H); Scharfenberg (fully automatic, with air and electric connections in the same head).
- **Roads** (have), with their crossings.
- **Lifting interfaces**, where a crane meets its load:
  - hooks;
  - ladle trunnions and beams (have, in part);
  - lifting lugs and eyes (have, on the ingots' hot tops);
  - tongs (have);
  - spreaders.
- **Container locks:** the ISO 1161 corner casting (female, 180 x 180 x 110 mm) and twistlock (male, turned 90 degrees). This is how stock containers stack and lock to trucks, trains and ships. The registry's crates (have) want this standard.
- Forklift pockets and pallets (have, in part).

### 9. People and pressure volumes
- Airlocks (have, in the crew post), pressurised corridors and galleries (have), hatches and doors (have, for ships).
- **Docking and berthing:** the International Docking System Standard is androgynous, so the same ring sits on both sides, with a soft-capture ring and a hard-capture ring. Ships, stations and pressurised buildings would share one.
- **Suit umbilicals:** air, power, coolant and data in one connector.

### 10. Ships and vehicles to the world
- Docking collars and clamps (have, as ship equipment), refuelling ports (have, as equipment), power umbilicals, cargo ramps and hatches (have), landing pads' service points.
- These want the same interface records as the plants. A ship's refuelling receptacle is the same kind of thing as a plant's hydrogen flange.

### 11. Structural mounting
- Hardpoint mounts (have, SFO 19), plant mounts (have, SFO 24), kit struts and their nodes (have, SFO 13), anchor bolts and base plates (have, inside units).

## Rules, so the inventory stays small

1. **Preferred sizes only.** One short series per interface, with no in-between sizes:
   - pipe: DN 25, 50, 100, 150, 200, 300, 400, 600, 800, 1000, 1200, 2000;
   - pressure: PN 1, 10, 25, 100;
   - voltage: 0.4, 6.6, 33 and 132 kV;
   - plug current: the IEC 60309 series;
   - belt width: 0.65, 0.8, 1.0 and 1.2 m.

   A layout rounds up to the next size, never asks for a new one.
2. **Families, not pieces.** Each conduit, joint and fitting kind is one generator: blender models it once, and every size comes from the record's numbers (section 8, 2026-10-16). Pieces that look alike share a model by `looks_as`.
3. **An interface record names its halves:** male, female or androgynous; how it is made and broken (permanent, bolted, quick by hand, automatic); its rating envelope; what media it takes. Two halves mate only if they are built to the same interface. **Adapters** are records too: a reducer, a plug adapter, KF to CF. So anything joins anything, and the joint costs mass and a fitting.
4. **The game draws the joint, not the unit.** A unit's model carries no stubs. Its ports name the interface, and the game draws the half and the conduit.
5. **Player-facing interfaces first.** Plugs, quick couplings, refuelling nozzles, docking rings and container locks are what a player handles by hand. They need the clearest standards, and their make-and-break is a game action.

## Order

1. **The `interface` record**, and the connector gaining its halves, gender, make-and-break mode and adapters. The existing connectors become interfaces with their halves.
2. **The chain checker** (agreed 2026-10-17): it matches layouts' edge ports by interface and commodity, sizes a chain's counts by rate, and lists unmet inputs, unused outputs and mismatched interfaces.
3. **Fill the gaps in the preferred series, by family**, in this order:
   - valves and expansion joints;
   - hose and the quick couplings;
   - festoons, cable reels and conductor rails, for the cranes and cars;
   - IEC 60309 plugs and sockets;
   - container locks;
   - rail couplers;
   - overhead lines;
   - the docking ring and refuelling interfaces, shared with ships;
   - heat pipes;
   - signal cores.
4. **Carrier ports** for rails and roads at layouts' edges.

Sources for the standards named:
- [IEC 60309 plugs and sockets](https://en.wikipedia.org/wiki/Industrial_and_multiphase_power_plugs_and_sockets)
- [IEC 60038 voltages](https://en.wikipedia.org/wiki/IEC_60038)
- [Cam-lock, EN 14420-7](https://www.pathel.com/wp-content/uploads/2019/06/camlock-technical-data-sheets.pdf)
- [Hydraulic quick couplings, ISO 7241 and 16028](https://www.tubes-international.com/products/hydraulics-and-high-pressure/hydraulic-quick-couplings/)
- [Cryogenic bayonets](https://patents.google.com/patent/US6513837)
- [ISO 45 refuelling](https://webstore.ansi.org/standards/iso/iso451990)
- [Twistlocks, ISO 1161](https://en.wikipedia.org/wiki/Twistlock)
- [Rail couplers](https://en.wikipedia.org/wiki/Railway_coupling)
- [IDSS docking](https://en.wikipedia.org/wiki/International_Docking_System_Standard)
- [Crane electrification](https://www.mazzellacompanies.com/resources/blog/crane-electrification-conductor-bars-cable-festoon-cable-reel)
- [Bulk conveying](https://www.processingmagazine.com/material-handling-dry-wet/conveyors/article/21275068/mechanical-conveying-part-2-six-options-for-mechanically-conveying-powders-and-bulk-solid-materials)
- [Vacuum flanges](https://www.idealvac.com/Adapter-KF-16-to-CF-1-13-in/pp/P101286)
- [Piping components](https://inspenet.com/en/articulo/what-is-a-flange-types-and-applications)
