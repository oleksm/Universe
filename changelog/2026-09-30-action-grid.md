# 2026-09-30 — Action grid, and R cancels a clearance

User: "put those modes like docking request, autopilot, map, arms etc into an action grid hud
somewhere top left. If I got docking request, I should be able to also cancel it by pressing d
again." D is roll, so this was read as R, the clearance key: pressing R again cancels.

- **Action grid** (pilot mode, top left, with the status text below it): 2×4 lit keys.

  | Row | Cells |
  |---|---|
  | 1 | `R CLEAR/DOCK/LAND/GATE`, `K AUTO`, `J HYPER`, `B ARMS` |
  | 2 | `T LOCK`, `M MAP`, `C CHASE/COCKPIT`, `F1 HELP` |

  Lamps:
  - off: available;
  - lit green: in use;
  - amber: in progress (weapons priming);
  - red: weapons hot;
  - dimmed: unavailable right now, e.g. clearance while armed, in hyperdrive or not flying, or
    LOCK with nothing on radar.
- **R toggles the clearance**: it requests one, or gives up the one held
  (`Avionics::cancel_clearance` / `Universe::cancel_clearance`). Cancelling also stops that
  clearance's autopilot and leaves the engines idle. The message is "CLEARANCE CANCELLED".

## Follow-up: cells show the next action; duplicate text removed

User: "should be R dock then R clear not the vice versa. Cleanup lines that were moved to the hud
actions."

- The R cell names what the key does next: `R DOCK` / `R LAND` / `R GATE` (for the nav target,
  else the nearest station) before a clearance, then `R CLEAR` (lit) while holding one.
- Removed from the text, since the grid shows them:
  - `HYPER OFF/ON/AUTO` on the throttle line;
  - `WEAPONS SAFE  B COMBAT MODE` on the hull line;
  - the `T TO LOCK` / `T NEXT` hints;
  - `K TO FLY` on the route line;
  - the `M NAV MAP  R REQUEST DOCKING` / `R REQUEST CLEARANCE` / `B TO GO SAFE` hints, leaving
    just `NAV <target>`;
  - the top line's `- WEAPONS HOT` / `- ARMING`, leaving just `COMBAT` in red.
