# Standards: the Standards Foundry Office and everyone else's

A standard is a published, versioned agreement on how things fit together: dimensions,
interfaces, units, codes. Anyone can found a standards body and publish standards; the game
seeds one, the **Standards Foundry Office (SFO)**, with the starting set. All the standards in
all the registers together are the tree that describes the civilisation: what its ships, ports,
containers, sockets and signals have in common.

Decided with the user, 2026-10-03. The roadmap's next big feature (`docs/roadmap.md`, 3½).

## What a standard is not

| Not this | Because |
|---|---|
| Physics | Dogma holds the laws. A standard can't change what's true; it fixes what people agreed to build to. |
| A product | Products (hulls, turrets, containers) conform to standards. A standard makes nothing. |
| Law | Voluntary. An authority may make one mandatory in its space ("pads here are PAD-L"): that's the authority's rule, recorded as such. |
| A guarantee | A product claims conformance. Certifying the claim is a service someone else performs; claims can be wrong or false, and the market punishes that. |
| Global truth | No register is everywhere at once. A standard is published at its body's seat and spreads over the hypernet and in ships, like news. |

## A standard's record

| Field | Holds |
|---|---|
| Id | body and permanent number: `SFO 14` (filed under a parent, never renumbered) |
| Title, scope | what it covers and what it doesn't |
| Body | who publishes it |
| Status | draft, published, superseded, withdrawn |
| References | the standards it builds on (a socket standard references units, fasteners, the power bus) |
| Parameters | typed values with units and tolerances: dimensions, loads, voltages, colours, codes |
| Requirements | what a conforming product must meet, as checks the game evaluates (fits within, at most, at least, equals, provides service) |
| Text | the explanation for people: why, not only what |
| Licence | open, or a fee per conforming product (open only, at first) |
| Published | where (its seat's station) and when |
| History | earlier versions and what changed |

The requirements are what make it a system rather than flavour: the shipyard checks a module
against a socket, a port a ship against its pad, without anyone writing those checks by hand.

## A tree to come

(Sketched before any standard was written; kept as a list of what will need covering, not as
the classification: that comes from the standards themselves.)

| Branch | Covers (the first standards in bold) |
|---|---|
| 0 Foundations | **units and measures**, naming and ids, time reference, coordinate frames, how to publish |
| 1 Matter | element and alloy grades, fuel and propellant grades (with roadmap 8) |
| 2 Mechanical interfaces | **hardpoint, nozzle and utility sockets**, latches and hatches, docking ports, fasteners |
| 3 Vessels | **size classes**, **the hull datasheet**, **nav lights** and markings, crash tolerance |
| 4 Ports and infrastructure | **pad and berth classes**, port markings, fuel couplings, gate interfaces |
| 5 Logistics | **cargo container units**, manifests |
| 6 Energy | **power bus**, coolant loops, heat rejection |
| 7 Information | transponder ids, hypernet message formats, beacon codes |
| 8 Safety and signals | hazard markings, distress codes, traffic signals |

A body grows its own branches; two bodies may publish rival standards for the same thing, and
adoption decides.

## Organisation

- **A standards body** is an organisation like any other: founded by a player, an NPC, a
  corporation or an authority; an owner, a seat (a station), a namespace (its prefix). Coarse
  first: the owner publishes. Committees and votes later.
- **The SFO** (seeded) is **like ISO**: independent and international, governing no one. Its
  members are the participating civilisations' standards bodies, with the makers; the first
  makers founded it. Its work is interoperability across civilisations (what exists and how it
  fits). Decided 2026-10-03: "like ISO and USA".
- **Governments are like the USA:** polities, each with its own charter (a way of life: rights,
  duties, conduct, safety law), its people and its systems (each settled system's authority
  belongs to one). A government adopts SFO standards, may mandate them in its systems (its law),
  and keeps its own national standards body for its own codes. Rivals are other governments
  with other charters.
- So two trees: the SFO register (technical, universal) and each government's charter and codes
  (its way of life). Its own record (`standards/SFO/metadata/SFO.yaml`, `about`) says so in full.
- **Adoption is a fact on products and places**, not on the standard: a product declares what
  it conforms to; a port, the pad and berth standards it offers; an authority, what it mandates.
- **Certification** is a service: a record, signed by its certifier, that a product was
  inspected and conforms. Room for trust, and for fraud.

## The registry

- **Kept by each body at its seat:** its standards and their history, the authoritative copy.
- **Copies travel:** stations on the hypernet keep copies, updated at the network's pace; off it,
  a ship has what its computer last heard. A new revision reaches the frontier late.
- **In the engine, facts only:** who published what, when and where, and what's been heard where.
  Choosing, adopting, certifying are the clients' (pilots, brands, authorities): the engine
  holds no intentions.
- **Seeded content:** the SFO's set in content files, one per standard, loaded like hulls and
  modules.

## Where they're kept

- **The source is YAML in the repo** (`standards/`, see its README): a folder per body, its
  standards flat beside it with **permanent numbers** (`SFO 12`, as ISO numbers), and free
  **topics** as tags. The classification (likely multi-dimensional) waits until the standards
  written show what it should be; nothing is renumbered when it comes.
- `python3 tools/standards/build.py` checks it all and writes `standards/index.html` (browse by
  topic or by number) and the game's content (`content/base/bodies.ron`, `standards.ron`,
  generated; the game's tree is the topics for now).
- **In the game:** docked or landed at a port, **V STANDARDS** shows the port's copy.

## Ships to standards (the first use)

- **Size classes** (SFO 3): the berth envelope ports lay out for: maximum length, width, height,
  loaded mass and footprint load per class. Ports offer pads and berths by class (SFO 4).
- **The hull datasheet** (SFO 3): envelope, dry mass, centre of mass, structural limits, and its
  **sockets**. A socket is a named place built to a socket standard (SFO 2): its frame, maximum
  mass, force and torque, its clearance (the volume a module may fill, deployed or swept: a
  turret's arc, a mining arm's reach, a plume's keep-out), its services (power, data, coolant,
  propellant), and a hatch or not.
- **The thrust layout** is the hull vendor's: nozzle sockets with allowed thrust ranges and
  axes, chosen so the ship is controllable about its centre of mass over its load range. No rule
  enforces balance: torque comes from where the nozzles really are, and a badly laid-out hull
  yaws under thrust (its vendor's reputation pays).
- **Module datasheets:** the socket standard and size a module is built to, its mass, its
  envelope stowed and deployed, its load at the mount, the services it draws, what it does. It
  fits when the standard matches, loads and services are within the socket's, and its envelope
  is inside the clearance.
- **Authored in Blender:** a socket is an empty (its arrow the mount frame; custom properties
  its standard, size, limits, services), its clearance a child box `CLR_*`, read on export like
  `nozzle_*` and `gear_*` today. Modules are made the same way, by whoever makes them.

## First cut (coarse)

1. The record, the registry, bodies; adoption declared on products and ports.
2. The SFO's first ten: units, size classes, pad classes, hardpoint, nozzle and utility
   sockets, cargo container unit, power bus, nav lights, the hull datasheet.
3. Today's slots (kind and size 1-4) become sockets built to those; the MC-07 and the existing
   modules declared against them.
4. Checks where things meet: fitting in the shipyard (module and socket), landing (ship and pad).
5. A registry browser: the tree, each standard's parameters, the products known to conform.

Later: publishing your own, licence fees, certification, mandates. The record holds room for
them from the start.
