# 2026-09-30 — Air: drag and re-entry heat; planetshine; queue status

User: "I like the planetary athmosphere. Few things to consider - the thickness can cause damage
similarly how earth does it. #2 more of a visual - surfaces should potentially reflect sun so in a
sunny side on a planet pretty much shaded side of a ship or anything is still lit. Show me docking
status - if I am queued then numnber of pilots ahead. [...] player (me) should be having no
different treatment of any other mechanics or pilots."

(On the last point: there was no special treatment. The turrets' miss was an aiming bug that hit
any ship burning hard. Pirates leaving you alone was because you were missing from their radar
picture. Both were fixed in the previous change.)

## Air (physics kernel)

- **`atmosphere`**:
  - temperate (Terran) worlds have Earth-like air: 1.225 kg/m³ at the base radius, falling by e
    every 8.5 km;
  - the scale height adjusts with the world's surface gravity (H = kT/mg);
  - the air is taken to end at 14 scale heights (~120 km);
  - the air turns with its world.
- **Drag in the integrator**: after each substep, quadratic drag acts relative to the air. It's
  solved exactly (v_rel / (1 + k·|v_rel|·h)), so it's stable at any step size.
  - Ships have a ballistic coefficient of mass / Cd·A (`DRAG_AREA` 60 m²), about 1500 kg/m².
  - They fall at about 150 m/s through sea-level air.
  - No drag in hyperdrive.
  - The flight planner and collision prediction use the same kernel, so they include it.

## Re-entry heat (`world::heat`)

- **Heating**: Sutton–Graves stagnation heating, q = 1.74e-4·√(ρ/r_n)·v³, on a 2 m nose, over an
  effective 150 m². It's scaled by how far the skin is below the air's recovery temperature
  (T + v²/2cp). So subsonic flight barely warms the hull, and moving air cools a hot one.
- **Cooling**: the skin radiates over 500 m² (emissivity 0.8). Its heat capacity is 3 MJ/K
  (about 3 t of metal).
- **Damage**: beyond 1500 K, 5% of the heat it can't shed burns into the hull. At zero hull the
  ship is destroyed by "RE-ENTRY HEAT".
- **What that means**:
  - orbital speed high up (~80 km): hot, but safe;
  - the same speed at 40 km: burnt up;
  - Mach 6 at sea level: glowing (~1350 K);
  - Mach 9 at sea level: burning.
- HUD: `SKIN 820 K / 1500 K` once it's warm, turning red near the limit, with "HULL BURNING -
  SLOW DOWN OR CLIMB".
- Tests:
  - kernel: drag reaches terminal velocity, and a huge step stays stable;
  - heat: shallow vs steep entry, airliner vs Mach 6 vs Mach 9, cooling in vacuum;
  - interactions: `a_steep_dive_into_the_air_burns_the_ship_up`,
    `the_air_slows_a_falling_ship_to_its_terminal_velocity`.

## Planetshine (engine)

- `Frame::reflector`: the planet or moon filling most of the sky lights the side of anything
  facing it, by:
  - how much of the planet's disc that face sees (a view factor running from s·cos β far off to
    (1 + cos β)/2 skimming the surface);
  - its albedo: Terran 0.30, gas giants 0.50, ice giants 0.45, rock 0.15, moons 0.12;
  - the sun's height over the ground beneath;
  - the planet's colour.
- It goes through the same eye adaptation as the starlight, so a tenth of the sun's light still
  looks about half as bright. Over the day side, the shaded side of a ship or station is lit.

## Queue status

- Corridors (a station's docking corridor, a gate's run) now keep a first-come line, like the
  pads. `request_corridor` says how many are ahead.
- HUD:
  - `QUEUED FOR THE DOCKING CORRIDOR - 2 PILOTS AHEAD - HOLD CLEAR`;
  - `QUEUED FOR A PAD - NEXT IN LINE - HOLD OVER THE PORT`.
- Test: `ships_waiting_for_a_corridor_know_their_place_in_line`.
