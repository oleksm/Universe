# Barycentres: bodies pull their parents back

- **Each pair moves about its shared centre of mass:** a body's own orbit is its place relative to its
  parent, and its parent is pulled back the other way by its share of their mass. The star wobbles
  round its system's balance point, a planet round its moons'. The system's centre of mass stays at
  the origin and doesn't move (checked).
- It's still closed form (a pure function of time): Dogma's `rails::positions`, `position`, `velocity`
  and `Ephemeris` all apply it, from shares worked out once when a set of bodies is made or changed
  (`settle`, `RailBody::pulled_by`): after a system is generated, its gates added, a field's swarm
  made, or a rock dug.
- Each share is the child's whole subsystem's mass (a planet with its moons) over its parent's. Bodies
  too light to matter (they don't attract) pull nothing back.
- A pair's relative orbit runs by their combined mass (planets: the star's plus theirs; moons: the
  planet's plus theirs), as two-body motion has it.
- Code that summed orbit chains by hand (an anchored ship's rock, a field body's state) uses Dogma's
  functions now. The simulation's cost is unchanged.

## Small bodies pull too

- Stations (10⁹ kg) and gates (10¹⁰ kg) now attract: everything with mass pulls. A station pulls a
  ship 800 m off at about 10⁻⁷ m/s². They're never the dominant body (checked). Asteroids already
  attracted.
- The charter's "simplified" and "planned" lists are updated.
