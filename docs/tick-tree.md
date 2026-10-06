# The tick tree: governing time as the engine grows

A proposal, 2026-10-06, for the integrator to build and the user to decide. It reviews how time
advances today (read from the code, file and line given), then proposes a tree of clocks:
universe, region, star system, and within a system the layers from contact physics at 60 Hz down
to civilization at days. The rule that places every piece of work on a thread, a process or a
machine is the user's: **where the exchange is slow, scale out to machines; where the data
exchange is critical, isolate and compute as close as possible, same thread, same process.**
Coupling decides placement; the tree is the map of couplings.

Figures marked *proposal* are starting points to measure, not facts.

## 1. Today

One global tick, everything in it (`crates/sim/src/universe.rs:327`).

| What | Cadence today | Where |
|---|---|---|
| The tick | 60 Hz, fixed 1/60 s of game time; up to 8 ticks a frame under warp, then ticks stretch | `universe.rs:16-18, 312` |
| Every craft in every system | every tick, side by side on all cores; a coasting ship 1,000 ly from anyone is integrated like one on final approach | `traffic.rs:137-154` |
| Integration | leapfrog with adaptive substeps, 0.05 s when powered or within 30 km of a collider, else from the orbit; up to 2,000 substeps a call | `physics/src/integrate.rs:18-22, 86` |
| Planets, stations, rotation | analytic, a pure function of time, two moments cached a system | `physics/src/rails.rs`, `world.rs:466` |
| Combat, projectiles, collisions | every tick; projectiles at 0.05 s; swept | `combat.rs:27`, `collisions.rs:155` |
| ATC presence | every 6 ticks | `universe.rs:21` |
| Ground ahead | every 30 ticks | `universe.rs:23` |
| Dead-man rule | every 60 ticks | `universe.rs:382` |
| Standings | every 5 s | `standing.rs:40` |
| Price boards | every 120 s, spread over the hypernet | `commerce.rs:19` |
| Economy (recipes, people, trading) | catch-up loop in 600 s steps, called every tick, returns early | `services/src/economy.rs:38, 731` |
| Land levy | 86,400 s, catch-up | `land.rs:122` |
| Ledger, law | event-driven, no step | `ledger.rs`, `law.rs` |
| NPC pilots | own pool; think every tick when busy, 0.5 s coasting, up to 5 s parked; commands due 2 ticks after | `pilots.rs:44-48, 407` |
| Player cockpit | per tick view; plan every 0.1–1 s | `cockpit.rs:73, 155` |
| Walking crew | not in the tick: per client frame, real dt | `main.rs:803`, `crew.rs:310` |
| Climate | a pure function (light, temperature) read by the heat step; no weather state | `world/src/climate.rs` |
| News, hypernet status | client side, per frame, 0.5–600 s | `main.rs:527, 1491` |
| Save | seed plus the input log, replayed from tick 0 | `audit.rs`, `operator.rs:285` |

What is already right: the fixed game-time tick; the economy's catch-up loop (`stepped_to`,
step when due) as the pattern for every slow clock; the pilots' think-rate levels of detail;
rails as pure functions of time; the per-system freeze for a lock-free parallel step; the input
deadline *k*. What does not scale: **one clock, one place, every ship every tick.** The core's
cost is proportional to ships, not to activity (rearchitecture §4.2 asked for the opposite, and
got it for the pilots only). There are no shards, bubbles or sleeping bodies in the core
(R8 reached 100,000 ships at about 25 ms on one machine by stepping all of them). Two things
sit outside the tick that belong in it: walking (per client frame, so not replayable) and the
news. Nothing of the planets ticks at all, which is right for orbits and rotation and will stay
right; weather, when it comes, must not be put on the 60 Hz clock.

## 2. The principle: three couplings

Every piece of work in the game exchanges data with something else at one of three tightnesses.
The tightness, not the kind of work, decides where it runs.

| Coupling | Exchange | Example | Placement |
|---|---|---|---|
| **Tight** | within a tick, exact order, swept contact; a late read is a wrong answer | two ships within reach of each other; a ship on a pad; a projectile and its target; a walker on a deck | **same thread**, shared memory, no messages: one bubble, one thread, one tick |
| **Fast-loose** | within a few ticks; a stale read is fine if the age is known | a pilot thinking on a snapshot; the cockpit; sensors; ATC | **same process, other threads**: read the tick's frozen snapshot, post commands due at tick N+k (as today) |
| **Slow-loose** | seconds to days; order by stamped time is all that matters | a trade, a recipe's hour, a census, a levy, the weather, a law record, a hypernet message | **messages**: other process, other machine; latency of a network is invisible under a 600 s step |

The rule's two halves: tight work is never split across a message boundary (a bubble is never
cut between two nodes; two players in one fight are on one thread), and slow work is never put
on a fast clock (a market does not tick at 60 Hz because a ship does). Everything in between is
a snapshot read plus a due-stamped post, the contract that exists.

## 3. The tree

```
Universe        the calendar; standards and registry releases; galaxy-wide records     days
└─ Region       200 ly: hypernet routing, boards, transit timetable, fleets' orders    1–60 s
   └─ Star system   the unit of ownership: one node owns whole systems
      ├─ L0 contact        bubbles: ships, projectiles, pads, walkers        60 Hz, fixed
      ├─ L1 flight         coasting ships on rails with a scheduled wake    on event / 1 Hz
      ├─ L2 system services  ATC, sensors, standings, dead-man, recorder    1–10 Hz
      ├─ L3 settlement      economy: recipes, stock, markets, construction  600 s (as today)
      ├─ L4 planetary       weather, tides, air state, seasons              1 h game time
      └─ L5 civilization    census, people, administration, levies, law     1 day
```

Each node of the tree has its own `stepped_to` and steps when due, exactly as the economy does
today; the parent passing a due time is what makes a child step. Faster layers read the slower
layer's last published state (the hour's weather field, the step's market board); slower layers
read a snapshot of the faster ones and post operations due at a tick. **No layer ever waits on a
slower one, and no participant runs inside another's step.** All periods are whole multiples of
the tick (60 Hz; 1 s = 60; 600 s; 3,600 s; 86,400 s), so every layer's inputs are stamped by the
same tick numbers and replay works per layer.

### 3.1 Universe

One clock, the slowest. It owns the **calendar** (world time's authority, the fixed 1× time
scale), the registry's releases (standards, products, prices boards' categories), galaxy-wide
records (law records that follow a ship across regions, the census of fleets, insurers' books).
It ticks in days and talks only in messages. It is never on the path of anything a player feels.

### 3.2 Region

The charted 200 ly region, and later more regions over more nodes. It owns what crosses systems:
**hypernet** routing and delivery (news, boards, requests), the **transit timetable** (a ship
between systems is a line in a table: departed at *t*, arrives at *t′*; nothing integrates in a
gate tube), fleets' ordering of replacements, regional price boards (the 120 s spread today).
Period 1–60 s *(proposal)*. Hand-over of a ship between systems is a region message carrying the
ship's full state and the tick it left at; the receiving system stamps it into its own tick.
Transit takes seconds to minutes of game time, so the wire's latency is invisible.

### 3.3 Star system: the unit of ownership

One process owns whole systems; a node owns as many systems as fit its 60 Hz budget. Nothing
inside a system is shared with another node except through the region. The system's own clock is
authoritative for its contents; it agrees with the calendar within one region tick (a bounded,
known drift, reconciled at the region step, never by stalling the tick). A system nobody is in
and nothing is happening in runs L0 empty, L1 by events, and the slow layers on schedule: its cost
is its economy's step every 600 s and nothing else.

### 3.4 L0: contact, 60 Hz, bubbles

The microcosm. A **bubble** is a cluster of cells (a cell sized to a tick's motion plus reach,
rearchitecture §4.1) that holds everything within reach of each other: ships, pads, projectiles,
walkers, the rails bodies they're near. One bubble is one thread's work for one tick; bubbles in a
system are independent within a tick (nothing in one can touch another until next tick, by
construction of the cell size), so they run side by side on cores, and the parallel unit becomes
the bubble, not the ship. The per-system freeze stays as the lock-free world the bubbles read.

What's in L0: any ship powered (thrust, RCS firing) or within reach of a collider (30 km today, a
tick's motion plus reach tomorrow); a ship on a pad, taxiing, in a hangar; projectiles; **walkers**
(moved here from the client frame: a walker is a core body stepping at 60 Hz in its ship's bubble,
replayable; rearchitecture §10 item 5, the proposed answer). Collision is swept per tick as now.

The player is always in L0: their ship, its bubble, its interior. Several players in one bubble
are on one thread on one node: the only true synchronisation point in the design, and the one
the input deadline *k* exists for.

### 3.5 L1: flight on rails, by event

The macrocosm's ships. A coasting ship (no thrust, nothing within reach) is **a rails body with a
scheduled wake**: its state is an orbit (or a straight line in deep space) valid from tick *t*;
its position at any time is a pure function, like a planet's. It is integrated only when
something changes: its pilot posts a command (it wakes into L0 if powered), or the wake it
scheduled arrives. The wake is the earliest of: closest approach to any collider or bubble within
the prediction horizon (reach plus speed × a few ticks, computed when it goes on rails), the
pilot's next think, atmosphere entry, a region hand-over. With no event, an L1 ship costs nothing
a tick. A 1 Hz sweep *(proposal)* re-checks wakes against bubbles that moved.

This is the pilots' think-rate idea applied to the physics: cost follows activity. Of the census's
~1,000 ships in Treistun, the share in L0 at any moment is the share docking, fighting, launching
or near a port: measure it; the pilots' own states (busy / coasting / parked) already say.

### 3.6 L2: system services, 1–10 Hz

ATC presence (6 ticks today, keep), sensor publishing for NPC pilots (decision 4 in the
rearchitecture, open: *proposal* 10 Hz for pilots, 60 Hz for the player's cockpit; a pilot in a
docking run can ask for the tick rate), standings (5 s), dead-man (1 s), the flight recorder's
slices, ground-ahead. These read the system snapshot and post; none is in the tick.

### 3.7 L3: settlement, the economy, 600 s

As today, and today's design is the pattern: `stepped_to`, step when due, each settlement a
place with its recipes, people's aims, stock, market, construction. What changes: it runs as an
actor per settlement (rearchitecture §4.4) in its own thread pool or process, reading the
system snapshot (who is docked, what was unloaded) and posting operations (a trade settles through
the ledger as a message; a built hull appears in a yard's stock at the step). A **trade is an
event, not a tick**: the market answers a request when it arrives, stamped; the 600 s step is for
production, wear (`life`), consumption and the people's aims. Wear, takers, construction, mines
drawing down a claimed deposit: all here. The registry's rates are kg/s, so the step integrates
rate × 600 and nothing in the registry depends on the period.

### 3.8 L4: planetary, hourly

Nothing of a body's motion ticks: orbits, rotation, tilt, tides' geometry are pure functions of
time and stay so. What L4 steps is **state**: weather (a field a body: pressure, wind, cloud,
rain, from the lab's climate and air tables, advanced hourly in game time *(proposal)*),
atmosphere state (temperature by latitude and hour; today's `climate` as a function stays the
fallback), tides as a scalar, seasons (a date). A ship in air reads the hour's field; the heat
step (every 0.1 s in air today) reads it as a constant between hours. Geology **never ticks in
game time**: it is the lab's history package; deposits change only by mining events (L3).

### 3.9 L5: civilization, daily

Census (who lives where, who works, births, deaths, moves between settlements), the land levy
(86,400 s today, here), administration (zoning breaches recorded, policies, elections when they
come), insurers' premiums, compulsory-stock checks, law's slow side (records aging, appeals
heard, the statute's periods), fleets' replacements ordered, the hypernet's digests. All in
messages; a day of game time is 24 hours of wall time at 1×, so this layer is nearly free and can
run anywhere.

## 4. The player's point of view

The player sees one place in full and the rest as information. In the tree that is: their bubble
at 60 Hz on one thread (ship, contacts, interior, crew walking); their system's L1 ships as rails
contacts on radar, promoted into their bubble before they can touch it (the prediction horizon
guarantees a promotion at least a few ticks before contact: a ship can never appear inside reach
unintegrated); the system's L2 services as the cockpit's picture; the economy as boards and
yards' stock that change on the 600 s step; the planets as pure functions with an hourly weather;
the civilization as news and records. Leaving a system is a hand-over message; arriving, the
new system's node owns them. **A player never notices a layer boundary except as a cadence of
information**: a board updated at the step, the weather turning on the hour.

Warp (single-player, a dev tool) becomes cheap: only watched bubbles run L0 at warp; L1 ships are
functions of time; the slow layers catch up by their steps as the economy already does.

## 5. Determinism, replay, checkpoints

Every layer's inputs are stamped with the tick they're due at; a layer's state at a step boundary
is a function of its state at the last boundary and the stamped inputs between. So replay works
per layer, and the **checkpoint problem of R9** (the seed plus the whole input log, replayed from
tick 0, grows without bound) has its answer: checkpoint each slow layer at its own boundary (the
economy at a step, the civilization at a day) and replay only L0/L1 since the last checkpoint. A
save is: seed, the slow layers' states at their last boundaries, the input log since. The
`state_hash` audit stays, per layer.

## 6. Physical resourcing

The user's rule, applied to the tree:

| Work | Exchange | Runs on | Why |
|---|---|---|---|
| A bubble (L0) | tight | one thread, one tick | contact needs exact order; a message boundary inside a bubble is a wrong answer |
| Bubbles of one system | independent within a tick | the system's cores, side by side | nothing crosses between bubbles inside a tick |
| Pilots, cockpit, L2 | fast-loose | other threads, same process | snapshot read, post due at N+k; stale by a known age |
| L3–L5 of a system | slow-loose | actors on a pool; another process or machine when wanted | 600 s to a day between exchanges: wire latency invisible |
| Systems | slow-loose through the region | **one node owns whole systems**; many systems a node | the only cross-system exchange is a hand-over, which rides a transit that takes seconds to minutes |
| Region, universe | slow-loose | their own services, any machine | messages at seconds to days |

**Where latency bites, and the answer.** Only three exchanges are latency-sensitive: the player's
input into their bubble (the deadline *k* = 2 ticks, 33 ms: a player's client must be within that
of the node owning their system, so **systems are placed by the players in them**, and a player's
hand-over moves them to the node that owns the next system); several players in one bubble (one
thread, one node, by construction); and the sensor picture to the player's cockpit (a tick behind,
as today). Everything else is a snapshot or a message, and tolerates the wire.

**Where management overhead comes from, and the answer.** A scheduler that coordinates nodes each
tick is the trap: it puts a network round trip inside the 16 ms. The tree has none. Each node runs
its own 60 Hz loop for its own systems; nodes agree only at the region step (seconds) and on
hand-overs. Clocks are reconciled by bounded drift, not by lockstep. Within a node, the work
queue per tick is the list of bubbles, scheduled on the node's cores by a work-stealing pool
(rayon, as now); no per-ship scheduling.

**Scaling path, one contract throughout.** The message bus is in-process first (rearchitecture
§10 item 6, decided), so the steps are:

1. **Now**: one process, one machine, one tick for all (R8: 100k ships ≈ 25 ms).
2. **Layers and bubbles in one process**: L1 rails with wakes, bubbles as the parallel unit, slow
   layers as actors on a pool. The cost moves from ships to activity. Measure L0 ms per tick per
   system: the one number that sizes everything after.
3. **Systems as process-local shards**: each system group on its own thread with its own 60 Hz
   loop, hand-overs as in-process messages. Tests the contract with no network.
4. **Systems as processes on one machine**: the same messages over a socket; proves isolation.
5. **Systems on machines**: the same again over a network; the region and universe services on
   their own nodes; systems placed by players.

Nothing is rewritten between steps; each one changes the transport under the same interface.

**Sizing, to be measured, not assumed.** The machine in use has 32 threads and 60 GB. If L0
holds only active ships, a system's L0 is tens of bubbles, not thousands of ships, and one node
holds many systems; the budget split (cores − 3, halved between crowd and pilots, `engine.rs:854`)
becomes per node: cores for bubbles, cores for pilots, a few for services. Memory: a system's
snapshot and rails cache (small), the planets' bakes read-only and shared across processes on a
machine (mapped, not copied). A node's capacity is the number of systems whose L0 fits 16 ms with
headroom for a fight; **a system is never split across nodes, and a bubble never across threads.**
A system too big for a node (a capital system in a battle) is the only case that forces a
decision, and the answer there is a bigger node, not a cut bubble.

## 7. What the registry carries

The registry's part is already cadence-free by design: rates are per second (recipes, needs,
wear by `life`), orbits are elements, climate is tables. Two additions follow from L4 and L5 if
the user wants them: the survey contract gains a **weather package** cadence (the lab's fields a
body an hour, or a generator the game runs from the climate tables; the lab's choice), and the
census's periods (a day) are stated in `docs/registry-people.md`. Nothing in the registry names a
tick rate, and nothing should.

## 8. Decisions for the user

1. L0 stays 60 Hz, and sensor publishing for NPC pilots goes to 10 Hz (rearchitecture decision 4).
2. Walking crew into the core tick (decision 5), as the bubble's bodies.
3. The economy step stays 600 s; trades are events.
4. Weather as hourly state from the lab's climate (L4), or none until later.
5. Systems placed by players (the node that owns a system is the one nearest its players), and
   the capital-system rule: a system never splits; a node grows.
6. Checkpoints per layer at their boundaries, replacing seed-plus-whole-log.

## 9. What breaks, and who does what

The integrator's build: steps 2 and 3 of §6 are engine work in `crates/sim` and `crates/world`
(rails with wakes, bubbles, actors for L3–L5, walking into the tick); nothing of the registry's
schemas changes. The lab's: a weather field format if decision 4 is yes. Mine: the survey
contract's weather package, the census periods, and the review of what each registry rate implies
at each step (a mine's kg/s at 600 s; a levy at a day). The ships session: nothing; a ship's
devices are read the same at any layer.
