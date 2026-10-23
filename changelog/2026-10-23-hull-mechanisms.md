# Hull-authored mechanism states and exits

Hull records can describe rigid node poses and multiple exits, bound to an exact model SHA256. Poses are exported rest or absolute parent-local translation in metres plus a unit quaternion. Mechanism states cover each owned node once; exits name ordered mechanism/state requirements and entry markers. The schema and cross-record checker reject ambiguous ownership, missing/default states, inconsistent pose coverage, nonunit quaternions, unknown exit requirements and missing climb/step endpoints.

The page displays declared states and exit requirements. This is the agreed record contract for Engine's discrete-state consumer; it adds no operating Explorer record, guessed pose or mass. Import must still verify model bytes, unique actual nodes/markers, rigid scale, posed contact and reachable routes. It does not qualify continuous motion, structure, pressure, suits or cargo. Legacy hulls without these optional declarations are unchanged.
