//! A side panel of HUD widgets, laid out top to bottom (a shared part: the planet LOD app's panel
//! first). Drawn immediate-mode: each frame the app lays the panel out in `draw`; the widgets that
//! take the mouse leave their boxes (`Hit`), which the app's next `update` reads against the cursor.
//! Slim as the browser panels it stands for: values in the text size `k`, labels a little dimmer,
//! notes (the browser's `<small>`) smaller and dim; the font is monospaced on the engine's 8 px grid
//! (`·`, `×`, `°` and `–` are in it).

use universe_engine::glam::Vec2;
use universe_engine::{Color, Frame, GLYPH};

pub const INK: Color = Color::hex(0xe8ebf0);
pub const INK2: Color = Color::hex(0xa9b0bb);
pub const INK3: Color = Color::hex(0x6c7380);
pub const LINE: Color = Color::hex(0x2c313a);
pub const ACCENT: Color = Color::hex(0xff6a00);
pub const GOOD: Color = Color::hex(0x5fd38a);
pub const WARN: Color = Color::hex(0xffb040);
pub const BAD: Color = Color::hex(0xff5a5a);
pub const BG: Color = Color([0.075, 0.082, 0.1, 0.88]);

/// Notes and section headers this share of the text size.
const SMALL: f32 = 0.86;
/// Inner margin (HUD px).
const PAD: f32 = 7.0;

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
    /// Text size (1: the engine's 8 px grid).
    k: f32,
    x: f32,
    y: f32,
    top: f32,
    w: f32,
    cmds: Vec<Cmd>,
    pub hits: Vec<Hit>,
}

/// How wide `s` is at text size `k`.
pub fn width(s: &str, k: f32) -> f32 {
    s.chars().count() as f32 * GLYPH * k
}

impl Panel {
    /// A panel at (x, y), `w` wide, its text at `k` times the engine's size.
    pub fn new(x: f32, y: f32, w: f32, k: f32) -> Self {
        Panel { k, x, y: y + PAD, top: y, w, cmds: vec![], hits: vec![] }
    }

    fn line_h(&self) -> f32 {
        (GLYPH + 5.5) * self.k
    }

    fn put(&mut self, p: Vec2, s: &str, c: Color, k: f32) {
        self.cmds.push(Cmd::Text(p, s.to_string(), c, k));
    }

    /// Parts set flush right on the line at `y`: (text, colour, small).
    fn right(&mut self, y: f32, parts: &[(&str, Color, bool)]) {
        let (k0, k1) = (self.k, self.k * SMALL);
        let ks = |small: bool| if small { k1 } else { k0 };
        let total: f32 = parts.iter().map(|(s, _, sm)| width(s, ks(*sm))).sum();
        let mut x = self.x + self.w - PAD - total;
        for (s, c, sm) in parts {
            let k = ks(*sm);
            // (Small text sits on the same baseline.)
            let dy = (k0 - k) * GLYPH * 0.85;
            self.put(Vec2::new(x, y + dy), s, *c, k);
            x += width(s, k);
        }
    }

    /// A badge: text in a coloured frame at `p`; returns its width.
    fn badge_at(&mut self, p: Vec2, s: &str, c: Color) -> f32 {
        let k = self.k * SMALL;
        let w = width(s, k) + 6.0;
        self.cmds.push(Cmd::Frame(p - Vec2::new(0.0, 2.0), Vec2::new(w, GLYPH * k + 4.0), c));
        self.put(p + Vec2::new(3.0, 0.0), s, c, k);
        w
    }

    /// The title, a badge before the far right, and a button (id) at the far right.
    pub fn title(&mut self, s: &str, badge: (&str, Color), button: (&str, u32)) {
        let y = self.y;
        self.put(Vec2::new(self.x + PAD, y), s, INK, self.k);
        let k = self.k * SMALL;
        let bw = width(button.0, k) + 8.0;
        let bx = self.x + self.w - PAD - bw;
        let size = Vec2::new(bw, GLYPH * k + 4.0);
        self.cmds.push(Cmd::Frame(Vec2::new(bx, y - 2.0), size, LINE));
        self.put(Vec2::new(bx + 4.0, y), button.0, INK2, k);
        self.hits.push(Hit { min: Vec2::new(bx, y - 2.0), size, id: button.1 });
        let w = width(badge.0, k) + 6.0;
        self.badge_at(Vec2::new(bx - 5.0 - w, y), badge.0, badge.1);
        self.y += self.line_h() + 3.0;
        self.rule();
    }

    pub fn rule(&mut self) {
        let y = self.y - 3.0;
        self.cmds.push(Cmd::Line(Vec2::new(self.x + 1.0, y), Vec2::new(self.x + self.w - 1.0, y), LINE));
        self.y += 3.0;
    }

    /// A big value with a note after it.
    pub fn big(&mut self, value: &str, note: &str) {
        let y = self.y + 1.0;
        let kb = 1.7 * self.k;
        self.put(Vec2::new(self.x + PAD, y), value, INK, kb);
        self.put(Vec2::new(self.x + PAD + width(value, kb) + 5.0, y + GLYPH * (kb - self.k * SMALL) * 0.85), note, INK2, self.k * SMALL);
        self.y += GLYPH * kb + 6.0;
    }

    /// A section header (small, dim) with badges after it.
    pub fn section(&mut self, s: &str, badges: &[(&str, Color)]) {
        self.y += 3.0;
        self.rule();
        let y = self.y;
        let k = self.k * SMALL;
        self.put(Vec2::new(self.x + PAD, y), s, INK3, k);
        let mut x = self.x + PAD + width(s, k) + 7.0;
        for (b, c) in badges {
            x += self.badge_at(Vec2::new(x, y), b, *c) + 4.0;
        }
        self.y += self.line_h() + 1.0;
    }

    /// A property: its name at the left, its value and a small dim note at the right.
    pub fn row(&mut self, label: &str, value: &str, note: &str) {
        let y = self.y;
        self.put(Vec2::new(self.x + PAD, y), label, INK2, self.k);
        if note.is_empty() {
            self.right(y, &[(value, INK, false)]);
        } else {
            self.right(y, &[(value, INK, false), (" ", INK3, true), (note, INK3, true)]);
        }
        self.y += self.line_h();
    }

    /// A row whose value is a button (id).
    pub fn row_button(&mut self, label: &str, button: &str, id: u32) {
        let y = self.y;
        self.put(Vec2::new(self.x + PAD, y), label, INK2, self.k);
        let k = self.k * SMALL;
        let bw = width(button, k) + 8.0;
        let bx = self.x + self.w - PAD - bw;
        let size = Vec2::new(bw, GLYPH * k + 4.0);
        self.cmds.push(Cmd::Frame(Vec2::new(bx, y - 1.0), size, INK3));
        self.put(Vec2::new(bx + 4.0, y + 1.0), button, INK, k);
        self.hits.push(Hit { min: Vec2::new(bx, y - 1.0), size, id });
        self.y += self.line_h();
    }

    /// A bar: the first share in the accent, the second (part of it) in white.
    pub fn bar(&mut self, a: f32, b: f32) {
        let (x, y, w) = (self.x + PAD, self.y + 0.5, self.w - 2.0 * PAD);
        self.cmds.push(Cmd::Rect(Vec2::new(x, y), Vec2::new(w, 3.0), LINE));
        self.cmds.push(Cmd::Rect(Vec2::new(x, y), Vec2::new(w * a.clamp(0.0, 1.0), 3.0), ACCENT));
        self.cmds.push(Cmd::Rect(Vec2::new(x, y + 1.0), Vec2::new(w * b.clamp(0.0, 1.0), 1.0), INK));
        self.y += 8.0;
    }

    /// Buttons side by side, the chosen one filled (ids from `id0`).
    pub fn seg(&mut self, options: &[&str], chosen: usize, id0: u32) {
        let (x, y, w) = (self.x + PAD, self.y, self.w - 2.0 * PAD);
        let bw = w / options.len() as f32;
        let h = GLYPH * self.k + 6.0;
        for (k, o) in options.iter().enumerate() {
            let p = Vec2::new(x + k as f32 * bw, y - 3.0);
            let size = Vec2::new(bw, h);
            if k == chosen {
                self.cmds.push(Cmd::Rect(p, size, ACCENT));
            }
            self.cmds.push(Cmd::Frame(p, size, LINE));
            let tx = p.x + (bw - width(o, self.k)) * 0.5;
            self.put(Vec2::new(tx, y), o, if k == chosen { Color::hex(0x111111) } else { INK2 }, self.k);
            self.hits.push(Hit { min: p, size, id: id0 + k as u32 });
        }
        self.y += h + 3.0;
    }

    /// Check boxes in a row, as chips: (label, on, enabled, id).
    pub fn chips(&mut self, items: &[(&str, bool, bool, u32)]) {
        let tinted: Vec<_> = items.iter().map(|&(label,on,enabled,id)| (label,on,enabled,id,ACCENT)).collect();
        self.colored_chips(&tinted);
    }

    /// Chips whose checked markers also serve as a colour legend.
    pub fn colored_chips(&mut self, items: &[(&str, bool, bool, u32, Color)]) {
        let k = self.k;
        let mut x = self.x + PAD;
        let y = self.y;
        let b = GLYPH * k * 0.8;
        for &(label, on, enabled, id, tint) in items {
            let w = b + 4.0 + width(label, k) + 9.0;
            let p = Vec2::new(x, y - 3.0);
            let size = Vec2::new(w, GLYPH * k + 6.0);
            let c = if !enabled { LINE } else if on { INK } else { INK2 };
            self.cmds.push(Cmd::Frame(p, size, if on && enabled { INK3 } else { LINE }));
            let bp = Vec2::new(x + 4.0, y + (GLYPH * k - b) * 0.5);
            self.cmds.push(Cmd::Frame(bp, Vec2::splat(b), c));
            if on {
                self.cmds.push(Cmd::Rect(bp + Vec2::splat(b * 0.22), Vec2::splat(b * 0.56), if enabled { tint } else { LINE }));
            }
            self.put(Vec2::new(bp.x + b + 4.0, y), label, c, k);
            if enabled {
                self.hits.push(Hit { min: p, size, id });
            }
            x += w + 4.0;
        }
        self.y += GLYPH * k + 9.0;
    }

    /// A slider (id): its label, the track, the value shown at the right.
    pub fn slider(&mut self, label: &str, value: f32, min: f32, max: f32, shown: &str, id: u32) {
        let y = self.y;
        self.put(Vec2::new(self.x + PAD, y), label, INK2, self.k);
        let x0 = self.x + PAD + width(label, self.k) + 6.0;
        let x1 = self.x + self.w - PAD - width("50", self.k) - 6.0;
        let t = ((value - min) / (max - min)).clamp(0.0, 1.0);
        let my = y + GLYPH * self.k * 0.5;
        self.cmds.push(Cmd::Rect(Vec2::new(x0, my - 1.0), Vec2::new(x1 - x0, 2.0), LINE));
        self.cmds.push(Cmd::Rect(Vec2::new(x0, my - 1.0), Vec2::new((x1 - x0) * t, 2.0), ACCENT));
        self.cmds.push(Cmd::Rect(Vec2::new(x0 + (x1 - x0) * t - 2.5, my - 4.0), Vec2::new(5.0, 8.0), ACCENT));
        self.right(y, &[(shown, INK, false)]);
        self.hits.push(Hit { min: Vec2::new(x0 - 4.0, y - 3.0), size: Vec2::new(x1 - x0 + 8.0, GLYPH * self.k + 6.0), id });
        self.y += self.line_h() + 1.0;
    }

    /// A line of small text (keys, notes, a legend).
    pub fn note(&mut self, s: &str, c: Color) {
        let y = self.y;
        self.put(Vec2::new(self.x + PAD, y), s, c, self.k * SMALL);
        self.y += (GLYPH + 4.0) * self.k * SMALL;
    }

    /// A box of `h` px for a chart: its corner and size (draw into it with `line`, `rect`, `text`).
    pub fn area(&mut self, h: f32) -> (Vec2, Vec2) {
        let p = Vec2::new(self.x + PAD, self.y);
        let s = Vec2::new(self.w - 2.0 * PAD, h);
        self.cmds.push(Cmd::Rect(p, s, Color([0.055, 0.063, 0.078, 1.0])));
        self.y += h + 4.0;
        (p, s)
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, c: Color) {
        self.cmds.push(Cmd::Line(a, b, c));
    }

    pub fn rect(&mut self, p: Vec2, s: Vec2, c: Color) {
        self.cmds.push(Cmd::Rect(p, s, c));
    }

    /// Small text at `p` (chart labels).
    pub fn text(&mut self, p: Vec2, s: &str, c: Color) {
        let k = self.k * SMALL * 0.9;
        self.put(p, s, c, k);
    }

    /// Draw it: the background, then everything in the order laid out. Returns the panel's box.
    pub fn finish(self, frame: &mut Frame) -> (Hit, Vec<Hit>) {
        let size = Vec2::new(self.w, self.y - self.top + 2.0);
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
