# People and traffic: who is where, and what flies

2026-10-06. How the NPCs are to be managed: from records, not dice. The engine holds no intentions;
the operator (a client) reads these and runs organisations, not loose ships.

## The records

| Record | Says | Tool |
|---|---|---|
| `module.staff` | who runs one of each works module, round the clock (coarse, by the kind of plant) | hand, 100 modules |
| `settlement.census` | its people by trade: the staff of its works, the trades its needs are served by (one doctor to 290…), pilots from the fleets based there; the rest dependants | `tools/standards/census.py` |
| `settlement.resupply` | where its supplies come from and how often: the fleet's cadence on the run | `census.py` |
| `org.*.fleet` | the ships an organisation operates at day 0: hull, count, home, what they do | `census.py` for the freight line; hand for the rest |
| the page's **Traffic** and **Census** reports | the same, read | `build.py` |

## Traffic, derived, to a thousand

The user wants about a thousand ships in Treistun's sky (`seeding.traffic.ships`: one ship to
220 people, between Earth's merchant fleet at one to 80,000 and a maritime frontier's boats at one
to ten). They are reckoned in three parts:

1. **Runs for people** (`settlement.resupply`): what a port's people take a day of stocked goods
   (about 1.6 kg a head), the flight one way at a torchship's cruise (1 m/s², a tenth of a g, fuel
   in mind; chosen) over the two orbits' typical separation, a hauler's 150 t hold, a day's
   turnaround each end. Heath to Hoar is 8 days each way; to the giants' moons 17 to 22.
2. **Runs for works:** what a port's facilities take in a day that the port does not make, from the
   nearest port that makes it or, made nowhere in the system, from the gate: Trethi's mill and yard
   draw 380 t a day of ores and stock through the gate, Eikir's works 216 t. The lines are taken at
   full rate (a capacity, as the page's other measures).
3. **Independents**, owner-operators based at the ports by population, fill the rest of the
   target: Drovers, couriers, prospectors, a few haulers (`settlement.independents`). What a
   player is.

| Who | Ships | Based |
|---|---|---|
| Treistun Freight (the runs, 1 and 2) | 132 haulers | Port Eikir |
| Hoar Line (passage, one shuttle to 10,000) | 22 Drovers | the station |
| Cormorant (two prospectors to a field) | 10 | Port Trethi |
| the administration's patrol (the code's one to 10,000, one at every port) | 23 interceptors | every port |
| Shikra Hold (raiders) | 12 interceptors | Biraidim |
| independents | 793 | the ports, by population |
| **all** | **1,000** | |

The delivery interval on each run is its haulers' cadence, which the stocking law reads for the
distribution stock.

## What the operator should do with it

1. Spawn fleets, not settlers: each organisation's ships, crewed from its home's census, flying
   for its business (hauling the runs, passage, mining, patrol, raiding).
2. A ship lost is one fewer; a pilot dead is one fewer in the census. The company orders a hull from
   a yard; the school trains a pilot in `profession.training.time`. No NPC respawns.
3. Ships far from any player move on the ledger (depart, arrive on the flight time); they are flown
   only within a bubble round players.
4. Raiders sortie from the hold to the gate approach, take cargo from weak, lone prey, go home;
   the patrol answers in `law.enforcement.response`; bounties make hunters; surrender is a way home.

## The finding, and the answer so far

The first census said **3 to 6% of people at work**; in the rich countries about half are. The
answer (2026-10-06, evening) is the buildings of commerce and construction: every building says
**one to how many people** (`per_people`) and **who runs it** (`staff`); a settlement's buildings
follow from its population (`settlement.buildings`), and the census sums their staff beside the
works' and the fleets'. Fourteen new buildings: shop (one to 70, five working), market hall,
builders' yard (one of fifty to 1,500: construction), repair shop, offices (one block of fifty-five
to 1,000: administration and business services), bank, depot (ground transport), tavern and inn,
theatre, press, care home, college; the clinic, hospital, school, flight school, watch house,
courthouse, gaol and dwellings have their ratios too. Port Eikir has 858 shops, 120 taverns, 60
office blocks, 52 clinics, 40 builders' yards, 30 dwelling blocks.

The census now reads **30 to 37% at work** across Treistun's ports (Shikra Hold 44%). The shares
behind it are a rich country's by sector, from memory and marked review: retail 7%, offices 6%,
construction 3–4%, transport 2–3%, hospitality 3%, repair 1–2%, health and care about 7%,
schooling about 2%. What is still thin against the real half: **manufacturing for people** at every
port (works making the consumer goods the shops sell, today only Eikir's food and chemical
works and Trethi's mill), **health** (a real economy's 13%) and **schooling** (9%). Those come
with the next parts: wear and consumer goods, Mind. The "Work" need stays unmet in the report
until the game pays wages: what meets it is a job, which the census now counts.
