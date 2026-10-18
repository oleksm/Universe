# Physical mounts: how equipment is held (proposal, 2026-10-18)

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
- **Poses** are the mount's symmetries only: a `corners` pattern on a rectangle turns 0° or 180°, on a square 0°, 90°,
  180° or 270°; `ring` turns through multiples of 360°/points; `saddles` and `trunnion` don't turn. The fit records the
  pose as one of these turns, never as a free transform.

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
  frames use. Each fixing's load is the mount's worst case shared over the fixings: the shares are invented and need
  review until a real load path is solved.
- Each adapter gets a kit-works recipe from its stock (the makers check covers it).

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

## 4. The design load

Each holding link is checked at the equipment's mass times a design acceleration: the hull's largest of its thrust
acceleration, landing deceleration and lift acceleration, times 1.5 (an invented ultimate factor; review). The hull
gives the figure, so a heavy hauler's cradles come out stouter than a commuter's.

## 5. What the fit says (ships' design format)

```yaml
fit:
  - equipment: equipment.power.battery.s1
    via: adapter.power-s1-grid1            # or panel.deck-1x1
    pose: 180                              # one of the mount's symmetries
    holder_at: { nodes: [n_4_2_1, n_11_2_1, n_4_9_1, n_11_9_1] }   # frame nodes or struts per fixing, by the frame's ids
```

- On a deck: `via: panel.deck-1x1`, `at: [track, stud]` per foot, the panel's own `at` naming its edge struts.
- A fit is invalid without `via` and every fixing named. Overlap, proximity and room membership place nothing.
- Wiring is unchanged: equipment ports join runs. A deck panel or an adapter may carry **run supports** (cable clips,
  pipe clamps) as their own ports (`medium: load`, interface `interface.run-clip-d<mm>`), so runs are held as
  explicitly as equipment.

## 6. Checks (`tools/standards/mount_check.py`, added to `reg check`)

- Every mount has sockets matching its points and pattern.
- Every equipment that fits a mount has a load port per socket.
- Every adapter offers a mount, its fixings land on its grid, and its ratings meet the mount's worst case.
- Every deck panel's tracks and fixings meet its rated load.
- For a design, ships' checker walks each fit's chain: equipment to holder to frame. Each link must join ports with
  matching interfaces and opposite halves and have its rating at least its duty, with the pose among the mount's
  symmetries.

## Asked

- Ships: agree the chain, the fit fields (`via`, `pose`, `holder_at`), the deck-track convention and the design load
  rule. Then the registry writes sockets, load ports, the adapter family and deck panels with recipes, and tells
  blender which templates to draw: one cradle template, the deck panel, the clamp and the insert.
