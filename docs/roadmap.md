# Roadmap: the living galaxy

The world's foundations, in the order they build on each other (the user's plan, 2026-10-02).
Economy tuning waits for these; `backlog.md` keeps the smaller game items.

Rules over all of it: **physics first** (information, matter and energy move only as they
physically can); **player parity** (anything an NPC, faction or corporation does, a player can do
too, with the same actions); **nothing set in stone** (preset factions, NPC assets, infrastructure
can all be built, taken, broken or lost); **the engine holds no intentions** (services keep the
world's facts; deciding is the clients': pilots, players, corporations, factions).

## 1. Hypernet: information at the speed it can travel

- **Today:** knowledge is instant and global (a trader knows every market's prices at once).
- **Physics:** a signal crosses a system at light speed (minutes to hours). Between systems it
  doesn't cross at all, except physically: in a ship's memory, through a gate.
- **Relay stations** (data retranslation) at inhabited places: stations, ports, planets, gates.
  They pass messages on at light speed across a system. Inside the network, news arrives after
  the light delay of its path; outside it, only what ships carry.
- **Gate relays:** a gate is modular. A relay module on it (a physical add-on, built and fitted)
  links two systems' networks through the gate. Messages cross with the gate's own latency, and
  still pay the light delay to and from the gate on each side. Sync lag grows with distance.
- **Knowledge, not truth:** every node and every ship holds what it has heard, and when. Pilots
  decide on what they know (stale prices, old news). Settlers connect to the hypernet; leaving the
  network changes what's possible.
- Built, owned, breakable, maintained (see 5): a relay down splits the network.

## 2. News, digests, broadcasts

- The event system goes onto the hypernet. Events become messages from where they happen,
  spreading at the network's pace. News outlets compile them into digests and broadcasts.
  The kill feed and trade log become news as heard here, not omniscience.
- **Done, coarse (2026-10-02):** kills and trades travel the hypernet (`sim/src/news.rs`); each
  settled system's station runs an outlet (`sim/src/newsroom.rs`) that hears what reaches it and
  puts out a digest every 10 minutes: its own system first (the fighting, the trade, the biggest
  deal), then a line for each system it heard from. A digest is a broadcast from its station,
  carried like any news. NEWS (F11) lists the digests heard; a ticker shows the latest as it
  arrives. Next: outlets as owned enterprises (factions, corporations), more kinds of news
  (shortages, prices moving, arrivals, discoveries), and ships carrying news off the net.

## 3. Law and standing

- **Done (2026-10-03):** every settled system enforces the law (fair game for 10 minutes after
  firing on the innocent); each system's authority keeps a standing for every pilot from the
  deeds it hears of; hostile, its turrets fire and its docks refuse (`docs/standing.md`).
- Factions to join, found and claim for were built and removed: too close to another game's
  pledging and powers. Politics, if it comes back, grows out of what players own and run.

## 4. Corporations

- Real ones, following the rules of life: brands that make products, governed by factions
  (licences, fees, taxes). Production, outpost construction, infrastructure, employees, projects,
  business. Players can found and run them; NPC ones play by the same rules.

## 5. Breakable constructions: maintenance and service

- Everything built wears and breaks: stations, relays, gates, outposts, ships. Maintenance and
  service become work, and trade (parts, materials).

## 6. Drones

- Local machines for routine work (repairs, simple tasks), so the game isn't a second job.
  Limited by the hull or structure carrying them and their state. Managed by players,
  corporations and factions.

## 7. Exploration and infrastructure

- Far systems hold rich veins: a reason to go, settle and start a civilisation.
- A typical build-out: navigation beacon, then comm relay, fuel depot, repair outpost, refinery,
  habitation, market, shipyard. Anything players want.
- The hypernet is what helps or makes it hard: off the network, you're on your own.

## 8. Depth of matter

- Start from actual elements: H, He, C, N, O, Al, Si, Ti, Fe, Ni, Cu, Pt, U, and so on.
  Ores are compositions; refining separates them; materials and parts are made of them.
- Instrumentation to build anything: small ships, large ones, capital ships, outposts,
  production chains, trade routes, an industrial empire.

## 8½. A real-density galaxy

- **Done, one region (2026-10-02):** the charted region, a 200 ly cube at real density (32,000
  stars, neighbours 4-6 ly), lanes 4-7 ly (`docs/world/galaxy.md`).
- Next: more regions from the seed as they're reached, over engine nodes; density by place in
  the galaxy; multiple stars.

## 9. Terraforming

- Projects that change a world to produce what people need: water, food, sustainable energy.

## Art target for built things

- Stations and outposts as dense engineered structures (trusses, rings, modules, radiators,
  solar arrays, docking arms): the reference is a large orbital complex over a planet. Built
  from modules, so what players build looks built.
