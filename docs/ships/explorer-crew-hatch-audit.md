# Explorer crew hatch and folding ladder, beside the rear ramp (engine, 2026-10-22, audit only)

Registry `20261022T231509-registry-ce5d`: the owner replaces the middle belly ramp with a crew hatch and a folding
ladder, keeping the rear compound ramp (Blender `internal-side-access-01`, a geometry study). What the game
supports now, from main at 22529ae10; nothing installed or changed.

## Hatch

- One node, named exactly `hatch` (`shape::Role::Hatch`; only the first is used, `crew::stair`). It is where the
  crew boards: its place and the way it points.
- Without a ramp (`crew::walks_out` false): standing at `hatch` inside and interacting puts the crew member at the
  foot of an implied stair. That's the ground below `hatch + 8 m` along the hatch's horizontal direction (`STAIR_RUN`;
  straight down: aft). Interacting there brings them back in at the hatch. It is a teleport both ways: nothing is
  walked, climbed or drawn between.
- With a ramp (`shape.ramp` some: a `*Ramp*` mesh): the ramp is the only way out. The hatch's interaction is off
  (`Reach::Hatch` needs `!walks_out`), and the crew walk down the ramp's surface.
- Only landed, not docked (`crew::hatch_body`).

## Ladder

- The only climbable ladders are deck-plan ladders (`deckplan::Ladder`, placed in the ships studio): between a deck
  and the deck above, through a 0.9 m hatch in its floor. A walker in one holds on and climbs, W up and looking
  down to climb down (`walk::Walker::step`, `climbable`; `crew` uses the deck layout's `climbing`).
- No ladder from an outer hatch to the ground: the ground is no deck. No ladder markers in the model are read.
- No ladder states (stowed, deployed), folding, or clearance checks.

## Animation

- None for hatches or ladders: `door_*` nodes are static aperture centres; a fitted visual with motion is refused
  (`mounted`, unless `pose: rest`, drawn still); the rigid ramp's swing is its only movement (two states).

## Coexistence with the rear ramp

- As things stand, a model with both a rear `CargoRamp` and a belly `hatch` boards only by the ramp. The hatch is
  inert, and the game has no way to choose between two ways out.
- Two ways out need: several named exits (e.g. `hatch_belly`, plus the ramp), each with its own kind (ramp walk,
  ladder climb), state and availability (landed, ground clearance); interaction choosing the nearest one.

## What a hatch-and-ladder exit would need (if chosen)

- Model: `hatch_belly` at the opening's centre pointing out (down); a ladder `ladder_belly_top` and
  `ladder_belly_foot` pair (deployed), with its rung spacing and width in the record or the names; the folding
  leaves as meshes with hinge origins, like `CargoRamp` (stowed pose exported).
- Engine: the ladder as a climbable segment in the ship's frame (the walker's `climbable` already holds on and
  climbs; it needs a climbable volume from the model, not only the deck plan's). The foot meets the terrain under
  it, refused where it's too high or the ground too steep. Exit choice among several. The ladder's deploy as a
  state (stowed or deployed, no motion first), and later its fold motion with sweep checks.
- Tests: climbing down to the ground and back, the foot on a slope, a too-high foot refused, the rear ramp still
  walked with the belly hatch present.
- Not in it: human, suit or carry envelope; strength and locks.
