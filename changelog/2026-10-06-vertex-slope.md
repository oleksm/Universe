# The ground's slope per vertex, smooth

- Each near-ground vertex carries its slope (rise over run) from the heights 600 m either side of
  it each way (central differences), in its data's fourth value (`±(1 + slope)`, + where the
  surface fields are read); the ground's material takes it (`GroundIn::slope`): true at the 600 m
  scale and smooth from vertex to vertex, where a triangle's normal steps at its edges and the 5
  km normal map has no steep faces. The lab's ask, for rock on steep ground.
