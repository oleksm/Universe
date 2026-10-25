# Audit: slots → fit → shipyard → flight (2026-10-24)

The owner's chain:
1. **Blender installs slots.** A slot is what the model supplies: a `mount_<slot>` frame, a kind, a size and a mount. Blender never needs to know the modules.
2. **The player fits modules** into slots. The slot's mount governs what fits.
3. **The shipyard validates the ship from the fitted modules**, not from the slots. That covers thrust, attitude, power, propellant, avionics and features.
4. **Engine flies it.**

The registry is the game's single source of truth, so it must describe every stage. The game implements what the registry describes.

Game evidence: ~/git/universe at 7b6f89435 (file:line below).

## Stage by stage

| Stage | Registry describes it? | Game today | Verdict |
|---|---|---|---|
| 1 Slots | Yes: hull `slots` (kind, size, mount), mounts, `ship-check.socket-qualification` S1–S5, `ship.py sockets/validate` | import.rs:256 `qualify` runs S1–S5 and F4, held to the record's IDs by a test | Holds. The criteria and the 1.0001 tolerance are also typed out in Rust. |
| 2 Fit | Yes: mount envelope/bears/feeds, equipment `fits`, F3 | ship.rs:243 `mount_fit` | Holds |
| 3a Thrust | **No.** Thrust geometry is the hull's: `hull.thrusters` (required) binds model nozzle nodes to slots. Engine products carry no nozzle position or axis. Mount `nozzle.diameter` is never read. | Push point and axis come only from hull nodes (ship.rs:376). A fitted engine on a socket that binds no nozzle is refused, "drives no nozzle" (ship.rs:753). There is no fallback to the product. | **Breaks the chain.** Blender would have to know the engine to place its nozzles. |
| 3b Role of an engine | No | Main or lift chosen by nozzle direction within cos 30° (`ENGINE_AXIS`, ship.rs:161). The Studio uses ±0.5 (interior.rs). Nozzle name rule `nozzle_main`/`nozzle_lift` (import.rs:93). | Hard-coded, and two thresholds disagree |
| 3c Required blocks | Yes: `ship-rule.base-blocks` | modules.rs:248 reads it | Holds |
| 3d Power, propellant, main thruster | No | Draw ≤ plant + battery (ship.rs:694); one fuel, and cold gas needs its own tank (ship.rs:660); needs a main thruster (ship.rs:747) | Rules hard-coded, figures from records |
| 3e Mass, centre of mass, inertia | No | Modules placed by shell search near `mount_<slot>` (ship.rs:804, 441). Box proportions per kind replace the product's dims (modules.rs:339). | Hard-coded. The product's dims are ignored. |
| 3f Avionics and features | Partly: equipment `function.*` (range, autopilot `features`, comm, governor, interlock) | ship.rs:697, 863; radar.rs:19 read them | Figures hold. No record says which features a ship has (the union of its fitted products). |
| 3g Minimum performance | No | No thrust-to-weight or attitude-authority minimum anywhere. TWR only in Studio info. | Missing |
| 4 Flight | n/a | `spec.thrusters` drives everything. Tuning such as `STRAIGHT`, `TRIM_SHARE` and `MAIN_SHARE_MIN` is hard-coded (thrusters.rs, trim.rs). | Engine's job, once 3 hands over the right thrusters |

### The Explorer, concretely
The hull is installed with 25 sockets and fit `[]`, and it passes stage 1. A player fitting any engine is refused at stage 3a, because no hull nozzle is bound and the product brings none. Main propulsion and attitude can never be satisfied by fitting, so the hull cannot reach flight through the shipyard.

### Product models already carry the data
Engine packages in freefall-assets already have exit nodes, but their names are inconsistent:
- `nozzle_px/nx/py/ny` on cold gas and ca;
- `nozzle_CHS1_exit` on chemical;
- `nozzle` on ft and nt;
- `Jet_*_original_nozzle` plus `nozzle_*` on the outboard pod;
- none on the fans or the swivel.

No record names the convention.

## What the registry must add, as records
1. **Nozzles belong to the engine product.** An equipment field `nozzles: [{node, share}]` names exit nodes in the product's own model: position and axis in the product's frame, the share of the product's thrust, sum 1. A naming rule for those nodes goes in a ship-rule. The game places them through the socket's `mount_<slot>` frame.
2. **`hull.thrusters` stops being required.** It stays only for the outdated stock hulls. A slot hull never binds nozzles.
3. **`ship-check.shipyard-fit`**, criteria judged from the fitted modules only:
   - every required block is filled (base-blocks);
   - every fitted engine has nozzles in its product model;
   - each engine's role (main, lift, attitude) comes from its exit axis in the hull frame, with a cone figure in the record (one value, replacing cos 30° and ±0.5);
   - power: draw ≤ plants + battery rate;
   - propellant: each engine's `burns` held by a fitted tank; one fuel; cold gas has its own tank;
   - mass and centre of mass: hull plus products at their socket frames, with the product's own dims;
   - features: the ship's features are the union of its fitted products' `function` (autopilots, sensor range, comm);
   - minimums the owner sets (thrust-to-weight for a lander, attitude authority on 3 axes), or recorded as none.
4. **`ship.py shipyard <hull> --candidate=fit.json`** runs 3 for anyone (Blender, Ships, the game's tests), alongside `validate`.
5. **Engine's task:** implement 1–3 from the records (push from the product's nozzle via the mount frame) and drop the hard-coded copies (`ENGINE_AXIS`, the name rule, box proportions). Flight is unchanged.

Also hard-coded and worth moving later: fallback mass, price, strength and drag constants (design.rs:25, import.rs:165), `standard_slots` (design.rs:435), trim tuning.
