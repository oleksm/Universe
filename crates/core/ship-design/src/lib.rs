//! Pure shared ship authoring: typed YAML document, schema and local semantics.
//! Does not resolve registry/assets, read files, migrate saves, or accept a ship
//! as structurally sound or flight-ready. Canonical emit is not a comment editor.
mod schema;
mod types;
mod validation;
mod yaml;
pub use schema::schema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub use types::*;
pub use validation::validate;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// JSON Pointer, independent of YAML formatting and collection order.
    pub path: String,
    pub message: String,
    /// One-based line/column when parsed from source; absent for typed validation.
    pub location: Option<Location>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub diagnostics: Vec<Diagnostic>,
}
impl Error {
    pub(crate) fn message(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostics: vec![Diagnostic {
                path: path.into(),
                message: message.into(),
                location: None,
            }],
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, d) in self.diagnostics.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            if let Some(p) = d.location {
                write!(f, "{}:{} ", p.line, p.column)?;
            }
            write!(f, "{}: {}", d.path, d.message)?;
        }
        Ok(())
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// Explicit parser resource ceilings, not engineering limits on a ship.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_bytes: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_scalar_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_depth: 64,
            max_nodes: 1_000_000,
            max_scalar_bytes: 1024 * 1024,
        }
    }
}

/// Original bytes and value locations retained for an editor's source bridge.
/// `emit` canonicalizes and discards comments; keep `source` for untouched drafts.
#[derive(Clone, Debug)]
pub struct SourceDocument {
    pub document: Document,
    pub source: String,
    pub locations: BTreeMap<String, Location>,
}
pub fn parse(source: &str) -> Result<Document> {
    Ok(parse_source(source)?.document)
}
pub fn parse_source(source: &str) -> Result<SourceDocument> {
    parse_with_limits(source, Limits::default())
}
pub fn parse_with_limits(source: &str, limits: Limits) -> Result<SourceDocument> {
    let (value, locations) = yaml::read(source, limits)?;
    let locate = |mut errors: Vec<Diagnostic>| {
        for d in &mut errors {
            let mut path = d.path.as_str();
            loop {
                if let Some(loc) = locations.get(path) {
                    d.location = Some(*loc);
                    break;
                }
                if let Some((parent, _)) = path.rsplit_once('/') {
                    path = parent;
                } else {
                    break;
                }
            }
        }
        Error {
            diagnostics: errors,
        }
    };
    let errors = schema::structural(&value);
    if !errors.is_empty() {
        return Err(locate(errors));
    }
    let document: Document = serde_json::from_value(value).map_err(|e| {
        locate(vec![Diagnostic {
            path: String::new(),
            message: e.to_string(),
            location: None,
        }])
    })?;
    let errors = validate(&document);
    if !errors.is_empty() {
        return Err(locate(errors));
    }
    Ok(SourceDocument {
        document,
        source: source.into(),
        locations,
    })
}
pub fn emit(document: &Document) -> Result<String> {
    let errors = validate(document);
    if !errors.is_empty() {
        return Err(Error {
            diagnostics: errors,
        });
    }
    yaml::write(&serde_json::to_value(document).map_err(|e| Error::message("", e.to_string()))?)
}

impl Document {
    /// Empty complete document with explicit format, coordinate and placement versions.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema: None,
            format: Format::FreefallShip,
            version: Version::V1Draft,
            design: Identity {
                id: id.into(),
                name: name.into(),
                notes: None,
            },
            coordinates: Coordinates::RightHandedXRightYUpZAftMetres,
            placement_convention: PlacementConvention::StudioCenteredV1,
            hull: None,
            requirements: None,
            modules: BTreeMap::new(),
            access: Access {
                points: BTreeMap::new(),
                paths: BTreeMap::new(),
                groups: BTreeMap::new(),
                side_doors: BTreeMap::new(),
                ends: BTreeMap::new(),
            },
            structure: Structure {
                members: BTreeMap::new(),
                plates: BTreeMap::new(),
                landing_pads: BTreeMap::new(),
            },
            decks: BTreeMap::new(),
            view: None,
            dependencies: None,
        }
    }
}
