# The standards registry: the Foundry Standards Office's first ten, browsed from the dock

- **Content:** `bodies.ron` (standards bodies: name, prefix, seat, their branches of the tree)
  and `standards.ron` (standards: id, version, status, what they build on, typed parameters,
  requirements as checks against them, text, licence). Checked at load.
- **The FSO's first ten:** units and measures; vessel size classes (S, M, L, XL envelopes);
  landing pads and berths by class; the vessel power bus; hardpoint, nozzle and utility sockets;
  the cargo unit; navigation lights; the hull datasheet. Numbers invented, each noted with what
  it's aimed at.
- **V STANDARDS** (docked or landed at a port): the registry as a tree to traverse (bodies,
  branches, standards), the one under the cursor in full at the side.
- Dev: scenario `standards` (`UNIVERSE_STANDARD=FSO/2.1/001` to open on one).
- Next: products declaring what they conform to, sockets in place of slots, the checks run.
