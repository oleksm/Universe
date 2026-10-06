#!/usr/bin/env bash
# The GPU's budget a pass, over fixed views at 4K (the user's screen): each view a screenshot
# run (on workspace 8: UNIVERSE_TEST) with UNIVERSE_GPU_BUDGET; any pass over its budget fails.
# Not in the test suite (each view takes seconds and needs the worlds store and a GPU).
#   tools/render/gpu_budget.sh            the views below
#   SIZE=1920x1080 tools/render/gpu_budget.sh
set -u
cd "$(dirname "$0")/../.."
SIZE=${SIZE:-3840x2160}
# Budgets (ms) at 60 fps with headroom: the scene at most 10, the rest small.
BUDGET=${BUDGET:-"scene=10,shadows=1.5,front=1,hud=0.5,upscale=0.5"}
OUT=${TMPDIR:-/tmp}/gpu_budget
mkdir -p "$OUT"
cargo build --release -q || exit 1
fail=0
# name  scenario  lat  lon  alt(m)  down
while read -r name lat lon alt down; do
    log=$(UNIVERSE_TEST=1 UNIVERSE_SHOT_SIZE=$SIZE UNIVERSE_GPU_BUDGET=$BUDGET UNIVERSE_SCENARIO=lowflight \
        UNIVERSE_BODY=body.treistun.treistun-d UNIVERSE_LAT=$lat UNIVERSE_LON=$lon UNIVERSE_ALT=$alt UNIVERSE_DOWN=$down \
        UNIVERSE_SPEED=0 UNIVERSE_SCREENSHOT_AT=300 UNIVERSE_SCREENSHOT="$OUT/$name.png" \
        timeout 300 ./target/release/freefall 2>&1)
    code=$?
    rm -f "$OUT/$name.png"
    echo "$name: $(grep -o 'GPU a pass.*' <<<"$log")"
    grep -o 'GPU budget:.*' <<<"$log"
    [ $code -ne 0 ] && fail=1
done <<'VIEWS'
heath-orbit-77km   -5 50 77000 0.4
heath-sea-30km     -5 50 30000 0.6
heath-sea-14km      5 70 14000 0.25
heath-range-3km  36.6 34.6 3000 0.35
VIEWS
exit $fail
