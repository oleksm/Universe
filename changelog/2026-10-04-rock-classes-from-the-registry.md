# Rock classes made from the registry

- **A rock class is its record.** `belt::RockClass` is no longer a list in code. It's a handle on
  one of the registry's eight rock classes (`rock-class.*`). Each class's density (rubble and
  solid), albedo, colour, composition ranges, cut energy and what it yields are read from its
  record. The seed still makes the four it always has (stony, carbonaceous, metallic, icy); its
  odds are next, with the seeding records.
- **What digging yields comes from the record too**: the class's `yields`, or its `rich_yields`
  where its platinum-group metals pass `rich_above`, as the game's ore that good is. The code's
  own PGM threshold (30 ppm) is gone; the record's (0.003%) is the same.
- **Composition:** each share is taken from its class's range, lean to rich, as before.
  Platinum-group metals are now spread by ratio across their range for every class. An M-type
  is as before (10 to 60 ppm). A C-type or S-type shows slightly less at middling grades than it
  did. That's a display only: they never reach the rich threshold.
- **Lists show each class's spectral type from its record.** The icy class now reads COMET-LIKE,
  its record's `letter`, where it read ICY.
- The good records (`good.*`) are read too, strictly typed, with the shared `physical` group.
