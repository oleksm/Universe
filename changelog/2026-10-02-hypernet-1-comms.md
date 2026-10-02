# Hypernet 1: comms and relays

- **A comm is a module** (`Does::Comm`): what it hears round it (capture), how far it links to
  another comm (the shorter reach of the two decides), its handling lag and its capacity (messages
  an hour). A **gate relay** (`Does::GateRelay`) links two systems' nets through the throat.
- **Every ship carries a comm:** a new base block, the comm slot, on every hull and design. The
  stock comm is Orbital Systems' COMM (hears 50,000 km, links 0.05 AU, 40 kg, 2 kW); Tallis Signal
  Works (a new maker) sells a LONG-RANGE COMM (300,000 km, 0.5 AU, 200 kW). The shipyard shows it.
- **Every structure has a relay** (`structures.ron` `fit`): stations and spaceports a BACKBONE
  RELAY (30 AU), outposts a BEACON RELAY (5 AU), gate rings a beacon and Halcyon's GATE RELAY.
  Content refuses a structure without a comm, or a gate relay off a ring.
- The charter (`docs/physics.md`) says what's real and simplified about comms; `docs/hypernet.md`
  lists the products; the glossary has the names.
- Next (step 2): the network itself — nodes, links by range and line of sight, gate relays, lag
  from the backbone, and the HUD status.
- The shipyard's slot help moved to the right column, under the maker's lines (the extra comm row had pushed it into the action bar).
