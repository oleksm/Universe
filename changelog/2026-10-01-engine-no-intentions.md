# 2026-10-01 — The engine knows no intentions (set in stone)

User: "correct engine owns universe, but it does not care about intentions… things like I am a
pirate or hey lets use this guiding path is not on engine for npc or player"; "lets make sure we
set the principle in stone and keep engine very clean from intentions and inner client
decisions"; NPC pools on their own threads, "because later we will breakdown engine to
clustering for scale up".

## The rule

**The engine (core and services) knows bodies, physics, hardware, declared status and
evidence. It never holds or decides what a client means to do.** It's written as §0 of
`docs/rearchitecture.md` and in the architecture doc's principles, and kept by a test
(`crates/sim/tests/boundary.rs`): engine code may not name a client module or type (the pool,
pilots, the cockpit, the operator, avionics) or an intention (roles, hunts, flight).

## What moved out of the engine

| Was in the engine | Now |
|---|---|
| Fleeing when hit, or after breaking off a fight | The NPC pilot decides (`pilots::flee`), from its own feed |
| Settler roles, names, first routes (`spawn_settlers`) | The operator (`operator::settlers`). The engine only registers ships (`Universe::register`, logged for replay) |
| New routes when one is done (`dispatch`) | The pilot, through its operator (`operator::new_route`) |
| What traders sell and buy, and where they go next (`craft_trades`, `plan_trip`) | The trader client. It requests quotes, the market service answers by message, and it posts trades and its declared plan (`Request::Quotes`, `Trade`, `Declare`) |
| Orders to pilots (`Order`: route, then, flee) | Gone. The world only tells pilots what happened to their ship, and services' answers |
| The pirate flag in status and snapshots | The pirates' own network, inside the pool (`Crew`) |
| Hunts in status; hunt and posse counts | The operator's own tally, reported for display |
| Kill credit by role | The law's evidence: the downed was fair game (`aggressors_downed`), or the killer was (`innocents_killed`) |
| Turret gunners thinking in the tick | In the client pool. Their orders are postings for the guns, due two ticks on |
| `pool: Pool`, `cockpit: Cockpit` fields | `npcs: Box<dyn Pilots>`, `player: Option<Box<dyn PlayerClient>>` (the contract's traits). Late and dropped counts are the world's own |
| `pirate` and `trader` in the client's view of crafts | Gone |

The player's requests (clearance, autopilot, follow, lock and so on), the pilots' test hooks and
the full save live in the client modules as `impl Universe` blocks. `Universe::new` (with
clients) is in `setup`.

**Clustering prep:** the world talks to its clients only through `contract`. The `Pilots`
trait is the seam where a network transport goes.

## Tests

- **New:**
  - `the_engine_knows_no_intentions` (it fails when a violation is planted);
  - `a_trader_asks_for_quotes_decides_and_trades_at_its_stop`: bought 19 sterile therapies,
    heading for Port Erinedso, expecting +1,659.
- 75 tests, all passing.
