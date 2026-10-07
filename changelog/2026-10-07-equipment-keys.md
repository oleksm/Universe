# Equipment keys renamed (fso merged: audit step 3, renames)

- 74 equipment keys to `equipment.<slot>[.<family>].<variant>` (`equipment.cargo.rack.s2`,
  `equipment.avionics.nav-basic.s1`), files after the key's tail; the 26 the game has entries for
  carry their old keys in `content/base/aliases.ron` (saves keep loading). The code's literals
  (operator, dev, two tests) follow; every `equipment.*` key written in crates/ resolves.
- Serials take the new codes (`TOL-CARGO-RACK-S2-…`); only day-0 hull serials exist, and hull keys
  are unchanged.
