# Read windowed fine terrain for the belt's 100 km band

Prefer `pyramid_deep/` when present. Level files can store a rectangular window in
global sample coordinates: use its row offset, column offset and row stride for
height, colour and mask reads, and validate actual file lengths against that window.
Read tile rows only; extend edge heights for normal calculation and mark samples
outside the stored window as absent. Full rasters remain compatible.

Refine only where listed children exist. At a cut window boundary, retain the whole
coarse parent until the fine window covers its full footprint, avoiding gaps outside
local fine data. The revised data selects 150 m detail at 100 km and 37.5 m closer to the belt.
Use the finest full-continent mask for static-package coverage; do not allocate an
implied full 196,608-square fine raster or leave duplicate static ground outside the
fine window. Ground sampling continues to fall back through cached coarse levels.

Regression tests exercise nonzero window offsets, stored row strides, absent samples,
normal halos, invalid windows, terminal parents, boundary cover and existing formats.
Full-fixture and native evidence: docs/diagnostics/planetlod-deep-20261021.json.
The selected working set remains protected under the existing soft cache policy;
the original worst-point error data used roughly 1 GiB at 100 km. The final percentile
error data reduces that working set; both captures are recorded in the audit.
