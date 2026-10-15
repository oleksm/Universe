# Contact group at runtime

Opt-in (`UNIVERSE_PGS1_CONTACT_GROUP`): the game builds contact groups on the ground pool from settled selections,
reserves each whole (16 MiB, active/uploading/retired together), stages it, and publishes it atomically in place of
the patches it replaces; the active group's patches are pinned in the drawn selection so it stands until the eye
nears its core's edge, and the next replaces it in the same frame. New engine call `Frame::release_mesh` drops a
retired mesh's buffers at once. Readiness trace (`UNIVERSE_PGS1_READINESS_TRACE`) in the planets lab's format and a
dev camera walk. Four GPU runs on the v8 source: ready from the first group on, contact within 1.7 µm, peak
8.3 MB extra. Defaults unchanged. See docs/diagnostics/contact-group-runtime.md.
