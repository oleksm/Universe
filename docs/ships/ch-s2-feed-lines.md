# ch.s2 feed lines across the gimbal: duty, size, joint and boundary (registry answer, 2026-10-10)

To blender's v10 question (`engine-ch-s2/refinement-v10/review/motion-fixture-audit.json`: hose loop bends to 13 mm
against a provisional 266.7 mm dynamic minimum for a DN32 braided hose, and stretches 7.1% over the envelope).

## The duty (derived from the record)

The lines cross the gimbal from the ship to the pump inlets: they carry the propellant at tank pressure, before the
pumps. The record burns 25 kg/s at mixture 5.5:

| Line | Flow | Volume (density) | Speed in a 32 mm bore | Bore for 10 m/s | Bore for 5 m/s |
|---|---|---|---|---|---|
| hydrogen | 3.85 kg/s | 0.054 m3/s (70.8 kg/m3) | 68 m/s | 83 mm | 118 mm |
| oxygen | 21.2 kg/s | 0.019 m3/s (1141 kg/m3) | 23 m/s | 49 mm | 69 mm |

A pump inlet wants the liquid slow, a few metres a second, or it cavitates; 10 m/s is taken here as the upper bound
(invented rule of thumb, review). So the 46 mm lines are too small for this engine's flow, the hydrogen one by far:
about **85-120 mm bore for hydrogen and 50-70 mm for oxygen**. Pressure is low (tank pressure, a few bar; the 7 MPa of
the record's chamber note is the coolant after the pump, not these lines); temperature 20 K and 90 K.

## The joint

A braided hose of that size bends no tighter than several times the 266.7 mm of the DN32 one, and a loop of it does not
fit beside the pumps. Gimballed engines cross the gimbal with **ducts that angulate, not hoses that loop**: a duct with
a bellows joint (held by a gimbal ring or tie rods so pressure cannot stretch it) at each end of a short rigid link, or
one gimballed bellows whose centre lies on the engine's gimbal axis line, so it sees only the gimbal's angle (0.07 rad
per stage) and almost no offset. The pump inlets then sit near the gimbal's elevation (0.43 m aft of the mount face in
the export), where the travel is least. Recommendation: **two-joint tied bellows duct per line, joint centres placed
so the link angulates within each joint's rating; no free hose loop.** The bellows' angular rating and length are the
maker's figures (as with the hose's bend radius: sourced, marked provisional).

## The boundary

The engine owns its feed ducts across the gimbal, from a fixed inlet flange at the mount face to its pump inlets, as it
owns its gimbal ring; the ship plumbs to those fixed flanges behind its closure (the owner's rule: vulnerable lines behind
the hull). The mount standard's `feeds.fuel` is what those flanges pass.

## For the motion file

A tied duct is rigid parts on joints, not a flexible tube: describe each link like an actuator body (a rigid node
aimed from one joint centre to the other, with both joints' parents), so `freefall-motion/1` needs no new kind; the
installer's sweep then checks each joint's angle against its rating once the schema carries it (registry, when the
duct design is set).

## Owners

| Step | Owner |
|---|---|
| Reroute: tied bellows ducts at the bores above, inlets near the gimbal elevation, fixed flanges at the mount face | blender |
| Bellows angular rating and size: a maker's figure, provisional | blender (source), registry (a duct product when needed) |
| `freefall-motion/1`: a joint's angle rating on rigid links, checked over the envelope | registry |
| Pump inlet speed limit (10 m/s here): confirm or replace with a sourced figure | registry, review |

## Joint mass and envelope targets (2026-10-10, for blender proposal-v03)

The WRN25 catalogue joints (40.6 kg for the four) are industrial PN25 hardware: 25 bar, heavy weld ends. These lines
see tank pressure. Derived targets for flight joints (review: design pressure 0.6 MPa = twice an assumed 0.3 MPa tank,
304 stainless at an allowable 140 MPa, invented):

| Item | DN100 (hydrogen) | DN65 (oxygen) | How |
|---|---|---|---|
| bellows wall needed | 0.24 mm | 0.16 mm | pressure x diameter / (2 x allowable); built as two plies of 0.3 mm |
| bellows | 0.44 kg | 0.29 kg | 60 mm active, 10 mm deep, 6 mm pitch convolutions |
| end rings and hinge hardware | about 1.1 kg | about 0.7 kg | carry the pressure thrust, 6.1 kN and 2.7 kN, with margin (estimate) |
| one joint | about 1.5 kg | about 1.0 kg | |
| rigid duct, 0.3 m, 1 mm wall | 0.85 kg | 0.57 kg | |
| one line (two joints, duct) | about 3.9 kg | about 2.6 kg | |

With the two fixed inlet flanges at the mount face (about 0.5 kg each), the feed crossing is **about 8 kg**, taken out
of CHE2-03's 45 kg (valves, lines and injector), which stays 45 kg. Envelope per joint: the catalogue's width (260 mm
and 215 mm) is for PN25 plates; take the bore plus 2 x 40 mm for hinge plates (about 195 mm and 150 mm) and an active
length of 60 mm plus 2 x 15 mm end rings. The joint angle each must take comes from the layout (hinge centres on the
gimbal axes: each hinge turns with one stage, up to 0.07 rad); rate them at least twice that (0.14 rad, 8 degrees),
which catalogue DN100 and DN65 joints exceed (14 degrees and up).

## Bellows surface (engine's open point)

A bellows bends, so one rigid mesh cannot show it. Proposal: split each bellows into its convolution rings as rigid
parts, ring i of n turned by i/n of its joint's angle about the hinge (a fan); the end rings ride their flanges. Rigid
parts only, which the game already draws. Not in freefall-motion/1 until engine agrees.
