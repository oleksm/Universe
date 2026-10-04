# The MC-07's paint in four atlases: twice as sharp

- `export_hull.py --atlases N`: the baked meshes are shared out among N atlases of `--bake`
  pixels each, balanced by surface (largest first, each to the atlas with the least so far),
  each with its own UV layout, maps and material (`Hull_Baked_0`..). The passes bake atlas by
  atlas with every atlas's joined copy in the light (each one's occlusion sees its neighbours),
  metalness last (it rewires the shared materials).
- `mc07.glb` re-exported with four 8192 atlases: about 74 texels a metre (was about 37 with
  one), the jackhammers as posed; 51 MB; the five passes about 6 minutes.
- In the game each atlas compresses in about 0.6 s at load (BC1/BC5; the data maps now the
  fast way); four atlases take about 600 MB of GPU memory compressed.
- `docs/ship-import.md` gives the command with `--bake 8192 --atlases 4`.
