# Player wiring in Studio

Add WIRE (W) to the Studio toolbar. Click an equipment port, choose a cable
from the panel (N cycles the list), place waypoints on the movable work plane,
then click the destination port. Markers show occupied and incompatible ports;
hover explains the channel, direction, connector and incompatibility. Backspace
removes the last draft waypoint; Escape/right-click cancels the draft; Delete
near a finished wire removes it. Normal save/reopen and undo/redo preserve wires.

Endpoints use stable equipment instance IDs and follow equipment poses; intermediate
waypoints stay where the player put them. Port positions use registry coordinates
and verified model-manifest bounds, independently of mesh loading. Missing port
or geometry data gives no snap target. Explicit routes replace inferred routing
and count cable mass once, plus a connector pair at each end.

Connection checks cover service/channel/key, flow direction, connector mating,
conductor size, voltage and an unambiguous contact-role map. They do not certify
current capacity, voltage drop, bend clearance, clips, or runtime power/control
nets. Authored wiring remains blocked from flight pending those checks. Existing
mount-validation restrictions remain in effect.

Deployment dependency: registry's fso port and multicore-cable records must be
merged before this tool can offer those ports/cables. They are not copied into
the game or replaced with guessed snap points.
