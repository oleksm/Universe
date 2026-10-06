# What the law gives: outlawry, bounties, fines

- **A charge applies its law's penalties** (`sim::order::charge`, from the offence's record):
  - **outlawry** for its term (`Law::outlaw`, `outlawed`): the pilot's, not the ship's, so a new
    ship doesn't clear it. An outlaw is refused clearance anywhere in that system ("REFUSED - YOU
    ARE OUTSIDE TREISTUN'S LAW"; `Snap::outlaw`), and no one there trades, refuels, refits, sells it
    a hull or repairs it (`Universe::barred`);
  - **a bounty**: its share of the offender's ship's worth, posted on its head (`Law::bounties`).
    Whoever brings the ship down is paid it by the administration that posted it (a turret, the
    administration's own, is paid nothing);
  - **a fine**: its share of the worth at stake (the offender's ship, for reckless flying), as far
    as its credits go, to the administration.
- Treistun's code: piracy is ten years' outlawry and half the ship's worth as bounty; murder thirty
  years and the whole worth; reckless flying a twentieth of the ship's worth.
- The pilot is told the charge and what it gave ("CHARGED UNDER TREISTUN'S LAW: MURDER - OUTLAWED
  30 YEARS, 52000 CR ON YOUR HEAD"); a bounty paid us, too.
- Not acted on yet: forfeiture (nothing seizes a ship), gaol, a warning.
- The combat test checks the outlawry, the bounty, the refusal and the bounty paid to the craft that
  brings the outlaw down.
