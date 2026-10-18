# Physical mounts: how equipment is held (proposal, 2026-10-18; revised after ships' review ffce)

Ships' task 20261018T122709-ships-4ec3, the owner's rule: no equipment snaps onto a frame. A piece of equipment is held
by a typed mount on a **structural adapter** or a **deck panel**, each a product made from stock, which is itself held
on the frame's struts and nodes. Being near a strut, or overlapping a room, holds nothing. Services are never implied
by a mount: power, data, coolant and fuel cross only at ports joined by runs (docs/proposals/equipment-ports.md).

## The chain

    equipment ──(mount sockets)── adapter or deck panel ──(node bolts or strut clamps)── frame

Every link is a pair of **load ports** (SFO 26 port, `medium: load`) built to one interface, with opposite halves, its
duty stated (mass borne, force and moment at the joint) and its rating at least that duty. A fit is valid only when each
link names both of its ports. Nothing is held by position alone.

| Link | Thing side (male) | Holder side (female) | Interface |
|---|---|---|---|
| equipment → holder | the equipment's feet, from its `fits` | the sockets a holder offers | the SFO 19 mount (`mount.<slot>-s<n>`) or the deck track |
| holder → frame | the adapter's or panel's fixings | a frame node's spare tapped hole, or a point along a strut | `interface.node-bolt-m<d>` or `interface.strut-clamp-d<mm>` |

## 1. Mounts get sockets (SFO 19)

A mount already says its envelope, the mass it bears, and its attachment `points`, `pattern` and per-point `each`
(tension, compression, shear). Added: `sockets`, one per point, written by a tool from the pattern and envelope, never
by hand:

```yaml
sockets:
  - { id: s1, at: [-6.9, -3.585, 0], facing: [0, 0, 1], interface: interface.insert-m16 }
```

- Mount frame: x along, y across, z up from the contact face; the origin is the middle of the contact face (the
  equipment's `mount` node, docs/equipment-mounting.md).
- A socket is a threaded insert (`interface.insert-m<d>`): its bolt is the smallest ISO size whose proof load carries
  the point's tension with a factor of 1.5. The factor is invented and needs review.
- Equipment that `fits` a mount gets one male load port per socket at the same place, written by
  `equipment_ports.py`. Its duty is the equipment's mass shared over the points, times the design load factor below.
- **Poses** are the mount's symmetries, and only those that keep the joint valid: after the turn, every socket still
  meets a foot with coincident opposed faces (positions within 1 mm, facings opposite within 0.5°), keyed sockets
  meet their keys (a mount may mark one socket `key: index`; the equipment's index foot must land on it), and the
  equipment's envelope stays inside the mount's and clear of everything the hull has already placed. A `corners`
  pattern on a rectangle can at most turn 0° or 180°, on a square 0°, 90°, 180° or 270°; `ring` turns by multiples of
  360°/points; `saddles` and `trunnion` don't turn. The fit records the pose as one of these turns, never as a free
  transform.

## 2. Adapters (new kind `adapter`, `standards/SFO/metadata/adapters/`)

An adapter is a product that offers one mount and hangs from the frame: a welded or bolted cradle of kit tube and plate.

```yaml
identity: { key: adapter.power-s1-grid1, name: "Power plant cradle, class 1, 1 m grid" }
offers: mount.power-s1            # its top face is that mount's contact face, its sockets that mount's sockets
grid: 1.0                         # it lands on frame nodes at this pitch (0.5 m multiples)
ports:                            # load ports: sockets (female, the mount's), fixings (male, to the frame)
  - { id: fix-1, medium: load, interface: interface.node-bolt-m20, half: male, make: bolted,
      at: [-7.0, -3.5, -0.3], facing: [0, 0, -1], mass: ..., force: ..., moment: ... }
rating: { fixing: { tension: ..., compression: ..., shear: ... } }
built_of: [...]                   # stock: tube, plate, inserts, bolts; mass calculated from them
```

- Fixings land only on grid points: a node (`node-bolt`, into a spare tapped hole of the MERO node, the node's own bolt
  size) or the mid-point of a strut (`strut-clamp`, a split collar the tube's diameter).
- Generated per mount and grid by `tools/standards/adapters.py` as a parametric family, one template model: a deck of
  cross-members under the sockets, legs down to the fixings, each member sized by the same strut sizing loop the
  frames use. Its fixings are rated for the mount's worst case. The duty
  of a fitted cradle is worked out per design (§4).
- Each adapter gets a kit-works recipe from its stock (the makers check covers it).
- An adapter is an instance in a design (§5): one cradle per piece of equipment, never shared.

## 3. Deck panels (panel `kind: deck`)

Small equipment (avionics, seats, lockers, racks, the distribution boards) stands on a deck. A deck panel is a sandwich
of 6061 skins on aluminium honeycomb, sized to the frame grid: 0.5 × 0.5, 1 × 0.5, 1 × 1 and 2 × 1 m. It carries:

- **Seat track** (`interface.deck-track`): extruded rails along x at 0.25 m spacing, with stud holes on a 25.4 mm
  pitch, the aircraft seat-track convention. Equipment with `fits: mount.deck` gets track-stud feet, any number on the
  pitch. Rating per stud: 9 kN tension, 13 kN shear. These figures are from memory of aircraft seat-track practice and
  need review.
- **Edge fixings**: at each corner and every 0.5 m along each edge, a clamp onto the strut under it
  (`interface.strut-clamp-d<mm>`), or a node bolt at a corner.
- A rated area load, in kPa, and a rated point load.
- Generated by `building_kit.py`'s panel family, beside the building floors; its template is `template.floor`.

## 4. The duty of a link

A link's duty is everything that passes through it, not only the weight of what hangs on it:

- **Inertia**: the mass outboard of the link (the equipment, plus the holder for the holder-to-frame link) times the
  hull's design acceleration in each direction: the largest of its thrust, lift and landing accelerations.
- **What the equipment does**: an engine's or lift's thrust, landing gear's reaction (the ship's mass at its landing
  deceleration, shared over the legs on the ground: the ship's mass, not the leg's), a weapon's recoil, a clamp's or
  winch's pull. Each comes from the equipment's function and the hull record, never from the mount's margin.
- **Moments**: each force times its offset from the joint's centre: a thrust line off the mount's middle, a heavy box
  standing tall on a deck.
- The sum is shared over the link's ports by their positions, as rigid-body statics. Each port's force and moment are
  its duty, and its interface's or product's rating must be at least the duty times the ultimate factor.

**Not approved, under review:** the ultimate factor (1.5), the bolt proof factor (1.5), the deck-track stud ratings and
the adapters' load shares. They are invented placeholders: every record that uses one carries `review: true` and a
basis line saying so. Ships and the owner approve them, or replace them with sourced figures, before any check treats
them as final.

## 5. What the fit says (ships' design format)

Every placed thing is an **instance** with its own id: equipment, adapters and deck panels alike. A deck panel that
holds several pieces of equipment is one instance that they all name. Links join an instance's port to another
instance's port (or to a frame node or strut, by the frame's own ids):

```yaml
instances:
  - { id: bat1,   key: equipment.power.battery.s1, pose: 180 }
  - { id: crd1,   key: adapter.power-s1-grid1 }
  - { id: deck3,  key: panel.deck-1x1 }
  - { id: av1,    key: equipment.avionics.nav-full.s1, pose: 0 }
  - { id: av2,    key: equipment.computer.fbw.s1, pose: 90 }
links:
  - [bat1:s1, crd1:s1]          # equipment foot to holder socket, one pair per socket
  - [bat1:s2, crd1:s2]
  - [crd1:fix-1, frame:n_4_2_1]  # holder fixing to a frame node (node-bolt) or strut point (strut-clamp)
  - [av1:foot-1, deck3:track-2@7]   # deck: rail 2, stud 7
  - [deck3:edge-1, frame:m_3_2_1@0.5]
```

- A fit is invalid unless every load port of a placed instance is in a link, and every link joins one male and one
  female port of the same interface, with coincident opposed faces. Overlap, proximity and room membership place
  nothing.
- Wiring is unchanged: equipment ports join runs, by the same instance ids. A deck panel or an adapter may carry
  **run supports** (cable clips, pipe clamps) as their own ports (`medium: load`, interface `interface.run-clip-d<mm>`),
  so runs are held as explicitly as equipment.

## 6. Checks (`tools/standards/mount_check.py`, added to `reg check`)

- Every mount has sockets matching its points and pattern.
- Every equipment that fits a mount has a load port per socket.
- Every adapter offers a mount, its fixings land on its grid, and its ratings meet the mount's worst case.
- Every deck panel's tracks and fixings meet its rated load.
- For a design, ships' checker walks each instance's chain to the frame: every load port linked, the interfaces
  matching, the halves opposite, the faces coincident and opposed. Each port's rating must be at least its duty (§4)
  times the factor, with the pose a valid symmetry.

## Asked

- Ships (agreed the chain in ffce): agree the revised instances and links, duty and pose rules. Then the registry writes sockets, load ports, the adapter family and deck panels with recipes, and tells
  blender which templates to draw: one cradle template, the deck panel, the clamp and the insert.
