Ship Studio Test Drive now shares normal flight's pilot input, manual numpad
mapping, rate controller, force-demand policy and bounded thruster allocator.
Fitted flight computers set rates; releasing steering brakes rotation without
levelling. W/S is forward throttle, Shift+WASDQE translation, G manual. Removed
the separate Studio angle-target controller and tilt cap. Chase/radar forward
now agrees with saved -Z designs. Balance's trim uses the shared allocator.
Fuel exhaustion limits realized thrust before the single angular integration.

Studio's local resource/environment/contact simulation remains distinct.
See docs/studio-shared-controls.md for scope and validation.

Validation: 146 workspace tests, registry build, both release binaries, and
inspected Vulkan assisted/manual/Balance captures pass.
