# Land: claim, buy and build at settlements

- **The land office** (`services::land`): at each settlement with ground recorded, who owns
  which lot and what stands on it. Seeded from the registry (Port Trethi: three companies'
  lots and facilities, and lots 4-6 for sale); changed by the same three actions for anyone
  (`Universe::pilot_claim`, `pilot_buy_parcel`, `pilot_build`):
  - **Claim free ground**: a rectangle squared to east and north, at least 50 m each way,
    within 4 km of the port (its flat ground), not over a lot, a street or the port's own
    ground. Zoning is the administration's law, not a wall: it is not checked (policies and
    enforcement come later).
  - **Buy a vacant lot**.
  - **Build** one of the registry's facilities (its modules) on a lot of one's own with
    nothing on it: laid out by the registry's own rule (ported from the build; checked to
    lay Trethi's three facilities out exactly as the registry does). Refused if it doesn't
    fit (Trethi Foundry needs a deeper lot than 500 m).
- **Money through the ledger**: land to the system's administration (new party), building to
  the world (no builders in the economy yet). Placeholder prices, invented: land 2 CR/m²,
  modules 0.5 CR/m³ (a power station 192,000 CR, Trethi's warehouse 560,000 CR).
- **Building takes time**: one module after another at 50,000 m³ a minute (invented): a power
  station in 8 minutes, a warehouse in 23.
- The actions are recorded for replay (new `Op`s), in the state hash, and in the client's
  view (shared, copied only where it changed).
- **In the zoning view**: drag to mark ground (price or why not, live), Enter to claim; click
  a vacant lot, Enter to buy; click your lot, B for the blueprint menu (each blueprint's cost
  and time on that lot, or why it won't fit), Enter to build. Your lots in amber, lots for
  sale dim; sites going up filled as far as built, with how long is left; credits shown.
- **In the world**: what's built stands solid; what's going up rises module by module.
- Dev: `UNIVERSE_LAND` ("buy4;claim:w,s,e,n;build4:<blueprint>") and `UNIVERSE_AFTER` (s)
  for the `zoning` and `settlement` scenarios; `UNIVERSE_PICK` takes `m` (the menu).
- Not yet: facilities producing (the economy's next step), power hookups, selling or
  demolishing, NPCs using the actions.
