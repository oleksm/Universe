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

## Joint mass and envelope targets (2026-10-10, revised for blender proposal-v03)

The catalogue joints (WRN25, 38.2 kg for the four in v03's DN100 + DN50 pairs) are industrial PN25 hardware: 25 bar,
heavy weld ends. These lines see tank pressure. Targets from v03's routes (`review/duct-study.json`): per line a rigid
spool of about 0.80 m between the joints (1.05 m of arc centre to centre less the joints), a riser of 0.53 m from the
fixed port to joint 1 and 0.37 m from joint 2 to the pump inlet (straight lines; elbows added below). Bores: hydrogen
107.1 mm (6.0 m/s), oxygen 54.5 mm (7.9 m/s), both under the 10 m/s inlet bound.

Assumptions (invented, review): design pressure 0.6 MPa (twice an assumed 0.3 MPa tank); 304 stainless at an allowable
140 MPa; the pressure needs 0.24 mm (DN100) and 0.12 mm (DN50) of wall, so the wall is set by handling and welding:
1.0 mm and 0.8 mm; elbows add 20% to the pipe; foam insulation 25 mm at 30 kg/m3.

| Item | Hydrogen (DN100, OD 114.3) | Oxygen (DN50, OD 60.3) |
|---|---|---|
| pipe, 1.70 m (spool 0.80 + risers 0.90), with elbows | 5.8 kg | 2.4 kg |
| two flight joints (bellows of two 0.3 mm plies, end rings, hinge plates for 6.1 kN / 1.7 kN pressure thrust) | 3.0 kg | 1.6 kg |
| fixed port flange and pump inlet flange | 0.8 kg | 0.6 kg |
| insulation | 0.7 kg | 0.4 kg |
| **line** | **10.3 kg** | **5.0 kg** |

The crossing is **about 15 kg**, inside CHE2-03's 45 kg (valves, lines and injector), leaving about 30 kg for valves and
injector; CHE2-03 stays 45 kg until that split is measured. All provisional.

Ratings: each hinge turns with one gimbal stage, at most 0.07 rad either way (v03's sweep: 0.07). The catalogue's 2aN
is the **total** angular movement: 14 degrees is +/-7 degrees, +/-0.122 rad, 1.75 times the need. That is an industrial
analogue, not a rating for cryogenic flight joints: a flight joint's rating is its maker's (provisional until sourced).
(Corrected: this section said "0.14 rad, 8 degrees" as a rating at twice the swing, mixing a total with an each-way
figure; the need is +/-0.07 rad, a total of 0.14 rad.)

Envelope per joint: v03's joint lengths (0.249 m and 0.230 m, catalogue) stand for now; a flight joint's width is the
bore plus about 2 x 40 mm of hinge plates (about 195 mm and 140 mm) against the catalogue's 260 mm.

## Bellows surface (engine's open point)

A bellows bends, so one rigid mesh cannot show it. Proposal: split each bellows into its convolution rings as rigid
parts, ring i of n turned by i/n of its joint's angle about the hinge (a fan); the end rings ride their flanges. Rigid
parts only, which the game already draws. Not in freefall-motion/1 until engine agrees.
