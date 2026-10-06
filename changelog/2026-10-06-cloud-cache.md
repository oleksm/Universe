# The clouds cached

- The lab's cloud cache plumbed (`engine::cloudcache`): two 512²×9 half-float texture arrays (the
  copy now and the one before), three nested levels round the point under the eye (4,000, 400,
  40 km across each way), three shells each. A compute pass fills two layers a frame
  (`cloud_cache.wgsl`; its first layer a uniform, `cc_layer0`); the levels are refreshed round
  the three, one at a time (the clouds move slowly), and re-centred when the eye has moved an
  eighth of the inner level; each change eases in from the copy before over 1.5 s. Bound at
  group 2, 16 to 18; the scene reads `clouds_over_cached` and `clouds_shadow_cached`, which fall
  back to the noise where no level holds a place. A new world bound: the cache starts again.
- Measured at 1080p, full scale, over Heath: from 77 km the scene 9.2 → 2.9 ms; from 30 km
  11.6 → 4.9; from 14 km over the sea 11.0 → 3.9; at 3 km/s from 14 km 3.1, no errors.
