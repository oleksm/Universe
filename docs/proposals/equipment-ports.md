# Equipment ports for laid wiring (contract, 2026-10-18)

The owner wants players to lay wires themselves and snap them onto module connectors, checking the plug type, the
circuit and the direction (ships, 20261018T113407-ships-1cd1). The registry already has the `port` (common.schema.yaml,
SFO 25/26) on equipment, units and layouts. This contract adds what a snap check needs, and states that a mount
connects nothing.

## What a port says (fields; new ones marked)

| Field | Holds |
|---|---|
| `id` | its name on the thing (`dc-in+`, `dc-in-`, `28v-out`, `data-a`) |
| `flow` | in, out or both: power goes from an `out` (a source) to an `in` (a load); `both` for a data link or a battery |
| `medium` | power, signal, gas, liquid, heat, load ... |
| `interface` + `half` | the mating face and which half the thing carries: a wire's connector must be the other half of the same interface |
| `connector` | the registry's own product for that interface (any other built to it and rated for the duty fits) |
| `voltage`, `current`, `power` | its duty: the run and its connector must be rated for it |
| `at`, `facing` | the mating face's middle and the way out of it, in the thing's frame (the model's datum) |
| **`channel`** | the circuit: `dc-28v`, `dc-270v`, `dc-3kv`, `ac-400v`; data `can-a`, `ethernet-1`. A wire joins only ports of one channel |
| **`pins`** | each contact of the interface and its role: `plus`, `minus`, `line`, `neutral`, `earth`, `screen`, `signal-high`, `signal-low`, `signal`, `return`, `spare`; a wire joins contact to contact and the roles must agree |

On the interface: **`contacts`**, the contacts in its face by id (and the largest conductor each takes).

## Snap rules (for the studio and the game; the registry only states the data)

1. Same interface, opposite halves (or androgynous).
2. Same channel.
3. Direction: one end `out` and the other `in`, or either end `both`.
4. Each pin's role is the same at both ends; DC polarity is held by single-pole connectors keyed by polarity (two ports,
   `plus` and `minus`) or by the pin map of a multi-pin one.
5. The run's rating (vacuum or air, `rating.current` / `current_air`) and the connector's must cover the port's duty.
6. A mount's `feeds` is a capacity to plan by, never a connection: fastening equipment joins nothing.

## Export

There is no copy: the studio and the game read `ports` straight from the equipment records (spec by reference). A
port's `at`/`facing` are in the record's frame, as the model is built (docs/asset-contract.md).

## To fill, once this is agreed (the commuters' equipment, docs/asset-usage.yaml)

equipment.command.s2, equipment.sensors.altimeter.s1, equipment.utility.fire.s1, equipment.utility.window.s2 (none
electrical: skip unless heated), equipment.utility.suits.s1, equipment.engine.cg.s1, equipment.access.airlock.crew,
equipment.avionics.nav-basic.s1, equipment.engine.fan.s3, equipment.power.battery-vtol.s1, equipment.cargo.food-store.s1,
equipment.hyperdrive.halcyon.s1, equipment.life-support.s0, equipment.life-support.s1, equipment.power.battery.s1,
equipment.thermal.air-cooler.s0, equipment.thermal.coolant-loop.s0, equipment.thermal.radiator.s0,
equipment.engine.cg.s2, equipment.engine.ch.s2, the tanks (fluid ports only), the gear struts (none), and the six
equipment.power.converter.* records. Each gets its power ports on the bus ships chose for its draw (28 V up to about
3 kW, 270 V to 40 kW, 3 kV above), a data port where it has controls, and its fluid and coolant ports. Positions come
from the installed models where they exist, else marked invented for review.
