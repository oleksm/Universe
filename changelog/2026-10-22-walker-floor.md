# Walker: the floor is the nearest standable surface down the ray

Ships `20261022T193211-ships-d554` (explorer 744a2573, open CargoRamp at 30° on diagnostic flat ground): 0 of 12
"landing via open ramp" and 6 of 12 "toe to ground" walks stayed supported. `walk::floor` took the nearest hit down
its ray, then refused it if too steep: a tread's side (rise 0.5, under SLOPE 0.643) hid the ramp under it (rise
0.866). The same masked the floor at a steep wall's foot, a step's height from the feet.

Now the floor is the nearest surface down the ray steep enough to stand on (`WalkMesh::ray_where`,
`Collider::ray_where`): steeper faces are passed through, left to the walker's sides. SLOPE and STEP unchanged.

The ships' cases with this walker: open ramp 36 of 36 (was 18), closed 72 of 72 (was 72). Test:
`walk::tests::walk_floor_is_the_nearest_standable_surface`; the workspace's library tests pass. Not qualified:
turns, seated egress, suit or carry, the ramp's own motion.
