#!/usr/bin/env python3
"""The hyperdrive's fuel law: dE/dx = m * E0 * (1 + (v/v*)^2) / eta, drawn from the tank.

Targets (docs/world/hyperspace.md):
  T1  a normal ship (a Drover: 91.5 t, 30 t of deuterium, an S2 drive, eta 0.6) goes about
      1.5 ly on a full tank at v* = 1,000 c: never the 4-7 ly to the next star;
  T2  an explorer (90% of its mass tank, the best drive, eta 0.75) about 5 ly at v*;
  T3  full throttle costs 10x the slow cost: V_TOP = 3 v*.
Everything else follows; printed below.
"""
C = 2.998e8; LY = 9.4607e15; AU = 1.496e11
FUEL = 3.45e14                  # deuterium, J/kg (catalysed D-D)
VSTAR = 1000 * C

def range_m(fuel_frac, eta, e0, v):
    # m cancels: fuel energy per kg of ship over the cost per kg per metre.
    return fuel_frac * FUEL * eta / (e0 * (1 + (v / VSTAR) ** 2))

drover = (30.0 / 91.5, 0.6)
explorer = (0.9, 0.75)
e0 = drover[0] * FUEL * drover[1] / (1.5 * LY * 2)       # T1, at v*
vtop = 3 * VSTAR                                           # T3
print(f"E0 = {e0:.3e} J/(kg m)    V_TOP = {vtop / C:.0f} c")
for name, (f, eta) in [("drover", drover), ("explorer", explorer)]:
    for label, v in [("slow", 0.1 * VSTAR), ("v*", VSTAR), ("top", vtop)]:
        print(f"  {name:9s} {label:5s} range {range_m(f, eta, e0, v) / LY:6.2f} ly")
m = 91_500
kg = lambda d, v=VSTAR: m * e0 * (1 + (v / VSTAR) ** 2) * d / (0.6 * FUEL)
print(f"drover: 1 AU hop {kg(AU, 0.3 * VSTAR):.2f} kg, 40 AU {kg(40 * AU, 0.3 * VSTAR):.1f} kg, 200 AU out and back {kg(400 * AU):.0f} kg, 1 ly {kg(LY) / 1000:.1f} t")
print(f"5 ly at v*: {5 * LY / VSTAR / 86400:.1f} days; at top {5 * LY / vtop / 86400:.1f} days")
print(f"vs a gate (TUBE_EPS 2.24e-12): {e0 / 2.24e-12:.1e} times dearer per kg per metre")
