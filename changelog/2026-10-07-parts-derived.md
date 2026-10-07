# Parts as a rule (fso merged: audit step 3, parts)

- 703 part files of 146 equipment records deleted: each equipment record lists its parts
  (`built_of.list`), and the registry reader derives the part records from the list through the
  same YAML the build writes (`registry::derived_part_yaml`), parsed with the generated `Part`. The
  Scientist checked them equal to the files field for field; `reg.parts`, the goods catalogue,
  recipes and marks see the same set. Measured parts (the MC-07's, the gate's) and the modules'
  keep their files.
