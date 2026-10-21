# Authored wires carry their ports' current

A power wire laid in the Studio's WIRE tool is now rated for what its ports
state: the greater of its two currents, the load's at its rating and the
feed's (a distribution board's way is its breaker, and a cable is protected by
what feeds it). The cable must carry that current by its registry rating in
vacuum (`rating.current`: cooled by radiation alone, since no bay is yet said
to hold air), each end connector must carry it by its own rating, and out and
back over the route at that current the wire may lose no more than the
studio's 5 % of its voltage, an electrical convention rather than a registry
rule. Conductor cross-section and conductivity come from the cable's record
and its conductor material's.

The tool refuses to lay a wire that fails any of these and says why: the
amperes needed against what the cable or connector carries, in vacuum and in
air, or the drop over the route, and points at a heavier cable (N) or a
shorter way. A laid wire's note gives its current, its cable's vacuum rating
and its drop. The cable panel shows the picked cable's current in vacuum and
in air and its voltage. Saved wires are re-checked on every report and on the
wiring sheet, one fault per wire, and the sheet's runs now carry the wire's
watts and drop. Signal wires state no current and are not checked.

Not yet validated: bundle derating (each wire is rated alone), bays with air
(the vacuum rating is applied everywhere), protection and fault currents, the
runtime nets that would power a module from its wire. Clip placement and
mechanical mounting remain open; flight stays blocked.
