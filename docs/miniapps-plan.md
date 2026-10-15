# Mini apps on one engine: implementation plan

*2026-10-14. The owner's goal: the planet view, the ship studio, later a factory layout studio and internal
scenes and scenarios, each a mini app stacked on the engine. Each app is isolated, uses shared components, and
declares which clocks it runs (for example planetary clocks for the planet app, player/realtime clocks for
another), separate from the live world's tick. Branch `engine-claude-miniapps`.*

## 1. Where we are (audit of main 41661fb6)

**Right already**
- Engine: one window/loop (`engine::app::Game`, `run_prepared`), one renderer and model cache, isolated retained
  scenes and several viewports a frame (`scene::Scene`, `Frame::scene_view`), a generic fixed step
  (`scene::TickClock`). The engine depends on no simulation crate.
- Ship Studio is the template: its own binary (`freefall-studio` → `studio_only::StudioOnly`), no universe loaded;
  DRIVE runs an owned minimal `World` through the normal `World::step_ship` with its own `TickClock`; WALK owns its
  walker, clock and scene (`docs/studio-shared-engine.md`).
- Core crates (physics, world, sim, services, registry) do not depend on the game crate.
- The tick tree is data: twelve clock records (`standards/Engine/metadata/scheduling`), bound exhaustively
  (`sim::clocks::Binding`, a new clock fails the build).

**Missing**
1. No mini-app host. `Game` is one app per window per process. The game's `App` (`game/src/lib.rs:154`, about 60
   public fields) hosts the planet studio and the shipyard as `Layer`s chosen by an if-chain (`top_layer`); 28 files
   take `&App`. Studio exists twice (its binary and the shipyard panel).
2. The planet view is not a component. The planet studio fakes a system (`studio_world`, `STUDIO_CACHE =
   usize::MAX`) and draws through `scene.rs`, `terrain_lod` and `caches.globes`, all of which read `App`; it can only
   open after `App::new` has started the universe.
3. No clock subscription. The live world has one realtime tick (`Universe::tick`, `sim/src/universe.rs:321`); slow
   clocks catch up inside it (`clocks::every`, `clocks::due`). `Binding` returns descriptions, not handlers.
   Apps that tick roll their own `TickClock` with no tie to the tree; the planet studio has none, and the live world
   runs under it unless paused.
4. Isolation by convention: panels write shared `App` state (observer focus, `caches.world_maps`, `send_layout`).

## 2. Target shape

```
crates/engine            window, renderer, assets, input, profiler, Scene/View   (no sim)
   └─ engine::apps       Host + App trait + per-frame clock (the player's)       (no sim)
crates/core/sim          clocks::Runtime: the tick tree over one world instance
crates/shell (new)       binds the two: Subscriptions -> Runtime; launcher; app registry
crates/apps/*            planet, studio, game (later factory, scenes) — each its own crate
crates/parts/*           shared components: planet view, HUD widgets/panels, flight input
```

### 2.1 The app contract (engine, sim-free)

```rust
pub trait App {
    fn name(&self) -> &str;
    /// Input and the host's frame clock (real dt). Return what the host should do next.
    fn update(&mut self, ctx: &mut Context) -> Next;          // Stay | Close | Open(Box<dyn App>) | Swap(..)
    fn camera(&self) -> Camera;
    fn draw(&self, frame: &mut Frame, ctx: &Context);
    /// Whether the apps under it keep running while it is on top (an overlay vs a full app).
    fn covers(&self) -> Cover { Cover::Pauses }               // Pauses | Draws | Runs
}
```

- The **Host** owns the window, renderer, asset leases, input routing, profiler, F3 HUD, captures
  (`UNIVERSE_SCREENSHOT`, `UNIVERSE_TEST`), and a stack of apps. Only the top app sees input. `Cover` says what the
  apps under it still do, so the planet app over the game can pause the game's world or let it run (decided per
  app, not by convention).
- **The player's clock** is the host's frame clock (real time, per frame): camera, UI, animation. It is never a
  world clock and never replayed.
- `engine::app::run` stays as the one-app case (`Host` with one entry), so nothing breaks while apps move.

### 2.2 Clocks an app runs (sim)

`sim::clocks::Runtime` is the tick tree as a runnable scheduler over one **world instance** (its own time, its own
time scale, its own state):

```rust
pub struct Subscriptions(BTreeSet<ClockKey>);                // e.g. {Celestial, Planetary}
impl Runtime {
    pub fn new(subs: Subscriptions, scale: f64) -> Self;     // periods from the records, as now
    pub fn advance(&mut self, real_dt: f64, w: &mut impl Clocked) -> Steps;
    pub fn scrub(&mut self, t: f64);                         // jump world time (planet timeline)
}
pub trait Clocked { /* one method per ClockKey, generated like ClockHandler: a new clock fails the build */ }
```

- A subscribed clock steps at its record's period: world clocks by catch-up (`while done_to + every <= now`, the
  economy's `stepped_to` pattern), realtime by fixed ticks (`TickClock`'s queue, kept). An unsubscribed clock does
  not run at all; its state is whatever the instance was built with.
- A parent not subscribed is not stepped, but its published state is still read (Celestial is a function of time,
  so a planet app without Galaxy still has positions).
- The live game is one instance with every clock. A lab app gets its own instance; an app that only looks at the
  live world reads its published snapshot (`Arc<sim::View>`, as now) and runs no clocks.

| App | World instance | Clocks |
|---|---|---|
| Planet viewer | own: one body (and its star) | Celestial, Planetary; scale or scrub |
| Ship Studio, DRIVE/WALK | own lab world | Realtime, Machinery |
| Factory layout | none, or own lab | none; or Economy, Market, Realtime |
| Scenes/scenarios | own, seeded from a record | per scenario file |
| Game | the live world (engine thread) | all |

### 2.3 Shared parts

- **Planet view** (`parts/planet-view`): globe, maps, ground patches (`terrain_lod`), air and cloud setup, its caches,
  taking a body, its surface and a camera. No `App`, no system index (`STUDIO_CACHE` goes).
- **HUD kit** (`parts/hud`): panels, lists, palette, fonts, tables with headers, key help. Pulled from `hud.rs` and
  the panels as each app moves, not ahead of need.
- **Flight input**: already shared (`flight_input`, `manual`, `world::flight_control`).

## 3. Steps

Each step is a commit (or a few), builds, passes the fast suite, and leaves both binaries working. Captures go to
`out/review/miniapps/`; test windows on workspace 8.

1. **Host and trait.** `engine::apps::{App, Host, Next, Cover}`; `engine::app::run` becomes a one-app Host. Test:
   a stack push/pop/cover unit test (CPU only), and the existing launches unchanged.
2. **Studio as the first app.** `StudioOnly` implements `App`; `freefall-studio` launches it through the Host.
   Acceptance: `freefall-studio <design>` opens, WALK and DRIVE behave as before (one capture each, matched against
   main's), `--report/--fit/--compare` unchanged.
3. **Clock runtime.** `sim::clocks::{Runtime, Subscriptions, Clocked}`, the trait generated beside `ClockHandler`.
   DRIVE and WALK move from hand-rolled `TickClock` to a Runtime subscribed to {Realtime, Machinery}. Test: a
   runtime with {Economy} steps it at its period and never steps Realtime; scrub and scale; replay of a
   subscribed set is identical (fast, CPU only).
4. **Planet view extracted.** Move globe/terrain drawing out of `scene.rs` and `App::caches` into
   `parts/planet-view`; the game draws its bodies through it. Acceptance: matched captures, orbit and 2 km,
   pixel-identical to main.
5. **Planet viewer app.** `apps/planet`: a world instance with one body, {Celestial, Planetary}, the planet studio's
   list, timeline (scrub), looks; its own launcher (`freefall --app planet <world>` and a binary). Opens without the
   universe. The game's WORLDS key pushes it over the game with `Cover::Pauses`. `studio_world`, `world_frame`,
   `world_look`, `Layer::Worlds` leave `App`.
6. **Game as an app.** `App` implements `apps::App`; the shipyard pushes the Studio app instead of being a Layer
   (one Studio, not two). Remaining panels stay inside the game for now.
7. **Launcher.** `freefall --app <name>` and an app list; an app named on the command line opens alone.
8. **Then, when wanted:** factory layout app (needs its registry pieces), scenes/scenarios (a scenario file names
   the world seed, the clocks and the app).

Steps 1 to 3 are small and independent of the planet work. Step 4 is the largest (the planet drawing is the most
entangled with `App`). Step 6 is where most of the 28 `&App` call sites stay untouched: the game keeps its
internals, it only stops hosting the other apps.

## 3.1 Where it stands (2026-10-14)

| Step | State | Commit |
|---|---|---|
| 1 Host and trait | done: `engine::apps` (App, Host, Next, Cover, plan) | c1219828 |
| 2 Studio as an app | done: empty Studio capture byte-identical to main | ace4995e |
| 3 Clock runtime | done: `sim::clocks::{Runtime, Subscriptions, Clocked}`; DRIVE and WALK on it; DRIVE/WALK captures byte-identical | b1989495 |
| 4 Planet view extracted | done: `game::planet_view`; game captures differ from main as main does from itself | 16d95e35 |
| 5 Planet viewer app | done: `freefall-planet [world id]`, opens in about 0.8 s without the universe | c008e653 |
| 6 Game as an app | done: WORLDS opens the planet viewer over the game; planet studio state gone from `App` | 41ce3ac5 |
| 7 Launcher | done: `freefall --apps`, `freefall --app <game, planet, studio> [arg]` | 02362b49 |
| 6b Shipyard as an app | done: `ShipyardApp` over the game; `Cover::Covers` and `Context::covered` (the game runs on under it, undrawn) | ce8229ed |
| 8a Scenes | done: `freefall --app scene <file>`; a lab copy of the home system, crafts on orbits, only the file's clocks (Realtime, Celestial, Planetary); `content/scenes/two-orbits.scene.yaml` | f2d3af8b |
| 8b Factory layout | done: `freefall --app factory [facility]`; a works' modules as boxes at their records' sizes on its parcel; move, turn, save; out of place counted; no clocks | 37134eb8 |
| 8c Planet LOD | done: `freefall --app planetlod [view]`; the planet-trees pattern's levels of detail, per-vertex or cluster DAG on the mesh pool; Yosemite (USGS) at 120 fps | engine-claude-planetlod |

Deviations from the plan above: apps are modules in the `game` crate, not crates of their own
(§5's second question, left open); DRIVE and WALK subscribe to Realtime only (Machinery has no work
of its own in a lab world yet); `Cover::Pauses` is named for what it does to the apps under it,
and the game under the planet viewer and the shipyard is covered (`Covers`: updated without input,
not drawn), so the live world never stops for an app. The shipyard needed no hand-back: its deck
document was always copied from the game once and kept to itself. The standalone Studio and the
shipyard still host the interior studio separately (the shipyard adds the 2D deck page); the
editor itself is one. §5: the live world runs on under an app (multiplayer: it never pauses for
one player); apps stay modules of `game` while the shared parts depend on its modules.

## 4. Rules kept

- The engine holds no simulation types; the sim holds no app or window types.
- Core and services hold no client intentions; an app's clocks run world laws, never an app's policy.
- Periods come from the clock records only; nothing in an app names a period.
- No copies of laws in an app: a lab world calls the same `World` steps as the live one.
- Fast tests only; no long simulations.

## 5. Open for the owner

- Whether an app over the game pauses the live world by default (`Cover::Pauses`) or lets it run (multiplayer: the
  live world never pauses for one player, so `Runs` there).
- Whether apps become separate crates now (cleaner isolation, slower to move) or modules in `game` first and crates
  at step 6.
