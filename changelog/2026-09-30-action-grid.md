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
