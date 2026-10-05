# Five cores in the shipyard only

The budget of five cores (`UNIVERSE_CORES`: another) was meant for the shipyard studio only (the
user), not the whole game. Now: in the shipyard (its 3D interior and deck studios), every thread of
the game is pinned to five cores (Linux); out of it, they're let go to every core the game was
allowed at the start. The world's crowd and pilot pools and the globe map's threads are sized to
the whole machine again, as before. Checked running: 39 threads on cores 0-4 in the studio, 0-31 in
flight.
