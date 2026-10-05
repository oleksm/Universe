# MC-07: to measure and decide

*A note left 2026-10-04 by the registry session, for when the Blender session is free. Nothing here
is done. The spec is `standards/SFO/metadata/hulls/mc-07.yaml`; the checks are in the registry
page, report "Structure: MC-07".*

## 1. The ore bay's depth (to measure in the model)

The bay is under the two top doors. Its opening is measured from the model: 9.95 by 8.4 m. Its
depth is not: 8 m was chosen, leaving the cargo deck below. The model's inside has never been
measured.

Two figures hang on it, both marked to review:
- `capacity.hold_volume`: 669 m3.
- `capacity.hold`: 1,200,000 kg, that volume full of stony ore at 1.8 t for each m3.

At 5 m deep it carries about 750 t; at 10 m, about 1,500 t.

**Wanted from the model (`assets/models/mc07.glb`, the Blender file it comes from):** the bay's
inside length, width and depth in metres, or its volume; and whether the cargo deck is below it.

## 2. Three sections that buckle under the main drive (to decide)

The rear pod (MC07-19), the connector (MC07-16) and the main hull (MC07-12) carry the main drive's
push forward: 5.4 MN, 8.1 MN with its factor. Each is skinned in 2 mm 6061 sheet.

- Strength is enough: 3.2 to 5.5 times.
- They buckle: 2 mm skin would need a stiffener every 11 to 14 cm, and 30 cm is taken as the
  closest that can be built.

Two ways out, either adding mass that is not counted in the 154 t:
1. Thicker skin on those three.
2. Frames and stringers as parts. The ship has none; the skin carries everything. (The registry
   session's suggestion: this is how a hull carries thrust, and it gives the yard parts to make.)

**Wanted:** the user's choice; and if frames, whether the model is to show them.

Also open: 29 other skin and plate parts have no load case; their 2 mm was chosen.
