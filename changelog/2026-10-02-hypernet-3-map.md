# Hypernet 3: the network on the map

- **NETWORK (K) on the system map** toggles the hypernet view:
  - **The chart to scale** (by the root of distance from the star, so the inner worlds keep room),
    in place of the schematic rings.
  - **The links** between relays, coloured by lag: green under a second, cyan under a minute,
    amber under an hour, orange past it; red for dark relays (off the net).
  - **Shaded: where your comm reaches a relay on the net**: your link range round each one, at
    least a few pixels so a small reach still shows. With the basic comm (0.05 AU) that's small
    patches round the worlds: out in a belt you're on your own.
  - **Gate relays** marked with the system each leads to (labels stack where gates share a spot);
    **you**, and your line to the relay you're through.
  - **The relays listed** in place of the targets (relay, kind, reach, lag; the one you're through
    marked), under your status and your comm's reach.
- Dev scenario `netmap`.
