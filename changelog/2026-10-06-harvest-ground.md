# Harvest's ground from the planet simulation

- **A body with a surface bake stands on it** (`worlds::Heights`, `Terrain::bake`): Harvest's 5 km
  heights over the whole sphere (read once, 0.3 s) and its 600 m tiles where there is land (each
  read from the worlds store the first time it's wanted, about 10 ms, and kept), for collision and
  drawing alike. Its craters and noise are gone; the sea fills below 0 as before. Without a store
  (or with a store whose bake isn't the record's), the seed's ground, said once on stderr.
- **Coordinates:** a body direction is the registry's latitude and longitude (north +Y,
  longitude `atan2(−z, x)`; `worlds::lon_lat`, `direction`): the bake's highest peak reads 8,920 m
  where its peak list puts it. The bake's cube tiles are in the body's own frame.
- **Ports stand where their records put them** (`settlement.position`). A port's plain is levelled
  to the ground at the port (or the sea, if it's under water) and blends back by 40 km.
- **Port Eikir's record position is at sea** (5,866 m of water on the grown Harvest): asked of the
  Scientist (`docs/registry-ssot-request.md`).
- The first frames near the ground wait on the tiles they draw (a few seconds, once): reading them
  in the background is to do.
- Dev: `UNIVERSE_BODY=<key>` points the planet scenarios at another body.
