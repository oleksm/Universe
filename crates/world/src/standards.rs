//! Standards: published, versioned agreements on how things fit together
//! (dimensions, interfaces, units, codes), from standards bodies; the Foundry
//! Standards Office is seeded, anyone may found another (see `docs/standards.md`).
//!
//! A standard is not physics (Dogma has the laws), not a product (products
//! conform to it), not law (an authority may mandate one: its rule), not a
//! guarantee (conformance is claimed; certifying it is a service). Its
//! **parameters** are typed values; its **requirements** are checks on a
//! product or place, against them.
//!
//! Each body's standards sit in its branches: a tree (`0 Foundations`,
//! `2.1 Sockets`...), the bodies at its roots.

use serde::Deserialize;

/// A standards body: who publishes, where, and its branches of the tree.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub key: String,
    pub name: String,
    /// Its namespace: the start of its standards' ids (`FSO`).
    pub prefix: String,
    /// Where it sits and keeps its register (a place, as `places.ron` has it,
    /// or `home`: the home system's station).
    pub seat: String,
    pub note: String,
    /// Its branches: (path, title), `0`, `2`, `2.1`... (a path's parent is
    /// the path without its last part).
    pub branches: Vec<(String, String)>,
}

/// Where a standard stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Status {
    Draft,
    Published,
    Superseded,
    Withdrawn,
}

/// On what terms products may be built to it.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum Licence {
    Open,
    /// Credits per conforming product made.
    Fee(f64),
}

/// A value it fixes.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Value {
    Num(f64),
    /// At least, at most.
    Range(f64, f64),
    Text(String),
}

/// One parameter: its key (dotted for a table: `M.length_max` is row M,
/// column length_max), its value, its unit (empty: none), a note (where an
/// invented number comes from, what it's aimed at).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Param {
    pub key: String,
    pub value: Value,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub note: String,
}

/// How a product's figure is held against a parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Check {
    AtMost,
    AtLeast,
    Equals,
    /// Its envelope inside the parameter's (a clearance).
    FitsWithin,
    /// It provides the service the parameter names.
    Provides,
}

/// A requirement: the product's `subject` (`ship.length`, `module.mass`...)
/// held to `param` by `check`; `per` names the row it's held to (the size
/// class or socket size the product declares), if the parameter is a table's.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub subject: String,
    pub check: Check,
    pub param: String,
    #[serde(default)]
    pub per: String,
}

/// A standard, as its register holds it.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Standard {
    /// Its id without the version: body prefix, branch, number (`FSO/3.1/001`).
    pub key: String,
    /// Its body (`body.fso`).
    pub body: String,
    /// The branch it's in (`3.1`).
    pub branch: String,
    pub version: u32,
    pub title: String,
    /// What it covers, and what it doesn't.
    pub scope: String,
    pub status: Status,
    /// The standards it builds on (their keys).
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default)]
    pub requires: Vec<Requirement>,
    /// For people: why, not only what.
    pub text: String,
    pub licence: Licence,
    /// When it was published (world time, s) at its body's seat.
    #[serde(default)]
    pub published: f64,
}

impl Standard {
    /// As it's cited: `FSO/3.1/001 V1`.
    pub fn id(&self) -> String {
        format!("{} V{}", self.key, self.version)
    }

    pub fn param(&self, key: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.key == key)
    }

    pub(crate) fn check(&self) -> Result<(), String> {
        if self.version == 0 {
            return Err("versions start at 1".into());
        }
        for p in &self.params {
            if let Value::Range(a, b) = p.value
                && b < a
            {
                return Err(format!("parameter {}: range upside down", p.key));
            }
        }
        for r in &self.requires {
            // (A table's parameter is held per row: the row names come from the product.)
            let found = self.params.iter().any(|p| p.key == r.param || p.key.ends_with(&format!(".{}", r.param)));
            if !found {
                return Err(format!("requirement on {}: no parameter '{}'", r.subject, r.param));
            }
        }
        Ok(())
    }
}

impl Body {
    pub(crate) fn check(&self) -> Result<(), String> {
        for (path, _) in &self.branches {
            if let Some((parent, _)) = path.rsplit_once('.')
                && !self.branches.iter().any(|(p, _)| p == parent)
            {
                return Err(format!("branch {path}: no parent branch {parent}"));
            }
        }
        Ok(())
    }

    /// A branch's title.
    pub fn branch(&self, path: &str) -> Option<&str> {
        self.branches.iter().find(|(p, _)| p == path).map(|(_, t)| t.as_str())
    }
}

/// A branch path's parent (`2.1` → `2`; a top branch's: None).
pub fn parent(path: &str) -> Option<&str> {
    path.rsplit_once('.').map(|(p, _)| p)
}
