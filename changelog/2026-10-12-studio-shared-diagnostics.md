Ship Studio now uses the existing live observer/recording transport through a host
adapter, with Shift+F3 support and the same frame/renderer/asset diagnostics as the
world. Added named scopes for shared Studio views and equipment projection so
loaded-ship frame costs can be traced in design, walk, balance and test drive.
World-specific status and graphics controls remain explicit host capabilities.
Added standalone observer and delayed-model GPU validation tools; see
`docs/diagnostics/studio-shared-tools.md` for evidence and remaining startup gates.
The observer camera altitude now uses the explicitly labelled body reference
radius; fetching /status cannot synchronously load terrain. HTTP observer work
has its own update/diagnostics scope, including recording overhead.
