# Hypernet 2: the network in a system

- **`hypernet` (world):** a system's nodes are its relays: the station, its gates and its
  spaceports, each with the comm its structure is fitted with (a gate by the smallest ring class
  that spans its lane). Two nodes link when each is within the shorter of their reaches and no
  star, planet or moon stands in the line between them.
- **Lag from the backbone:** the station is the system's backbone; each node's lag is the quickest
  way in (light time plus each node's handling). A port on the far side of its world can't see the
  station: it relays the long way round, or drops off the net until its world turns it back.
- **A ship's status:** its comm links to the node that gets a message to it soonest. The HUD's
  instruments show `NET 0.07 S  STATION` (the lag, the relay it's through), or `OFFLINE`, amber,
  with how long ago it was last on. Worked out twice a second. With the basic comm (0.05 AU) a
  ship out in a belt is off the net; a long-range comm (0.5 AU) reaches farther.
- One fast test: the home net has a backbone and its gates on it; a ship by the station is on it,
  one far out isn't; a world blocks the line. (78 tests.)
