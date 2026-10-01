# A vending machine between the pads

- **Every spaceport has a vending machine** on the ground in the middle of its pads, between
  the four middle ones: a solid red box, 2 m tall, its lit front to the north.
- **On foot within reach: "F USE VENDING MACHINE".** F opens its panel:
  - COLA (2 CR), SPARKLING WATER (1), SALTED CRISPS (3), CHOCOLATE BAR (3), PROTEIN BAR (4);
  - ↑/↓ to pick, ENTER to buy, F or ESC to close (it holds you there while it's open).
- **A purchase is paid to the port's market through the ledger,** like any trade. The machine
  checks you're within reach: "COLA - 2 CR. ICE COLD. THE CAN HISSES OPEN." With no credits,
  the machine refuses.
- (A generic cola: no real brands in Freefall.)
- Test: out of reach, nothing's bought; by it, a cola for 2 CR. Dev scenarios `vending` and
  `vendingopen`.
