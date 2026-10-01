# The mining rig

- **Spine to the rock.** Closing on a rock (3), a ship turns its spine to it, nose along the
  surface. Over a rock (closing in or anchored) the chase view moves off to one side, level
  with the gap between ship and rock, the rock below.
- **The rig.** Anchored with the excavator on (H), two laser emitters slide up out of the
  spine on struts and out to the sides, the spine's cargo hatch swings open, the beams cut
  into the rock (the cut glows), and what they break loose streams up into the hatch, as thick
  as the dig rate. Off, it all stows again (2 s). Any ship's rig shows: NPC miners at work
  too (anchored ships are now drawn).
- **The flow.** The prospector reads EXTRACTING (ore) at kg/s (t/h) and when the next tonne
  goes in; each tonne into the hold is announced (+1 T STONY ORE TO THE HOLD — so many in
  all, hold so full).
- **Cargo (4, on the mode bar).** What's in the hold: each good's kind, units, mass, volume as
  stowed (from its kind's bulk density) and worth; the loose ore in the hopper; loaded against
  capacity; the ship's mass, dry, fuel and cargo.
- **Rocks get smaller.** What's dug out of a rock is gone: it's drawn smaller (its volume goes
  with its mass) and it is smaller — ships touch and anchor to it as it is now
  (`World::field_bodies_now`). A rock hundreds of metres across hardly notices a hold; a
  boulder worked hard shrinks.
- The own ship isn't drawn over the HUD while a panel is up (cargo, T's list, mining mode) or
  over a rock.
