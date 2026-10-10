#!/usr/bin/env bash
# Fixed development-only Vulkan comparisons; no world installation.
set -euo pipefail
pgs_export=${1:-../planet-trees/out/planet/earth_s15/surface_model_006}
pgs_output=${2:-/tmp/pgs-material-pilot}
mkdir -p "$pgs_output"
sites=(summit lake)
if [ "${3:-all}" != all ]; then
    case "$3" in summit|lake) sites=("$3");; *) exit 2;; esac
fi
for site in "${sites[@]}"; do
    if [ "$site" = summit ]; then
        lat=-50.927589054482716; lon=-19.166702846855568; hours=2; alt=1000; down=0.25
    else
        lat=59.01000627043136; lon=-103.1714991841128; hours=9.3065; alt=10000; down=0.6
    fi
    for mode in neutral categories materials; do
        UNIVERSE_PGS1="$pgs_export" UNIVERSE_PGS1_COLOURS="$mode" UNIVERSE_PGS1_SHADOWS=off \
        UNIVERSE_BODY=body.treistun.treistun-e UNIVERSE_SCENARIO=lowflight \
        UNIVERSE_ENGINE_THREAD=0 UNIVERSE_LAT="$lat" UNIVERSE_LON="$lon" \
        UNIVERSE_HOURS="$hours" UNIVERSE_ALT="$alt" UNIVERSE_DOWN="$down" UNIVERSE_HEADING=90 \
        UNIVERSE_SPEED=0 UNIVERSE_SETTLERS=0 UNIVERSE_PAUSED=1 \
        UNIVERSE_SCREENSHOT="$pgs_output/$site-$mode.png" UNIVERSE_SCREENSHOT_AT=480 \
        ~/bin/capped target/release/freefall > "$pgs_output/$site-$mode.log" 2>&1
    done
done
