# freefall-label/1: the package label

What a works stamps on each package it fills (SFO 21): who, what, where, when, and a barcode. The data is the `label`
shape (standards/common.schema.yaml): game state, stamped at packing. This page is its layout; the reference renderer is
`tools/standards/label_render.py` (`--sample` draws one), and the game prints the same layout onto a package model's
label plates.

## Data

| Field | From |
|---|---|
| brand | the maker (`org.*`): its `brand` block gives the printed name, primary and accent colours, logo (SVG) |
| product | the stock (`stock.*`): its name |
| lot | the batch (`batch.lot`), `TICKER-000000` |
| serial | this package: `TICKER-000000-0000` (lot and a count within it); the barcode carries it |
| made at | the works (`facility.*`, `rig.*`): its name |
| made on | world time the lot was begun, its first output (s); printed as `DAY d HH:MM` |
| packed on | world time it was filled and labelled (s); printed the same way |
| net, gross | kg of contents; kg with the package's tare |
| package | the template (`package.*`): its name, `stack` and `keeps` print as handling marks |
| barcode | Code 128 set B of the serial |

## Layout (2:1, drawn at any size; the reference at 1024 x 512 px)

- Top quarter: a band in the brand's primary colour, the logo at its left (or the ticker on the accent colour when the
  brand has none), the brand name in white.
- Under it: the product in bold; then lot and serial; made at; made and packed; net, gross and package; handling.
- Bottom fifth: the barcode with ten modules of quiet zone each side, the serial printed under it.
- Ground off-white, text near-black; nothing else.

## On the model

The package record says where its plates go (`labels`: node, face, centre, size), so every model of it puts them in the
same place. The package's model frame: origin at the centre of its base, X along its length, Z across, Y up (glTF
metres). Pallets and crates carry two 0.30 x 0.15 m plates on their long faces (+Z and -Z) at six tenths of their height;
the bar bundle carries 0.16 x 0.08 m tags on its two ends; the ladle on its two sides. Each plate is a flat quad under a
node of its name (`label`, `label_2`), 1 mm proud of the face, UV 0 to 1 across it (u along its width, v up), in one
material named `label`. The record's centre fixes the plate across and up its face (within 2 cm); along the face's normal the plate lies on the
package's real surface, 1 mm proud of it, anywhere from 1 cm outside the record's face to 15 cm inside it (a tapered skip, a
round dewar, a bulging bale, a coil's recessed bands), flat or tilted up to 15 degrees with a tapered side. The installer checks those, and its size within 1 cm. The
game draws each packed unit's label into that material. The plate is part of the shared template; the label is the
unit's.
