# System map: names on everything, distances between the rings

- **Every object named on the chart,** small (0.6 size) and faint: planets, moons, stations,
  gates, ports, asteroid fields. The one picked in the list keeps its full, bright title.
- **How far apart the rings are:** along the axis to the right of the star, the gap out to each
  ring ("0.14 AU", then "+0.10", "+0.22"... in AU), above and below the axis by turns so they
  stay apart (the rings are evenly spaced on the chart; the worlds' orbits aren't).
- Engine: `Frame::text_scaled` (text at any size).
- Names of unpicked objects smaller still (0.45 size).
- The ring distances on one line above the axis, "AU" once after the last.
- **A legend,** small, bottom right: star, planet, moon, station, gate, spaceport, asteroid field, you, what the rings and their numbers are; with NETWORK, the relay links, the transceiver radius and the lag colours.
