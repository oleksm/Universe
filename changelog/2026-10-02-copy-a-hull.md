# Copy a hull to design your own

- On the planner's HULLS page, **SHIFT+ENTER copies the hull picked onto the design board**
  ("COPY OF DROVER"). Change any of its numbers, then commission it as your own.
- **The copy is read off the hull** (`Design::after`):
  - its body's length, width, height and tapers (from the body's own points);
  - its wings' span and sweep, and its fins (from its other parts: wide and flat, a wing; thin,
    a fin);
  - its size class and racks, hardpoints and utility slots;
  - its drive nozzles;
  - where its thruster quads and lift sit, and its engine room, tank, hold and bridge (from its
    mounts).
- **Close, not the same:** the designer draws every body the one way (a tapered hexagonal
  section), so the copy has the original's size, layout and balance, not its exact lines. Its
  stock fit is the designer's (the cheapest that fits); refit it in the planner.
- **Shapes keep their parts' points** (`Shape::part_points`).
- Test: a copy of each hull comes out close to it: length within 15%, span within 25%, the same
  cargo racks. For example, the Drover is 42 m to 42 m with a 38 m span; the hauler 85 m to
  86 m. Dev scenario `designcopy`.
