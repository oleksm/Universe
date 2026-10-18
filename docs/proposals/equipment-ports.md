# Equipment ports for laid wiring (contract, agreed by ships 2026-10-18)

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
| **`key`** | the keying its half is cut to, one of the interface's `keys`: an HV DC pole is keyed `plus` or `minus`, and halves mate only at the same key |
| **`pins`** | each contact of the interface and its role: `plus`, `minus`, `line`, `neutral`, `earth`, `screen`, `signal-high`, `signal-low`, `signal`, `return`, `spare`; a wire joins contact to contact and the roles must agree |

On the interface: **`contacts`**, the contacts in its face by id (and the largest conductor each takes), and **`keys`**
where one face comes in variants that must not meet (HV DC poles: plus, minus; single-pole 400 A: L1, L2, L3, N, PE).

On the thing: **`internal`**, how its own ports are joined inside it, never inferred from where they lie:

| kind | means |
|---|---|
| `joined` | the listed ports' contacts of the same role and line are one conductor inside (a bus bar; parallel pole pairs; the control bus passing through) |
| `switched` | the same, each port through its own breaker the game may open (switchgear, distribution boards) |
| `converts` | the `to` ports are live only while the `from` ports are fed, at most the function's rating (a converter) |

Ports not named in any `internal` are joined to nothing.

On a cable (run): **`conductors`**, by id, with the role a core's colour fixes (an installation cable's L, N, PE) or none
(a single hook-up wire or feeder takes the role of the contacts it joins). A laid link stores, per conductor, the
contact it lands on at each end (ships' `pins (conductor, from, to)`); its channel and roles come from the ports.

## Halves

Sources carry the female half (a dead plug when unplugged), loads the male, data ports female; a cable's end is always
the other half of the port it meets, so a cable may have two male ends (between two sockets) or one of each. A cable
whose two ends are different interfaces (a shell 25 at a converter, a pole pair at a board) is a made harness: each end
is checked against its own port.

## Frame

`at` and `facing` are in the record's frame: x along its length, y across its width, z up, the origin at the middle
of its base. The glTF export is +Y up, so record (x, y, z) = glTF (x, -z, y); the game centres a model on its place by
its manifest's `bounds_m`, so a port in the studio sits at `at` less the bounds' centre (in glTF terms), then turned as
the fit is turned. Places come from the installed model's empties where it has them (`power_data`, `power_entry_*`, the
converters' `input_*`, `output_*`, `data`, `coolant_*`: an empty's +Y arrow is the facing); otherwise they lie on the
-x end face and the port's note says invented, for review.

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

## Filled (2026-10-18)

`tools/standards/equipment_ports.py` writes the ports of the powered equipment the experimental designs use
(docs/asset-usage.yaml; 31 records) and `converters.py` the converters' and the boards'; `reg check` runs
`equipment_ports.py --check` (keys, contacts, connectors, internal names). Each record:

- Its power ports on its bus by its draw (ships' limits: 28 V DC to 1 kW, 270 V DC to 30 kW, 3 kV DC above and every
  source): on the low buses one circular shell by current (shell 17 to 15 A, 25 to 55 A, 33 to 120 A: plus, minus,
  earth where it has the contact), above that a pole pair; on 3 kV a keyed pole pair (`dc-in+`, `dc-in-`), several in
  parallel (`dc-in1+` ... joined) where the current passes 650 A, about 500 A a pair (the fan S3: four pairs of 650 A).
- Sources (batteries both ways, the fusion plant out) feed 3 kV the same way; female.
- Two data ports (`data-1`, `data-2`, circular shell 11 on `can-a`: signal-high, signal-low, return, screen) joined, so
  units daisy-chain; the converters one.
- The switchgear S1: 32 ways of 1,000 A pole pairs in four groups at its model's four `power_entry` empties, any way a
  source's or a load's, all `switched` onto one bus bar.
- The converters: the 3 kV pair in (150 A poles), the low side out (a pole pair, or two shell ways joined), `converts`
  from in to out; coolant in and out (flat-face) at the model's empties.
- New: two distribution boards, where a low bus branches, `equipment.power.distribution.28v-16way` (12 ways of shell 17,
  4 of shell 25) and `.270v-10way` (8 of shell 25, 2 of shell 33), each fed by a pole pair, a shell 33 or a shell 25
  (whichever its converter has), every way `switched`. Invented, no model yet.

Not yet: fluid ports on the tanks, the rocket engines' feeds and the coolant loops' and radiators' pipes (their models
have the empties); equipment with no draw on record (window, airlock, cargo lift, rocket engines) has no power port.

## The original list (the commuters' equipment, docs/asset-usage.yaml)

equipment.command.s2, equipment.sensors.altimeter.s1, equipment.utility.fire.s1, equipment.utility.window.s2 (none
electrical: skip unless heated), equipment.utility.suits.s1, equipment.engine.cg.s1, equipment.access.airlock.crew,
equipment.avionics.nav-basic.s1, equipment.engine.fan.s3, equipment.power.battery-vtol.s1, equipment.cargo.food-store.s1,
equipment.hyperdrive.halcyon.s1, equipment.life-support.s0, equipment.life-support.s1, equipment.power.battery.s1,
equipment.thermal.air-cooler.s0, equipment.thermal.coolant-loop.s0, equipment.thermal.radiator.s0,
equipment.engine.cg.s2, equipment.engine.ch.s2, the tanks (fluid ports only), the gear struts (none), and the six
equipment.power.converter.* records. Each gets its power ports on the bus ships chose for its draw (28 V up to 1 kW,
270 V to 30 kW, 3 kV above), a data port where it has controls, and its fluid and coolant ports. Positions come
from the installed models where they exist, else marked invented for review.
