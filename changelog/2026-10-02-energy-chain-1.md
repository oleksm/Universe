# The energy chain, part 1: real fuel, capacitor banks, a hyperdrive on power

- **Fuel at its real density:** ships' tanks hold liquid D–He3 fusion fuel at 100 kg/m³ (the physics
  sheet's `FUSION_FUEL_DENSITY`), so tanks are about 6.7× the volume they were. Content refuses a tank
  denser than that. The Sprint now carries the 8 t tank and the Interceptor a new 4 t tank (40 m³):
  the bigger ones don't fit.
- **Capacitor banks** (`Does::Capacitor { capacity, rate }`, a new slot on every hull; Meridian
  Power's 5, 15 and 40 GJ banks): energy for bursts, charged by the plant's spare power, the reactor
  burning fuel for it (`REACTOR_EFFICIENCY`, `FUSION_ENERGY`). Content refuses one storing past
  `CAPACITOR_DENSITY` (1e7 J/kg). The ship holds `energy`; the HUD shows it (CAP).
- **The hyperdrive runs on power** (`hyperdrive::cruise`, the sheet's rules):
  - the medium's slack from the nearest surface (`slack`);
  - holding the field takes `m·s·P_FLOOR`, pushing `m·s·P_PUSH·(v/v*)³`, over `ETA_FIELD`;
  - its speed is what the plant's spare power plus the banks' rate can buy;
  - the plant's share burns fuel, the rest drains the banks;
  - short of holding it, **the field collapses**.
  - It **won't form where the medium is slack** (between the stars).
  - In a system it costs next to nothing, as before. Heading out, a stock ship stalls and collapses a
    couple of hundred AU out: the system's edge.
  - The old flat 0.2 kg/s hyperdrive fuel draw is gone.
- New events: `FieldCollapsed`, `FieldWontForm`, with messages on screen.
