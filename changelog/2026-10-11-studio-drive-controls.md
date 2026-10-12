# Studio drive control feedback

Test drive now shows controls for the fitted flight computer state. Manual-only ships identify the missing flight computer, show their upward-engine numpad keys persistently, and no longer promise mouse steering or throttle control. Assisted flight distinguishes Shift+E upward thrust from W/S forward throttle. Crashed tests show the restart key.

The input and physics remain shared with normal flight. No flight computer, fuel or engine direction is silently added to a design. A regression exercises the shared manual key sampler on a lift-only craft without a flight computer: W fires nothing, numpad input produces upward thrust.

Validation: 166 workspace tests and registry build passed. Vulkan capture on a copy of commuter-1 confirms readable manual controls. Read-only simulation checks of commuter-1/2/3 and design-3 all produced upward thrust with their manual lift jets; this does not qualify the designs for stable flight. Evidence: out/review/drive-controls/.
