# Spaceports stand on their ground

A port's pads, its markers, the landing autopilot's pads and holding circle, the pad presence check and
`pad_position` used the body's radius: the datum, right only where the ground at a port was levelled to 0.
They use `spaceport::ground_radius` now (the body's radius and the ground at the port's centre). On a world
whose ground is a land package the ground round a port is no longer levelled for contact: the package is
drawn as it is, so contact meets the drawn ground (the registry puts such a port on a plain: Nacaubun on the
Shasta plain, 2.26 m of relief).

Checked with the registry's Nacaubun move (fso c6b70217f) applied: 400 m over the port, drawn ground
836.81 m and contact ground 836.83 m at its centre; buildings, pad outlines and landed ships on the plain
(out/review/e104-game/game/port_400.png). A ship saved landed keeps its body-frame place: one saved at
the old Nacaubun loads at the old coordinates, not moved to the new port.
