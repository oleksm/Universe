# Proposed recognition rules for 66 catalogue kinds

Owner request: full landscape, via Planets task20261022T194255-planets-574e. These definitions fill the S (no trigger) rows from unplaced_kinds_needs.csv. They describe detectors to implement, not 66 implemented generators, an occurrence census, or an Earth-calibrated classifier. Existing physical process parameters are unchanged.

The authoritative rules and numeric detector settings are each feature record's `trigger` and optional `trigger_params`. Code must resolve the feature key and read those settings; do not copy a hand list of kinds or thresholds into the simulator. The same name can be scoped to different feature records. A process may consume a feature-local parameter by key/name; do not silently duplicate it with a different process default. All new thresholds remain `open: true` and `used: false` until a detector actually reads them and documents calibration. A missing setting must not fall back to a hidden code constant.

## Shared interpretation

- Work in the chosen present/checkpoint state. `planet-field.height` is already relative to the sea of its own epoch. Prefer the declared present/recent height when that is the run mode; do not subtract sea level twice or mix recent water with ancient heights. Crust age is seconds, temperature kelvin, rain/runoff m/s and discharge m3/s. Convert an annual rain amount with31557600seconds/year. `planet-field.aridity` is PET/P: high is dry; it is the inverse of the common P/PET aridity index. This was read from planet-forge coupled.rs, not inferred from the name.
- Every required scalar/table/record must be finite, current and joined by a stable identifier. Missing input is unknown, not zero or false. Keep the exact checkpoint, source record/hash, detector revision, threshold values, native cell spacing/area and supporting parent/feature IDs in each detector result. A candidate based on incomplete evidence may be kept with explicit proxy/uncertainty status but cannot be counted as a confirmed type or a measured absence. `built` changes only after the real writer and emitted-kind evidence land.
- Use physical areas on adaptive grids, geodesic distances and actual neighbour topology. A feature needs a resolved footprint/profile at its declared size. A coarse130km lattice cannot certify a1km waterfall, abyssal hill, narrow pass, lava dome or glacier valley; record the regional candidate and leave detailed geometry to the permitted finer-scale phase. Do not enlarge real size or invent detail to satisfy a cell-count threshold.
- Polygons/lines follow connected physical boundaries; do not turn a scalar band into straight coordinate edges. Cluster islands with bounded total extent, split rivers at real confluences/water bodies, and prevent duplicate features at adjacent samples. Emit edges as child boundaries, not additional primary-cover area. Preserve overlaps between different registers intentionally (for example a continental realm and an ice cover); report conflicts among competing primary classes instead of accepting evaluation-order accidents.
- Annual temperature and moisture can define explicit climate-potential vegetation. They cannot uniquely establish seasonality, forest composition, groundwater, persistent snow/permafrost, marine sea ice or glacier flotation. Separate a modelled suitability candidate from supported biome/water-body identity; enrich missing signals instead of pretending all66 types are confirmed from mean climate.
- A topographic high, depression or flat region needs the origin/topological context named in its own trigger. No all-purpose rule converts all old volcano centres into geothermal fields, all ocean highs into guyots, all lowland into wetlands, or all small ice patches into caps.

## Definition versus screening choice

The50,000km2 ice-sheet/cap area split and two-year ground-temperature duration are sourced conventions. Ice-sheet equality is assigned to the larger class here as an explicit boundary convention. A cap also needs dome/unconfined morphology; valley confinement takes precedence. All other supplied numeric defaults are provisional screening/model choices, including density approximations, regional distances, biome proxies and terrain thresholds. They are not inferred Earth quantiles or calibrated distribution weights. The Mediterranean dry-summer tests use monthly/seasonal rates; an annual mean cannot fill those inputs. No rainfall or thermal threshold changes the underlying physical climate process.

Flotation screening requires a marine bed below sea and compares ice column load against displaced water, rho_ice*H <= rho_water*water_depth, with actual grounding and contiguous glacial-ice provenance. It does not turn frozen seawater into an ice shelf. Rho values are provisional; firn, salinity and transport remain separate uncertainties. Peak prominence uses summit-to-key-col topology; maximum height within a finite search alone is only a lower-bound/local result, and a missing higher parent at the search boundary must remain flagged.

## Sources and scope

The sources below support qualitative definitions. The explicit detector decisions and all invented thresholds remain reviewable registry proposals. NASA supports the dependence of vegetation on temperature and water; Beck et al. provide an example of seasonally informed climate classification, not a source for our annual-mean proxy boundaries. NSIDC distinguishes land ice, floating glacial shelves, sea ice and persistent frozen ground. NOAA separates ocean-floor morphology; USGS describes groundwater discharge, viscous extrusive domes, hydrothermal systems, topographic prominence and plate-boundary motion. None of these sources establishes this simulator's occurrence counts or arbitrary association radii.

- climate: https://science.nasa.gov/earth/earth-observatory/global-maps/land-surface-temperature-vegetation/
- seasonality: https://www.nature.com/articles/sdata2018214
- ice bodies: https://nsidc.org/learn/parts-cryosphere/ice-sheets
- permafrost: https://nsidc.org/learn/parts-cryosphere/frozen-ground-permafrost
- shelves: https://nsidc.org/learn/parts-cryosphere/ice-shelves
- sea ice: https://nsidc.org/learn/parts-cryosphere/sea-ice
- ocean morphology: https://www.noaa.gov/education/resource-collections/ocean-coasts/ocean-floor-features
- guyots: https://oceanservice.noaa.gov/facts/guyot.html
- springs: https://www.usgs.gov/water-science-school/science/springs-and-water-cycle?page=1
- peaks: https://www.usgs.gov/communications-and-publishing/news/earthword-prominence
- volcanic domes: https://pubs.usgs.gov/gip/volc/types.html
- geothermal: https://pubs.usgs.gov/gip/volc/geysers.html
- plate boundaries: https://apps.usgs.gov/thesaurus/term-simple.php?code=217&thcode=61

## Parameter inventory

This is a review index; the feature-local records are the consumer source. Units are SI.

| Name | Value | Unit | Meaning |
|---|---:|---|---|
| DESERT_PHI_MIN | 5 | 1 | PET/P threshold for a dry-climate desert candidate; chosen, not a universal biome law. |
| HOT_DESERT_MEAN_MIN_K | 291.15 | K | Annual-mean split of hot/cold desert potential; chosen proxy, not monthly Koppen classification. |
| COVER_FRACTION_MIN | 0.5 | 1 | Fractional cover threshold for boundary extraction; chosen. |
| BOREAL_MEAN_MIN_K | 273.15 | K | Lower annual-mean boreal climate-potential threshold; chosen. |
| BOREAL_MEAN_MAX_K | 278.15 | K | Upper annual-mean boreal climate-potential threshold; chosen. |
| TREE_WARM_MONTH_MIN_K | 283.15 | K | Warmest-month tree-limit screening value; growing-season/cover evidence still required. |
| FOREST_PHI_MAX | 2 | 1 | Upper PET/P for provisional forest moisture suitability; chosen. |
| EDGE_WIDTH_M | 10000 | m | Presentation transition width around a resolved edge, not evidence of its positional accuracy; capped by feature width. |
| MEDITERRANEAN_DRY_MONTH_RAIN_MAX_M_S | 1.52102821507e-08 | m/s | 40mm over an average month expressed as rate; seasonal dry-month screening, not annual rainfall. |
| MEDITERRANEAN_DRY_WET_RATIO_MAX | 0.333333333333 | 1 | Summer driest/winter wettest monthly rainfall ratio for dry-summer candidate; temperature seasonality also required. |
| ICE_THICKNESS_MIN_M | 1 | m | Minimum modeled land-ice thickness for regional ice-body candidates; chosen resolution threshold, not permanence proof. |
| PERMAFROST_DURATION_S | 63115200 | s | At least two successive years of ground temperature at/below freezing, per definition. |
| TROPICAL_MEAN_MIN_K | 291.15 | K | Annual-mean warm-climate proxy only; not the monthly tropical climate criterion. |
| SAVANNA_PHI_MIN | 1.5 | 1 | Provisional PET/P lower water-limitation bound for savanna suitability; chosen. |
| STEPPE_PHI_MIN | 2 | 1 | Provisional PET/P lower water-limitation bound for non-tropical steppe suitability; chosen. |
| TEMPERATE_MEAN_MIN_K | 278.15 | K | Annual-mean temperate climate-potential bound; chosen. |
| TEMPERATE_RAIN_MIN_M_S | 1.90128526884e-08 | m/s | 0.6m annual rain as SI rate, provisional forest-suitability threshold. |
| RAINFOREST_RAIN_MIN_M_S | 6.33761756281e-08 | m/s | 2m annual rain as SI rate, provisional wet-forest suitability threshold, not sufficient for rainforest. |
| RAINFOREST_PHI_MAX | 1 | 1 | PET/P upper bound for wet-forest suitability; chosen. |
| TUNDRA_MEAN_MIN_K | 263.15 | K | Annual-mean cold vegetation potential bound; chosen, not a growing-season criterion. |
| ISLAND_GROUP_GAP_M | 150000 | m | Maximum marine separation for a proposed archipelago member edge; chosen. |
| ISLAND_GROUP_MAX_DIAMETER_M | 1500000 | m | Bound on group diameter to prevent unbounded single-link chaining; chosen. |
| COASTAL_REACH_M | 150000 | m | Regional search inland from a coast for low-plain candidates; chosen. |
| COASTAL_PLAIN_HEIGHT_MAX_M | 200 | m | Maximum present sea-relative height for coarse coastal-plain candidates; chosen. |
| PLAIN_RELIEF_MAX_M | 100 | m | Maximum relief over resolved context for plain candidates; chosen. |
| CONTINENT_AREA_MIN_M2 | 1e+12 | m2 | Operational continent/island size convention of one million km2; geology and topology remain required, not a universal definition. |
| PLAIN_SLOPE_MAX | 0.01 | 1 | Rise/run upper bound for provisional alluvial plain geometry; chosen. |
| RIVER_Q_MIN_M3_S | 10 | m3/s | Resolved coarse river-network discharge threshold; chosen, not a physical minimum river flow. |
| SPRING_Q_MIN_M3_S | 0.001 | m3/s | Discharge threshold for cataloguing a demonstrated spring; chosen. |
| WATERFALL_DROP_MIN_M | 10 | m | Minimum resolved abrupt channel drop for a waterfall candidate; chosen. |
| WATERFALL_LENGTH_MAX_M | 1000 | m | Maximum along-channel length of a resolved waterfall step; coarse lattice drops remain unresolved. |
| WETLAND_HYDROPERIOD_MIN_SHARE | 0.25 | 1 | Provisional inundated/saturated fraction of declared year for wetland screening; hydroperiod/vegetation must be demonstrated. |
| ICE_SHEET_AREA_MIN_M2 | 50000000000 | m2 | 50,000km2 conventional ice-sheet/cap split (NSIDC); smaller valley-confined glaciers are not automatically ice caps. |
| ICE_DENSITY_KG_M3 | 917 | kg/m3 | Provisional pure ice density for flotation screening; firn and salinity corrections unresolved. |
| SEAWATER_DENSITY_KG_M3 | 1025 | kg/m3 | Provisional sea-water density for flotation screening, not a selected planet salinity. |
| WATER_DEPTH_MIN_M | 0.1 | m | Numerical nonzero water-depth threshold for connected water bodies; chosen. |
| ABYSSAL_DEPTH_MIN_M | 3000 | m | Depth candidate threshold, never sufficient without marine morphology; chosen. |
| ABYSSAL_HILL_RELIEF_MIN_M | 50 | m | Lower resolved relief threshold for hill morphology; chosen. |
| ABYSSAL_HILL_RELIEF_MAX_M | 1000 | m | Upper hill relief screening threshold; isolated volcanic highs require separate classification. |
| ABYSSAL_SLOPE_MAX | 0.005 | 1 | Rise/run upper bound for deep-plain screening; chosen. |
| TECTONIC_REACH_M | 250000 | m | Regional maximum association distance to actual boundary geometry; not a claim of uniform boundary width. |
| YOUNG_OCEAN_AGE_MAX_S | 6.31152e+14 | s | Young seafloor screening age; active boundary evidence remains required. |
| RISE_SLOPE_MAX | 0.02 | 1 | Upper rise gradient screening value; sediment and margin topology required. |
| CONTINENTAL_SLOPE_MIN | 0.02 | 1 | Lower continental-slope rise/run screening value; margin topology required. |
| FRACTURE_AGE_JUMP_MIN_S | 1.57788e+14 | s | Across-line crust age contrast for inherited fracture candidates; chosen. |
| SEAMOUNT_RELIEF_MIN_M | 1000 | m | Catalogue screen for a resolved isolated marine volcanic high; smaller knolls remain distinct. |
| SUMMIT_RELIEF_MAX_M | 100 | m | Flat-summit local relief screen; past truncation evidence required for guyot interpretation. |
| SEA_GATE_WIDTH_MAX_M | 300000 | m | Gateway width search convention for semi-enclosed sea candidates; explicit enclosure still required. |
| BOUNDARY_SPEED_MIN_M_S | 3.1688087814e-11 | m/s | Relative normal/tangential motion screening of1mm/yr, chosen numerical activity threshold. |
| OCEAN_PLATEAU_RELIEF_MIN_M | 500 | m | Broad oceanic anomaly height screen; crustal/igneous evidence required. |
| OCEAN_PLATEAU_AREA_MIN_M2 | 100000000000 | m2 | 100,000km2 operational large plateau screen; chosen. |
| TRENCH_ANOMALY_MIN_M | 1000 | m | Local depression anomaly screen near convergence, not absolute ocean depth. |
| AXIAL_VALLEY_DEPTH_MIN_M | 100 | m | Resolved cross-axis depression screen; chosen. |
| CANYON_INCISE_MIN_M | 100 | m | Resolved incision below shoulders for submarine-canyon screening; chosen. |
| FAN_SLOPE_MAX | 0.02 | 1 | Upper rise/run gradient screen for depositional submarine fan candidates; chosen. |
| PEAK_PROMINENCE_MIN_M | 300 | m | Regional summit/key-col prominence catalogue screen; chosen. |
| TERRAIN_CONTEXT_RADIUS_M | 200000 | m | Declared regional terrain context, not resolution of subcell summits or passes; chosen. |
| OROGEN_RELIEF_MIN_M | 500 | m | Regional relief screen for tectonically supported mountain belt; chosen. |
| GEOTHERMAL_HEAT_FLUX_MIN_W_M2 | 0.2 | W/m2 | Anomalous local shallow conductive flux screening value; hydrothermal transport can differ and needs independent evidence. |
| GEOTHERMAL_CLUSTER_RADIUS_M | 20000 | m | Provisional spatial grouping radius for known hydrothermal sites; not inferred from coarse mantle heat alone. |
| DOME_RELIEF_MIN_M | 50 | m | Resolved viscous extrusive mound relief screen; vent/eruption evidence required. |

## First consuming implementation

Planet-forge c20fe5f adds the relief-landscape record writer and consults25 literal feature-local parameter pairs; only those pairs are marked used. This means consulted, not calibrated or proof of the complete trigger. The other115 pre-existing process parameter lists remain unchanged. The new record-only classifier was revised by its owner: SI storage/conversion,62process settings,9added and6removed settings and three further recognition-value changes are recorded in the handoff audit. Its terrain-identical e119 rerun is reported by Planets and has not been independently rerun by Registry.

Planet-forge 1bc9fd4 closes the previously recorded ice-threshold override: the feature's 1m thickness and 5e10m2 area now control those comparisons directly; the two process duplicates were removed (60 process settings remain). Required feature trigger parameters missing or non-finite cause an error rather than a literal fallback. This does not establish the full ice-cap morphology or adaptive-area calculation.

The writer now labels 24 stand-in-evidence kinds with trigger.proxy and proxy_for, and the report counts proxies separately from confirmed placements. Planets reports e119 as 168 confirmed kinds plus 23 proxy-only kinds of 241; these counts were not independently regenerated by Registry. Seasonal, groundwater, flotation and fine-morphology gaps remain. The pattern feature-proximity consumer is a separate screening signal; proxy outputs must not silently become confirmed physical evidence.
