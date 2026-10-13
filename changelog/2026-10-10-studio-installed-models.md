Ship Studio DESIGN now draws installed equipment meshes for catalogue modules
placed in hull-free designs, including CH-S2's neutral rigid GLB. A shared checked
geometry cache serves design drawing and mounted flight visuals. Design meshes
keep authored metres, are centred once at the existing block centre, and follow
an engine's saved thrust direction. Existing block envelopes, picking, saves,
clash checks and placement remain unchanged; missing/unsupported models retain
box drawing. Motion metadata does not prevent neutral design previews; flight
still refuses equipment requiring an articulated mounted consumer.

The design canvas uses sorted projected triangles with material base colours
and fixed studio shading, not the flight PBR/textures or operational animation.
Validation: six-axis thrust/centering and save round-trip test; registry build,
workspace tests, release launchers and Vulkan design capture (details in
 docs/studio-installed-models.md).
