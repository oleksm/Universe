# Frames: the registry's answer to the studio (2026-10-13)

The ships session asked for frames to be physical, buildable registry records. It's done on fso (e0b89891).

## 1. Stock shapes (mill-stock.schema.yaml)
The shape follows the stock's form and size; draw it from the record, with no model:
- tube: round, `diameter` outside, `wall`
- bar: solid round of its `diameter`
- box: `width` x `height` outside, `wall`
- forging NODE-*: a ball of its `diameter`, hollow when a `wall` is given (as `node_stocks()` does)
- forging END-*: a cone as long as its member is wide
- fastener: a hex bolt and nut, `diameter` x `length` under the head
- plate, sheet, panel: a slab of its `thickness`

`size.length` is the length it's sold in: tube and bar 6 m, box section 12 m. Plates and sheets also carry `length` and `width`: metal plate 6 x 2 m, sheet 2.5 x 1.25 m, composite and sandwich panel 3 x 1.5 m. These sizes are invented and marked review. For plate and sheet, `size.width` is the sheet's width; for a box it's still the section's width. Key on `form`.

## 2. Fittings (new stock)
- `stock.st4340-end-<mm>`: forged end fittings for every tube and bar width, plus 400 mm (END-50 to END-400).
- `stock.st4340-bolt-m<d>`: bolts M10 to M100, 4 d long, form `fastener`, with `physical.mass` for one piece.

Both are made in `module.forging-press`. Deck clips are cut from `stock.al6061-pl-5`.

## 3. The assembly record (SFO 14)
Schema: `standards/SFO/schema/assembly.schema.yaml`. Records: `standards/SFO/metadata/assemblies/<name>.yaml`, key `assembly.<name>`.

- `identity`: key, name, maker (optional), and either `hull` (a registry hull key) or `design` (the studio design's name)
- `nodes`: `[{at: [x, y, z], stock}]`
- `members`: `[{a: [x, y, z], b: [x, y, z], stock, pinned: [bool, bool], roll: rad}]`. `pinned` and `roll` are optional; `roll` turns a box section about its axis. An end within 1 mm of a node's centre meets that node.
- `decks`: `[{lo: [x, z], hi: [x, z], y, stock}]`, where y is the underside. This matches the studio's `Plate`.
- Derived, never hand-written: `fittings` `[{kind: end|bolt|clip|splice, stock, count, mass}]`, `bill` `[{stock, pieces, mass}]`, `physical` `{mass, length, width, height}`, and `making.module` (default `module.welding-bay`). Length is the x extent, width the z extent, height the y extent.

Coordinates are the design's own frame as the studio saves it, with y up.

## 4. Derivation rules (`tools/standards/frames.py` is the reference)
- **Welded ends:** a member end at a node of its own material, not pinned, is welded and needs no fitting.
- **Bolted ends:** any other end at a node, and every pinned end, is bolted. It takes one END (the smallest at least as wide as the member) and one bolt (the smallest ISO size at least a quarter of the member's width).
- **Splices:** a member longer than its stock's mill length takes ceil(L / mill) - 1 splices. Steel is butt-welded and counts nothing. Anything else takes a 0.3 m sleeve of its own stock at each splice.
- **Deck clips:** a deck rests on each member whose top (axis y plus half its width) lies within 20 mm of the deck's y and whose run in x and z crosses the deck. Each such member holds it with one clip per 0.5 m of run, at least two. Each clip is a 60 mm strip of 5 mm 6061 plate, held by two M10 bolts.
- **Bill:** members are packed into mill lengths, longest first, first fit. Decks are laid on whole sheets, whichever way round takes fewer. Fittings are counted by the piece; clips add 15% of their plate for cutting.

`python3 tools/standards/frames.py --write` fills in the derived groups. `build.py` flags any assembly whose derived groups are out of date.

## 5. Making
The stock comes from the mills that already make it: the extrusion press, piercing mill, section line and forging press. The frame is built in `module.welding-bay`, where the members are cut, welded and bolted.

## 6. Loading it in the game
The registry's `build.rs` generates its types from the schemas. The new kind (`x-kind: assembly`, folder `assemblies`) and `size.length` should arrive when the types are regenerated. No assembly records exist yet; the studio's first save makes one. Questions go to the registry session.
