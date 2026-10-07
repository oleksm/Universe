# 2026-10-07: the generated RON files are gone

- The eight `content/base/*.ron` files the build wrote for the game (bodies, brands, celestial,
  galaxy, industry, rock_classes, settlements, standards) are deleted and build.py no longer writes
  them: the game reads the records themselves through crates/registry (the engine audit found
  nothing read them). `content/base` keeps only the game's own hand-kept files: aliases, prices,
  shapes, sheet. README, standards.md and the integration doc say so.
