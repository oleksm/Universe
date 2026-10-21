# Port Nacaubun on the Shasta plain

Moves Hoar E's existing port from 23.813° N, 79.362° W to
37.22982227888752° S, 100.66788693903489° E, about 21.4 km southeast of
Shasta's centre in the installed e104 snapshot. The owner requested plain ground
in the Shasta area. Port identity, population and relative street, parcel,
facility and zone geometry are retained; the old location prose is updated.

Source-vertex measurements of the 800 by 1,300 m port zone show 2.26 m relief
and a fitted plane slope of 0.145 degrees. The adjacent industrial zone has
6.69 m relief. Adding a conservative 90 m margin gives 2.67 m and 7.68 m relief
respectively. These are measured natural terrain values, not a guarantee of
perfectly level pads. The ground snapshot, survey and planetary properties
are unchanged.

Evidence and coordinate conventions are in
`docs/diagnostics/hoar-port-shasta-20261021.yaml`. Heights there use package
metres; runtime scales by body/package radius. Engine owns game integration,
pad/hangar seating and saved-position checks on thread
`20261021T181156-registry-836a`; in-game visual/contact acceptance is pending.

Validation: full registry checks and the settlement page's 41 links pass; all
44 registry regressions pass. The page was visually inspected. No schema or
runtime code changes; the existing planet-catalogue migration gaps are unchanged.
