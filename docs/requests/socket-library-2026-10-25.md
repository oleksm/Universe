# Socket library (registry to blender, 25 Oct 2026)

Owner: a hull designer must place sockets exactly, so the socket's face (shape and size) is what blender designs and
knows. First the library, then hulls are built from it by ship class.

Standard: SFO 19 (standards/SFO/metadata/0019-mounts.yaml), each mount in standards/SFO/metadata/mounts/.

## Ask

1. **Socket faces.** For every mount (`reg find mount`): the exact flat area the hull gives at the mating plane:
   `face: {shape: rectangle, length, width}` or `{shape: circle, diameter}`, centred on the mount frame, x along. It
   must hold the mount's sockets (`sockets[].at`) and the holder that sits on it (adapter or deck track). Model each as a
   library piece: a plate at z 0 with the mount_<slot> node at its centre, local +Z out of the hull.
2. **Library by ship class.** Which sockets (mount kind and size class, how many) each ship class carries: mining ship,
   hauler, courier, explorer... (`hull.identity.class`). A class's set is what a hull of that class is built with.
3. **Deliver** a request file (faces per mount key, the class sets) and the library .blend; the registry writes the
   faces into the mounts and the class sets as records.

Unchanged: clocking, approach and motion per mount (`mating`), the sockets' bolt pattern (mounting.py).
