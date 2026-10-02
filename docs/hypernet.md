# Hypernet

Information is physical and local. It has to be **captured** by something in range when it
happens, then **carried**, at light speed between comm nodes in range of each other, through gate
relays between systems, or in ships' memories, before anyone elsewhere knows it. Knowledge is
what a place or ship has heard, and when. (See `roadmap.md` §1–2.)

## Capture

- Every event happens somewhere: a shot, a kill, a trade, an arrival, a crash, a price change, a
  discovery.
- It's **captured** by any comm in range of where it happened, after the light time to it. Ships'
  comm modules hear what's near them (aggression, events round them). Infrastructure comms
  (stations, ports, outposts, beacons) are stronger: they hear farther.
- Nothing in range: nobody knows. A fight in an empty belt goes unseen.

## Carrying

- **Nodes:** comms that pass messages on: relay stations on infrastructure, gate relay modules,
  and ships (weakly).
- **Links:** two nodes link when each is within the other's link range (the shorter of the two
  decides) and nothing solid stands between them (a planet blocks the line). A message crosses a
  link at light speed, plus each node's handling lag.
- **Gate relays:** a relay module fitted to a gate (a physical add-on) links the two systems' nets
  through the gate pair. Crossing takes the module's lag (about the gate's transit). A gate without
  one is dark: news crosses only in ships.
- **Ships carry it too:** docking syncs a ship's memory with the place, both ways. Beyond the net,
  couriers are how news travels.
- **Capacity (coarse, v1):** a node handles so many messages an hour. Over that, they queue by
  priority: money and law, then markets and traffic, then news, then bulk (charts, designs).
  Bit-level link budgets come with equipment tuning.

## Equipment (brands with their own parameters)

| Kind | Capture range | Link range | Lag | Capacity | Notes |
|---|---|---|---|---|---|
| Ship comm, basic | about 50,000 km | about 0.05 AU | small | low | hears what's round it |
| Ship comm, long-range | about 300,000 km | about 0.5 AU | small | medium | power-hungry |
| Port or station relay | about 1,000,000 km | about 30 AU | small | high | the backbone in a system |
| Beacon relay (outpost) | about 500,000 km | about 5 AU | small | medium | for chains into belts and far systems |
| Gate relay module | (its system's relay) | the gate pair | about the transit | high | links two systems |

Brands differ: range against power, capacity against cost, robustness (later: wear and failure).
Numbers are first guesses, to tune.

## Who's connected

- A settlement, ship or outpost is **on the net** when its comm has a chain of links to the
  system's backbone. Its **lag** is the time a message takes from the backbone to it.
- Settlers alone in a far belt may fall out of range. To stay connected they build a beacon
  relay chain (exploration, `roadmap.md` §7).
- Off the net: you know only what you capture and what ships bring in.

## Seeing it

- **Hypernet mode** (a view of the nav map): every node, its capture and link ranges as
  coverage, the links between them (lit by load), lag by colour, the dark parts of the system,
  and gate relays with their partner.
- **HUD:** network status: `NET 4.2 S VIA <node>`, or `NET OFFLINE (LAST SYNC 3 H AGO)`.

## Effects (what it changes)

- **Feeds and news:** the kill and trade feeds show what you've heard, when you heard it, not
  everything at once.
- **Markets:** prices carry an age and a source. Traders act on what their port knows and get
  it wrong when it's stale. Being faster than the net pays.
- **Law:** a crime is known where it's heard, and spreads at the net's pace. Beyond the net there's
  no law.
- **Money:** settlement over the net, with lag. Off it: open questions (credit checked later, cash,
  barter).
- **Politics:** a faction's reach is its network. Relays are its borders, and worth fighting over.

## Build order (coarse first)

1. Comm equipment as modules and as infrastructure (content, brands); every ship fitted with a
   basic comm.
2. The network: nodes, links (range, line of sight), gate relays, lag from the backbone. The
   HUD status.
3. Hypernet mode on the nav map: coverage, links, lag.
4. Capture and delivery: events captured in range, carried over the net and by docking. The kill and
   trade feeds go local.
5. Markets on knowledge (quotes with age and source); then news and digests (`roadmap.md` §2).
