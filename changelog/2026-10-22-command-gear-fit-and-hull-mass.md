# Command and gear fit carried; a registered hull's missing mass refused

The registry's explorer dependency (`20261022T185626-registry-1eee` route review): command stations and landing
gear a hull's record fits were dropped (no game slot kind), and a registered hull with no weighed mass was stocked
at a tonne and commissioned at that.

- `modules::SlotKind::Command` and `Gear`, from the registry's `command` and `gear` slots. Their equipment is an
  inert module: carried in the fit (its mass in the dry mass; drawn where `mounted` draws fit), working nothing.
  Never fitted by default (`stock_fit` skips them; a design's standard slots have none). The legs the ground meets
  are still the hull's gear nodes; no gear motion.
- The catalogue carries an unpriced one at price 0 (never sold: what is sold is stock, at prices.ron's); one with
  no mass or volume stays out (its fit held, with a warning), not failing the content.
- `goods::hull_mass`: a registered hull's bare mass and its basis (recorded, its parts', partial naming the
  unweighed parts, none). `import` refuses partial and none for a registered hull instead of the tonne.
- Access, handling, thermal and engine slots still have no game kind (their fit is held, as before).

MC-07, before and after: frame 141,840 kg both; fit 15 → 19 (its four legs, 3,972 kg), dry 159,016 → 162,988 kg
(+2.5 %); every other entry the same. Other stock hulls: no command or gear slots, unchanged. Test:
`import::tests::import_keeps_recorded_gear_fit_and_its_mass`; the workspace's library tests pass.
