# Zoning view: a settlement's ground from above, in the economy panel

- In the economy panel (5), Enter on a place opens its ground, if the registry records any
  (Port Trethi for now): a plan from above, fitted to the screen, north up, with a scale bar.
- Layers, each shown or hidden: Z zones (tinted by use), P parcels (outlined, numbered), S
  streets (paving, named), W power lines (amber, poles at the bends), F facilities (each
  module's footprint as laid out by the registry). The port's pads and hangar are drawn for
  scale.
- Click a parcel for its owner, zone, area and what's built on it; a facility for its kind,
  parcel, owner, modules and the most it can do (Trethi Foundry: ingot up to 79.6 t/h,
  320 MW flat out; Trethi Power Station: up to 400 MW). Esc back to the list.
- The registry now writes each parcel owner's name and each facility's maxima into
  `content/base/settlements.ron` (from the Maker House and the build's own working-out).
- Dev scenario `zoning` (`UNIVERSE_PICK=f<k>` or `p<n>` to pick one).
- Not yet: other settlements (none recorded), changing anything from the view (leasing,
  building, rezoning).
