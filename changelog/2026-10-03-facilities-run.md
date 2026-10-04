# Facilities run in the economy

- Every built facility runs on the economy's step (10 minutes of world time), at the
  settlement's market, for its owner (companies are ledger parties now, like pilots):
  - **Power**: the settlement's power stations' supply shared among what draws it; a line
    held back if there isn't enough. What draws it pays the stations 40 CR/MWh (invented);
    each station buys the fuel it burned (deuterium, as the game's fuel) from the market.
  - **Lines** (Trethi Foundry): as fast as power and the market's stock of what they take
    allow; the owner buys what it takes from the market and sells it what it gives, at the
    market's prices (the middle of the kind's catalogue range per tonne, by the place's want).
  - Goods with no game kind yet aren't traded: what's taken of them comes from outside, what's
    given goes unsold. Today that is most of steelmaking (ingot included), so Trethi Foundry
    runs flat out and loses money (about 2,600 CR a step) until the registry maps its goods;
    the mapping will close the gap without code changes.
- The registry writes each facility's flows flat out an hour (what it takes in, gives off and
  burns, each with its game kind if it has one) into `settlements.ron`; built facilities
  remember their blueprint (a player's runs as the registry's facility it copies).
- The zoning view shows how each facility ran over the last step: its rate, what held it
  back, what it earned, and which of its goods aren't traded yet.
- Not yet: storage (warehouses), power lines' capacity between parcels, facilities' own
  stocks, the economy's totals counting facilities.
