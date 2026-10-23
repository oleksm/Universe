# Cold-gas RCS beside another fuel: its own propellant, natively

Registry `20261022T192515-registry-6ce7` and ships' allocation audit (`20261022T192646-ships-b439`): the explorer's
CGS1 cold-gas blocks push with nitrogen beside hydrolox engines, which the native one-fuel rule refused.

- Cold-gas engines (`cycle: cold_gas`) are `Does::Engine { pressure_fed }`: pushed by their tank's pressure, so the
  fuel-energy check doesn't apply to them; attitude thrusters wherever they point (RCS role); one stands for the
  Thrusters base block.
- A gas only cold-gas engines push with, on a ship with another fuel, is a propellant of its own
  (`ClassSpec::propellants`, the device fit's `Propellant`): its tanks' capacity, the thrusters that draw it. Its
  tanks stay out of the fuel and the one-fuel rule, their contents weighed full in the dry mass. With no tank of it
  the ship is refused. A ship of such a class gets the pool in its device state (`Ship::own_propellants`, on
  build, refresh and refit), so its RCS draws the nitrogen and its engines the fuel (the studio's device path).
- An engine's nozzle shares must sum to exactly 1: a four-nozzle 0.5 kN block is 125 N a nozzle, never 4 × 0.5 kN.
- Limits stated: cold gas at its rated exhaust (not from tank pressure and temperature); every engine's jet heat
  aboard at full (heat_to_hull × jet power) not simulated.

MC-07: no own propellant, no limits; fit, thrusters and masses identical. Test:
`import_cold_gas_rcs_draws_its_own_propellant`; the workspace's library tests pass.
