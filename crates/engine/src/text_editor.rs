//! Reusable plain-text HUD editor. UTF-8 byte offsets always lie on scalar boundaries.
//! Unicode is preserved; cursor cells use scalar values (not grapheme clusters).
use crate::{
    Color, Frame, GLYPH, Input, KeyCode, MouseButton,
    clipboard::{ClipboardAccess, MAX_TEXT_BYTES},
    input::TextEvent,
};
use glam::Vec2;
use std::ops::Range;

#[derive(Clone, Copy, Debug)]
pub struct EditorRect {
    pub min: Vec2,
    pub size: Vec2,
}
impl EditorRect {
    pub fn new(min: Vec2, size: Vec2) -> Self {
        Self { min, size }
    }
    fn contains(self, p: Vec2) -> bool {
        p.cmpge(self.min).all() && p.cmplt(self.min + self.size).all()
    }
    fn rows(self) -> usize {
        ((self.size.y - 24.).max(10.) / 10.) as usize
    }
    fn cols(self) -> usize {
        ((self.size.x - 12.).max(GLYPH) / GLYPH) as usize
    }
}
#[derive(Default, Clone, Copy, Debug)]
pub struct EditorResponse {
    pub changed: bool,
    pub focused: bool,
}
#[derive(Clone)]
struct Snapshot {
    text: String,
    cursor: usize,
    anchor: usize,
}
pub struct TextEditor {
    text: String,
    cursor: usize,
    anchor: usize,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    pub focused: bool,
    pub scroll_line: usize,
    pub scroll_column: usize,
    error: Option<String>,
    dragging: bool,
    preferred_column: Option<usize>,
}
impl Default for TextEditor {
    fn default() -> Self {
        Self::new(String::new())
    }
}
impl TextEditor {
    pub fn new(text: String) -> Self {
        Self {
            text: normalize(&text),
            cursor: 0,
            anchor: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            focused: false,
            scroll_line: 0,
            scroll_column: 0,
            error: None,
            dragging: false,
            preferred_column: None,
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Replace a document, resetting selection/history and viewport.
    pub fn set_text(&mut self, text: String) {
        let focused = self.focused;
        *self = Self::new(text);
        self.focused = focused;
    }
    pub fn set_error(&mut self, error: Option<String>) {
        self.error = error;
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
    }
    pub fn selection(&self) -> Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }
    pub fn cursor_line_column(&self) -> (usize, usize) {
        let before = &self.text[..self.cursor];
        (
            before.bytes().filter(|&c| c == b'\n').count() + 1,
            before.rsplit('\n').next().unwrap_or("").chars().count() + 1,
        )
    }
    /// Jump to a one-based diagnostic location without altering text.
    pub fn goto(&mut self, line: usize, column: usize) {
        self.cursor = self.offset(line.saturating_sub(1), column.saturating_sub(1));
        self.anchor = self.cursor;
        self.preferred_column = None;
        self.scroll_line = line.saturating_sub(2);
        self.scroll_column = column.saturating_sub(2);
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            cursor: self.cursor,
            anchor: self.anchor,
        }
    }
    fn restore(&mut self, s: Snapshot) {
        self.text = s.text;
        self.cursor = s.cursor;
        self.anchor = s.anchor;
        self.preferred_column = None;
    }
    fn remember(&mut self) {
        self.undo.push(self.snapshot());
        self.redo.clear();
        while self.undo.len() > 100
            || self.undo.iter().map(|s| s.text.len()).sum::<usize>() > 64 * 1024 * 1024
        {
            self.undo.remove(0);
        }
    }
    pub fn undo(&mut self) -> bool {
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(s);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(s);
            true
        } else {
            false
        }
    }
    pub fn insert(&mut self, value: &str) -> bool {
        let value = normalize(value);
        let range = self.selection();
        if value.contains('\0') {
            self.error = Some("Text contains a NUL character".into());
            return false;
        }
        if self.text.len() - range.len() + value.len() > MAX_TEXT_BYTES {
            self.error = Some("Editor text exceeds 16 MiB".into());
            return false;
        }
        if self.text[range.clone()] == value {
            return false;
        }
        self.remember();
        self.text.replace_range(range.clone(), &value);
        self.cursor = range.start + value.len();
        self.anchor = self.cursor;
        self.preferred_column = None;
        true
    }
    fn offset(&self, line: usize, column: usize) -> usize {
        let mut start = 0;
        for (i, s) in self.text.split('\n').enumerate() {
            if i == line {
                return start + s.char_indices().nth(column).map_or(s.len(), |(j, _)| j);
            }
            start += s.len() + 1;
        }
        self.text.len()
    }
    fn previous(&self) -> usize {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }
    fn next(&self) -> usize {
        self.cursor
            + self.text[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8)
    }
    fn move_to(&mut self, target: usize, shift: bool) {
        self.cursor = target;
        if !shift {
            self.anchor = target;
        }
    }
    fn reveal(&mut self, rect: EditorRect) {
        let (line, col) = self.cursor_line_column();
        let (line, col) = (line - 1, col - 1);
        self.scroll_line = self
            .scroll_line
            .min(line)
            .max(line.saturating_sub(rect.rows() - 1));
        self.scroll_column = self
            .scroll_column
            .min(col)
            .max(col.saturating_sub(rect.cols() - 1));
    }
    fn hit(&self, p: Vec2, rect: EditorRect) -> usize {
        let pos = (p - rect.min - Vec2::splat(6.)).max(Vec2::ZERO);
        self.offset(
            self.scroll_line + (pos.y / 10.) as usize,
            self.scroll_column + (pos.x / GLYPH + 0.5) as usize,
        )
    }
    fn key(
        &mut self,
        key: KeyCode,
        ctrl: bool,
        shift: bool,
        clipboard: &mut impl ClipboardAccess,
        rows: usize,
    ) -> bool {
        use KeyCode::*;
        if ctrl {
            match key {
                KeyA => {
                    self.select_all();
                    return false;
                }
                KeyZ => return if shift { self.redo() } else { self.undo() },
                KeyY => return self.redo(),
                KeyC | KeyX => {
                    let range = self.selection();
                    if range.is_empty() {
                        return false;
                    }
                    match clipboard.write_text(&self.text[range]) {
                        Ok(()) => {
                            if key == KeyX {
                                return self.insert("");
                            }
                        }
                        Err(e) => self.error = Some(e),
                    }
                    return false;
                }
                KeyV => {
                    return match clipboard.read_text() {
                        Ok(s) => self.insert(&s),
                        Err(e) => {
                            self.error = Some(e);
                            false
                        }
                    };
                }
                _ => {}
            }
        }
        let range = self.selection();
        match key {
            Backspace | Delete => {
                if range.is_empty() {
                    self.anchor = if key == Backspace {
                        self.previous()
                    } else {
                        self.next()
                    };
                }
                return self.insert("");
            }
            Enter | NumpadEnter if !ctrl => return self.insert("\n"),
            Tab if !ctrl => return self.insert("  "),
            ArrowLeft | ArrowRight => {
                let p = if !shift && !range.is_empty() {
                    if key == ArrowLeft {
                        range.start
                    } else {
                        range.end
                    }
                } else if key == ArrowLeft {
                    self.previous()
                } else {
                    self.next()
                };
                self.move_to(p, shift);
                self.preferred_column = None;
            }
            Home | End => {
                let (line, _) = self.cursor_line_column();
                let p = if ctrl {
                    if key == Home { 0 } else { self.text.len() }
                } else {
                    self.offset(line - 1, if key == Home { 0 } else { usize::MAX })
                };
                self.move_to(p, shift);
                self.preferred_column = None;
            }
            ArrowUp | ArrowDown | PageUp | PageDown => {
                let (line, col) = self.cursor_line_column();
                let column = self.preferred_column.unwrap_or(col - 1);
                let amount = if matches!(key, PageUp | PageDown) {
                    rows
                } else {
                    1
                };
                let target = if matches!(key, ArrowUp | PageUp) {
                    (line - 1).saturating_sub(amount)
                } else {
                    line - 1 + amount
                };
                let maxline = self.text.bytes().filter(|&c| c == b'\n').count();
                self.move_to(self.offset(target.min(maxline), column), shift);
                self.preferred_column = Some(column);
            }
            _ => {}
        }
        false
    }
    /// Caller must route modal input exclusively here while focused (avoid game hotkeys).
    /// Reads the ordered engine text event queue; does not clear other consumers' Input.
    pub fn update(
        &mut self,
        input: &Input,
        rect: EditorRect,
        clipboard: &mut impl ClipboardAccess,
    ) -> EditorResponse {
        let mut changed = false;
        if input.button_pressed(MouseButton::Left) {
            self.focused = rect.contains(input.cursor);
            self.dragging = self.focused;
            if self.focused {
                let shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
                self.move_to(self.hit(input.cursor, rect), shift);
                self.preferred_column = None;
            }
        }
        if !input.button_down(MouseButton::Left) {
            self.dragging = false;
        }
        if self.dragging {
            if input.cursor.y < rect.min.y {
                self.scroll_line = self.scroll_line.saturating_sub(1);
            }
            if input.cursor.y > rect.min.y + rect.size.y - 18. {
                self.scroll_line += 1;
            }
            self.cursor = self.hit(input.cursor, rect);
        }
        if self.focused {
            for event in &input.text_events {
                match event {
                    TextEvent::Text(t) => changed |= self.insert(t),
                    TextEvent::Key {
                        key,
                        control,
                        shift,
                    } => changed |= self.key(*key, *control, *shift, clipboard, rect.rows()),
                }
            }
            if !input.text_events.is_empty() {
                self.reveal(rect);
            }
        }
        if rect.contains(input.cursor) && input.scroll != 0. {
            let shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
            let value = if shift {
                &mut self.scroll_column
            } else {
                &mut self.scroll_line
            };
            *value = (*value as f32 - input.scroll * 3.).max(0.) as usize;
        }
        let maxline = self.text.bytes().filter(|&c| c == b'\n').count();
        self.scroll_line = self.scroll_line.min(maxline);
        let maxcol = self
            .text
            .split('\n')
            .map(|s| s.chars().count())
            .max()
            .unwrap_or(0);
        self.scroll_column = self.scroll_column.min(maxcol);
        EditorResponse {
            changed,
            focused: self.focused,
        }
    }
    pub fn draw(&self, frame: &mut Frame, rect: EditorRect) {
        let fg = Color([0.85, 0.89, 0.94, 1.]);
        let bg = Color([0.035, 0.045, 0.065, 1.]);
        frame.hud_rect(rect.min, rect.size, bg);
        let origin = rect.min + Vec2::splat(6.);
        let rows = rect.rows();
        let cols = rect.cols();
        let selection = self.selection();
        frame.hud_clipped(rect.min, rect.min + rect.size, |frame| {
            let mut start = 0;
            for (line, text) in self.text.split('\n').enumerate() {
                if line >= self.scroll_line + rows {
                    break;
                }
                if line >= self.scroll_line {
                    let y = origin.y + (line - self.scroll_line) as f32 * 10.;
                    for (column, (byte, _)) in text
                        .char_indices()
                        .enumerate()
                        .skip(self.scroll_column)
                        .take(cols)
                    {
                        if selection.contains(&(start + byte)) {
                            frame.hud_rect(
                                Vec2::new(
                                    origin.x + (column - self.scroll_column) as f32 * GLYPH,
                                    y,
                                ),
                                Vec2::new(GLYPH, 10.),
                                Color([0.15, 0.29, 0.46, 1.]),
                            );
                        }
                    }
                    let shown: String = text
                        .chars()
                        .skip(self.scroll_column)
                        .take(cols)
                        .map(|c| if c == '\t' { '·' } else { c })
                        .collect();
                    frame.text(Vec2::new(origin.x, y), &shown, fg);
                }
                start += text.len() + 1;
            }
            let (line, col) = self.cursor_line_column();
            let (line, col) = (line - 1, col - 1);
            if self.focused
                && line >= self.scroll_line
                && line < self.scroll_line + rows
                && col >= self.scroll_column
                && col < self.scroll_column + cols
            {
                frame.hud_rect(
                    origin
                        + Vec2::new(
                            (col - self.scroll_column) as f32 * GLYPH,
                            (line - self.scroll_line) as f32 * 10.,
                        ),
                    Vec2::new(1., 10.),
                    fg,
                );
            }
            let status = self.error.clone().unwrap_or_else(|| {
                let (l, c) = self.cursor_line_column();
                format!("Ln {l}, Col {c} | Ctrl+A/C/V/X  Ctrl+Z/Y | Shift+wheel: horizontal")
            });
            let status: String = status
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(cols)
                .collect();
            frame.text(
                Vec2::new(origin.x, rect.min.y + rect.size.y - 12.),
                &status,
                if self.error.is_some() {
                    Color([1., 0.5, 0.4, 1.])
                } else {
                    fg
                },
            );
        });
    }
}
fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Clip {
        text: String,
        fail: bool,
    }
    impl ClipboardAccess for Clip {
        fn read_text(&mut self) -> Result<String, String> {
            if self.fail {
                Err("read failed".into())
            } else {
                Ok(self.text.clone())
            }
        }
        fn write_text(&mut self, s: &str) -> Result<(), String> {
            if self.fail {
                Err("write failed".into())
            } else {
                self.text = s.into();
                Ok(())
            }
        }
    }
    #[test]
    fn unicode_selection_cut_paste_and_undo_transaction() {
        let mut e = TextEditor::new("aé🌍\nlast".into());
        let mut c = Clip::default();
        e.goto(1, 2);
        e.key(KeyCode::ArrowRight, false, true, &mut c, 10);
        e.key(KeyCode::ArrowRight, false, true, &mut c, 10);
        assert_eq!(&e.text()[e.selection()], "é🌍");
        assert!(e.key(KeyCode::KeyX, true, false, &mut c, 10));
        assert_eq!(e.text(), "a\nlast");
        assert_eq!(c.text, "é🌍");
        assert!(e.undo());
        assert_eq!(e.text(), "aé🌍\nlast");
        assert_eq!(&e.text()[e.selection()], "é🌍");
        assert!(e.redo());
        assert!(e.key(KeyCode::KeyV, true, false, &mut c, 10));
        assert_eq!(e.text(), "aé🌍\nlast");
    }
    #[test]
    fn failed_clipboard_never_deletes_selection_or_changes_document() {
        let mut e = TextEditor::new("keep".into());
        e.select_all();
        let mut c = Clip {
            fail: true,
            ..Default::default()
        };
        assert!(!e.key(KeyCode::KeyX, true, false, &mut c, 10));
        assert_eq!(e.text(), "keep");
        assert_eq!(e.selection(), 0..4);
        assert!(!e.key(KeyCode::KeyV, true, false, &mut c, 10));
        assert_eq!(e.text(), "keep");
        assert!(e.error().is_some());
        assert!(!e.undo());
    }
    #[test]
    fn paste_normalizes_newlines_and_single_undo_restores_selection() {
        let mut e = TextEditor::new("old".into());
        e.select_all();
        assert!(e.insert("new\r\nmultiline\rfinal"));
        assert_eq!(e.text(), "new\nmultiline\nfinal");
        assert_eq!(e.cursor_line_column(), (3, 6));
        assert!(e.undo());
        assert_eq!(e.text(), "old");
        assert_eq!(e.selection(), 0..3);
    }
    #[test]
    fn vertical_navigation_retains_column_and_shift_range() {
        let mut e = TextEditor::new("abcdef\nx\nuvwxyz".into());
        let mut c = Clip::default();
        e.goto(1, 5);
        e.key(KeyCode::ArrowDown, false, true, &mut c, 10);
        assert_eq!(e.cursor_line_column(), (2, 2));
        e.key(KeyCode::ArrowDown, false, true, &mut c, 10);
        assert_eq!(e.cursor_line_column(), (3, 5));
        assert_eq!(&e.text()[e.selection()], "ef\nx\nuvwx");
    }
    #[test]
    fn ordered_events_preserve_repeat_and_shortcut_text_suppression() {
        let mut input = Input::default();
        input.text("a");
        input.key(KeyCode::ArrowLeft, true);
        input.text("b");
        input.key(KeyCode::ArrowLeft, true);
        input.text("c");
        let mut e = TextEditor::default();
        e.focused = true;
        let rect = EditorRect::new(Vec2::ZERO, Vec2::new(400., 100.));
        e.update(&input, rect, &mut Clip::default());
        assert_eq!(e.text(), "cba");
        input.end_frame();
        input.key(KeyCode::ControlLeft, true);
        input.key(KeyCode::KeyA, true);
        input.text("a");
        e.update(&input, rect, &mut Clip::default());
        assert_eq!(e.text(), "cba");
        assert_eq!(e.selection(), 0..3);
    }
    #[test]
    fn mouse_drag_and_scroll_do_not_mutate_text() {
        let mut e = TextEditor::new("abcdef\nsecond\nthird\nfourth\nfifth".into());
        let rect = EditorRect::new(Vec2::ZERO, Vec2::new(100., 44.));
        let mut i = Input::default();
        i.cursor = Vec2::new(14., 7.);
        i.button(MouseButton::Left, true);
        e.update(&i, rect, &mut Clip::default());
        i.end_frame();
        i.cursor = Vec2::new(38., 7.);
        e.update(&i, rect, &mut Clip::default());
        assert_eq!(&e.text()[e.selection()], "bcd");
        i.button(MouseButton::Left, false);
        i.scroll = -1.;
        let before = e.text().to_owned();
        e.update(&i, rect, &mut Clip::default());
        assert_eq!(e.text(), before);
        assert_eq!(e.scroll_line, 3);
    }
    #[test]
    fn invalid_paste_is_atomic_and_edit_after_undo_clears_redo() {
        let mut e = TextEditor::new("safe".into());
        e.select_all();
        assert!(!e.insert("bad\0data"));
        assert_eq!(e.text(), "safe");
        assert!(!e.insert(&"x".repeat(MAX_TEXT_BYTES + 1)));
        assert_eq!(e.text(), "safe");
        assert!(e.insert("ok"));
        e.undo();
        assert!(e.insert("new"));
        assert!(!e.redo());
    }
}
