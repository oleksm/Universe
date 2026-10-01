# The designer explains itself; designs have names; 1,000,000 CR to start (for now)

- **Every design knob explains itself.** With a row picked, the design page says what it is, and
  what turning it UP and DOWN does for the ship and costs it. For example:
  - THRUSTERS CENTRE: "put it on the centre of mass; aft of it, shoves turn the ship, so the
    front set works harder and you lose push…"
  - TANK AT: "off the centre of mass, the balance shifts as the tank empties".
  - WING SPAN: "looks; no lift in air yet. Costs frame mass far out (slower roll) and a wider hull
    to clip others".
- **Every slot explains itself** on the PLAN page: what it's for, and what a BIGGER or SMALLER
  module in it trades (the drive: "more push… heavier, burns more at full" / "lighter, sips
  fuel; slower to get moving").
- **Clearer names:** QUADS APART is THRUSTER SPREAD, QUADS AT is THRUSTERS CENTRE, LIFT AT is
  LIFT CENTRE.
- **Designs have names:**
  - The NAME row at the top of the design page: ENTER to type it (letters, digits, spaces,
    dashes; BACKSPACE), ENTER when done.
  - Commissioned, the hull carries it. Left as "DESIGN", it's numbered; a name already used gets
    a number.
  - The engine's input now has the text typed each frame (`Input::typed`).
- **Fixed: drive nozzles shared the drive.** On a designed hull every drive nozzle got the
  drive's full rating, so more nozzles was free thrust. They now share it: two nozzles' worth
  however many, as a stock hull's two (tested: 1 to 4 nozzles, the same push).
- **Fixed: global keys while a panel has the keyboard.** TAB, which turns the shipyard's pages,
  also switched to the observer; while typing a name, `,` and `.` would have changed the time
  warp. Global keys stand down while a name is typed, and TAB while a panel is open.
- **Starting credits: 1,000,000** for now, while ships are being built and tried. The economy's
  balance pass sets it for real.
