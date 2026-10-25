
# The registry as a service

The registry (Freefall Facts, `~/git/universe-fso`) answers through one CLI. Run it; don't ask the registry agent
what the CLI can tell you. Add `--json` for machine-readable output.

    S=~/git/universe-fso/tools/standards/ship.py

| You need | Command |
|---|---|
| What a ship must carry to fly, what is optional | `python3 $S requirements` |
| Sockets (mounts) a size class offers, which fit a hull | `python3 $S sockets --class=1 [--kind=engine] [--hull=hull.x \| --dims=L,W,H]` |
| A slot's spec: every parameter a product there must meet (min, max, one of, push direction), and where each comes from | `python3 $S spec hull.x:slot` |
| Products that fit a socket (judged on its whole spec), a mount or a kind | `python3 $S modules --socket=hull.x:slot [--all]` / `--mount=mount.engine-s1c` / `--kind=tank --class=1` |
| One product's sheet: size, mass, power, ports and pins, mount, holder, model | `python3 $S product equipment.sensors.camera.s1` |
| Any record: material (density), cable/wire run (OD, bend, amps), connector, interface, rule | `python3 $S get material.ptfe` |
| Find a key | `python3 $S find shell 25` |
| A hull's class, sockets, required blocks offered/filled, readiness | `python3 $S classify hull.x [--candidate=fit.json]` |
| Validate a hull, a candidate fit or an export before install | `python3 $S validate hull.x [--candidate=record-candidate.json] [--model=export.glb]` (exit 1 on fault) |
| The ship as fitted, from its fitted products: each against its slot's spec, required blocks filled, the hull's spec (lift_to_weight, power_margin, attitude_axes, unfed_engines) | `python3 $S shipyard hull.x --candidate=fit.json` (exit 1 on fault) |
| The registry's design decisions for a ship | `python3 $S decisions explorer` |
| A model against its record (size, nodes, budget) | `python3 ~/git/universe-fso/tools/standards/precheck_model.py ...` (existing) |

Criteria: `ship-check.socket-qualification` (S1-S5 sockets, F1-F4 fit, T1-T3 nozzles, H1 mass) and
`ship-rule.base-blocks` (R1 a socket for every required block, R2 filled when fitted) - the game's import runs the
same rules. Delivery scope: `ship-rule.scope` 1.1.1 (hull and sockets unless an equipped ship is asked for).

## When something is missing

Only then message the registry. The CLI prints the exact command, e.g.

    python3 $S missing "a 28 V 2 A data connector" --for=equipment.sensors.camera.s1

The registry makes the record and it becomes available through the same CLI. Never invent a record, rating or
figure locally; never copy registry data into your own files - read it through the CLI each time.
