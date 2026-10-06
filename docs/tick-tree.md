# The tick tree: governing time as the engine grows

A proposal, 2026-10-06, reviewed by the integrator against the code the same day (their seven points taken in), then settled with the user into the tree of §3, which the registry now holds as records under `standards/Engine` for the engine to read. It reviews how time
advances today (read from the code, file and line given), then proposes a tree of clocks:
galaxy, region, star system, and within a system the clocks from realtime contact at 5 ms up to
the star chart at a day. The rule that places every piece of work on a thread, a process or a
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

## 3. The tree, as the registry holds it

The user's design (2026-10-06) after the review of §1 and §2, and now **data**: one record per
clock in `standards/Engine/metadata/scheduling/` (schema `Engine/schema/clock.schema.yaml`;
words `clock_scope`, `coupling`, `trigger_kind` in the dictionary). The engine reads the periods
there and nowhere else; the build's **Clocks** report draws the tree and checks that every period
is a whole number of the realtime tick.

```
Galaxy ····························· 1 day   star chart publish; the calendar; galaxy-wide records
└─ Region ·························· events  transit timetable, hand-overs, hypernet, supernovae
   └─ Star system (group) ··········         one node owns whole systems; never split
      ├─ Celestial ················· 60 s    body positions published; bodies stay formulas
      │   └─ Planetary (per body) ·· 60 s    weather and air where a history is needed; geology an empty step
      │       └─ Economy (per settlement) 10 s   production, consumption, construction, wear, mines
      │           └─ Market ········ 1 s     boards and matching; a trade is an event
      ├─ Machinery (per craft) ····· 1 s     power, fuel, heat, life support; schedules its cut-offs onto realtime
      ├─ Rails (per craft) ········· event   coasting and FTL cruise as functions of time, physics wakes
      ├─ NPC lane (per bubble) ····· 1 s     unwatched NPC vs NPC, coarse; promoted when a player can see
      └─ Realtime (per bubble) ····· 5 ms    the critical lane: only seamless cross-player physics, only where seen
```

Each clock record says: its `parent` (whose due time makes it step), `scope` (one per galaxy,
region, system, body, settlement, craft or bubble), `coupling` (tight, fast-loose, slow-loose:
which decides thread, process or machine), `trigger` (a period in seconds, an event list, or a
group), `runs` (the work bound to it, one line each: the engine binds a handler per clock key),
`reads` and `posts` (which clocks' published state it reads, which it posts stamped operations
to), and `rule` (what keeps it lean). Each step, a clock reads the slower layers' last published
state and posts to the faster ones with a tick stamp; **no clock waits on a slower one, and no
participant runs inside another's step.** All periods are whole multiples of the 5 ms tick
(1 s = 200; 10 s = 2,000; 60 s = 12,000; a day = 17,280,000), so every input is stamped by the same
tick numbers and replay works per clock.

### 3.1 The guidance

Keep the realtime lane lean: only physics that requires seamless cross-player interaction, and
only where a player can see. Non-visible effects are cleaned off it: power, fuel, heat and life
support to **Machinery** (which predicts the exact tick of every cut-off and posts it stamped, so
what a player sees happen arrives on time while a gauge may lag a second); coasting ships and FTL
cruise to **Rails** (a straight line or an orbit as a function of time with a scheduled exit, so
in-system FTL that crosses 0.1 AU in seconds is never stepped and never regrouped); unwatched
NPC action to the **NPC lane**, promoted the moment a player's view reaches it. Realtime cost
scales with players, not ships.

### 3.2 Bubbles

A bubble is everything transitively within reach of each other, found by a spatial hash sized to
a tick's motion plus reach, anchored to the nearest body's frame near bodies and the star's frame
in deep space (a fixed 0.1 AU cell would drift against the planets and be crossed by FTL every
tick). One bubble is one thread's work for one tick; results are applied in a fixed order (lowest
craft id); bubbles merge or split only at tick boundaries. The realtime lane evaluates the bodies'
formulas itself for the bodies a bubble is near (an equator moves at 460 m/s; a minute-old pad is
28 km away); the Celestial clock is the cache for everyone else. A rails ship is promoted into a
bubble a few ticks before it can touch anything: wakes are re-checked whenever a bubble's powered
set or bounds change, with horizon reach + v·T + a_max·T²/2, and come from physics or an arriving
command only (a pilot's next think is the client's; the core holds no intentions).

### 3.3 Bodies as the unit of CPU scaling

The economy sits under its body (Celestial → Planetary → Economy → Market) because it is local:
what crosses bodies is hauled stock (a ship, on rails or realtime) or a message (a board over the
hypernet). So a body's branch is one unit of work with slow edges to everything else: bodies fan
out across cores inside a node, systems across nodes, and the realtime lane stays as small as the
players make it.

### 3.4 The network, tied to the tick

The input deadline *k* is in ticks: at 5 ms, k = 2 means a player within 10 ms of the node that
owns their system. A player further away gets a larger k (round trip ÷ 5 ms), per player, stamped
the same way; nobody should expect 10 ms across an ocean.

## 4. The player's point of view

The player sees one place in full and the rest as information. In the tree that is: their bubble
at 5 ms on one thread (ship, contacts, interior, crew walking); their system's rails ships as
contacts on radar, promoted into their bubble before they can touch it (the prediction horizon
guarantees a promotion at least a few ticks before contact: a ship can never appear inside reach
unintegrated); the system's L2 services as the cockpit's picture; the economy as boards and
yards' stock that change on the 10 s step; the planets as pure functions with an hourly weather;
the civilization as news and records. Leaving a system is a hand-over message; arriving, the
new system's node owns them. **A player never notices a layer boundary except as a cadence of
information**: a board updated at the step, the weather turning on the hour.

Warp (single-player, a dev tool) becomes cheap: only watched bubbles run realtime at warp; rails
ships are functions of time; the slow layers catch up by their steps as the economy already does.

## 5. Determinism, replay, checkpoints

Every layer's inputs are stamped with the tick they're due at; a layer's state at a step boundary
is a function of its state at the last boundary and the stamped inputs between. So replay works
per layer, and the **checkpoint problem of R9** (the seed plus the whole input log, replayed from
tick 0, grows without bound) has its answer: checkpoint each slow layer at its own boundary (the
economy at a step, the civilization at a day) and replay only the realtime and rails lanes since the last checkpoint. A
save is: seed, the slow layers' states at their last boundaries, the input log since. The
`state_hash` audit stays, per layer.

## 6. Physical resourcing

The user's rule, applied to the tree:

| Work | Exchange | Runs on | Why |
|---|---|---|---|
| A bubble (realtime) | tight | one thread, one tick | contact needs exact order; a message boundary inside a bubble is a wrong answer |
| Bubbles of one system | independent within a tick | the system's cores, side by side | nothing crosses between bubbles inside a tick |
| Pilots, cockpit, L2 | fast-loose | other threads, same process | snapshot read, post due at N+k; stale by a known age |
| Economy, Market, Planetary, Machinery of a system | slow-loose | actors on a pool; another process or machine when wanted | a second to a day between exchanges: wire latency invisible |
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
its own 200 Hz loop for its own systems; nodes agree only at the region step (seconds) and on
hand-overs. Clocks are reconciled by bounded drift, not by lockstep. Within a node, the work
queue per tick is the list of bubbles, scheduled on the node's cores by a work-stealing pool
(rayon, as now); no per-ship scheduling.

**Scaling path, one contract throughout.** The message bus is in-process first (rearchitecture
§10 item 6, decided), so the steps are:

1. **Now**: one process, one machine, one tick for all (R8: 100k ships ≈ 25 ms).
2. **Measure, then layers and bubbles in one process**: first ms per tick split by powered /
   near / coasting ships; then bubbles as the parallel unit, slow layers as actors on a pool, and
   rails with wakes if coasting is where the time goes. The cost moves from ships to activity.
   Realtime ms per tick per system is the one number that sizes everything after.
3. **Systems as process-local shards**: each system group on its own thread with its own 200 Hz
   loop, hand-overs as in-process messages. Tests the contract with no network.
4. **Systems as processes on one machine**: the same messages over a socket; proves isolation.
5. **Systems on machines**: the same again over a network; the region and universe services on
   their own nodes; systems placed by players.

Nothing is rewritten between steps; each one changes the transport under the same interface.

**Sizing, to be measured, not assumed.** The machine in use has 32 threads and 60 GB. If the realtime lane
holds only watched ships, a system's realtime work is tens of bubbles, not thousands of ships, and one node
holds many systems; the budget split (cores − 3, halved between crowd and pilots, `engine.rs:854`)
becomes per node: cores for bubbles, cores for pilots, a few for services. Memory: a system's
snapshot and rails cache (small), the planets' bakes read-only and shared across processes on a
machine (mapped, not copied). A node's capacity is the number of systems whose realtime lane fits 5 ms with
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

## 8. Decided (2026-10-06)

The periods in §3 are the user's and are configuration in the registry; each may be tuned with a
reason on its record. Still the user's to confirm as the build meets them: walking crew into the
realtime lane as the bubble's bodies (decks as core data); checkpoints per clock at their
boundaries in place of seed-plus-whole-log; whether the NPC lane resolves fights or only movement.

## 9. What breaks, and who does what

The integrator's build: the engine reads `reg.clocks` and binds a handler to every clock key
(exhaustively, so a new clock fails the build until it is handled); `TICK_HZ`, `TICK`, the
economy's `STEP`, `BOARD_EVERY` and the rest become reads of the records; then steps 2 and 3 of §6
in `crates/sim` and `crates/world` (bubbles, rails with wakes, actors per body, walking into the
tick). The schema and the records are on fso. The lab's: a weather field format if decision 4 is yes. Mine: the survey
contract's weather package, the census periods, and the review of what each registry rate implies
at each step (a mine's kg/s at 600 s; a levy at a day). The ships session: nothing; a ship's
devices are read the same at any layer.
