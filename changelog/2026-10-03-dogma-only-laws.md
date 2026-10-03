# Dogma holds only laws

Anything that isn't exactly how the world's physics behaves moved out of Dogma (`config/dogma.ron`):
- **A drive's top speed** → the hyperdrive product's spec (`top_c`): Kestrel 3,000 c, Halcyon 3,500 c.
- **The speed limit near bodies** (`K·d`) → the nav computer's **governor** spec (2/s), beside its
  interlock. Without avionics a drive goes as fast as the throttle says, wherever it's pointed.
- **A ring's capture speed** (300 m/s) → the gate ring structures' spec (`capture`); the HUD reads it.
- **A tube's 3 s settle** → the relays' **cadence** spec (hyper relay, gate relay): capsules can't
  pass in the flow, so they go in batches, as often as the relay throws them.
- **The Sun's luminosity** → the world sheet (Real), as the unit. Each star's own luminosity is now
  seeded: by its own mass (class mass give or take 15%), L ∝ M^3.5 (was one value per class).
- Dogma now: the speed of light, Stefan–Boltzmann, the field's cost law, the tube laws.
- World generation: a station never goes round a scorched inner world (starlight at most 4×
  Earth's), and without a station a system's gates orbit its most temperate planet (were the
  innermost): a hull at a gate mustn't cook.
