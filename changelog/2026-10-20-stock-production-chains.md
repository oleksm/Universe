# Stock production chains

The kit catalogue had final recipes for all 378 items, while seven ingredient
types and all empty packaging had no recorded manufacturing route.

- Add draft specialty chemical works, packaging works and factory works. The
  chemical routes are ideal integrated mass budgets; chosen formulations,
  construction, rates and power remain marked for review.
- Make all 17 empty package types as stock (`supplies.item: package.*`), with
  recipe inputs separate from the payload. Their masses equal existing tares.
  `rule.packing-stock` defines reservation, return and gross-mass accounting;
  it requires a game inventory implementation before those operations run.
- Give the eight kit works and the three new works construction recipes and
  coarse component budgets. These are drafts, not plant modelling briefs.
- Connect kit recipes to manufactured insulation and seal feedstocks. Add the
  existing meteoritic-iron magnetic-separation yield to the crushing plant.
- Audit every stock's ingredient graph and its empty packaging, including shop
  parts and by-products. A production cycle needs an independently supplied
  route. The audit is conditional on declared resource/element supply and
  establishes coverage rather than installed capacity or qualified chemistry.
- Add the stock-chain report and pages for bulk stock and handling packages;
  include graph and generator checks in `reg.py check`.

The 28 V board/cable vacuum-current mismatch remains a separate sizing decision.
