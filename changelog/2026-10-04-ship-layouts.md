# Ship layouts: the MC-07's inside as compartments you walk

- **SFO 18, Ship layouts** (registry): a hull's inside as decks, compartments and openings.
  Compartments have a purpose (cockpit, quarters, galley, restroom, airlock, holds, tanks, reactor,
  hyperdrive, engines, shafts...), an address (unit, section, deck: SFO 9), whether they're sealed,
  and boxes in metres back from the nose, above the keel and from the centre line, the parts' frame.
  Openings: doors, pressure doors, hatches, cargo hatches, chutes, the ramp, windows.
- **Checked by the build:** no overlaps, each opening on a face its two compartments share, person
  doors at least 0.8 x 2.0 m, every compartment reachable from outside. Volumes and floor worked out;
  a layouts report; `content/base/layouts.ron` written for the game.
- **Checked against the model:** `tools/standards/hulls/check_layout.py <glb> <hull>`: nothing of
  the hull through a compartment (it names the meshes that cut in).
- **The MC-07 laid out** (invented, to review): cockpit, avionics bay, crew corridor, two cabins,
  galley, restroom and stores forward; cargo hold by the ramp (round the ramp's rams), airlock,
  ladders, ore hold, fuel tank bay, life support, stores, workshop and passages amidships; hyperdrive
  bay, tank bay, engine room, reactor room and upper engineering aft, on the rear pod's floor. 29
  compartments, 35 openings, 6,979 m3, 2,164 m3 sealed.
- **In the game:** an imported hull with a layout gets its interior at import: every compartment
  lined (floor, ceiling, walls set 2 cm in, openings cut through), walked on and bumped into with the
  hull, drawn unlit in flat shades with edges (no interior lamps yet). On foot the HUD names the room
  and its address. Ladder shafts and hatch columns are climbed: forward climbs, up looking level or
  up, down looking down.
- **Walking:** steps up to 0.6 m (was 0.45), so a ramp's lip onto a deck is taken.
- **Not yet:** door leaves (openings are open), air and sealing, equipment placed in compartments,
  services, structure: the next layers of SFO 18.
