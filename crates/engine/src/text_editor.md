# Multiline text editor API

`engine::text_editor::{TextEditor, EditorRect, EditorResponse}` and
`engine::clipboard::{ClipboardAccess, SystemClipboard}`.

```rust,ignore
let mut editor = TextEditor::new(source_string);
editor.focused = true;
let mut clipboard = SystemClipboard::new();
let rect = EditorRect::new(panel_origin, panel_size);
let response = editor.update(&ctx.input, rect, &mut clipboard);
if response.changed { /* debounce validation; do not apply invalid text */ }
editor.set_error(validation_error);
editor.draw(frame, rect);
```

Route modal input exclusively while the code panel owns focus. `update` borrows
Input and does not consume it or suppress other game hotkeys. Opening or closing
panels and Apply/Save transactions belong to the caller. Errors can be displayed
with `set_error`; `error()` also exposes clipboard failures. Avoid clearing errors
every frame: update validation on edits/actions. `goto(line,column)` uses one-based
Unicode-scalar positions for validation locations. `cursor_line_column` has the
same convention. Selection is a UTF-8 byte range. `set_text` resets history and
viewport; `text`, `insert`, `select_all`, `undo`, `redo` support caller actions.

EditorRect has public `min: Vec2` and `size: Vec2`; EditorResponse has public
`changed` and `focused`. Clicking inside focuses/selects; dragging extends the
selection; Shift-click extends from the anchor. Arrows, Shift-arrows, Home/End,
Ctrl+Home/End, PageUp/Down move/select. Ctrl+A/C/V/X and Ctrl+Z/Shift+Z/Y provide
clipboard and undo. Wheel scrolls vertically; Shift-wheel horizontally. Typing
reveals the caret. Tab inserts two spaces. CRLF/CR imports normalize to LF.
Pastes are single undo operations; typing is per delivered text event. Undo is
bounded to100 snapshots/64MiB. Edits reject NUL and text beyond16MiB atomically.

The backend uses installed wl-copy/wl-paste under Wayland or xclip under X11,
without shell interpolation. No display/helper, non-UTF8 data, nonzero exit,
oversized input and a2second timeout are explicit errors. Cut deletes text only
after a successful clipboard write. SystemClipboard is stateless; GUI callers
may retain it. Helpers own clipboard lifetime after a successful write. The
trait permits deterministic tests and other platform backends. Non-Linux desktop
backends are not supplied by this change.

UTF-8 is preserved, but cursor movement counts scalar values rather than grapheme
clusters. The existing engine font still substitutes unsupported glyphs; this
change does not add font shaping or IME composition. Tabs already in a document
occupy one cell with a visible marker. Rendering clips and draws visible lines
only; source text is plain text, not syntax highlighting. The status row shows
errors and current position. Clipboard access is synchronous but time-bounded;
an unavailable helper may stall a shortcut for up to2seconds.
