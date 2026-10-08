//! The standards registry, browsed (docked: the station's copy): a tree, its
//! roots the standards bodies, then their branches and the standards in them.
//! UP/DOWN moves, RIGHT/ENTER opens, LEFT closes or goes up; what's under the
//! cursor shown in full at the side (see `docs/standards.md`).

use std::collections::HashSet;

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame, KeyCode, GLYPH};
use universe_sim::world::content::content;
use universe_sim::world::standards::{Body, Check, Licence, Standard, Status, Value};

use crate::App;
use crate::palette::{AMBER, DIM, TEXT};

const HEAD: Color = Color::hex(0x60ffb0);

/// A place in the tree.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Node {
    Body(String),
    /// (body key, branch path).
    Branch(String, String),
    Standard(String),
}

/// The browser's state: what's open, where the cursor is.
#[derive(Default)]
pub struct StandardsView {
    open: HashSet<Node>,
    cursor: usize,
}

fn bodies() -> Vec<&'static Body> {
    content().bodies.iter().map(|(_, b)| b).collect()
}

fn standards_in(body: &str, branch: &str) -> Vec<&'static Standard> {
    content().standards.iter().map(|(_, s)| s).filter(|s| s.body == body && s.branch == branch).collect()
}

/// A branch's direct sub-branches.
fn sub_branches<'a>(body: &'a Body, path: Option<&str>) -> impl Iterator<Item = &'a (String, String)> {
    body.branches.iter().filter(move |(p, _)| universe_sim::world::standards::parent(p) == path)
}

/// Everything under a branch, counted (its standards and its sub-branches').
fn count(body: &Body, path: &str) -> usize {
    standards_in(&body.key, path).len() + sub_branches(body, Some(path)).map(|(p, _)| count(body, p)).sum::<usize>()
}

impl StandardsView {
    pub fn new() -> Self {
        // (The bodies open at first: their top branches in view.)
        StandardsView { open: bodies().iter().map(|b| Node::Body(b.key.clone())).collect(), cursor: 0 }
    }

    /// The cursor on standard `key`, the branches down to it open.
    pub fn focus(&mut self, key: &str) {
        let Some(s) = content().standards.iter().map(|(_, s)| s).find(|s| s.key == key) else { return };
        self.open.insert(Node::Body(s.body.clone()));
        let mut path = Some(s.branch.as_str());
        while let Some(p) = path {
            self.open.insert(Node::Branch(s.body.clone(), p.to_string()));
            path = universe_sim::world::standards::parent(p);
        }
        if let Some(i) = self.rows().iter().position(|(_, n)| *n == Node::Standard(key.to_string())) {
            self.cursor = i;
        }
    }

    /// The tree as shown: (depth, node), open nodes' children under them.
    fn rows(&self) -> Vec<(usize, Node)> {
        let mut rows = Vec::new();
        for b in bodies() {
            let node = Node::Body(b.key.clone());
            let open = self.open.contains(&node);
            rows.push((0, node));
            if open {
                self.branch_rows(b, None, 1, &mut rows);
            }
        }
        rows
    }

    fn branch_rows(&self, body: &Body, path: Option<&str>, depth: usize, rows: &mut Vec<(usize, Node)>) {
        for (p, _) in sub_branches(body, path) {
            let node = Node::Branch(body.key.clone(), p.clone());
            let open = self.open.contains(&node);
            rows.push((depth, node));
            if open {
                self.branch_rows(body, Some(p), depth + 1, rows);
                for s in standards_in(&body.key, p) {
                    rows.push((depth + 1, Node::Standard(s.key.clone())));
                }
            }
        }
    }
}

/// Keys while open. False when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let Some(view) = app.standards.as_mut() else { return false };
    let rows = view.rows();
    let input = &ctx.input;
    if input.pressed(KeyCode::ArrowDown) {
        view.cursor = (view.cursor + 1).min(rows.len().saturating_sub(1));
    }
    if input.pressed(KeyCode::ArrowUp) {
        view.cursor = view.cursor.saturating_sub(1);
    }
    if input.pressed(KeyCode::PageDown) {
        view.cursor = (view.cursor + 10).min(rows.len().saturating_sub(1));
    }
    if input.pressed(KeyCode::PageUp) {
        view.cursor = view.cursor.saturating_sub(10);
    }
    if let Some((depth, node)) = rows.get(view.cursor).cloned() {
        let opens = !matches!(node, Node::Standard(_));
        if (input.pressed(KeyCode::ArrowRight) || input.pressed(KeyCode::Enter)) && opens {
            view.open.insert(node.clone());
        }
        if input.pressed(KeyCode::ArrowLeft) {
            if opens && view.open.contains(&node) {
                view.open.remove(&node);
            } else if depth > 0 {
                // Up to what it's in.
                if let Some(up) = rows[..view.cursor].iter().rposition(|(d, _)| *d < depth) {
                    view.cursor = up;
                }
            }
        }
    }
    !(crate::keys::pressed(input, crate::keys::Act::Standards) || input.pressed(KeyCode::Escape))
}

/// What a row says.
fn label(node: &Node) -> (String, Color) {
    let c = content();
    match node {
        Node::Body(k) => {
            let b = c.bodies.iter().map(|(_, b)| b).find(|b| &b.key == k).expect("a body");
            (format!("{} ({})", b.name, b.prefix), HEAD)
        }
        Node::Branch(k, p) => {
            let b = c.bodies.iter().map(|(_, b)| b).find(|b| &b.key == k).expect("a body");
            let n = count(b, p);
            (format!("{p}  {}  ({n})", b.branch(p).unwrap_or("?")), if n > 0 { TEXT } else { DIM })
        }
        Node::Standard(k) => {
            let s = c.standards.iter().map(|(_, s)| s).find(|s| &s.key == k).expect("a standard");
            (format!("{}  {}", s.cite, s.title), AMBER)
        }
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let Some(view) = app.standards.as_ref() else { return };
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, crate::palette::panel(1.0));
    let line = GLYPH + 4.0;
    let (x, top) = (16.0, 16.0);
    frame.text(
        Vec2::new(x, top),
        &format!("STANDARDS REGISTRY - THIS STATION'S COPY   UP/DOWN MOVE  RIGHT OPEN  LEFT CLOSE   {} CLOSES", crate::keys::key(crate::keys::Act::Standards)),
        TEXT,
    );
    // The tree, left; a window of it round the cursor.
    let rows = view.rows();
    let tree_w = size.x * 0.42;
    let fit = ((size.y - top - line * 3.0) / line) as usize;
    let first = view.cursor.saturating_sub(fit / 2).min(rows.len().saturating_sub(fit));
    let mut y = top + line * 2.0;
    for (i, (depth, node)) in rows.iter().enumerate().skip(first).take(fit) {
        let (text, colour) = label(node);
        let mark = match node {
            Node::Standard(_) => "  ",
            n if view.open.contains(n) => "- ",
            _ => "+ ",
        };
        let at = Vec2::new(x + *depth as f32 * GLYPH * 2.0, y);
        if i == view.cursor {
            frame.hud_rect(Vec2::new(x - 4.0, y - 2.0), Vec2::new(tree_w, line), Color([0.1, 0.25, 0.2, 1.0]));
        }
        let room = ((tree_w - (at.x - x)) / GLYPH) as usize;
        let shown: String = format!("{mark}{text}").chars().take(room.saturating_sub(1)).collect();
        frame.text(at, &shown, colour);
        y += line;
    }
    // What's under the cursor, right.
    let dx = x + tree_w + 16.0;
    let width = ((size.x - dx - 16.0) / GLYPH) as usize;
    let mut lines: Vec<(String, Color)> = Vec::new();
    match rows.get(view.cursor).map(|r| &r.1) {
        Some(Node::Body(k)) => {
            let b = bodies().into_iter().find(|b| &b.key == k).expect("a body");
            lines.push((b.name.clone(), HEAD));
            lines.push((format!("PREFIX {}   SEAT {}", b.prefix, if b.seat == "home" { "THE HOME STATION".to_string() } else { b.seat.to_uppercase() }), DIM));
            lines.push((b.note.clone(), TEXT));
            let n = content().standards.iter().filter(|(_, s)| s.body == b.key).count();
            lines.push((format!("{} BRANCHES, {n} STANDARDS", b.branches.len()), TEXT));
        }
        Some(Node::Branch(k, p)) => {
            let b = bodies().into_iter().find(|b| &b.key == k).expect("a body");
            lines.push((format!("{} {p}  {}", b.prefix, b.branch(p).unwrap_or("?")), HEAD));
            let subs: Vec<_> = sub_branches(b, Some(p)).collect();
            let here = standards_in(k, p);
            if subs.is_empty() && here.is_empty() {
                lines.push(("NOTHING PUBLISHED HERE YET".into(), DIM));
            }
            for (sp, st) in subs {
                lines.push((format!("{sp}  {st}  ({})", count(b, sp)), TEXT));
            }
            for s in here {
                lines.push((format!("{}  {}", s.id(), s.title), AMBER));
            }
        }
        Some(Node::Standard(k)) => {
            let s = content().standards.iter().map(|(_, s)| s).find(|s| &s.key == k).expect("a standard");
            standard_lines(s, width, &mut lines);
        }
        None => {}
    }
    let mut y = top + line * 2.0;
    for (text, colour) in lines {
        if y > size.y - line {
            frame.text(Vec2::new(dx, y), "...", DIM);
            break;
        }
        frame.text(Vec2::new(dx, y), &text, colour);
        y += line;
    }
}

fn value(v: &Value) -> String {
    let num = |n: f64| if n == n.trunc() && n.abs() < 1e7 { format!("{n:.0}") } else { format!("{n}") };
    match v {
        Value::Num(n) => num(*n),
        Value::Range(a, b) => format!("{} - {}", num(*a), num(*b)),
        Value::Text(t) => t.clone(),
    }
}

fn standard_lines(s: &Standard, width: usize, out: &mut Vec<(String, Color)>) {
    let c = content();
    let body = c.bodies.iter().map(|(_, b)| b).find(|b| b.key == s.body);
    out.push((format!("{}  {}", s.id(), s.title), HEAD));
    let status = match s.status {
        Status::Draft => "DRAFT",
        Status::Published => "PUBLISHED",
        Status::Superseded => "SUPERSEDED",
        Status::Withdrawn => "WITHDRAWN",
    };
    let licence = match s.licence {
        Licence::Open => "OPEN".to_string(),
        Licence::Fee(f) => format!("{f:.0} CR A PRODUCT"),
    };
    out.push((format!("{status}   LICENCE {licence}   BY {}", body.map_or("?", |b| b.name.as_str())), DIM));
    out.push((String::new(), TEXT));
    for l in crate::fmt::wrap(&s.scope, width) {
        out.push((l, TEXT));
    }
    if !s.refs.is_empty() {
        out.push((String::new(), TEXT));
        out.push(("BUILDS ON".into(), DIM));
        for r in &s.refs {
            let x = c.standards.iter().map(|(_, s)| s).find(|x| &x.key == r);
            out.push((format!("  {}  {}", x.map_or(r.as_str(), |x| x.cite.as_str()), x.map_or("?", |x| x.title.as_str())), AMBER));
        }
    }
    // Parameters: the plain ones as a list; ROW.column ones as a table.
    let plain: Vec<_> = s.params.iter().filter(|p| !p.key.contains('.')).collect();
    let mut rows: Vec<&str> = Vec::new();
    let mut cols: Vec<&str> = Vec::new();
    for p in s.params.iter().filter(|p| p.key.contains('.')) {
        let (r, col) = p.key.split_once('.').expect("dotted");
        if !rows.contains(&r) {
            rows.push(r);
        }
        if !cols.contains(&col) {
            cols.push(col);
        }
    }
    if !rows.is_empty() {
        out.push((String::new(), TEXT));
        // (Each column as wide as its widest entry, its header with the unit.)
        let head_of = |col: &str| {
            let unit = s.params.iter().find(|p| p.key.ends_with(&format!(".{col}"))).map(|p| p.unit.as_str()).unwrap_or("");
            if unit.is_empty() { col.to_uppercase() } else { format!("{} {}", col.to_uppercase(), unit.to_uppercase()) }
        };
        let entry = |r: &str, col: &str| s.param(&format!("{r}.{col}")).map_or("-".to_string(), |p| value(&p.value));
        let widths: Vec<usize> = cols.iter().map(|col| rows.iter().map(|r| entry(r, col).len()).chain([head_of(col).len()]).max().unwrap_or(4) + 2).collect();
        let mut head = format!("{:<6}", "");
        for (col, w) in cols.iter().zip(&widths) {
            head.push_str(&format!("{:<w$}", head_of(col)));
        }
        out.push((head, DIM));
        for r in &rows {
            let mut l = format!("{:<6}", r);
            for (col, w) in cols.iter().zip(&widths) {
                l.push_str(&format!("{:<w$}", entry(r, col)));
            }
            out.push((l, TEXT));
        }
    }
    if !plain.is_empty() {
        out.push((String::new(), TEXT));
        out.push((format!("{:<18}{:<34}{}", "PARAMETER", "VALUE", "UNIT"), DIM));
        for p in plain {
            out.push((format!("{:<18}{:<34}{}", p.key.to_uppercase(), value(&p.value), p.unit.to_uppercase()), TEXT));
        }
    }
    let notes: Vec<_> = s.params.iter().filter(|p| !p.note.is_empty()).collect();
    if !notes.is_empty() {
        out.push((String::new(), TEXT));
        out.push(("NOTES".into(), DIM));
        for p in notes {
            for l in crate::fmt::wrap(&format!("{}: {}", p.key.to_uppercase(), p.note), width) {
                out.push((l, DIM));
            }
        }
    }
    if !s.requires.is_empty() {
        out.push((String::new(), TEXT));
        out.push((format!("{:<20}{:<14}{:<16}{}", "REQUIRES", "CHECK", "AGAINST", "PER"), DIM));
        for r in &s.requires {
            let check = match r.check {
                Check::AtMost => "AT MOST",
                Check::AtLeast => "AT LEAST",
                Check::Equals => "EQUALS",
                Check::FitsWithin => "FITS WITHIN",
                Check::Provides => "PROVIDES",
            };
            out.push((format!("{:<20}{:<14}{:<16}{}", r.subject.to_uppercase(), check, r.param.to_uppercase(), r.per.to_uppercase()), TEXT));
        }
    }
    out.push((String::new(), TEXT));
    for l in crate::fmt::wrap(&s.text, width) {
        out.push((l, TEXT));
    }
    out.push((String::new(), TEXT));
    out.push(("PRODUCTS DECLARED TO IT: NONE YET".into(), DIM));
}
