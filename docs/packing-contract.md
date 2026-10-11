# Packing: how stock becomes labelled packages (registry contract, 2026-10-11)

For engine's handoff (out/review/package-label-integration/handoff.md on main). Decisions the label needs before the game
stamps one. Coarse first; each can be refined once it runs.

## 1. When a package is packed

- **Automatically, at the works, as output accrues.** A works fills its output into the stock's package (the stock's
  `package`) as it is made. A package is sealed and labelled when it is full (its `holds` volume or mass, whichever
  comes first) or when its lot closes. A part-full package is labelled with its true net mass.
- **One lot to a package.** Mixed lots never share a package.
- **Unpacking ends the label.** Opening a package (to use, to split) returns its contents to loose stock that keeps its
  lot; repacking makes a new package with a new serial in the same lot. A package moved whole (sold, hauled, stored)
  keeps its label for ever.
- **Tare is physical.** A package's tare travels with it: its gross mass is what a hold carries and a crane lifts.
  Empty packages are not yet traded (later: packages as stock of their own).

## 2. What `made_on` means

The lot's first output: the moment the works opened the lot (the economy's open daily lot, as engine has it). Labels
print it as MADE; `packed_on` is when that package was sealed. (The label's and batch's wording said "finished"; now
"begun".)

## 3. Serials

`LOT-NNNN`, 0001 to 9999 within a lot. When a lot reaches 9999 packages the works closes it and opens a new lot with a
new lot number; a serial is never wrapped or reused. Every stock a works makes has a lot, so all stock can be packed;
what is gathered from a place (air, water, carbon dioxide drawn in) is not stock until a works makes it.

## 4. What the game reads (no copies)

All from the registry the game already loads: a stock's `package` and `tint`; a package's size, tare, `holds`,
`handling`, `stack`, `keeps`, `label` (plate count), `shows_contents` and `visual`; an organisation's `brand` (name,
primary, accent, logo). Logos come as SVG and as a 256 px PNG beside it (`MakerHouse/logos/<name>.png`, made by
`tools/standards/logos.py`), so the game need not read SVG.
