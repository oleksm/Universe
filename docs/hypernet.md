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
| Gate relay module | (its system's relay) | the gate pair | 1 s (the throat itself: microseconds) | high | links two systems |

Brands differ: range against power, capacity against cost, robustness (later: wear and failure).
Numbers are first guesses, to tune.

In the base world (`content/base/modules.ron`). Two kinds of infrastructure, **in space only**
(nothing of it works from a world's ground):

- **Transceivers:** what ships' comms connect to, within the transceiver's radius, at light speed:
  the hypernet's towers. They hear what happens round them (capture).
- **Hyper relays:** link two sites through hyperspace (Dogma's hyper-signal, at the medium's limit:
  slower near worlds), so news crosses a system in seconds, not light-hours.

| Product | Kind | On | Radius / reach | Hears | Lag | Capacity (msgs/h) | Power |
|---|---|---|---|---|---|---|---|
| COMM (`comm.basic.s1`, every ship's) | ship comm | ships | 0.05 AU | 50,000 km | 0.05 s | 600 | 2 kW |
| LONG-RANGE COMM (`comm.long.s1`) | ship comm | ships | 0.5 AU | 300,000 km | 0.05 s | 3,000 | 200 kW |
| STATION TRANSCEIVER (`transceiver.station`) | transceiver | stations | 0.5 AU | 1,000,000 km | 0.02 s | 100,000 | 1 MW |
| BEACON TRANSCEIVER (`transceiver.beacon`) | transceiver | gates, orbital sites, claim beacons | 0.5 AU | 500,000 km | 0.05 s | 20,000 | 300 kW |
| HYPER RELAY (`relay.hyper`, Halcyon) | relay | every space structure | hyperspace | - | 0.1 s | 100,000 | 1.5 MW |
| GATE RELAY (`relay.gate`, Halcyon) | relay | gate rings | through the throat | - | 1 s (the throat: microseconds) | 50,000 | 2 MW |
| GROUND TERMINAL (`terminal.ground`) | terminal | spaceports, outposts | up to orbit | 100,000 km | 0.02 s | 50,000 | 100 kW |

**The system's net:** its sites in space (the station, the gates, an orbital site round every
planet and moon of a settled system, claim beacons) each carry a transceiver and a hyper relay.
Each day, as the sites stand at its start, the relays link them **the shortest way all told** (a
minimum spanning tree: each link to a near neighbour, no more than it takes). Lag over a link is
the hyper-signal's time along it. Ports on the ground talk up to a transceiver in sight (their own
world's orbital site always is); ships connect to a transceiver within its radius. In the home
system: every port on the net within seconds to under a minute; the asteroid fields, between the
worlds' 0.5 AU radii, are off it.

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

1. ✓ Comm equipment as modules and as infrastructure (content, brands); every ship fitted with a
   basic comm.
2. ✓ The network: nodes, links (range, line of sight), gate relays, lag from the backbone. The
   HUD status. (`crates/world/src/hypernet.rs`: a system's nodes are its station, gates and ports;
   the station is the backbone, its ports where it has none. Worlds block the line, so a port on
   its world's far side drops off the net and comes back as the world turns. Ships link to nodes
   but don't relay yet: that comes with capture and delivery.)
3. ✓ Hypernet mode on the nav map: coverage, links, lag. (NETWORK on the map, `K`: a layer over the
   chart and the list, not another map. Each relay's uplink drawn, coloured by its lag; relays dark
   in red; a ring round each asteroid field, by whether your comm would be on the net there; your
   line in; a NET column of lag (or DARK) for every place; your status over the list.)
   **A relay round every planet and moon** of a settled system (`structure.relay`, a
   constellation: it sees past its own world). **Routes are planned each day**, as the worlds
   stand at its start: every place that needs the net (station, ports, gates, beacons) gets its
   quickest way to the backbone, hop by hop, and only the relays on those ways are switched on (the
   rest stand by, drawn dim). A line on the day's routes that closes mid-day: that branch finds
   another way then. The map draws each switched-on relay's reach to scale (its true circle
   through the chart's mapping). With every relay reaching 0.5 AU, the outer worlds (gaps wider
   than that) are off the net: bridging them takes stronger relays or relays between the worlds.
   **Routing is a tree, not a mesh:** each relay keeps one uplink, to the neighbour that gets it to
   the backbone soonest, and messages hop relay to relay. As the worlds go round their orbits, a
   line closes and the relay switches to another neighbour: the net reshapes with the seasons.
4. ✓ Capture and delivery (`crates/sim/src/news.rs`): a kill is heard where it happens, by our own
   comm in its capture range, or by a relay on the net in its capture range (the light passes once:
   unseen then, never known); a trade by its market's relay (a dark port passes it on when it's back
   on the net). From its system's backbone through gate relays (the crossing and the relays' lags)
   to ours, and out to us while we're on the net; off it, everything that came in meanwhile arrives
   when we're back. The kill and trade feeds show what we've heard, from when we heard it, with
   where (other systems) and how old it was on arrival. Not yet: ships carrying news in their
   memories (couriers), and anyone but us acting on what they know (step 5).
5. ✓ Markets on knowledge (`crates/sim/src/commerce.rs`, `Boards`): every market puts out its price
   board every 2 minutes; another market has it once it's crossed the net (both markets' lags from
   the backbone). Traders decide on the boards their market has, so a far port's prices are old
   and the trip may find otherwise; a market off the net hears nothing new. The world's first
   boards are long known everywhere. The market screen: live where you're docked; elsewhere, the
   board as it reached you, with its age ("76 S OLD", or long known), or no word at all.
   Next: news and digests (`roadmap.md` §2); boards across systems (through gate relays).
