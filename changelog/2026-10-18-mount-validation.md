# Require verified mechanical mounting

Studio budget faults and flight compilation now reject every placed module
without a verified mechanical attachment, including landing legs. Diagnostics
identify the module instance and its registry mount family, or a missing family.
Nearby frame joints and room/floor overlap do not prove attachment.

This is the validation-first gate requested by the owner. Saved designs do not
yet represent explicit compatible mount instances and structural load paths;
therefore existing populated designs remain editable but cannot test-fly until
that authoring and resolution work is implemented. The older inferred bearing
analysis remains an estimate, not evidence that a design has valid attachments.
No saved commuters are modified or automatically fitted.

Regression coverage checks floating and frame-touching landing gear, equipment
at frame joints, and rejection at the flight compilation boundary.
