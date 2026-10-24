# A registered import is found by its record's key too; the Explorer's ground launcher

Registry `20261024T113151-registry-5205`, Blender `20261024T113153-blender-2db0`.

- `Registry::alias`: a second key for an entry already loaded (refused where it's another entry's key).
- `import::commission`: an imported hull keeps its path key (`hull.import.<stem>`) as its identity: saves and studio
  designs restore by it. Where a hull record names this exact model path and these exact bytes (`model_sha256`), its
  own key (`hull.explorer-study`) finds the same hull. Unregistered imports and records without a hash (the MC-07) are
  as before. Test: `content::tests::registry_alias_finds_the_same_entry`.
- `tools/review/explorer_ground.sh`: the Explorer ground stage in the game, landed at Port Trethi, ramp and ladder down.
