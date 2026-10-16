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
- **`head` and `tail`** (conveyors with `terminals`): rigid end machinery. The head's origin is the run's delivering end on its axis: the drive pulley's centre, the belt wrapping it, its gearbox and motor beside it, reaching no more than 1 m on along +x. The tail's origin is the receiving end: the tail pulley's centre on a take-up frame, reaching back along -x. Exteriors only, masses from the record.
- **`support_head`:** what holds the run at its axis (the origin): a shoe and clamp, a trestle's cross beam, a tray post's arm. A child empty `leg_top` marks its underside, where the leg starts. On a lagged run the straight's jacket is whole and the game cuts nothing: the head brings its own bearing, a low-conductance insert (fused silica) filling the lagging's thickness under the pipe, inside an outer saddle clamped over the jacket; on airless ground this also keeps the hot line's heat from leaking down its supports.
- **`inlet` and `outlet`** (drag conveyors only): the tail piece with a round nozzle on the casing's top, its flange face at the casing's top at the origin, facing +z, of the connector's bore (chute-dn800: 0.8 m). The head piece has the same nozzle on the casing's bottom, facing -z. The casing stays rectangular; the round joint is the nozzle's.
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
