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
| made on | world time the lot was finished (s); printed as `DAY d HH:MM` |
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

Each package model carries `label` plates (its record's `label`: 1 or 2): flat quads named `label` and `label_2`, 2:1,
UV 0 to 1 across them (u along the long side, v up), 1 mm proud of the surface they sit on, on the faces seen most, in
one material named `label`. The game draws each packed unit's label into that material. The plate is part of the
shared template; the label is the unit's.
