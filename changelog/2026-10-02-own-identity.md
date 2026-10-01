# Our own names and our own ship

So nothing borrowed from other games stands in ours:

- **Star names from our own generator.** Syllables of an onset, a vowel and sometimes a coda,
  with soft endings after consonants. The old letter-pair table is gone. Every system has a new
  name (from the same seeds, so saves keep their place). For example: Thabro, Seivarum, Laima,
  Mogorn, Cerithar, Brosai.
- **The starting hull is the DROVER** (`hull.drover`), with a new shape: a long hexagonal hull,
  blunt-nosed, broadening to an engine block with its two drive outlines on the rear plate. It
  is 40 m long and 20 m wide.
  - It has the same frame, slots, fit and price as before (60 t dry, 30 m/s² main, 25 m/s² lift).
  - Its shape makes it pitch a little slower (1.4 rad/s²) and roll faster (3.5).
  - Its thrusters are laid out balanced about its centre of mass.
- **All five hull shapes are given about their centre of mass:** points, nozzles, gear and
  cockpit in one frame. Before, the T5 hulls' nodes were laid out about it but their points
  weren't, so the courier's drive nozzles sat 4.6 m off its rear plate.
- **Old saves:** a save whose hull (or a fitted module) is gone from the content loads as the
  starting hull with its stock fit (`save::forget_missing`) instead of failing to load.
- **References removed:** other games' names in comments, docs, changelogs and tests. Labels show
  the ship's hull name rather than a fixed one.
