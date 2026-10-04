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

use serde::{Deserialize, Serialize};

/// A standards body: who publishes, where, and its branches of the tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub key: String,
    pub name: String,
    /// Its namespace: the start of its standards' ids (`SFO`).
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Draft,
    Published,
    Superseded,
    Withdrawn,
}

/// On what terms products may be built to it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Licence {
    Open,
    /// Credits per conforming product made.
    Fee(f64),
}

/// A value it fixes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Num(f64),
    /// At least, at most.
    Range(f64, f64),
    Text(String),
}

/// One parameter: its key (dotted for a table: `M.length_max` is row M,
/// column length_max), its value, its unit (empty: none), a note (where an
/// invented number comes from, what it's aimed at).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub subject: String,
    pub check: Check,
    pub param: String,
    #[serde(default)]
    pub per: String,
}

/// A standard, as its register holds it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Standard {
    /// Its key in the registry (`standard.sfo.18`).
    pub key: String,
    /// As it's cited, without the version: its body's prefix and its number
    /// (`SFO 18`).
    pub cite: String,
    /// Its body (`org.sfo`).
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
    /// As it's cited: `SFO 18 V1`.
    pub fn id(&self) -> String {
        format!("{} V{}", self.cite, self.version)
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

/// As the game writes it: in capitals, on one line.
pub(crate) fn caps(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase()
}

/// The standards bodies and their standards, from the registry: each body an
/// organisation of kind `standards_body`; each standard's body the one whose
/// prefix its key names (`standard.sfo.18`: SFO's), its branch its first
/// topic (its body's branches: its standards' topics, for now).
pub fn from_registry(reg: &crate::registry::Registry) -> (Vec<Body>, Vec<Standard>) {
    use crate::registry::{Check as C, Licence as L, OrgKind, ParamValue, StandardStatus as S};
    let orgs: Vec<_> = reg.organisations.iter().filter(|o| o.kind == OrgKind::StandardsBody).collect();
    let prefix_of = |o: &crate::registry::Organisation| o.prefix.clone().unwrap_or_else(|| panic!("{}: a standards body with no prefix", o.identity.key));
    let body_of = |key: &str| {
        let segment = key.split('.').nth(1).unwrap_or_else(|| panic!("{key}: no body in its key"));
        orgs.iter().find(|o| prefix_of(o).eq_ignore_ascii_case(segment)).unwrap_or_else(|| panic!("{key}: no standards body {segment}"))
    };
    let cite = |key: &str| {
        let number = key.rsplit('.').next().unwrap_or_default();
        format!("{} {number}", prefix_of(body_of(key)))
    };
    let key_of_cite = |c: &str| {
        let (prefix, number) = c.split_once(' ').unwrap_or_else(|| panic!("{c}: not a citation"));
        format!("standard.{}.{number}", prefix.to_lowercase())
    };
    let topic = |s: &crate::registry::Standard| s.topics.first().cloned().unwrap_or_else(|| "all".into());
    let standards: Vec<Standard> = reg
        .standards
        .iter()
        .map(|s| Standard {
            key: s.identity.key.clone(),
            cite: cite(&s.identity.key),
            body: body_of(&s.identity.key).identity.key.clone(),
            branch: topic(s),
            version: s.version.unwrap_or(1),
            title: caps(s.title.as_deref().unwrap_or_default()),
            scope: caps(s.scope.as_deref().unwrap_or_default()),
            status: match s.status.unwrap_or(S::Draft) {
                S::Draft => Status::Draft,
                S::Published => Status::Published,
                S::Superseded => Status::Superseded,
                S::Withdrawn => Status::Withdrawn,
            },
            refs: s.refs.iter().map(|r| key_of_cite(r)).collect(),
            params: s
                .params
                .iter()
                .map(|p| Param {
                    key: p.key.clone(),
                    value: match &p.value {
                        ParamValue::Number(n) => Value::Num(*n),
                        ParamValue::Range([a, b]) => Value::Range(*a, *b),
                        ParamValue::Text(t) => Value::Text(caps(t)),
                    },
                    unit: p.unit.clone().unwrap_or_default(),
                    note: caps(p.note.as_deref().unwrap_or_default()),
                })
                .collect(),
            requires: s
                .requires
                .iter()
                .map(|r| Requirement {
                    subject: r.subject.clone(),
                    check: match r.check {
                        C::AtMost => Check::AtMost,
                        C::AtLeast => Check::AtLeast,
                        C::Equals => Check::Equals,
                        C::FitsWithin => Check::FitsWithin,
                        C::Provides => Check::Provides,
                    },
                    param: r.param.clone(),
                    per: r.per.clone().unwrap_or_default(),
                })
                .collect(),
            text: caps(s.text.as_deref().unwrap_or_default()),
            licence: match s.licence {
                Some(L::Fee { fee }) => Licence::Fee(fee),
                _ => Licence::Open,
            },
            published: s.published.unwrap_or(0.0),
        })
        .collect();
    let bodies = orgs
        .iter()
        .map(|o| {
            let mut topics: Vec<String> = reg.standards.iter().filter(|s| body_of(&s.identity.key).identity.key == o.identity.key).map(topic).collect();
            topics.sort();
            topics.dedup();
            Body {
                key: o.identity.key.clone(),
                name: caps(&o.identity.name),
                prefix: prefix_of(o),
                seat: o.seat.clone().unwrap_or_default(),
                note: caps(o.note.as_deref().unwrap_or_default()),
                branches: topics.into_iter().map(|t| { let title = caps(&t.replace('-', " ")); (t, title) }).collect(),
            }
        })
        .collect();
    (bodies, standards)
}
