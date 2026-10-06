# The planet studio's timeline

- A world's history from the store (`planet-sim-history/1`: globes in time order, each with its
  land, plates, highest and deepest then), read on going to it and checked against the store's
  index (`worlds::History`). PAGE UP steps back in time from today, PAGE DOWN on to today (the
  arrows turn the observer); the panel gives the frame (n of 18, Gyr since the world began and
  before today) and its figures.
- The frame's globe is drawn as the world's colour, on today's relief (the history has no
  heights); today's sea and clouds aren't drawn on it, they aren't the world's then. Back to today
  restores them.
- Dev: `UNIVERSE_FRAME=n` with the `worlds` scenario opens on frame n.
- (The bake's image decoding is shared with the history's: `worlds::decode`.)
