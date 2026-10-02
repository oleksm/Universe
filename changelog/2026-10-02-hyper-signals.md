# What a hyper-signal is

- **Not photons:** ships talk to transceivers by light (radio, laser); relays turn a message into
  field pulses in the hyper-medium, a tiny field round each bit, and the far relay turns them back.
  Dogma: `HYPER_BIT_MASS` (10⁻⁹ kg, Invented), the field each bit is carried in.
- **Speed is the medium's limit** (as for ships): unchanged, about 10-15 s an AU in a system.
- **Energy buys throughput, not speed:** a bit costs `m_bit · s · (P_FLOOR + P_PUSH) · L / v* / η`:
  about 10⁻¹¹ J across 1 AU in a system, about 4 J across 5 ly of slack between stars; a relay's
  throughput is its power × η over that (`hyper::bit_energy`, `hyper::relay_throughput`).
- **Signals cross a gate's throat at light speed:** the throat is about 3 km (300 m/s × 10 s); a
  signal takes about 10 µs, not matter's 10 s. News between systems is now mostly the hops within
  them, plus the gate relays' handling (1 s).
- The world article `docs/world/hyperspace.md`, the charter and the physics sheet updated; the
  Dogma checks test both (free in a system, dear between stars; a throat in microseconds).
