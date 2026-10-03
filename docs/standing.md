# Law and standing

Who is welcome where. There are no factions to join, found or claim for (removed 2026-10-03: too
close to another game's pledging and powers); what's left is local and earned by deeds.

## Law

- Every **settled system** (one with a station or a port) enforces the law: one who opens fire
  on the innocent there is **fair game** for 10 minutes (`standing::FAIR_GAME`), to anyone and to
  its defence turrets. Unsettled space has no law.
- The law is a service (`services/law.rs`): it rules on weapon hits as they happen; a collision
  is no deed.

## Standing

Each settled system's authority keeps a **standing** for every pilot (the player, settlers,
pirates alike), -100 to +100, from the deeds done in its system as they reach its station over
the hypernet (`sim/src/standing.rs`): a deed far out counts late, one nobody saw never does.

| Deed (in that system) | Standing | Kind |
|---|---|---|
| Opening fire on someone not fair game | -10 | Tuning |
| Destroying someone not fair game | -30 | Tuning |
| Destroying someone fair game | +5 | Tuning |
| A trade at one of its markets | +0.2 | Tuning |
| A collision, however it ends | 0 | (an accident, not a deed) |

| Standing | Means |
|---|---|
| -50 and below | **Hostile**: its turrets fire, its docks refuse (`standing::HOSTILE`) |
| -49 to -10 | Unwelcome |
| -9 to +9 | Neutral |
| +10 to +49 | Friendly |
| +50 and up | Honoured |

- A lost ship pays the debt where it comes back: hostile there, it's lifted to just above.
- Shown on the HUD (STAND), with an alert when hostile; saved with the game.
