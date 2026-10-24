#!/bin/sh
# The Explorer ground stage in the game (registered hull.explorer-study, model 5ba56b39): landed on a pad at Port
# Trethi, Rime F (airless), its two-leaf ramp and belly ladder down, seen from aft. TAB: the pilot's seat view;
# F: on foot. No flight: it carries no power, drive or thrusters.
cd "$(dirname "$0")/../.." || exit 1
exec env UNIVERSE_HULL=assets/models/explorer-study.glb UNIVERSE_SCENARIO=settlement \
    UNIVERSE_DIST="${UNIVERSE_DIST:-30}" UNIVERSE_PITCH="${UNIVERSE_PITCH:-0.2}" UNIVERSE_YAW="${UNIVERSE_YAW:--0.6}" \
    cargo run --release --bin freefall "$@"
