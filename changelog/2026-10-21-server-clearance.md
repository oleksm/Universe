# Traffic control refuses by its own records

The rules that refuse a clearance (a system's enemies and those outside its law; a port on ground the ship's
lift can't hold it over or its legs stand it on; a gate whose crossing it can't pay for) were the pilot's alone:
traffic control granted any pad or corridor posted to it. They are one rule now, `services::clearance::refusal`:
the pilot asks it before it asks (what it sees of itself), and traffic control asks it again before it grants a
pad or a corridor, from its own records (the system's standing of the pilot, its law, the ledger's credits, the
gate's span). A request it refuses grants nothing and queues nothing; the ATC journal notes it
(`What::Refused`, with the reason). The pilot's words to the player are as before.

Checked: a unit test (enemies and outlaws refused, others cleared); a busy port scene's traffic, 30 s, no
refusal (the pilots ask the same rule first). Not changed: corridors are still let go on the pilot's reported
final approach (`status.clearance`); a physical check there is a further step.
