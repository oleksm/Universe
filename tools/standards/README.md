# Ship registry CLI

Everything a hull builder needs from the registry, without asking the registry agent. The registry is the game's
single source of truth: the game runs these same records.

    S=~/git/universe-fso/tools/standards/ship.py
    python3 $S <command> [--json]

Every command takes `--json`. `validate` and `shipyard` exit 1 on any fault.

## The idea: a slot is a spec

A hull offers **slots**. Each slot is a full spec of what may be fitted there: every parameter a product must meet,
with mins, maxes, allowed values and requirements. That spec is the base every validation runs on.

A slot's spec is built from:

| Part | From | Example |
|---|---|---|
| kind and size class | the slot | `engine`, size 1 |
| a mount of its kind, no bigger class | the slot's `mount` | `mount.engine-s1c` |
| envelope: length, width, height (max) | the mount's `envelope` | 1.42 x 1.32 x 0.88 m |
| mass, thrust, recoil (max) | the mount's `bears` | 100 kg, 31.2 kN |
| power (max) | the mount's `feeds` | 1.21 MW |
| the standard's extra limits | the mount's `spec` | |
| this place's limits, and the role | the slot's `spec` and `role` | nozzles >= 1; cycle one of chemical...; push within 30 deg of the nose |

A hull can also carry a **hull spec**: what the ship as fitted must meet, for example `ship.lift_to_weight` >= 1.0 at
1.281 m/s2. The ship is judged from its **fitted products**, never from the empty slots.

Each limit has the shape `common.schema.yaml#/definitions/spec_limit`:

    {of: <parameter>, min, max, one_of, present, axis + within (deg), gravity, note}

The parameter `of` can be:
- **A product parameter:** a path into the fitted product's record, such as `physical.mass`, `function.thrust`,
  `function.cycle`, `function.range`, `needs.power` or `nozzles`.
- **`push`:** each nozzle's push direction in the hull's frame.
- **A ship parameter:** `ship.mass`, `ship.main_thrust`, `ship.lift_thrust`, `ship.main_to_weight`,
  `ship.lift_to_weight`, `ship.power_margin`, `ship.attitude_axes` or `ship.unfed_engines`.

## Frames and nozzles

- **Slots in the model.** Each slot is a node `mount_<slot>` in the hull model (glTF; -Z is the nose, +Y is up). The
  node is placed exactly on existing structure, with a 0 mm holder offset. It has no receivers, holders or geometry.
  The fitted module brings those.
- **Placing a fitted product.** The product's model root sits at its slot's `mount_<slot>` frame.
- **Engine nozzles.** An engine's nozzles belong to the product. They are listed in its record (`nozzles: [{node,
  share}]`) as nodes of its own model. A nozzle node's local -Z is its exhaust, and push is the opposite way. The hull
  binds no nozzles: `thrusters` is empty on a slot hull.
- **Catalogue engines.** Main and lift engines (`ch.s1`, `ch.s3`, `ch.s4`, `ft.*`, `nt.s1`, `nt.s2`) exhaust along
  their model's +Z. So on an engine slot, the `mount_<slot>` frame's +Z points where the exhaust goes:
  - aft for a main engine;
  - down for a lift engine.

  Cold-gas pods have nozzles on several sides. You don't have to know any of this: `modules --socket` reports, for
  each product, how far it pushes from the slot's required direction.

## Commands

| You need | Command |
|---|---|
| What a ship must carry to fly, what is optional | `requirements` |
| The mounts a size class offers; which fit a hull | `sockets --class=1 [--kind=engine] [--hull=hull.x \| --dims=L,W,H]` |
| A slot's whole spec, each limit with where it comes from | `spec hull.x:slot` |
| Products that pass a slot's whole spec (`--all`: the rejected ones and why) | `modules --socket=hull.x:slot [--all]` |
| Products for a mount or a kind | `modules --mount=mount.engine-s1c` / `modules --kind=tank --class=1` |
| Every product of a slot kind with its model file, to open and review the look | `modules --kind=engine --models [--installed]` (or `--socket=hull.x:slot --models`) |
| One product's sheet: size, mass, power, ports and pins, mount, holder, model | `product <equipment-key>` |
| Any record: material, cable run, connector, interface, rule | `get <key>` |
| Find a key | `find <words...>` |
| A hull's class, sockets, required blocks offered and filled, readiness | `classify hull.x [--candidate=c.json]` |
| Hull and sockets check: names, mounts, envelopes in the hull, `mount_<slot>` nodes | `validate hull.x [--candidate=c.json] [--model=export.glb]` |
| The ship as fitted, from its fitted products | `shipyard hull.x --candidate=fit.json` |
| The registry's design decisions | `decisions <words...>` |
| Something is not in the registry | `missing "<what>" [--for=<key>]` |

A candidate is JSON that replaces the record's fields before it is adopted, for example `{"slots": [...]}` or
`{"fit": [{"slot": "main_port", "item": "equipment.engine.ch.s1"}]}`.

## Hull and sockets workflow, step by step

1. **Pick mounts that fit the hull:**
   `sockets --class=1 --hull=hull.x`, or `--dims=L,W,H` before the hull has a record.
2. **Write the slots** (name, kind, size, mount; for an engine slot also its role) into a candidate. Place the
   `mount_<slot>` nodes in the model.
3. **Check the export before installing it:**
   `validate hull.x --candidate=slots.json --model=export.glb`. It must pass with 0 faults.
4. **Check each slot accepts real products:**
   `spec hull.x:<slot>`, then `modules --socket=hull.x:<slot> --all`.
   - Every required kind must have at least one product that passes.
   - On engine slots, `push` tells you whether the frame points the right way.
5. **Check readiness:** `classify hull.x`. It must say **hull and sockets ready**.
6. **Leave the fit empty.** Players fit modules. `shipyard` is the check their fit must pass. Run it with a trial fit
   only to prove the slots can make a ship that flies.

## Criteria records

| Record | What it holds |
|---|---|
| `ship-check.socket-qualification` | H1 mass; S1-S5 sockets; F1-F4 fit; T1-T3 nozzles on old stock hulls |
| `ship-check.shipyard-fit` | Y1 each product against its slot's spec; Y2 required blocks filled; Y3 the hull's spec |
| `ship-rule.base-blocks` | Required and optional blocks |
| `ship-rule.scope` 1.1.1 | A delivery is hull and sockets, empty fit, unless an equipped ship is asked for |

## When something is missing

Message the registry only then. `missing "<what>" --for=<key>` prints the one command to send. The registry makes
the record and exposes it through this CLI.

Code: `ship.py` (commands), `slot_spec.py` (spec, judge, ship values), `ship_qualify.py` (socket qualification). Tests:
`test_ship_cli.py`, `test_slot_spec.py`, `test_ship_qualify.py`.
