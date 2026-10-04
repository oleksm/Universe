#!/usr/bin/env python3
"""Opening and holding a tube, by the same formula as a pass, the tube weighing
by its diameter: mu(d) = RHO * (d / 1 m)^K.

Targets (stated, to argue with):
  A. Opening a typical gate (a 3 km ring, 5 ly) at its natural pace costs about a year
     of a 100 GW industry (3.2e18 J): a faction's project.            -> RHO
  B. Holding it costs about 1% of the opening a month: small next to opening, big in
     absolute terms, so a lapse (and re-opening) is ruinous.          -> TAU_HOLD
  C. Opening a tube sized for one ship (100 m) just for one pass costs far more than
     passing a held gate: holding a gate is the economic choice.      -> checks K
Run it: python3 tools/experiments/tube_holding.py
"""
import math
from gate_transit import EPS, LY, t_nat, energy, when

S5 = 5 * LY
OPEN_TARGET = 100e9 * 3.156e7           # A: a year of 100 GW
TAU_HOLD = 200 * 30 * 86400             # B: 0.5% a month -> drains in 200 months

for K in (2, 3):
    # RHO so a 3 km, 5 ly tube opened at natural pace costs the target (solved numerically:
    # mu enters t_nat too, through mu^(1/3))
    lo, hi = 1e-12, 1e30
    for _ in range(200):
        RHO = math.sqrt(lo * hi)
        mu = RHO * 3000 ** K
        e = energy(mu, S5, t_nat(mu, S5))
        lo, hi = (RHO, hi) if e < OPEN_TARGET else (lo, RHO)
    print(f"\n=== K = {K}: RHO = {RHO:.3g} kg ===")
    print(f"{'tube':<26}{'equiv. mass':>13}{'open (natural)':>16}{'taking':>12}{'hold':>12}")
    for name, d, S in [("gate 3 km, 5 ly", 3000, S5), ("gate 3 km, 1 ly", 3000, LY),
                       ("one-ship 100 m, 5 ly", 100, S5), ("relay 1 cm, 0.5 AU", 0.01, 0.5 * 1.496e11),
                       ("relay 1 cm, 5 ly", 0.01, S5)]:
        mu = RHO * d ** K
        tn = t_nat(mu, S)
        eo = energy(mu, S, tn)
        print(f"{name:<26}{mu:>11.3g}kg{eo:>14.3g} J{when(tn):>12}{eo / TAU_HOLD:>10.3g} W")
    # C: one pass through a held gate vs opening your own 100 m tube for it
    ship = energy(1e5, S5, t_nat(1e5, S5))
    own = energy(RHO * 100 ** K, S5, t_nat(RHO * 100 ** K, S5))
    print(f"  C: a 100 t ship's pass {ship:.3g} J; its own 100 m tube {own:.3g} J ({own / ship:.2g}x)")
    hold_day = OPEN_TARGET / TAU_HOLD * 86400
    print(f"     a held gate's day {hold_day:.3g} J = {hold_day / ship:.3g} passes' worth; break-even vs own tubes at {hold_day / own:.3g} passes/day")
