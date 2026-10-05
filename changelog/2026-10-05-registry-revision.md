# Registry: a record says how firm it is

*Registry only (`fso`).*

- **A `revision` section on records:** whether it is a draft, in review or complete; who
  decides a change; what another agent does when it does not fit its work; why. Each kind has a
  default, so only records that differ say their own.
- **A dictionary file** (`standards/dictionary.schema.yaml`): the shared lists of values, each
  value with its meaning.
- Two new reports on the registry page: Dictionary and Revisions.
- **A change tracker** (`standards/changes.yaml`, generated): one line a record with when it last
  changed, its status, what changed and the hash of its file. A record that is complete may not
  change: validation fails until the user reopens it.
