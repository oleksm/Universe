# Smooth flight over mountain terrain

Flight altitude is now relative to a broad terrain reference instead of the exact
height under the eye every frame. Sample the published ground at 25 directions in
a disc of radius twice the nominal altitude and average available samples. A
critically damped spring follows that mean, settling about 95% in one second.

Pyramid samples come from the submitted resident cover and its actual morph weights;
prefetched fine data cannot replace a coarse fallback in this reference. Pattern
chunks retain the build snapshot for their published mesh, so terrain sampling follows
their drawn stand-ins. Packed pooled vertices can be queried without copying meshes.

The finest available ground is kept separately for clearance. Only if needed, raise
the reference immediately so the eye stays at least 0.3 times nominal altitude above
that ground. Missing drawn samples hold the reference instead of resetting to zero.
The altitude label now says above average ground, and the panel shows actual clearance.

Viewer tests and engine pool tests pass. Native 10 km pan evidence and limitations:
docs/diagnostics/planetlod-flight-20261021.json.
