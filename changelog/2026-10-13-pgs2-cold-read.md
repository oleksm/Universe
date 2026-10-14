# PGS1 revision 4: cold-read measurement

- `pgs2_cold` example (universe-world): reads a revision-4 package mapped, through the cell index, with the
  manifest's opening records and per-block hashes checked on touch; reports cold open + first query time, bytes read,
  and answers every v8 fixture against the canonical reader.
- v8: region first query 11–22 ms, root 1.3 ms, cold; all 4,352 fixtures identical to the canonical reader
  (docs/diagnostics/pgs2-cold-read.md).
