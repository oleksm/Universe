# Fit unimplemented equipment as inert modules

- Registry kinds dispatched as `not_made` can now retain their physical fitting
  in supported game slots. They keep their key, mass, volume, mount and authored
  box, but provide no device capability and draw no operating power.
- Only explicitly priced inert records enter the playable catalogue; unpriced
  prototypes remain excluded. The WIP mining-hammer rename already has a price.
  Normal fit checks still apply. Inert modules cannot satisfy mandatory functional
  base blocks, so an inert sensor or drive does not make a ship flight-ready.
- Regression checks cover not-made dispatch, physical metadata, zero function,
  fitted hardpoint mass/boxes, absent weapons and rejection of an inert base block.
- The breaker schema/key rename is still owned by registry-wip. No registry branch
  was merged; no breaker mechanics or invented operating figures were implemented.
