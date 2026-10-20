# Wire hold and bend checks

Each authored wire is now checked against its cable's `hold` and `elbow` records
(registry rule.run-support). The WIRE panel shows the chosen cable's run clip,
its most spacing aboard a ship and its least bend radius. Cables the registry
does not lay aboard a ship (no ship hold spacing) are no longer offered.

Laying a wire computes the stations it must be clipped at: one at each end and
the fewest between them so none are farther apart than the spacing. The stations
are drawn along the route (faulted colour: wanted, none placed). The checks
list one fault per wire, "NEEDS n X run-clip-dNN EVERY s M, NOT PLACED", and
the report prints the same. Nothing places clips yet: no clip, strut clamp or
lug geometry is authored, and the clamp lug datum is still open in the registry,
so a wire stays unsupported until that exists. Clips are not counted in mass.

A corner at a waypoint turning `theta` at the cable's least radius `r` takes
`r·tan(theta/2)` of straight on each leg; a leg shorter than its two corners'
tangents is too tight. The tool refuses to lay such a route (the note names the
waypoint, Backspace moves it) and marks the corner; a saved wire with one lists
the waypoints in its fault. A reversal has no radius. Bend mass is not added.

The conductor-size check treated a conductor with no size (a data cable's
screen) as infinitely large, so no screened cable could be laid; only sized
conductors are checked now. The wiring sheet (runs, mass, faults) is shown for
authored wiring, not only the inferred harness. `UNIVERSE_TOOL=wire` opens the
Studio in the wire tool.

Verified: the game crate's suite and the ignored registry route test pass; a
Studio run on a scratch copy of commuter-1 with one staged data wire showed the
panel, the hold line, the stations along the route and the sheet's faults. The
commuters' saves are unchanged. Electrical loads, clip placement and runtime nets
are still not validated; authored wiring remains blocked from flight.
