# Blender: the plant kit (SFO 25)

2026-10-15, from the registry. The owner: no monolith layout models. A factory is snapped together by the game from records: unit models at their bays, runs laid from kit pieces along their paths, connectors at the ports (docs/proposals/factory-kit-assembly.md). Layout models are no longer made. The 12 installed ones stay as reference pictures.

## 1. Connector models (19, `standards/SFO/metadata/connectors/`)

One model per record, at the record's `physical` size (the installer checks it).
- **Frame:** the joint's mating plane (gasket plane) at the origin, the run's axis along +x. A flange pair is symmetric about x = 0.
- **Node `face`:** at the origin, +x along the axis.
- **What to draw, from each record's `built_of` note:** both flanges with bolts and nuts at the stated count and circle; for a double seal, the leak-off tap. A lined flange shows the lining's joint ring. A chute flange is a plate pair. A belt transfer is the hood and the drop chute. Its origin is the head pulley's place, and the chute runs 3 m down along -z. A termination is the lugs and gland plate at a box face.
- **Size:** low poly, under 12k triangles (a large flange with 60 bolts may need it); small ones far less, terminations a few hundred.

## 2. Run kits (19, `standards/SFO/metadata/runs/`)

One model per run class, holding these named pieces, each in its own frame:
- **`straight`:** 1 m long from x = 0 to x = 1, its section the record's `outside` (width across y, height along z), centred on the axis. The game stretches it along x.
- **`elbow`:** 90° at the record's `elbow.radius` to its centreline. It enters at the origin along +x and leaves along +y. A belt has none.
- **`tee`:** the main along x from -1.5 bore to +1.5 bore, the branch out along +y.
- **`cap`:** a blind end (pipes and ducts), its face at the origin facing +x.
- **Repeated pieces** (a run's `repeat`, belts today): rigid pieces at their own datum (the run's axis at x 0), which the game sets along the straight at the record's pitch and never stretches: a belt's `idler` (the troughing set, every 1.2 m) and `return_roll` (every 3 m). The belt's `straight` then holds only what stretches: the belt's two runs and the stringers.
- **`head` and `tail`** (conveyors with `terminals`): rigid end machinery. The head's origin is the run's delivering end on its axis: the drive pulley's centre, the belt wrapping it, its gearbox and motor beside it, reaching no more than 1 m on along +x. The tail's origin is the receiving end: the tail pulley's centre on a take-up frame, reaching back along -x. Exteriors only, masses from the record. A terminal stands on its own frame: give it child empties `leg_top` under its base rails, and the game hangs a `support_leg` from each to the ground (no support head there, no offset in the record). Each draws its own `transition` (1 m for the belt: the trough flattening into the pulley), and the game stops the straight that far short of the end.
- **Cables:** the run's axis is the cables' centres (three cores laid flat across y). `straight` is the cores and the tray's two side rails (stretched); `rung` is a rigid ladder rung under the cores, repeated every 0.3 m. `elbow` turns in plan (+x to +y) at the record's radius; `riser` turns up (+x to +z), the tray bent on edge. `tee` is a joint box over the tray: the tray's outside width plus 0.2 m across, 0.6 m along, 0.3 m tall, centred on the axis, the branch leaving its +y face. A run of several circuits (`count`) is laid as that many copies side by side, a tray's width apart.
- **`support_head`:** what holds the run at its axis (the origin): a shoe and clamp, a trestle's cross beam, a tray post's arm. A child empty `leg_top` marks its underside, where the leg starts. On a lagged run the straight's jacket is whole and the game cuts nothing: the head brings its own bearing, a low-conductance insert (fused silica) filling the lagging's thickness under the pipe, inside an outer saddle clamped over the jacket; on airless ground this also keeps the hot line's heat from leaking down its supports.
- **Drag conveyors:** their `elbow` turns in the vertical plane (enters along +x, leaves along +z, radius 6 m); no tee, no cap. Their `tail` is the take-up sprocket with the round inlet nozzle on top of the casing, its origin the nozzle's flange face (the casing's centre 0.406 m under it); their `head` is the drive sprocket and motor with the outlet nozzle under the casing, its origin that nozzle's face (the centre 0.406 m over it). Each draws its 1.2 m `transition`; the casing stays rectangular, the round joint is the nozzle's.
- **`support_leg`:** 1 m from the origin down to z = -1. The game places its origin at `leg_top` and stretches it to the ground. Every piece's root sits at identity at its own datum, the pieces overlapping in the file; previews may show them apart.

Draw from each record's `per_m.built_of`, `support` and the class's description:
- pipes: wall, MLI lagging where named;
- lined ducts: shell and lining at the ends;
- belts: belt, troughing idlers every 1.2 m, return rolls, two stringers;
- drag conveyors: casing and insulation;
- cables: three cores on a ladder tray.

Under 12k triangles a kit. The installer requires the named nodes. It can't yet check a straight's section against `outside`, so please state the section in the about file.

**Who draws the ring:** a port's `at` is the mating plane. Unit models end there in a plain stub (the run's `outside` across), with no flange ring. The connector model is the whole joint, and the game draws it at the plane.

## 3. The trial's units: ports and envelopes

The 8 unit kinds in `layout.shaft-furnace-airless` now carry `ports` (id, flow, connector, `at`, `facing` in the unit's frame):
- shaft-furnace-200th-hp
- recuperator
- top-gas-condenser-8bar
- recycle-compressor-8bar
- gas-heater-119mw-8bar
- heater-transformer-131mva
- crew-post
- radiator-field-345k

For each:

a. **Check each installed model against its ports.** Is there a flange, terminal or chute at `at`, facing `facing`, of the connector's size? List the mismatches. Model changes (new unit versions) only where a port is missing or misplaced. Those that changed for the trial:
   - the compressor's cooler flanges moved to its -x face, low (-6, -2.2/-1.6, 0.3), and its cable entry to (0, -1.5, 5);
   - the heater's cable entry moved to the unit's -x face (-12, 1.45, 1.575);
   - the furnace's top gas stub ends at x 6.0, its port moved to match; a 400 V drives' terminal box (0.8 x 0.6 x 1 m, its own part) on the ring girder's west face, its entry at (-4.7, 2, 12);
   - the radiator field has a pump power entry at (-24, 1.8, 0.3);
   - the condenser's water-in is at x -0.6 and its water-out at x 0.6.

b. **Measure an envelope:** a few boxes (6 or fewer), in the unit's frame, that hold its solid parts. The fit checker uses each unit's whole box now, so a pipe under radiator panels, or in a terminal house's notch, looks like a clash. Send them as JSON, `{unit key: [[x0,y0,z0,x1,y1,z1], ...]}`, and the registry will bind them into the unit records.

## Order

1. Ports and envelopes for the 8 units (data, quick).
2. The run kits and connectors that `layout.shaft-furnace-airless` uses.

Install each with `tools/standards/install_model.py` as usual. The kinds `connector` and `run` are now accepted.

## 4. The airless hydrogen plant (2026-10-16)

`layout.hydrogen-plant-airless` is now laid of runs (131 runs, 0 faults). What it needs drawn:

1. **Units:**
   - `electrolyser-block-20mw-10bar` (new): as the 3 bar block's model, with 25 mm separators and its DC terminals at (0, -6, 2.4);
   - `hydrogen-purifier-10bar` (new);
   - `water-treatment-sealed` (new);
   - `rectifier-transformer-21mva` v2: an HV cable box (3 x 0.8 x 0.8 m) over the bushings, its gland plate on top at (0, -1.5, 4.4); the DC terminals become one bus joint at (0, 3, 2.4), the poles' pads at x -0.3 (+) and 0.3 (-).

   Each ends at its ports in plain stubs, as in section 3. Please send envelopes for them as before.
2. **Kit pieces without a model** (15 runs, 15 connectors): the stainless pipes DN 50 to 250 (PN 10, and PN 16 double seal), the loop pipes DN 200, 250, 300 and 600, cables 33 kV 150 mm2 and 400 V 25, 185 and 500 mm2 with their terminations, and the new busbar kit with its joint.

**The busbar** (run kind `busbar`, `run.busbar-dc-44ka`):
- its axis is midway between its two poles across y, + at -y: each pole 0.30 m wide, their centres at y -0.3 and +0.3 (0.3 m clear between them). The units' pads match: centres at x -0.3 (+) and 0.3 (-) on the unit's face;
- each pole is 7 hollow copper tubes 60 x 10 mm in two rows of four, 20 mm apart;
- `straight` is the poles and their clamps;
- `elbow` turns in plan at 0.6 m and `riser` turns up at 0.6 m;
- no tee and no cap;
- `support_head` is a steel cross-arm with two post insulators under the poles; `support_leg` is a steel post.

**The bus joint** (`connector.bus-joint-dc-44ka`): at the plane, each pole's tubes are brazed into a pair of 30 mm copper plates, bolted with sixteen M20, with an insulating hose from each tube's end.

## 5. The airless pellet plant (2026-10-16)

`layout.pellet-plant-airless` is now laid of runs (135 runs, 0 faults). The fit check takes the hall's sections and heads as shells (`shells`, the registry's from their geometry), so their boxes are no longer needed.

**Unit versions** (each to its ports in plain stubs; the records say where):
1. `pressure-hall-30m-end` v2:
   - a second, high nozzle at (7.5, 0, 14.5), 3 m, facing +x;
   - the service panel: a boxed plate whose pass-throughs' outer faces are at x 6.5 and inner faces at x 5.0:
     - water 200 mm at y -10.2 and -9.6, z 10;
     - stainless 50 mm at y -9.0, z 10;
     - cable glands for 6.6 kV at y -10.2 and for 400 V at y -9.4, z 10.8;
     - outer only: nitrogen 50 mm at (6.5, -8.4, 10) and the lock pumps' 400 V at (6.5, -8.6, 10.8).
   - Both nozzles and every pass-through are spares a layout may blind: draw them all open, with stubs.
2. `pressure-hall-30m` v2: the fan coils' 400 V gland at (-12.4, -12), up through the deck.
3. `ore-lock-hopper` v2: its -x face braced round a 3.2 m opening at z 6.9 to 10.1, where the ore gallery leaves (the layout turns the lock 180 degrees).
4. `pellet-gas-loop` v2: a sealed 2 m3 dust bin under each rotary lock, no dust chutes; the cold gas outlet DN600, not 700.
5. `balling-disc-7m5` v2: a small fines elevator on the frame's -x side, from the screen's tray to the cake chute; its 400 V entry at (4.5, 1.0, 0.5), facing +x.
6. `plant-transformer-35mva` v2:
   - an HV cable box on top (3 x 0.8 x 0.8 m, gland plate at (0, -0.9, 5.2));
   - an LV box on its +y face (4 x 0.5 x 1.2 m, gland plate facing +y at (0, 2.35, 2.9));
   - cooler ports at (-2.9, ±0.6, 1), facing -x.
7. `radiator-field-330k` v2: ports to its south-west corner, as the 345 K field's v3.
8. `ball-mill-4m4`: plain stubs at its new ports:
   - the ore hopper at (-8, 0, 4);
   - water 100 mm at (-8, 1.2, 4);
   - slurry 150 mm at (8, 3, 0.5);
   - the 6.6 kV box at (6, -4.4, 0.5).

**Kit pieces without a model** (14 runs, 16 connectors): the 0.8 and 1.0 m belts, open and sealed, with their transfers and chutes (DN400, 600, 1000); pipes DN450, DN700 lagged and DN50 PN16 double, with their flanges; the gallery flange DN3000 PN1; cables 33 kV 185 mm2, 6.6 kV 95 and 400 mm2, 400 V 35, 50, 95 and 120 mm2, with their terminations.

**A sealed belt** (`run.belt-800-sealed`, `-1000-sealed`) is the open belt inside a 3 m gallery tube of 4 mm plate with a ring every 3 m, under foil. The run's axis is the tube's; its outside is 3 x 3 m. Add a `straight` for the tube along with the belt's pieces. Its head and tail are inside sealed boxes the size of the tube.

**The sealed belt's contract** (answering blender, 2026-10-16):
- `straight` stretches: the belt's runs, the stringers and the 4 mm tube with its 25 mm foil, 3.05 m outside.
- `ring` is a rigid repeated piece every 3 m, alongside `idler` and `return_roll`.
- `support_head` is a steel saddle cradling the tube from below, its `leg_top` under it, every 9 m, with no trestle. `support_leg` as usual.
- `head` and `tail` are inside sealed boxes the tube's size, 3 m long, 6 mm plate, with the motor turned along x inside. The head box has an outlet in its floor at its origin; the tail box has an inlet in its roof at its origin. The boxes' masses are in the record's terminals.
- The connector is `connector.belt-transfer-800-sealed` / `-1000-sealed`: a closed chute from the head box's floor (1.525 m under the pulley's centre) down to the port 3 m under it, flanged and sealed at both ends. The hood is the box's. The receiving port (a shaft's charge lock, the mill's hopper) is flanged for it.
- Where a sealed belt's head drops onto another sealed belt, the head box seals onto the lower belt's tail box (inside its 0.5 m drop).

## 6. The airless arc furnace shop (2026-10-16)

`layout.arc-furnace-airless` is now laid of runs (63 runs, 8 layout ports, 0 faults). The gantry crane's legs sweep x -35 to 11 along y ±12, so no run crosses them: the feed belt comes in along x between the legs, and the off-gas duct goes east 19 m up and turns north past the crane's east end.

**A new unit:** `offgas-cooler-sealed` (6 x 4 x 18 m). The sealed furnace's gas is CO alone, 1.37 kg/s, so it gives 2.2 MW, not the 7.2 MW of the old jacketed duct.
- A water-walled shaft 1.6 m across, z 9 to 17, on four box legs (answering blender: the walls start above the outlet).
- A plain collecting section, the shaft's size, 10 mm plate, z 7 to 9. The gas leaves straight out of its side.
- A steam drum 1 m across and 3 m long along y, axis at (-2, 0, 16.5).
- A hopper under the collector, with a dust lock and a closed bin.
- Ports:
  - gas in on top at (0, 0, 18), a 1.2 m lined flange;
  - gas out of the collector's side at (3, 0, 8), facing +x, DN 1200;
  - steam out at (-3, 0.4, 16.5) and water in at (-3, -0.4, 16.5), facing -x;
  - the lock drives' 400 V at (2, -2, 1), facing -y.

**Unit versions** (plain stubs at the ports; the records say where):
1. `eaf-vessel-80t-sealed` v4:
   - the deck manifold's flanges are DN 400, facing up, side by side at (-4.2, 4.95, 0.5) in and (-3.4, 4.95, 0.5) out, 0.8 m apart, in front of the cradle's deck (it tilts, so no opening through it); the manifold is the unit's and reduces from the hoses' 300 mm;
   - the manifold keeps inside local y 5.3. v3 reaches 5.49, and the crane's legs pass at 5.43;
   - the bus joint at the flexible cables' fixed ends, (0, -6.2, 2.3), facing -y;
   - the feed lock's top at (0, 1.8, 10.1);
   - the off-gas reducer's flange at (2, 1.5, 9.7), facing up.
2. `furnace-transformer-90mva` v2:
   - a 33 kV cable box over the HV bushings, 3 x 0.8 x 0.8 m, its gland plate on top at (0, -0.85, 6.8);
   - the secondary pads respaced to the bus joint's 0.6 m, at (0, 2.5, 4), facing +y;
   - the cooler's water at (-3.76, -2.38, 1.35) in and 2.35 out, facing -x, as the v1 pipes met them.
3. `dri-bin-200t`:
   - a roof nozzle at (0, 0, 15.3), facing up, where the hot drag conveyor drops in;
   - a terminal box at (3, 0, 1), facing +x.
   - The layout turns the bin 90 degrees.
4. `offgas-filter-sealed`: a DN 1200 flange on its inlet at (-6, 0, 8), and a 400 V box at (5, 0, 1), facing +x.
5. `radiator-field-485k`:
   - the ports stay at the middle of its -x edge (v1): steam in DN 150 at (-24, -0.6, 1), water out DN 100 at (-24, 0.6, 1);
   - add the pump's box at (-24, -1.4, 0.3);
   - the layout turns the field 180 degrees so they face the plant.

**Kit pieces without a model** (7 runs, 7 connectors):
- the AC busbar, 65 kA: three phases of 11 tubes, 0.6 m apart, `run.busbar-ac-65ka`, with `connector.bus-joint-ac-65ka`;
- the lined duct DN 1200 PN 1 and its lined flange;
- pipes DN 1200 PN 1, DN 400 PN 10, DN 150 PN 25 lagged and DN 100 PN 25, with their flanges;
- the 33 kV 630 mm2 cable and its termination.

## 7. The airless ladle station (2026-10-16)

`layout.ladle-station-airless` is now laid of runs (24 runs, 7 layout ports, 0 faults). The maintenance crane's legs sweep x -29 to 17 along y -11 and 13; nothing crosses them. Its rails moved 1 m north (y -11 and 13), so the alloy bins fit inside its span on a 2.8 m plinth with the stair tower. The filter moved inside the span too, east of the furnace. The cooling mains leave west, past the crane's reach.

**Unit versions** (plain stubs at the ports; the records say where):
1. `ladle-furnace-80t-sealed` v2:
   - the bus joint at the cables' hanger, (0, -5.34, 6), facing -y;
   - the manifold's water, DN 150 facing -y: out at (-4.8, -4.85, 1), in at (-4.2, -4.85, 1);
   - the alloy chute's mouth at (0, 3, 9), facing up: a DN 400 chute flange; the belt's head drops into it through a hood, 0.4 m over;
   - the roof elbow's fume flange, DN 200 stainless, at (2, 0, 7.5), facing up;
   - argon in, DN 50, at (-5.5, -2.5, 1.5) on the west frame, facing -x.
   - Envelope: the registry split your masts box in two from the description. The masts and arms are at y -3.22 to 0.9; the alloy chute alone is x ±0.4, y 0.9 to 3.3, up to z 9. The fume pipe runs north over the roof at x 2, z 9.5. Please confirm on the model.
2. `furnace-transformer-14mva` v2:
   - a 33 kV cable box over the HV bushings, 2 x 0.6 x 0.5 m, its gland plate at (0, -0.45, 3.7);
   - the secondary bus joint at (0, 1.35, 2.13), the phases 0.6 m apart;
   - the cooler's water at (-2.15, -0.3, 1) in and (-2.15, 0.3, 1) out, facing -x.
3. `alloy-bins-8` v2:
   - the transfer chute's foot kicked 1 m to +x at 45 degrees, to (1, 0, 7), DN 400, so the belt passes between the centre and east legs under the deck;
   - its 400 V box at (6.3, 0, 1), facing +x.
4. `degassing-stand`: on the argon cabinet's -x face, argon in DN 50 at (-3.3, -3.1, 0.6) and 400 V at (-3.3, -2.6, 1.2).
5. `offgas-filter-sealed`: a second, small inlet, DN 200 stainless, at (-6, 1.4, 8), facing -x. Both inlets are spares a layout may blind: draw both open.

**Kit pieces without a model** (5 runs, 5 connectors):
- the 0.65 m belt and its transfer;
- the AC busbar, 27 kA, with its joint;
- pipes DN 200 PN 1 and DN 50 PN 16, both stainless, with their flanges;
- the 33 kV 120 mm2 cable and its termination.

## 8. Kit families, not pieces (2026-10-16)

The owner noticed the kit took most of a day: 129 of the 162 models installed on 2026-10-16 were pipes, flanges, cables, terminations, belts and transfers. Each was built by its own copied script, whose numbers differ only where the record already gives them, and each went through its own round of QC, install, validation and two messages. From now on:

1. **One generator per family.** Write one script per family: pipe, flange, lined duct, lined flange, cable, termination, belt, belt transfer, chute, busbar, bus joint. It reads the run or connector record (`outside`, `bore`, `elbow`/`riser` radius, `pitch`, `repeat`, `support`, `physical`, `built_of`) and builds the model from it. A new size is then a run of the script, not a new script.
   - Measure each family's archetype once. Bind a size's masses only where they differ from the formula by more than 5%.
2. **Batches.** Install a family's new sizes together. Send one QC report and one message per batch, listing the keys and their install commits. The registry records the batch's loader validation as one entry per key, in one run and one commit (`validate_model.py <key> <key>...`).
3. **Shared looks.** A kit record whose kind, outside size and material match another's carries `looks_as: <that record's key>`. It is drawn with that model and wants none of its own (next_asset skips it). Mass, rating and connector stay its own. Today every kit record has its own model, so none carries `looks_as` yet; new sizes will, where they match.
4. **Existing models stay.** Nothing is rebuilt. Bring the old pieces under the generators only when a family next changes.

## 9. The arc furnace shop for air (2026-10-16)

`layout.arc-furnace-air` is now laid of runs: 15 runs, 6 layout ports, 0 faults. Its furnace's off-gas is not a flanged joint. A fixed, water-cooled catch hood stands 0.15 m over the tilting furnace's elbow, and the gap draws in the air that burns the CO. The duct after it is water-jacketed, its water brought by pipes that end on the jacket. SFO 25 gains:
- the run kind `jacketed-duct`;
- the connector kind `hood`;
- layout ends `jacket:<run>`.

**Kit, one batch per family** (section 8):
- New family `jacketed-duct`: `run.jacketed-duct-dn2000`. A 2 m inner shell (10 mm), a 100 mm water gap and a 6 mm outer shell, 2.232 m outside, mitred bends one bore in radius. Pipes end on its jacket's underside: draw a stub per `jacket:` end where the layout puts it, as tees are.
- New family `hood`: `connector.hood-dn2000`. A double-walled, water-cooled ring 2.4 m across and 0.5 m deep, its mouth 0.15 m from the port it serves, the way the port faces (never touching it). It is a sleeve on the duct's end, with a 1 m stroke along its axis.
- Flange family: `connector.flange-dn2000-pn1`.
- Nothing else: the 6.6 kV 50 mm2 cable and its termination carry `looks_as` (the 400 V 120 mm2 pieces).

**Unit versions** (plain stubs at the ports; the records say where):
1. `eaf-vessel-80t` v4: the sealed furnace's port changes, without its lock and bellows:
   - the deck manifold's DN 400 barrels at (-4.2, 4.95, 0.5) in and (-3.4, 4.95, 0.5) out;
   - the bus joint at (0, -6.2, 2.3);
   - the off-gas elbow (revised after your tilt sweep, 2026-10-16) turns horizontal from the fourth hole and runs north past the roof's edge to a vertical mouth at (3, 4.5, 8.5), facing +y. The duct's water-cooled sleeve stands 0.15 m beyond it. Before the furnace tilts (arc off, to tap or deslag), the sleeve is drawn back 1 m along +y, parking its mouth in the plane y 5.65. A tilt about y then slides the elbow's mouth across the sleeve's plane, never into it;
   - the fifth hole's mouth at (0, 1.8, 8.3), now a belt's transfer: the belt's head 3 m over it. Before the furnace tilts, its chute telescopes up so its bottom parks at z 10.8;
   - for your diagnostic sweep: the y axis through the rockers' centres (0, 0, 1.0), at rest and +/-15 degrees, with the sleeve and the chute parked;
   - measure its envelope: the registry lends it the sealed furnace's boxes until then.
2. `cooling-tower` v2:
   - flanges on its east end (x 12, z 0.75) facing +x: the hall loop DN 400 out at y -0.6 and in at 0.6, the duct jacket's DN 300 out at 3.9 and in at 5.1;
   - make-up water DN 100 at (-12, 0, 0.75) and a 400 V box at (-12, -3, 1), facing -x.
   - Measure its envelope.
3. `baghouse-800k` v2:
   - a DN 2000 inlet flange at (0, -6, 15), facing -y;
   - a 6.6 kV box at (12, 3, 1), facing +x.
   - Measure its envelope.
4. `hall-crane-20t`: its envelope only.
