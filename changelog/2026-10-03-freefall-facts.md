# Freefall Facts merged

The registry branch (`fso`) is merged into `main`. `standards/` now holds Freefall Facts, the YAML
the game is to be sourced from, built by `python3 tools/standards/build.py` into
`standards/index.html` and the game's `content/base/{brands,bodies,standards}.ron`:

- **SFO** (Standards Foundry Office, was the FSO): 11 standards, 118 elements, 24 materials,
  30 processes, 12 industrial modules, 14 goods.
- **Maker House**: the twelve makers (the game's brands, now written from here) and two companies
  that are not makers (a warehouse company, an exchange).
- **Local Administration**: the home system Treistun as the game makes it (rocky planets, moons,
  the station, the ten ports with their latitude and longitude), and at Port Trethi its streets,
  a power line, zones, parcels and three facilities (a foundry, a power station, a warehouse).
- **Production Register** and **Address Registry**: references, worked out.

For the game: the standards body's key is `body.sfo` (was `body.fso`); `brands.ron` is the same
twelve brands in alphabetical order (the content hash changes). Nothing else in the registry is
loaded by the game yet.
