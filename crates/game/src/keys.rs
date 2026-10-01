//! Key bindings, by scope. Actions live in a tree of scopes — the global
//! ones, flight (shared by every flying mode), then each mode's own (NAV,
//! COMBAT, MINING), and the screens (the map) and the observer. Each action
//! takes the first letter of its name that's free in its scope (not taken by
//! an action of its own scope, an enclosing one or one nested in it, nor
//! one of the keys the scope keeps for flying or walking); failing that,
//! the next letter along its name. The letter is lit where it stands in
//! the name. A few keys are fixed (F1 help, TAB watch, SPACE gun, the
//! F-keys for the system's switches, the flight and walking keys), and a
//! few actions are pinned to a key of their own (`PINNED`: X cancels).

use std::sync::OnceLock;

use universe_engine::{Input, KeyCode};

/// A scope of actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Flight,
    Nav,
    Combat,
    Mining,
    /// Set down: docked at a station or landed.
    Docked,
    Map,
    Observer,
}

impl Scope {
    pub fn parent(self) -> Option<Scope> {
        match self {
            Scope::Global => None,
            Scope::Flight | Scope::Map | Scope::Observer => Some(Scope::Global),
            Scope::Nav | Scope::Combat | Scope::Mining | Scope::Docked => Some(Scope::Flight),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Scope::Global => "EVERYWHERE",
            Scope::Flight => "FLYING (ANY MODE)",
            Scope::Nav => "NAV MODE",
            Scope::Combat => "COMBAT MODE",
            Scope::Mining => "MINING MODE",
            Scope::Docked => "DOCKED OR LANDED",
            Scope::Map => "NAVIGATION MAP",
            Scope::Observer => "OBSERVER",
        }
    }

    /// The letters it keeps for moving (flying, walking, browsing).
    fn reserved(self) -> &'static str {
        match self {
            Scope::Flight | Scope::Nav | Scope::Combat | Scope::Mining | Scope::Docked => "WASDQE",
            Scope::Map | Scope::Observer => "WS",
            Scope::Global => "",
        }
    }

    /// Itself and what's nested in it.
    fn within(self) -> Vec<Scope> {
        ALL_SCOPES.iter().copied().filter(|&s| {
            let mut t = Some(s);
            while let Some(u) = t {
                if u == self {
                    return true;
                }
                t = u.parent();
            }
            false
        }).collect()
    }
}

const ALL_SCOPES: [Scope; 8] = [Scope::Global, Scope::Flight, Scope::Nav, Scope::Combat, Scope::Mining, Scope::Docked, Scope::Map, Scope::Observer];

/// The actions that take a letter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Map,
    Market,
    Cargo,
    Economy,
    Combat,
    Mining,
    View,
    Foot,
    Shipyard,
    Lock,
    Hyperdrive,
    Autopilot,
    Keep,
    Orbit,
    Cancel,
    Clearance,
    Proximity,
    Laser,
    Prospect,
    ZeroIn,
    Anchor,
    Excavate,
    Target,
    Untarget,
    AddStop,
    DropStop,
    EmptyRoute,
    SettlerRoute,
    Galaxy,
    NextStar,
    PreviousStar,
    Home,
    TrackSettler,
    Systems,
    Refuel,
    Repair,
}

/// The table, in the order letters are given out (what's used most first,
/// within each scope): (action, scope, name).
const TABLE: &[(Act, Scope, &str)] = &[
    (Act::Map, Scope::Global, "MAP"),
    (Act::Market, Scope::Global, "MARKET"),
    (Act::Cargo, Scope::Global, "CARGO"),
    (Act::Economy, Scope::Global, "ECONOMY"),
    (Act::Combat, Scope::Global, "COMBAT"),
    (Act::Mining, Scope::Global, "MINING"),
    (Act::View, Scope::Global, "VIEW"),
    (Act::Foot, Scope::Global, "FOOT"),
    (Act::Lock, Scope::Flight, "LOCK"),
    (Act::Hyperdrive, Scope::Flight, "HYPERDRIVE"),
    (Act::Autopilot, Scope::Flight, "AUTOPILOT"),
    (Act::Keep, Scope::Flight, "KEEP"),
    (Act::Orbit, Scope::Flight, "ORBIT"),
    (Act::Cancel, Scope::Flight, "CANCEL"),
    (Act::Clearance, Scope::Nav, "CLEARANCE"),
    (Act::Proximity, Scope::Nav, "IMPACT WARNING"),
    (Act::Laser, Scope::Combat, "PULSE LASER"),
    (Act::Prospect, Scope::Mining, "PROSPECT"),
    (Act::ZeroIn, Scope::Mining, "ZERO IN"),
    (Act::Anchor, Scope::Mining, "ANCHOR"),
    (Act::Excavate, Scope::Mining, "DIG"),
    (Act::Target, Scope::Map, "TARGET"),
    (Act::Untarget, Scope::Map, "CLEAR TARGET"),
    (Act::AddStop, Scope::Map, "ADD STOP"),
    (Act::DropStop, Scope::Map, "DROP STOP"),
    (Act::EmptyRoute, Scope::Map, "EMPTY ROUTE"),
    (Act::SettlerRoute, Scope::Map, "SETTLER ROUTE"),
    (Act::Galaxy, Scope::Map, "GALAXY"),
    (Act::NextStar, Scope::Observer, "NEXT STAR"),
    (Act::PreviousStar, Scope::Observer, "PREVIOUS STAR"),
    (Act::Home, Scope::Observer, "HOME SHIP"),
    (Act::TrackSettler, Scope::Observer, "TRACK SETTLER"),
    // (Newer ones last: the keys already learnt stay as they were.)
    (Act::Shipyard, Scope::Global, "SHIPYARD"),
    (Act::Systems, Scope::Flight, "POWER"),
    (Act::Refuel, Scope::Docked, "FUEL UP"),
    (Act::Repair, Scope::Docked, "MEND HULL"),
];

/// Actions whose key is set, not taken from the name (given out first).
// (POWER: every letter of its name is taken in flight; J is the one free.)
const PINNED: &[(Act, char)] = &[(Act::Cancel, 'X'), (Act::Systems, 'J')];

/// An action's binding: its letter, and where it stands in the name (None:
/// not in it — given the first free letter instead).
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub act: Act,
    pub scope: Scope,
    pub name: &'static str,
    pub letter: char,
    /// (The tests hold every key to its name.)
    #[allow(dead_code)]
    pub at: Option<usize>,
}

fn assign() -> Vec<Binding> {
    let mut out: Vec<Binding> = Vec::new();
    for &(act, letter) in PINNED {
        let &(_, scope, name) = TABLE.iter().find(|t| t.0 == act).expect("pinned actions are in the table");
        out.push(Binding { act, scope, name, letter, at: name.find(letter) });
    }
    for &(act, scope, name) in TABLE.iter().filter(|t| !PINNED.iter().any(|p| p.0 == t.0)) {
        // Taken: the letters kept for moving in it or anything nested in it,
        // and those given to its own scope, the enclosing ones, the nested ones.
        let within = scope.within();
        let mut related = within.clone();
        let mut up = scope.parent();
        while let Some(s) = up {
            related.push(s);
            up = s.parent();
        }
        let taken = |c: char| {
            within.iter().any(|s| s.reserved().contains(c)) || out.iter().any(|b| related.contains(&b.scope) && b.letter == c)
        };
        let found = name.char_indices().find(|&(_, c)| c.is_ascii_alphabetic() && !taken(c));
        let (letter, at) = match found {
            Some((i, c)) => (c, Some(i)),
            None => (('A'..='Z').find(|&c| !taken(c)).unwrap_or('?'), None),
        };
        out.push(Binding { act, scope, name, letter, at });
    }
    out
}

pub fn bindings() -> &'static [Binding] {
    static B: OnceLock<Vec<Binding>> = OnceLock::new();
    B.get_or_init(assign)
}

pub fn binding(act: Act) -> Binding {
    *bindings().iter().find(|b| b.act == act).expect("every action is bound")
}

fn code(c: char) -> KeyCode {
    use KeyCode::*;
    [KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ][(c as u8 - b'A') as usize]
}

/// The action's key was pressed this frame.
pub fn pressed(input: &Input, act: Act) -> bool {
    input.pressed(code(binding(act).letter))
}

/// The action's key is held.
pub fn down(input: &Input, act: Act) -> bool {
    input.down(code(binding(act).letter))
}

/// The key, as shown.
pub fn key(act: Act) -> String {
    binding(act).letter.to_string()
}

/// The keys that aren't letters by name: (key, what, where).
pub const FIXED: &[(&str, &str, Scope)] = &[
    ("F1", "HELP", Scope::Global),
    ("TAB", "WATCH (OBSERVER)", Scope::Global),
    ("BKSP", "RESPAWN AT HOME", Scope::Global),
    ("F2 F4 F6", "LABELS, GRID, PAUSE", Scope::Global),
    ("F3 F5 F9", "PROFILER, QUICKSAVE, LOAD", Scope::Global),
    ("F7", "THRUSTERS PANEL", Scope::Flight),
    ("F8 F12", "MUTE, SCREENSHOT", Scope::Global),
    (", .", "TIME WARP DOWN / UP", Scope::Global),
    ("W S", "THROTTLE", Scope::Flight),
    ("A D Q E", "ROLL, YAW (SHIFT: THRUSTERS, E LIFT)", Scope::Flight),
    ("ARROWS", "PITCH AND ROLL; CLICK: MOUSE FLIGHT", Scope::Flight),
    ("SPACE", "GUN", Scope::Combat),
    ("ENTER DEL", "LOCK / CLEAR NAV TARGET (LIST)", Scope::Map),
    ("ARROWS", "SELECT, SYSTEM", Scope::Map),
    ("WHEEL [ ]", "ZOOM, BODIES", Scope::Observer),
];

/// The bindings as a tree, for the help: each scope's actions under it,
/// indented by depth.
pub fn tree() -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    fn depth(s: Scope) -> usize {
        s.parent().map_or(0, |p| depth(p) + 1)
    }
    fn walk(s: Scope, lines: &mut Vec<(usize, String)>) {
        let d = depth(s);
        lines.push((d, s.label().to_string()));
        for b in bindings().iter().filter(|b| b.scope == s) {
            lines.push((d + 1, format!("{:<5} {}", b.letter, b.name)));
        }
        for (k, what, _) in FIXED.iter().filter(|f| f.2 == s) {
            lines.push((d + 1, format!("{k:<5} {what}")));
        }
        for c in ALL_SCOPES.iter().filter(|c| c.parent() == Some(s)) {
            walk(*c, lines);
        }
    }
    walk(Scope::Global, &mut lines);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_its_own_letter_in_its_scopes_and_mostly_from_its_name() {
        let b = bindings();
        for x in b {
            for y in b {
                if x.act != y.act && x.letter == y.letter {
                    // Only siblings (neither within the other) may share a letter.
                    assert!(!x.scope.within().contains(&y.scope) && !y.scope.within().contains(&x.scope), "{:?} and {:?} both {}", x.act, y.act, x.letter);
                }
            }
        }
        let off: Vec<_> = b.iter().filter(|x| x.at.is_none() && !PINNED.iter().any(|p| p.0 == x.act)).map(|x| x.name).collect();
        assert!(off.is_empty(), "letters from outside the name: {off:?}");
        for (d, l) in tree() {
            eprintln!("{}{l}", "  ".repeat(d));
        }
    }
}
