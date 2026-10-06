# Order: law, enforcement and the people who are finite

2026-10-06. What the registry says so that a mind has something to reason with, and why the
pirates stop swarming the station. The engine holds no intentions (rule in stone); the operator,
or the mind that replaces it, reads these records and decides.

## Why the sim does what it does today

Roles are dice (`operator.rs`: `seed % N` makes a pirate), nobody dies (a killed ship respawns
4 km behind the home station, in flight, full tank, pirates and victims alike), and nothing
pushes back (no law, no cost, no enforcement). So the station is the one place everyone keeps
arriving armed, forever.

## What the registry now says

| Record | Says | For the mind |
|---|---|---|
| `org.treistun.law` | **jurisdiction** (the whole system); **offences** with penalties: piracy is ten years' outlawry, the ship forfeit and half its worth as bounty; murder thirty years and the whole ship; theft, smuggling, zoning breach, unlicensed mining, reckless flying, fraud, evasion each with theirs; **enforcement**: a constable to 333, a judge to 11,000, an interceptor to 10,000 people and one at every port, 600 s to a patrol under way; **policies**: a 2% duty on sales, a land rate of 1% a year, mining under licence, no insurance needed to dock | the risk of a crime where the law reaches; the bounty that makes hunting a pirate worth it; where to dock as an outlaw (nowhere in Treistun) |
| `building.watch-house`, `.courthouse`, `.gaol` | where constables, judges, lawyers and clerks work; what each serves | the need for safety has places to be met; the Needs report's gaps for constable and judge close |
| `org.treistun-mutual` (`insurance`) | covers hull, fit and cargo; 3% of the insured worth a year; a tenth of the loss as excess; **a replacement is delivered at a yard, parked**; refuses losses brought on by piracy, murder, smuggling or reckless flying | being killed costs something, so fleeing is rational; a pirate's losses are his own; nobody is put back into the fight |
| `org.shikra-hold` (a **band**) and `rig.biraidim.shikra-hold` | 300 people in an asteroid in Biraidim, a system with **no administration and so no law**, one jump from Treistun's traffic; they take cargo first and go home when a patrol comes out | piracy has a home, a reason (traffic high, law absent), a finite population and a place to retreat to |
| `dictionary`: offence, penalty, jurisdiction | the shared words | one vocabulary for the law, the game's events and the UI |

## The ways back (`law.redress`, 2026-10-06)

A law with no way back is a ban. Treistun's: **appeal** within 30 days (the record struck and the
fine returned if upheld); **restitution first** where there is a victim (an insurer that paid stands
in the victim's place); **settlement** of everything but murder by paying the fine and forfeit;
**surrender** before the bounty is collected halves a term; **service** under the administration
(patrol escort, hauling the reserve, salvage: a letter of marque) works off two days of outlawry a
day; warnings and fines **lapse** after a year; the administration may **pardon**. It **recognises**
no other administration's outlawry yet, so an outlaw of Treistun lives where its law does not reach:
the pirates' economy. The bounty hunter delivers to the court, not to the grave: the code has no
capital penalty. A hold's people have a road home, one by one: surrender, restitution, service.

## The rules a mind should follow from this

1. **People are finite.** A settlement's `population` and, next, its census by profession are the
   pool. A pilot who dies is subtracted; a new one is trained (`profession.training.time`) at a
   school. No NPC respawns. The player's return is the insurer's delivery, at a yard, parked.
2. **Crime is a choice with a price.** Piracy where law reaches costs outlawry and a bounty on your
   head; where it does not (Biraidim), nothing. A mind weighs traffic against enforcement.
3. **Enforcement is people and craft, not a wall.** Constables and patrol craft exist in the
   numbers the law says; a breach is seen, recorded and answered after `response`. Nothing is
   hard-blocked, as with zoning.
4. **Outlawry is a door closing.** An outlaw cannot dock where the administration reaches, so a
   pirate's economy runs through ports that do not ask: the hold, and any port outside a
   jurisdiction.

## Not yet

- The **census** (people by profession per settlement), so spawning comes from the pool.
- A **bank** and what lending is (Money); **schools** with their rates (Mind), which set how fast
  a settlement replaces its dead.
- Patrol craft as fitted ships with crews in the census, and the watch house's craft bay.
- Law for Biraidim, if Treistun ever administers it: the hold is the first thing it reaches.
