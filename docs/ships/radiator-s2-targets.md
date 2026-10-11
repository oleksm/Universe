# radiator.s2: targets for the model (registry, 2026-10-10)

For `equipment.thermal.radiator.s2` (blender task 20261010T150317-blender-ca37). Record updated on fso; brief:
`python3 tools/standards/next_asset.py --key equipment.thermal.radiator.s2`.

## Physics

8 MW at 1,000 K from both faces (emissivity 0.9): 102 kW/m2, 78 m2: the 12.5 x 6.26 m panel. Both faces must see
space, so the panel cannot lie on the hull (one face: half the rejection): it **deploys**.

## Geometry (authoritative)

| Item | Target |
|---|---|
| Whole, as exported | **stowed**: 12.5 m (out from the hinge) x 6.26 m (along the hinge) x 0.08 m; origin and `mount` node at the root edge, on the hull side |
| Panel (RAD2-01) | 12.5 x 6.26 x 0.03 m; both faces dark, high emissivity |
| Headers (RAD2-02) | supply and return side by side along the root edge, 80 mm across (the full 0.08 m there); U-tubes out and back, no tip header |
| Hinge and lines (RAD2-03) | two low hinge brackets at the root corners, hinge line along the root edge 40 mm off the hull; two tip latches; two rotary coolant joints on the hinge axis; all within 0.08 m |
| Mount | the mount's four corners are the two hinge brackets (root) and the two latches (tip); its envelope 13.8 x 6.89 x 1.65 m holds the stowed panel |
| Deployed | 90 degrees about the root edge (not exported now) |

Nodes: `mount*` (required). Name the hinge parts and the panel group apart (`RAD2_hinge_*`, `RAD2_panel`) and record the
hinge axis and pivot in the `--about` account (`evidence.motion`), so deployment can be added without a new model.

## Runtime

Static, stowed. Deployment needs a device state (stowed 0 to deployed 1) the game does not have, and a hinge driver in
`freefall-motion/1` (it drives only a gimbal today): registry and engine, later; no motion.json now.

## Open (registry)

Titanium Ti-6Al-4V is not a 1,000 K material (strength falls away above about 700 K, from memory). Carbon-carbon fins on
niobium-alloy or stainless headers, or a cooler, larger panel (700 K needs 326 m2), is the registry's to settle; the
look (a dark panel) does not change. The stock stays a mass proxy meanwhile.
