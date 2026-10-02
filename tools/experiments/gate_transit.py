#!/usr/bin/env python3
"""Gate transit: how long and how dear a crossing is, for data and for ships.

The golden formula (docs/world/hyperspace.md, "Crossing a tube"):

    t_nat(m, S) = 0.2 s * (S / 1 ly) * (m / 1 kg)^GAMMA     natural crossing time
    E(m, S, t)  = EPS * m * S * exp(t_nat / t)               energy to cross in time t

Targets it's fitted to (design choices, stated so they can be argued with):
  1. Data (a 1 kg capsule) crosses at 200 ms per light year at natural speed;
     faster gets punishing.                                     -> the 0.2 s/ly
  2. Heavier payloads are naturally slower: a regular ship under a minute
     through a typical (5 ly) gate, a hauler a couple of minutes, a capital
     ship several minutes.                                      -> GAMMA = 1/3
  3. A 100 t ship at natural speed through a 5 ly gate costs about an hour
     of an S2 fusion plant (8 MW).                              -> EPS
Run it: python3 tools/experiments/gate_transit.py
"""
import math

LY = 9.4607e15          # m
D_ENERGY = 3.45e14      # J/kg, deuterium (catalysed D-D), content/base/materials.ron
GAMMA = 1 / 3
PLANT_HOUR = 8e6 * 3600 # an S2 fusion plant for an hour (J)
S5 = 5 * LY
EPS = PLANT_HOUR / (math.e * 1e5 * S5)   # target 3


def t_nat(m, S):
    return 0.2 * (S / LY) * (m / 1.0) ** GAMMA


def energy(m, S, t):
    return EPS * m * S * math.exp(t_nat(m, S) / t)


def when(t):
    return f"{t:.1f} s" if t < 120 else (f"{t / 60:.1f} min" if t < 7200 else f"{t / 3600:.1f} h")


if __name__ == "__main__":
    print(f"EPS = {EPS:.3g} J/(kg m)\n")
    print(f"{'payload':<20}{'5 ly natural':>14}{'cost at natural':>18}{'3 ly natural':>14}")
    for name, m in [("data capsule 1 kg", 1.0), ("ship 100 t", 1e5), ("hauler 1,000 t", 1e6), ("capital 100 kt", 1e8), ("super 1 Mt", 1e9)]:
        tn = t_nat(m, S5)
        print(f"{name:<20}{when(tn):>14}{energy(m, S5, tn):>16.2e} J{when(t_nat(m, 3 * LY)):>14}")
    print("\nRushing (any payload): cost against crossing at natural speed")
    for k in (1, 2, 3, 5, 10):
        print(f"  {k:>2}x faster: {math.exp(k) / math.e:>10.1f}x the cost")
    print("\nData streams on a 5 ly gate (1 kg capsules, both ways, windows alternate)")
    for lat in (0.5, 1.0, 2.0, 5.0, 20.0):
        best = None
        for i in range(1, 2000):
            t = lat * i / 2000                  # crossing time
            cycle = 2 * (lat - t)               # one-way latency = cycle/2 + crossing
            if cycle < 2 * t or t_nat(1.0, S5) / t > 600:
                continue
            P = 2 * energy(1.0, S5, t) / cycle  # two throws a cycle
            if best is None or P < best[0]:
                best = (P, t, cycle)
        P, t, cycle = best
        print(f"  latency {lat:>4.1f} s: crossing {t:.2f} s, cycle {cycle:.2f} s -> {P:.3g} W")
