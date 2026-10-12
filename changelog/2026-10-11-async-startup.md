# Responsive startup and native model preparation

Both Studio entry points and the game now use engine `run_prepared`: a responsive shell appears before constructing the application on a worker. Flight/collision data, imported hull commissioning and procedural hull meshes remain prerequisites; only a complete application is published. Preparation failures remain visible, and Escape/window close do not join the worker.

Native hull drawing, developer showcases and mounted-equipment frame discovery request the engine-owned shared visual cache. Model reads/decodes happen on workers, leases follow PBR draw clones, and pending visuals retain existing hull fallbacks. Hash-bound mount overlays are read during startup preparation; moving replacements remain held. Studio's MC-07 commissioning still runs, even for hull-free designs, but no longer blocks the window thread.

Startup logs distinguish first shell CPU submission from CPU/physics readiness and record hull, world/traffic and procedural-model stages. These are not GPU presentation timings or proof of faster final scene readiness. Native paths/options are immutable per session; restart after replacing a native file. Development overlay metadata is limited to 4 MiB.
