//! A side panel of HUD widgets, laid out top to bottom (a shared part: the planet LOD app's panel
//! first). Drawn immediate-mode: each frame the app lays the panel out in `draw`; the widgets that
//! take the mouse leave their boxes (`Hit`), which the app's next `update` reads against the cursor.
//! The engine's font is ASCII on an 8 px grid; `big` text is drawn at twice the size.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Frame, GLYPH};

pub const INK: Color = Color::hex(0xe8ebf0);
pub const INK2: Color = Color::hex(0xb4bac4);
pub const INK3: Color = Color::hex(0x7d8592);
pub const LINE: Color = Color::hex(0x2c313a);
pub const ACCENT: Color = Color::hex(0xff6a00);
pub const GOOD: Color = Color::hex(0x5fd38a);
pub const WARN: Color = Color::hex(0xffb040);
pub const BAD: Color = Color::hex(0xff5a5a);
pub const BG: Color = Color([0.07, 0.08, 0.1, 0.9]);


/// A box the mouse can act on: its corner, size and what it is (the app's own number).
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub min: Vec2,
    pub size: Vec2,
    pub id: u32,
}

impl Hit {
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.y >= self.min.y && p.x < self.min.x + self.size.x && p.y < self.min.y + self.size.y
    }
}

enum Cmd {
    Text(Vec2, String, Color, f32),
    Rect(Vec2, Vec2, Color),
    Frame(Vec2, Vec2, Color),
    Line(Vec2, Vec2, Color),
}

/// The panel being laid out: commands kept until `finish`, which draws its background first.
pub struct Panel {
    /// Text size (1: the engine's 8 px grid) and the glyph's width at it.
    k: f32,
    g: f32,
    x: f32,
    y: f32,
    top: f32,
    w: f32,
    cmds: Vec<Cmd>,
    pub hits: Vec<Hit>,
}

impl Panel {
    /// A panel at (x, y), `w` wide, its text at `k` times the engine's size.
    pub fn new(x: f32, y: f32, w: f32, k: f32) -> Self {
        Panel { k, g: GLYPH * k, x, y: y + 10.0 * k, top: y, w, cmds: vec![], hits: vec![] }
    }

    fn row_h(&self) -> f32 {
        self.g + 7.0 * self.k
    }

    fn text_at(&mut self, p: Vec2, s: &str, c: Color, scale: f32) {
        self.cmds.push(Cmd::Text(p, s.to_string(), c, scale));
    }

    fn right(&mut self, y: f32, parts: &[(&str, Color)]) {
        let n: usize = parts.iter().map(|(s, _)| s.chars().count()).sum();
        let mut x = self.x + self.w - 12.0 - n as f32 * self.g;
        for (s, c) in parts {
            self.text_at(Vec2::new(x, y), s, *c, self.k);
            x += s.chars().count() as f32 * self.g;
        }
    }

    /// A badge: text in a coloured frame; returns its width.
    fn badge_at(&mut self, p: Vec2, s: &str, c: Color) -> f32 {
        let w = s.chars().count() as f32 * self.g + 10.0;
        self.cmds.push(Cmd::Frame(p - Vec2::new(0.0, 3.0), Vec2::new(w, self.g + 6.0), c));
        self.text_at(p + Vec2::new(5.0, 0.0), s, c, self.k);
        w
    }

    /// The title, a badge at the right, and a button (id) at the far right.
    pub fn title(&mut self, s: &str, badge: (&str, Color), button: (&str, u32)) {
        let y = self.y;
        self.text_at(Vec2::new(self.x + 12.0, y), s, INK, self.k);
        let bw = button.0.chars().count() as f32 * self.g + 10.0;
        let bx = self.x + self.w - 12.0 - bw;
        self.cmds.push(Cmd::Frame(Vec2::new(bx, y - 3.0), Vec2::new(bw, self.g + 6.0), LINE));
        self.text_at(Vec2::new(bx + 5.0, y), button.0, INK2, self.k);
        self.hits.push(Hit { min: Vec2::new(bx, y - 3.0), size: Vec2::new(bw, self.g + 6.0), id: button.1 });
        let w = badge.0.chars().count() as f32 * self.g + 10.0;
        self.badge_at(Vec2::new(bx - 8.0 - w, y), badge.0, badge.1);
        self.y += self.row_h() + 2.0;
        self.rule();
    }

    pub fn rule(&mut self) {
        let y = self.y - 4.0;
        self.cmds.push(Cmd::Line(Vec2::new(self.x + 1.0, y), Vec2::new(self.x + self.w - 1.0, y), LINE));
        self.y += 4.0;
    }

    /// A big value with a small note after it.
    pub fn big(&mut self, value: &str, note: &str) {
        let y = self.y + 2.0;
        self.text_at(Vec2::new(self.x + 12.0, y), value, INK, 2.0 * self.k);
        self.text_at(Vec2::new(self.x + 20.0 + value.chars().count() as f32 * self.g * 2.0, y + self.g), note, INK2, self.k);
        self.y += 2.0 * self.g + 12.0;
    }

    /// A section header with badges after it.
    pub fn section(&mut self, s: &str, badges: &[(&str, Color)]) {
        self.y += 6.0;
        self.rule();
        let y = self.y;
        self.text_at(Vec2::new(self.x + 12.0, y), s, INK3, self.k);
        let mut x = self.x + 12.0 + (s.chars().count() + 2) as f32 * self.g;
        for (b, c) in badges {
            x += self.badge_at(Vec2::new(x, y), b, *c) + 6.0;
        }
        self.y += self.row_h() + 2.0;
    }

    /// A property: its name at the left, its value (and a dim note) at the right.
    pub fn row(&mut self, label: &str, value: &str, note: &str) {
        let y = self.y;
        self.text_at(Vec2::new(self.x + 12.0, y), label, INK2, self.k);
        if note.is_empty() {
            self.right(y, &[(value, INK)]);
        } else {
            self.right(y, &[(value, INK), (" ", INK3), (note, INK3)]);
        }
        self.y += self.row_h();
    }

    /// A row whose value is a button (id).
    pub fn row_button(&mut self, label: &str, button: &str, id: u32) {
        let y = self.y;
        self.text_at(Vec2::new(self.x + 12.0, y), label, INK2, self.k);
        let bw = button.chars().count() as f32 * self.g + 12.0;
        let bx = self.x + self.w - 12.0 - bw;
        self.cmds.push(Cmd::Frame(Vec2::new(bx, y - 3.0), Vec2::new(bw, self.g + 6.0), INK3));
        self.text_at(Vec2::new(bx + 6.0, y), button, INK, self.k);
        self.hits.push(Hit { min: Vec2::new(bx, y - 3.0), size: Vec2::new(bw, self.g + 6.0), id });
        self.y += self.row_h();
    }

    /// A bar: the first share in the accent, the second (part of it) in white.
    pub fn bar(&mut self, a: f32, b: f32) {
        let (x, y, w) = (self.x + 12.0, self.y, self.w - 24.0);
        self.cmds.push(Cmd::Rect(Vec2::new(x, y), Vec2::new(w, 5.0), LINE));
        self.cmds.push(Cmd::Rect(Vec2::new(x, y), Vec2::new(w * a.clamp(0.0, 1.0), 5.0), ACCENT));
        self.cmds.push(Cmd::Rect(Vec2::new(x, y + 1.5), Vec2::new(w * b.clamp(0.0, 1.0), 2.0), INK));
        self.y += 12.0;
    }

    /// Buttons side by side, the chosen one filled (ids from `id0`).
    pub fn seg(&mut self, options: &[&str], chosen: usize, id0: u32) {
        let (x, y, w) = (self.x + 12.0, self.y, self.w - 24.0);
        let bw = w / options.len() as f32;
        for (k, o) in options.iter().enumerate() {
            let p = Vec2::new(x + k as f32 * bw, y - 4.0);
            let size = Vec2::new(bw, self.g + 10.0);
            if k == chosen {
                self.cmds.push(Cmd::Rect(p, size, ACCENT));
            }
            self.cmds.push(Cmd::Frame(p, size, LINE));
            let tx = p.x + (bw - o.chars().count() as f32 * self.g) * 0.5;
            self.text_at(Vec2::new(tx, y + 1.0), o, if k == chosen { Color::hex(0x111111) } else { INK2 }, self.k);
            self.hits.push(Hit { min: p, size, id: id0 + k as u32 });
        }
        self.y += self.row_h() + 6.0;
    }

    /// Check boxes in a row: (label, on, enabled, id).
    pub fn chips(&mut self, items: &[(&str, bool, bool, u32)]) {
        let mut x = self.x + 12.0;
        let y = self.y;
        for &(label, on, enabled, id) in items {
            let w = (label.chars().count() + 3) as f32 * self.g + 10.0;
            let p = Vec2::new(x, y - 4.0);
            let size = Vec2::new(w, self.g + 9.0);
            let c = if !enabled { LINE } else if on { INK } else { INK2 };
            self.cmds.push(Cmd::Frame(p, size, if on && enabled { INK3 } else { LINE }));
            self.cmds.push(Cmd::Frame(Vec2::new(x + 5.0, y), Vec2::new(self.g, self.g), c));
            if on {
                self.cmds.push(Cmd::Rect(Vec2::new(x + 5.0 + 0.25 * self.g, y + 0.25 * self.g), Vec2::splat(0.5 * self.g), if enabled { ACCENT } else { LINE }));
            }
            self.text_at(Vec2::new(x + 5.0 + 2.0 * self.g, y), label, c, self.k);
            if enabled {
                self.hits.push(Hit { min: p, size, id });
            }
            x += w + 6.0;
        }
        self.y += self.row_h() + 4.0;
    }

    /// A slider (id): its label, the track, the value shown at the right.
    pub fn slider(&mut self, label: &str, value: f32, min: f32, max: f32, shown: &str, id: u32) {
        let y = self.y;
        self.text_at(Vec2::new(self.x + 12.0, y), label, INK2, self.k);
        let x0 = self.x + 12.0 + (label.chars().count() + 1) as f32 * self.g;
        let x1 = self.x + self.w - 16.0 - 4.0 * self.g;
        let k = ((value - min) / (max - min)).clamp(0.0, self.k);
        let my = y + self.g * 0.5;
        self.cmds.push(Cmd::Rect(Vec2::new(x0, my - 1.5), Vec2::new(x1 - x0, 3.0), LINE));
        self.cmds.push(Cmd::Rect(Vec2::new(x0, my - 1.5), Vec2::new((x1 - x0) * k, 3.0), ACCENT));
        self.cmds.push(Cmd::Rect(Vec2::new(x0 + (x1 - x0) * k - 4.0, my - 5.0), Vec2::new(8.0, 10.0), ACCENT));
        self.right(y, &[(shown, INK)]);
        self.hits.push(Hit { min: Vec2::new(x0 - 4.0, y - 4.0), size: Vec2::new(x1 - x0 + 8.0, self.g + 8.0), id });
        self.y += self.row_h() + 2.0;
    }

    /// Plain lines of text (keys, notes).
    pub fn note(&mut self, s: &str, c: Color) {
        let y = self.y;
        self.text_at(Vec2::new(self.x + 12.0, y), s, c, self.k);
        self.y += self.row_h() - 2.0;
    }

    /// A box of `h` px for a chart: its corner and size (draw into it with `line`, `rect`, `text`).
    pub fn area(&mut self, h: f32) -> (Vec2, Vec2) {
        let p = Vec2::new(self.x + 12.0, self.y);
        let s = Vec2::new(self.w - 24.0, h);
        self.cmds.push(Cmd::Rect(p, s, Color([0.055, 0.063, 0.078, 1.0])));
        self.y += h + 8.0;
        (p, s)
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, c: Color) {
        self.cmds.push(Cmd::Line(a, b, c));
    }

    pub fn rect(&mut self, p: Vec2, s: Vec2, c: Color) {
        self.cmds.push(Cmd::Rect(p, s, c));
    }

    pub fn text(&mut self, p: Vec2, s: &str, c: Color) {
        self.text_at(p, s, c, self.k);
    }

    /// Draw it: the background, then everything in the order laid out. Returns the panel's box.
    pub fn finish(self, frame: &mut Frame) -> (Hit, Vec<Hit>) {
        let size = Vec2::new(self.w, self.y - self.top + 4.0);
        frame.hud_rect(Vec2::new(self.x, self.top), size, BG);
        frame.hud_box(Vec2::new(self.x, self.top), size, LINE);
        for c in self.cmds {
            match c {
                Cmd::Text(p, s, c, k) => {
                    frame.text_scaled(p, &s, c, k);
                }
                Cmd::Rect(p, s, c) => frame.hud_rect(p, s, c),
                Cmd::Frame(p, s, c) => frame.hud_box(p, s, c),
                Cmd::Line(a, b, c) => frame.hud_line(a, b, c),
            }
        }
        (Hit { min: Vec2::new(self.x, self.top), size, id: 0 }, self.hits)
    }
}
