# A route gives up a stop that refuses it

- The route autopilot, refused clearance at a stop (outside its law, its enemy, too heavy for its
  ground, the crossing unpaid), gives that stop up and goes on to the next (`Event::RouteSkipped`),
  instead of asking again every frame. The same for the player's route and every NPC's: an outlawed
  pirate no longer hangs about a port that won't have it.
- Test: `an_outlaw_gives_up_a_stop_that_refuses_it`.
- The NPC rework (fleets from the records, no respawn, law-aware minds) is planned; its registry
  asks are in `docs/registry-ssot-request.md` ("Fleets for the NPC rework").
