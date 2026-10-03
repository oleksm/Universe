# Glossary: the names we use

The one place to look a name up. Keep it current when a name is added or changed.

## The game

| Name | What it is |
|---|---|
| **FREEFALL** | The game. |
| **Plurence** | The company making it. |
| **The universe** | The setting: simply *the universe* (no other name). |

## The engine and the world

| Name | What it is | Where |
|---|---|---|
| **Dogma** | The core engine: the laws every world runs on (nature's constants, mechanics, orbits, contact, the hyper layer). Settled in the game; changed only by deliberate review. | `crates/physics` (`universe-physics`), its laws `config/dogma.ron` (`universe_physics::laws`, `universe_physics::hyper`) |
| **The world** | What's built on Dogma: a seeded instance, its initial conditions (matter, makers, products, structures, production, economy). Anyone in it can build anything Dogma allows. | `content/base/` (the base world), the world crates |
| **The charter** | What's real, simplified and invented, and the rules of the invented. | `docs/physics.md` |
| **The sheets** | Every constant with its unit, kind and reason: Dogma's laws, and the world's fixed numbers. | `config/dogma.ron`, `content/base/sheet.ron`; report `docs/physics-sheet.md` |
| **The dogma checks** | Tests of the charter's promises against the sheets. | `crates/world/tests/dogma.rs` |
| **The charted region** | The cube of space play happens in: 200 ly a side, stars at real density (neighbours 4-6 ly), made from the seed. The galaxy beyond is a backdrop for now. | `universe_world::galaxy`, `docs/world/galaxy.md` |
| **Kinds of constant** | Real, Grounded, Simplified, Invented, Tuning, Planned. | the sheets |

## The hyper layer (invented, in Dogma)

| Name | What it is |
|---|---|
| **The hyper-medium** (the medium) | What space carries: it holds what moves in it under a limit growing with distance from the nearest surface (`v ≤ K·d`). |
| **The field** | What a hyperdrive holds a ship in. It burns fuel by the metre, more the faster (`dE/dx = m·E0·(1 + (v/v*)²)/η`), the same anywhere: fuel is the only limit. |
| **Interlock** | An avionics product's spec: its drive drops out rather than fly into a body. Not a law. |
| **The tube** | What a gate pair holds open along its route: as long as the span, millions of c inside, crossed by mass and chosen speed (`docs/world/hyperspace.md`). (Was "the throat", a wormhole: being replaced.) |
| **Data capsule** | How information crosses a tube: thrown and caught in batches (light can't be caught reliably in the tube's flow). |
| **Ring classes** | A gate ring's greatest span: I 10 ly, II 25 ly, III 50 ly. |
| **Explorer** | A future ship built around next-generation reactors that can cross between stars (40 ly: an epic). |
| **Hypernet** | The information network to come: events captured in range, carried by relays, gates and ships (`docs/hypernet.md`). |
| **Comm** | A ship's hypernet equipment: what it hears (capture), its lag. Every ship carries one. |
| **Backbone** | A system's hub on the hypernet: its station's site. Lag is counted from it. |
| **Transceiver** | A space structure's tower: ships' comms connect to it within its radius (0.5 AU), at light speed. |
| **Hyper relay** | Links two sites through hyperspace: a system's relays link its sites the shortest way all told, daily. A **gate relay** links two systems' nets through the throat. |
| **Orbital site** | A transceiver and hyper relay in orbit round a planet or moon. |

## Law and standing (`docs/standing.md`)

| Name | What it is |
|---|---|
| **Settled system** | One with a station or a port: its law runs, its authority keeps standings. |
| **Fair game** | Fired on the innocent in a settled system: anyone may shoot back, for 10 minutes. |
| **Standing** | A system's authority's view of a pilot, -100 to +100, from deeds it hears of; hostile at -50. |

## The world's makers (brands)

| Brand | Makes |
|---|---|
| Kestrel Driveworks | drives, thrusters, lift, standard hyperdrives |
| Hadley Heavy Industries | rugged fusion plants, tanks, racks, cabins, life support |
| Aurel Fusion Works | light, efficient fusion plants |
| Halcyon Field Systems | premium hyperdrives, gate rings, gate relays |
| Meridian Power | capacitor banks |
| Orbital Systems | computers, sensors, transponders, nav computers, basic comms |
| Tallis Signal Works | long-range comms, backbone and beacon relays |
| Garrick Arms | mass drivers, lasers |
| Cormorant Mining | mining gear, the Prospector |
| Tolland Yards | hulls: Drover, Bulk Hauler |
| Vireo Spaceworks | hulls: Sprint Courier, Interceptor |
| Halvard Construction | stations (platforms), spaceports, outposts |

## Matter (the world's materials)

Deuterium (ships' fuel today), helium-3, D–He3 blend, D–T blend, enriched uranium, methalox, kerolox,
hydrolox, hydrogen: `content/base/materials.ron`.

## World articles (`docs/world/`)

How things work in the universe, one subject an article, to refer to later.

| Article | What |
|---|---|
| `docs/world/hyperspace.md` | The hyper layer whole: the medium, fields, gates as tubes, data in capsules; the crossing formula; numbers; open questions. |
| `docs/world/galaxy.md` | The galaxy and the charted region: real star density, the class mix, home, the map's slab. |
| `docs/world/README.md` | How a number earns its place: real, derived, invented (with a target), tuning. |

## The plans

| Doc | What |
|---|---|
| `docs/roadmap.md` | The foundations, in order: hypernet, news, law and standing, corporations, breakables, drones, exploration, matter, a real-density galaxy, terraforming. |
| `docs/backlog.md` | Smaller game items. |
| `docs/art-direction.md` | The look. |
| `docs/architecture.md` | The code's shape (Dogma and the world, the layers). |
