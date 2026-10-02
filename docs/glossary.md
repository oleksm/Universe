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
| **Kinds of constant** | Real, Grounded, Simplified, Invented, Tuning, Planned. | the sheets |

## The hyper layer (invented, in Dogma)

| Name | What it is |
|---|---|
| **The hyper-medium** (the medium) | What space carries: stiff near masses, slack between the stars. |
| **Slack** (`s`) | How slack the medium is where you are: 0 deep in a system, 1 between stars. |
| **The field** | What a hyperdrive holds a ship in. It draws power; it forms only where the medium is stiff; short of power, it collapses. |
| **The wall** | The power per kg needed to hold a field between stars (about 12.5 kW/kg with the best drive): no ship today reaches it. |
| **The throat** | A gate pair's wormhole: its upkeep grows with the cube of its span. |
| **Ring classes** | A gate ring's greatest span: I 10 ly, II 25 ly, III 50 ly. |
| **Explorer** | A future ship built around next-generation reactors that can cross between stars (40 ly: an epic). |
| **Hypernet** | The information network to come: events captured in range, carried by relays, gates and ships (`docs/hypernet.md`). |
| **Comm** | A ship's hypernet equipment: what it hears (capture), its lag. Every ship carries one. |
| **Backbone** | A system's hub on the hypernet: its station's site. Lag is counted from it. |
| **Transceiver** | A space structure's tower: ships' comms connect to it within its radius (0.5 AU), at light speed. |
| **Hyper relay** | Links two sites through hyperspace: a system's relays link its sites the shortest way all told, daily. A **gate relay** links two systems' nets through the throat. |
| **Orbital site** | A transceiver and hyper relay in orbit round a planet or moon. |

## Factions (`docs/factions.md`)

| Tag | Faction |
|---|---|
| HCD | Halden Concord (holds the home system) |
| FRA | Free Reach Assembly |
| TSD | Tessaly Directorate |

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
| `docs/world/hyperspace.md` | The hyper layer whole: the medium, fields, throats, hyper-signals; numbers; open questions. |

## The plans

| Doc | What |
|---|---|
| `docs/roadmap.md` | The foundations, in order: hypernet, news, factions, corporations, breakables, drones, exploration, matter, a real-density galaxy, terraforming. |
| `docs/backlog.md` | Smaller game items. |
| `docs/art-direction.md` | The look. |
| `docs/architecture.md` | The code's shape (Dogma and the world, the layers). |
