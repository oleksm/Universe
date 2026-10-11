# Keep cargo package identities through saves

- Save format v3 preserves keyed all-pilot cargo provenance, works inventories,
  open/partial lots and both serial allocation counters; old saves remain readable.
- Restore replaces live inventory, checks NPC identity, refreshes market views and
  recalculates gross cargo mass/volume including packaging.
- Trade uses Ledger's physical cargo accounting; breakage removes package records
  before settling destroyed quantities to zero.
- Extend save, trade, breakage and replay regression coverage.

Depends on engine package foundation a962a100, view refresh 03cedb17, exact
ledger quantities e71a7556/f68a3d83, and registry build fix 5873279c.
