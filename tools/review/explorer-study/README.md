# Explorer study in the game renderer

These are the exact static assembled open and closed snapshots supplied by Blender
for the owner's 2026-10-23 install request. `packet.json` is Blender's frozen intake,
not a registry package manifest. The source is pinned there at `b29b247085bb`.
Open is `1399990f6ddd`; closed is `50a33c16150f`. Both are 1015-node static scenes.

From this game checkout:

```sh
python3 tools/review/explorer_showcase.py --pose open --binary /home/alexm/git/universe/target/release/freefall
python3 tools/review/explorer_showcase.py --pose closed --binary /home/alexm/git/universe/target/release/freefall
```

Use `--check` to verify bytes and print launch settings. With no `--binary`, the
launcher uses this checkout's `target/release/freefall`. It opens the existing
showcase with its model-only presentation and a separate XDG save directory under
`~/.cache/freefall/explorer-study-review/data`. The two poses are separate launches.

The snapshots contain shell, floor, rear ramp, pocket hatch, ladder and edited
study gear. They use the review export frame. Command and propulsion are deferred.
No inventory product, registry visual package, player ship or physical operation
is created by this staging. Movement, loads and current human clearance are not
accepted by a static display. The original source and mechanism QC remain with
Blender; no prior model's passes transfer.

Commissioning requires the formal hull-only export/frame and real markers,
truthful current frame/parts mass and dimensions, selected equipment/slot/mount
roster, real engine nozzle bindings/shares, power/fuel/thermal/control allocation,
and supported compound ramp/hatch/ladder/gear consumers. The hull record must bind
those exact released bytes at its canonical relative model path. Empty fit must
not be used to trigger inferred stock equipment. Current route audit:
`docs/ships/explorer-consumer-refresh-2026-10-23.md`.
