# The registry made solid: keys, refs, SI

*Registry only (`fso`). Nothing the game loads has changed: the generated files are byte-identical.*

- **One key on every record:** `<kind>.<name>`, the kind being its schema's (`hull.mc-07`,
  `equipment.drive.torch.s1`, `gate.ring.i`, `body.treistun.treistun-f`, `org.hadley`). The build
  checks each is there, well-formed, matches its file and is no other record's.
- **Schemas merged:** one organisation schema for companies, the standards body and
  administrations; one body schema (a star is a body; comets, centaurs and the like are bodies the
  game does not make yet); one population schema for asteroid fields and far regions. Local
  Administration no longer keeps its own copy of planets and moons: a settlement is at a celestial
  body.
- **Records name each other by key:** 1,216 references. Each such property says in its schema which
  kinds it may name, and the build checks every one.
- **SI throughout:** every value with a dimension is in kg, m, s, K, W, N, J, Pa (angles in
  degrees), and its property says its unit. 2,322 values converted. The registry page still reads
  in tonnes, km, hours and AU.
