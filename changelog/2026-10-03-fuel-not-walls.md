# Hyperdrive: fuel, not walls

- **Slack is gone** (and "stiff", `V_OPEN_C`, `STIFF_SLACK`, `P_FLOOR`, `P_PUSH`, the explorer
  references): the field forms anywhere, no inside or outside. You could strand yourself at 200 AU
  because the field "wouldn't form"; now nothing walls a ship in.
- **One formula, the same everywhere:** the drive burns fuel straight from the tank by the metre,
  `dE/dx = m·E0·(1 + (v/v*)²)/η`; `E0` = 2.4e-3 J/(kg·m), `v*` = 1,000 c, top speed 3,000 c,
  chosen from targets (`tools/experiments/hyper_fuel.py`): a Drover's full tank goes about 1.5 ly
  at v* (3 slow), never the 4-7 ly to the next star; an explorer that's nine tenths tank about
  5 ly; a 1 AU hop 0.2 kg, 200 AU out and back about 130 kg. Out of fuel, it drops out where it is.
- **The interlock is a product's spec**, not a law: nav computers carry it (`interlock: 1000 m`);
  with none fitted, the drive flies into what's ahead (and the ship's wrecked).
- Dogma checks: no hull's own tank reaches the next star; an explorer's does; hops round a system
  cost little; gates beat a field per kg by far. Docs: `physics.md`, `world/hyperspace.md`, glossary.
