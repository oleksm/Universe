# The energy chain, part 3: engines are products

- **Every engine (drive, thrusters, lift) says its own exhaust velocity, efficiency and fuel** (`Does::Drive { thrust, exhaust, efficiency, burns }`, the same for thrusters and lift): today's Kestrel torches exhaust at 10,000 km/s, 30% efficient, burning deuterium. The world sheet's single `EXHAUST_VELOCITY` is gone.
- **Content checks an engine against its fuel:** its jet can't carry more energy per kg (v²/2) than the fuel gives at its efficiency (a torch: 5e13 J/kg against deuterium's 1e14 at 30%).
- **A ship holds one fuel:** its tanks, plants and engines must hold and burn the same.
- **Each thruster carries its engine's figures:** fuel flow is each jet's thrust over its own exhaust; `Ship::engine_heat` is the heat the engines make now (each jet's power over its efficiency, less the jet's): ready for the heat model.
- The shipyard shows each engine's exhaust; fuel-hours readouts use the ship's own exhausts (`ClassSpec::exhaust_of`).
- **Not yet:** chemical engines as products (methalox, kerolox, hydrolox craft): they need ships carrying more than one fuel, and markets refuelling by material.
