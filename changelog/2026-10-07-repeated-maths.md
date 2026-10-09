# Repeated maths, once (audit phase 4, #14)

- `services::lots`: a good's lots where it lies, oldest first, with the two moves (pieces in,
  joining a lot already there; kilograms out, oldest first). The economy's pools and the ledger's
  holds both use it, in place of two copies (a hold now joins pieces of one lot too, as a pool did).
- `Body::surface_gravity`, in place of four copies (`legs::surface_gravity`, the rules' landing
  check, the legs test, the HUD and pilots through legs).
- `hypernet::delays_to`: the quickest way over the backbone from each system to us, in place of
  the news's and the price boards' two copies; `hypernet::relay_of`: where a facility's relay sits,
  in place of two matches.
- `World::settled`, `Charts::settled` (`gate::settled`): the settled systems, in place of eight
  copies of the gate-links sort-and-dedup.
- glam's `reject_from_normalized` in avionics where the part across an axis was written out (five
  places; every axis there a unit vector, so the same numbers).
- Not made one: the two holding patterns (a station's ring and a port's circle under gravity) fly
  differently; sharing them would change how ships hold.
