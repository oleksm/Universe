# Fuel bought as stock

- **What a facility burns or holds may be a material** (the registry's deuterium is now one
  material, moved and sold as a stock item made from it). What it's traded as, and so what the
  facility buys at its market, is the market category of the stock made from it: `goods.fuel` for
  deuterium. Goods are traded as their record says, stock items as theirs.
- **The market's categories** (`market.*`) and **mill stock** (`stock.*`) are read from the
  registry, strictly typed.
- **A facility that burns something nothing is traded as stops the game at start**, so a
  power station can't stop buying its fuel unnoticed.
