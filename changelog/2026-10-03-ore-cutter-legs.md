# ORE CUTTER: landing legs, a belly hatch, two main bells; ships stand on their feet

- **Ships rest on their own feet.** Set down (raw ground, a spaceport pad, a station deck,
  taxiing, the hangar) a modelled hull stands at the depth of its lowest `gear_*` contact
  instead of everyone's 12 m (`ClassSpec::rest_height`; hulls without a model keep 12 m and
  their drawn legs). `station::rest`, `port::settle`, `port::pad` and `port::hangar` take the
  height. The ORE CUTTER now stands about a metre lower than before, its feet on the ground.
- **Landing legs, not wheels**: it sets down on its belly side, so four struts splayed out to
  round footpads, each a sleeve with a chrome piston (a shock absorber), braced fore and aft,
  out of a bay whose two doors hang open beside it. Two under the keel forward, two under the
  ore pods aft.
- **A belly crew hatch** in the keel (orange door in a dark frame, a lamp over it). New import
  convention: an empty named `hatch`; stepping out, the stair runs aft from it to the ground and
  you face along it. Hulls without one keep the port-side ramp.
- **Two main bells** instead of three (bigger: 2.9 m mouth).
- **"MASS LIMIT 900 T"** sits lower on the engine block's flat side; placards sink 0.3 m into
  the hull so over a recessed plate they still sit on metal, not float.
- Not yet: the legs are always out (no retracting in flight), and touching down still triggers
  on the hull's contact spheres, then sets the ship on its feet.
