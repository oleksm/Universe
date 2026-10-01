# The drive fires evenly at the throttle

**What the pilot saw** (F7, holding a straight line):
- the two main drives very unequal (81% and 17%);
- thrusters pushing against each other to cancel the turn that made (one nose thruster at full
  pushing backwards, the side thrusters left against right);
- about 14% more fuel than the drive alone for the same push.

**Why:** the allocator finds *a* setting that gives the push and turn wanted, and many do. Started
each step from the last, it kept whichever it had found, a mix left over from a turn included.

**Now, as a flight computer does it:**
- **The main drive's nozzles are held together at the throttle** (as far as the drive goes
  straight; `allocate_with`). The thrusters and the lift do the rest: they make up the
  translation and the turn, and hold off whatever turn the drive's line makes off the centre of
  mass (that's the balance cost).
- **A small fuel cost** (`FUEL_WEIGHT`): of settings that give the same, the one using less wins.
- **In flight the translation thrusters start each step from 80% of their last settings**
  (`FORGET`). What's still needed comes straight back; a pair pushing against each other dies
  away within a fraction of a second.
- **The lift isn't faded:** it carries the ship, and faded each step a climb off a pad lost its
  footing and crashed (the route test caught it). Its imbalances unwind through the fuel cost
  alone, more slowly.
- **The drive's authority** (how hard it pushes without turning the ship) is worked out the
  same way, its nozzles together.

**Result:** at full throttle on the Drover, the two drives at 1,350 kN each, and a small couple at
the nose (about 30 kN down, 40 kN up) holding off the drive's line 6 cm above the centre of mass:
about 2–3% more fuel than the drive alone.

**Not yet:** that couple isn't always the cheapest one. A true minimum-fuel solve would need a
proper optimiser per ship per step.

Tests:
- from the pilot's uneven setting, half a second of flight: the drives even (within 2%), the
  translation thrusters under 2% of the push, the lift's leftover under a fifth;
- full throttle: both drives at full, no turn;
- the fuel burned: the drive's and a little more (under 10%).
