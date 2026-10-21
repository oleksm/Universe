# Smooth shading for pyramid terrain

Pyramid tiles now use smooth per-vertex normals derived from centred height-grid differences
in the planet's frame. A one-sample halo makes shared tile edges use identical neighbouring
samples, and skirt vertices inherit the surface normal. Colours remain per vertex and interpolate
across triangles through the existing pooled-mesh shader, without face-colour overrides.

Tests cover unit normals, the expected terrain slope, identical edge normals/colours, and smooth
mesh flags. Matching before/after captures use a frozen copy of the rebuilt Shasta package with
the 30 km pyramid hole. See docs/diagnostics/planetlod-smooth-20261021.json.
