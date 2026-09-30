# 2026-09-30 — Combat mode: the master arm

User: "I also want a switch that would prime ship to a combat mode, assuming inside combat mode
is for battle."

A ship state (world), not a HUD toggle:

- **Master arm** (`Ship::armed`, `ShipCommands::arm`, `weapons::master_arm`). B toggles it.
  - Turning it on starts priming (`ARM_TIME` = 2 s), after which the weapons are hot
    (`Ship::weapons_hot`).
  - Turning it off makes the ship safe and releases the triggers.
  - Events: `WeaponsArming`, `WeaponsHot`, `WeaponsSafe`.
- **Weapons fire only when hot.** Space or V while safe says "WEAPONS SAFE - B FOR COMBAT MODE".
- **Traffic control** (a world service) refuses clearance to an armed ship ("WEAPONS ARMED -
  DISARM FIRST (B)"), and a clearance lapses when the ship arms, which also cancels a
  docking/landing autopilot. Combat and traffic procedures exclude each other: disarm to dock.
- **HUD in combat mode**:
  - The top line turns red: `COMBAT - ARMING` / `COMBAT - WEAPONS HOT`.
  - The hull/gun/laser line is amber, with `WEAPONS PRIMING n S` while priming.
  - Fire control (tracking, lead ready) and the lead circle appear only when armed.
  - The cockpit crosshair turns red, with a gunsight ring.
  - The clearance hint is replaced by "WEAPONS ARMED - NO CLEARANCE".
- **HUD when safe**: `HULL [..] WEAPONS SAFE  B COMBAT MODE`.
- **Sounds**: a rising priming whine, a double beep when hot, a falling tone when safe.

Tests: `weapons_fire_only_when_armed_and_primed` (world) and
`combat_mode_and_clearance_exclude_each_other` (sim). The shoot-down test now arms first.

Not yet: combat mode as a signature others can see (armed ships are more visible, or suspicious
to traffic control), and zones where arming is illegal. Both belong to the combat-mode/zones step.
