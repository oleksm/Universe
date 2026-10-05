//! The registry's Rust types, generated from its JSON schemas
//! (`standards/**/schema/*.yaml`, `standards/*.schema.yaml`) each time a
//! schema changes. Nothing here is hand-kept: a schema change is a rebuild.
//!
//! - a schema with `x-kind` is a record: a struct named after its first kind,
//!   and a list of them on `Registry`;
//! - an object is a struct (unknown fields refused), its required properties
//!   plain, the rest `Option` (a list: empty when left out);
//! - an `enum` is a Rust enum (a `null` in it: the field is an `Option`);
//! - a `oneOf` whose shapes each have a constant `kind` is an enum tagged by
//!   it; any other `oneOf` an untagged one;
//! - `[T, null]` is `Option<T>`; an integer with a minimum of 0 or more is
//!   `u32`, any other `i64`; a number `f64`, or `Degrees` where its unit is
//!   `deg`;
//! - a `$ref` is the type of what it names (shared definitions once);
//! - every property marked `x-ref` is listed by `Refs`, for the registry to
//!   check that each names a record of a kind it allows.
//!
//! The generator refuses what it can't type: it says where.

use serde_norway::{Mapping, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

fn main() {
    let root = Path::new("../../standards");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    schemas(root, &mut files);
    files.sort();
    let mut g = Gen::default();
    for f in &files {
        println!("cargo:rerun-if-changed={}", f.display());
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let v: Value = serde_norway::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        g.files.insert(f.canonicalize().unwrap(), v);
    }
    let order: Vec<PathBuf> = g.files.keys().cloned().collect();
    // (Records' names first: they are the kinds', and nothing else takes them.)
    for v in g.files.values() {
        if let Some(k) = v.get("x-kind").and_then(Value::as_sequence).and_then(|k| k.first()).and_then(Value::as_str) {
            g.reserved.insert(camel(k));
        }
    }
    let mut records = Vec::new();
    for f in &order {
        let v = g.files[f].clone();
        let Some(kinds) = v.get("x-kind").and_then(Value::as_sequence) else { continue };
        let kinds: Vec<String> = kinds.iter().map(|k| k.as_str().expect("a kind is text").to_string()).collect();
        let name = camel(&kinds[0]);
        g.record = Some(name.clone());
        g.object(&name, &v, f);
        records.push((name, kinds));
    }
    let mut out = String::from("// Generated from the registry's schemas by build.rs: edit the schemas, not this.\n\n");
    out.push_str(&g.out);
    // The registry: a list of each kind of record.
    out.push_str("\n/// Every record the registry holds, by kind.\n#[derive(Clone, Debug, Default, Serialize, Deserialize)]\npub struct Records {\n");
    for (name, kinds) in &records {
        writeln!(out, "    /// `{}`.\n    pub {}: Vec<{name}>,", kinds.join("`, `"), plural(&snake(&kinds[0]))).unwrap();
    }
    out.push_str("}\n\nimpl Records {\n    /// Parses `text`, a record of `kind`, into its list. False: no schema has that kind.\n    pub fn parse(&mut self, kind: &str, text: &str) -> Result<bool, serde_norway::Error> {\n        match kind {\n");
    for (name, kinds) in &records {
        let pat = kinds.iter().map(|k| format!("{k:?}")).collect::<Vec<_>>().join(" | ");
        writeln!(out, "            {pat} => self.{}.push(serde_norway::from_str::<{name}>(text)?),", plural(&snake(&kinds[0]))).unwrap();
    }
    out.push_str("            _ => return Ok(false),\n        }\n        Ok(true)\n    }\n\n    /// Each record's references: (the record's key, what it names, the kinds that may be named).\n    pub fn refs(&self, f: &mut dyn FnMut(&str, &str, &'static [&'static str])) {\n");
    for (name, kinds) in &records {
        let _ = name;
        writeln!(out, "        for r in &self.{} {{\n            let key = r.identity.key.as_str();\n            r.refs(&mut |to, kinds| f(key, to, kinds));\n        }}", plural(&snake(&kinds[0]))).unwrap();
    }
    out.push_str("    }\n}\n");
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("generated.rs");
    std::fs::write(dest, out).unwrap();
}

/// Every schema file under `dir`.
fn schemas(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if p.is_dir() {
            if name != "metadata" && name != "sources" {
                schemas(&p, out);
            }
        } else if name.ends_with(".schema.yaml") || (p.parent().is_some_and(|d| d.ends_with("schema")) && name.ends_with(".yaml")) {
            out.push(p);
        }
    }
}

#[derive(Default)]
struct Gen {
    files: BTreeMap<PathBuf, Value>,
    out: String,
    /// Types emitted, by name.
    names: HashSet<String>,
    /// Definitions already typed: (file, name) → the Rust type.
    defs: HashMap<(PathBuf, String), String>,
    /// The records' names, kept for them.
    reserved: HashSet<String>,
    /// The record about to be emitted: it takes its reserved name.
    record: Option<String>,
    /// Structs of which nothing is required: they derive `Default`.
    defaultable: HashSet<String>,
}

/// How a field's value is found again for `Refs`: a call to make on it.
#[derive(Clone)]
enum Walk {
    /// Nothing to look into.
    None,
    /// A string naming records of these kinds.
    Ref(Vec<String>),
    /// A generated type: it walks itself.
    Typed,
    /// A list, or an option, of something walked so.
    Each(Box<Walk>),
}

struct Ty {
    rust: String,
    walk: Walk,
    /// Already an `Option` (a type list or enum with null).
    optional: bool,
}

impl Ty {
    /// Left out, it's empty, not missing: a list, an option already, or a
    /// group of which nothing is required (it derives `Default`).
    fn defaults(&self, g: &Gen) -> bool {
        self.rust.starts_with("Vec<") || self.optional || g.defaultable.contains(&self.rust)
    }
}

impl Gen {
    fn emit_name(&mut self, want: &str) -> String {
        if self.record.as_deref() == Some(want) {
            self.record = None;
            assert!(self.names.insert(want.to_string()), "the record {want} is named twice");
            return want.to_string();
        }
        let taken = |g: &Self, n: &str| g.names.contains(n) || g.reserved.contains(n);
        let mut name = want.to_string();
        if taken(self, &name) {
            name = format!("{want}Entry");
        }
        let mut n = 2;
        while taken(self, &name) {
            name = format!("{want}Entry{n}");
            n += 1;
        }
        self.names.insert(name.clone());
        name
    }

    /// The type of the schema node `v` (in `file`); `hint` names a type made for it.
    fn ty(&mut self, hint: &str, v: &Value, file: &Path) -> Ty {
        let at = format!("{} ({hint})", file.display());
        if let Some(r) = v.get("$ref").and_then(Value::as_str) {
            return self.reference(r, file);
        }
        if let Some(alts) = v.get("oneOf").and_then(Value::as_sequence) {
            return self.one_of(hint, alts, file);
        }
        if let Some(c) = v.get("const") {
            assert!(c.is_string(), "{at}: a constant that isn't text");
            return Ty { rust: "String".into(), walk: Walk::None, optional: false };
        }
        if let Some(e) = v.get("enum").and_then(Value::as_sequence) {
            let has_null = e.iter().any(Value::is_null);
            let variants: Vec<&str> = e.iter().filter_map(Value::as_str).collect();
            assert_eq!(variants.len() + usize::from(has_null), e.len(), "{at}: an enum of other than text");
            let name = self.emit_name(hint);
            let mut s = format!("{}#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]\npub enum {name} {{\n", doc(v, ""));
            let mut seen = HashSet::new();
            let mut arms = String::new();
            for x in &variants {
                let mut id = camel(x);
                while !seen.insert(id.clone()) {
                    id.push('_');
                }
                writeln!(s, "    #[serde(rename = {x:?})]\n    {id},").unwrap();
                writeln!(arms, "            {name}::{id} => {x:?},").unwrap();
            }
            s.push_str("}\n\n");
            writeln!(s, "impl {name} {{\n    /// As the registry writes it.\n    pub fn as_str(self) -> &'static str {{\n        match self {{\n{arms}        }}\n    }}\n}}\n").unwrap();
            self.out.push_str(&s);
            return Ty { rust: if has_null { format!("Option<{name}>") } else { name }, walk: Walk::None, optional: has_null };
        }
        let (t, nullable) = match v.get("type") {
            Some(Value::String(t)) => (t.clone(), false),
            Some(Value::Sequence(ts)) => {
                let ts: Vec<&str> = ts.iter().filter_map(Value::as_str).collect();
                let t: Vec<&str> = ts.iter().copied().filter(|t| *t != "null").collect();
                assert_eq!(t.len(), 1, "{at}: a type list other than [T, null]");
                (t[0].to_string(), ts.contains(&"null"))
            }
            None if v.get("properties").is_some() => ("object".into(), false),
            None => panic!("{at}: no type"),
            Some(_) => panic!("{at}: a type that isn't text"),
        };
        let unit = v.get("x-unit").and_then(Value::as_str);
        let refs: Option<Vec<String>> = v.get("x-ref").and_then(Value::as_sequence).map(|k| k.iter().filter_map(Value::as_str).map(String::from).collect());
        let base = match t.as_str() {
            "string" => Ty { rust: "String".into(), walk: refs.map_or(Walk::None, Walk::Ref), optional: false },
            "number" => Ty { rust: if unit == Some("deg") { "Degrees".into() } else { "f64".into() }, walk: Walk::None, optional: false },
            "integer" => {
                let unsigned = v.get("minimum").and_then(Value::as_f64).is_some_and(|m| m >= 0.0);
                Ty { rust: if unsigned { "u32".into() } else { "i64".into() }, walk: Walk::None, optional: false }
            }
            "boolean" => Ty { rust: "bool".into(), walk: Walk::None, optional: false },
            "array" => {
                let items = v.get("items").unwrap_or_else(|| panic!("{at}: a list with no items"));
                let mut inner = self.ty(&singular(hint), items, file);
                if let (Walk::None, Some(r)) = (&inner.walk, v.get("x-ref").and_then(Value::as_sequence)) {
                    inner.walk = Walk::Ref(r.iter().filter_map(Value::as_str).map(String::from).collect());
                }
                // (A list of exactly N plain values, a point or a colour, is an array of N.)
                let (lo, hi) = (v.get("minItems").and_then(Value::as_u64), v.get("maxItems").and_then(Value::as_u64));
                if let (Some(lo), Some(hi)) = (lo, hi)
                    && lo == hi
                    && lo <= 4
                    && matches!(inner.rust.as_str(), "f64" | "i64" | "u32" | "Degrees")
                {
                    return Ty { rust: format!("[{}; {lo}]", inner.rust), walk: Walk::None, optional: false };
                }
                Ty { rust: format!("Vec<{}>", inner.rust), walk: Walk::Each(Box::new(inner.walk)), optional: false }
            }
            "object" => {
                let name = self.object(hint, v, file);
                Ty { rust: name, walk: Walk::Typed, optional: false }
            }
            other => panic!("{at}: type {other}"),
        };
        if nullable {
            Ty { rust: format!("Option<{}>", base.rust), walk: Walk::Each(Box::new(base.walk)), optional: true }
        } else {
            base
        }
    }

    fn reference(&mut self, r: &str, file: &Path) -> Ty {
        let (path, def) = r.split_once('#').unwrap_or((r, ""));
        let target = if path.is_empty() { file.to_path_buf() } else { file.parent().unwrap().join(path).canonicalize().unwrap_or_else(|e| panic!("{}: $ref {r}: {e}", file.display())) };
        let def = def.trim_start_matches("/definitions/").to_string();
        let node = self.files.get(&target).unwrap_or_else(|| panic!("{}: $ref {r}: no such schema", file.display()))["definitions"][def.as_str()].clone();
        assert!(!node.is_null(), "{}: $ref {r}: no such definition", file.display());
        // (A definition is typed once; a plain one, as what it is.)
        let key = (target.clone(), def.clone());
        if let Some(name) = self.defs.get(&key).cloned() {
            return self.ty_of_named(&name, &node, &target);
        }
        let named = node.get("properties").is_some() || node.get("enum").is_some() || node.get("oneOf").is_some() || node.get("type").and_then(Value::as_str) == Some("array");
        if !named {
            return self.ty(&camel(&def), &node, &target);
        }
        let t = self.ty(&camel(&def), &node, &target);
        self.defs.insert(key, t.rust.clone());
        if t.rust.starts_with('[') {
            return Ty { walk: Walk::None, ..t };
        }
        t
    }

    /// A definition typed already, by the name it was given.
    fn ty_of_named(&self, rust: &str, node: &Value, _file: &Path) -> Ty {
        if rust.starts_with('[') || matches!(rust, "f64" | "i64" | "u32" | "bool" | "String" | "Degrees") {
            return Ty { rust: rust.to_string(), walk: Walk::None, optional: false };
        }
        let walk = if rust.starts_with("Vec<") {
            Walk::Each(Box::new(Walk::Typed))
        } else if node.get("enum").is_some() {
            Walk::None
        } else {
            Walk::Typed
        };
        // (A list of plain values walks to nothing, which `Each(Typed)` would get wrong: check.)
        let walk = match (&walk, node.get("items")) {
            (Walk::Each(_), Some(items)) if items.get("properties").is_none() && items.get("$ref").is_none() && items.get("oneOf").is_none() => {
                Walk::Each(Box::new(items.get("x-ref").map_or(Walk::None, |k| Walk::Ref(k.as_sequence().unwrap().iter().filter_map(Value::as_str).map(String::from).collect()))))
            }
            _ => walk,
        };
        Ty { rust: rust.to_string(), walk, optional: rust.starts_with("Option<") }
    }

    /// A struct for the object `v`; its name.
    fn object(&mut self, hint: &str, v: &Value, file: &Path) -> String {
        let at = format!("{} ({hint})", file.display());
        assert!(v.get("additionalProperties") == Some(&Value::Bool(false)), "{at}: an open object (additionalProperties must be false)");
        let props = v.get("properties").and_then(Value::as_mapping).cloned().unwrap_or_else(Mapping::new);
        let required: HashSet<&str> = v.get("required").and_then(Value::as_sequence).map(|r| r.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let name = self.emit_name(hint);
        let mut fields = String::new();
        let mut walks = String::new();
        for (k, pv) in &props {
            let k = k.as_str().unwrap_or_else(|| panic!("{at}: a property that isn't text"));
            let t = self.ty(&format!("{name}{}", camel(k)), pv, file);
            let (id, rename) = field(k);
            let req = required.contains(k);
            let (rust, walk, attr) = if req {
                (t.rust.clone(), t.walk.clone(), String::new())
            } else if t.defaults(self) {
                (t.rust.clone(), t.walk.clone(), "#[serde(default)]".into())
            } else {
                (format!("Option<{}>", t.rust), Walk::Each(Box::new(t.walk.clone())), "#[serde(default)]".into())
            };
            let mut attrs = Vec::new();
            if let Some(r) = rename {
                attrs.push(format!("rename = {r:?}"));
            }
            if !attr.is_empty() {
                attrs.push("default".into());
            }
            fields.push_str(&doc(pv, "    "));
            if !attrs.is_empty() {
                writeln!(fields, "    #[serde({})]", attrs.join(", ")).unwrap();
            }
            writeln!(fields, "    pub {id}: {rust},").unwrap();
            if let Some(w) = walker(&walk, &format!("self.{id}")) {
                walks.push_str(&w);
            }
        }
        let mut s = doc(v, "");
        // (Nothing in it required, and nothing in it that can't be empty: it can be empty.)
        let empty = required.is_empty() && !fields.contains("pub kind:") && self.all_default(&fields);
        let derive = if empty { "Clone, Debug, Default, PartialEq, Serialize, Deserialize" } else { "Clone, Debug, PartialEq, Serialize, Deserialize" };
        if empty {
            self.defaultable.insert(name.clone());
        }
        writeln!(s, "#[derive({derive})]\n#[serde(deny_unknown_fields)]\npub struct {name} {{\n{fields}}}\n").unwrap();
        writeln!(s, "impl Refs for {name} {{\n    #[allow(unused_variables)]\n    fn refs<'a>(&'a self, f: &mut dyn FnMut(&'a str, &'static [&'static str])) {{\n{walks}    }}\n}}\n").unwrap();
        self.out.push_str(&s);
        name
    }

    /// Whether every field (as written) can be empty.
    fn all_default(&self, fields: &str) -> bool {
        fields.lines().filter_map(|l| l.trim().strip_prefix("pub ")).all(|f| {
            let t = f.split_once(": ").map_or("", |(_, t)| t.trim_end_matches(','));
            t.starts_with("Option<") || t.starts_with("Vec<") || self.defaultable.contains(t) || matches!(t, "f64" | "i64" | "u32" | "bool" | "String") || t.starts_with('[')
        })
    }

    fn one_of(&mut self, hint: &str, alts: &[Value], file: &Path) -> Ty {
        let at = format!("{} ({hint})", file.display());
        let tagged = alts.iter().all(|a| a.get("properties").and_then(|p| p.get("kind")).and_then(|k| k.get("const")).is_some());
        let name = self.emit_name(hint);
        let mut s = String::new();
        let mut walks = String::new();
        let mut variants: Vec<(String, String, String, bool)> = Vec::new();
        if tagged {
            writeln!(s, "#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]\n#[serde(tag = \"kind\", deny_unknown_fields)]\npub enum {name} {{").unwrap();
            for a in alts {
                let kind = a["properties"]["kind"]["const"].as_str().unwrap_or_else(|| panic!("{at}: a kind that isn't text"));
                assert!(a.get("additionalProperties") == Some(&Value::Bool(false)), "{at}: {kind}: an open object");
                let variant = camel(kind);
                let not_made = a.get("x-in-game").and_then(Value::as_str) == Some("not made");
                let required: HashSet<&str> = a.get("required").and_then(Value::as_sequence).map(|r| r.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                // (Each kind its own struct, which the variant holds and its handler takes: a
                // figure added to a kind changes no handler.)
                let inner = self.emit_name(&format!("{name}{variant}"));
                s.push_str(&doc(a, "    "));
                writeln!(s, "    #[serde(rename = {kind:?})]\n    {variant}({inner}),").unwrap();
                let mut st = String::new();
                st.push_str(&doc(a, ""));
                writeln!(st, "#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]\n#[serde(deny_unknown_fields)]\npub struct {inner} {{").unwrap();
                let mut body = String::new();
                for (k, pv) in a["properties"].as_mapping().unwrap() {
                    let k = k.as_str().unwrap();
                    if k == "kind" {
                        continue;
                    }
                    let t = self.ty(&format!("{name}{variant}{}", camel(k)), pv, file);
                    let (id, rename) = field(k);
                    let (rust, walk) = if required.contains(k) || t.rust.starts_with("Vec<") || t.optional { (t.rust.clone(), t.walk.clone()) } else { (format!("Option<{}>", t.rust), Walk::Each(Box::new(t.walk.clone()))) };
                    st.push_str(&doc(pv, "    "));
                    let mut attrs = vec!["default".to_string()];
                    if required.contains(k) {
                        attrs.clear();
                    }
                    if let Some(r) = rename {
                        attrs.push(format!("rename = {r:?}"));
                    }
                    if !attrs.is_empty() {
                        writeln!(st, "    #[serde({})]", attrs.join(", ")).unwrap();
                    }
                    writeln!(st, "    pub {id}: {rust},").unwrap();
                    if let Some(w) = walk_at(&walk, &format!("self.{id}"), 0) {
                        body.push_str(&w);
                    }
                }
                st.push_str("}\n\n");
                writeln!(st, "impl Refs for {inner} {{\n    #[allow(unused_variables)]\n    fn refs<'a>(&'a self, f: &mut dyn FnMut(&'a str, &'static [&'static str])) {{\n{body}    }}\n}}\n").unwrap();
                self.out.push_str(&st);
                variants.push((kind.to_string(), variant.clone(), inner.clone(), not_made));
                writeln!(walks, "            {name}::{variant}(x) => x.refs(f),").unwrap();
            }
            s.push_str("}\n\n");
            s.push_str(&handler(&name, &variants));
        } else {
            // (Untagged: each shape tried in turn. A text and a list of texts, a constant and an object.)
            writeln!(s, "#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]\n#[serde(untagged)]\npub enum {name} {{").unwrap();
            for (i, a) in alts.iter().enumerate() {
                let t = self.ty(&format!("{name}{i}"), a, file);
                let one_field = a.get("properties").and_then(Value::as_mapping).filter(|p| p.len() == 1).and_then(|p| p.keys().next()).and_then(Value::as_str).map(camel);
                let variant = if a.get("const").is_some() {
                    "Constant".to_string()
                } else {
                    match t.rust.as_str() {
                        "String" => "Text".into(),
                        "f64" | "Degrees" => "Number".into(),
                        "i64" | "u32" => "Integer".into(),
                        "bool" => "Flag".into(),
                        r if r.starts_with("Vec<") => "List".into(),
                        r if r.starts_with('[') && r.ends_with("; 2]") => "Range".into(),
                        r if r.starts_with('[') => "Array".into(),
                        _ => one_field.unwrap_or_else(|| format!("Shape{i}")),
                    }
                };
                writeln!(s, "    {variant}({}),", t.rust).unwrap();
                if let Some(w) = walk_at(&t.walk, "x", 2) {
                    writeln!(walks, "            {name}::{variant}(x) => {{\n{w}            }}").unwrap();
                }
            }
            s.push_str("}\n\n");
        }
        let all = if walks.is_empty() { String::new() } else { format!("        match self {{\n{walks}            #[allow(unreachable_patterns)]\n            _ => {{}}\n        }}\n") };
        writeln!(s, "impl Refs for {name} {{\n    #[allow(unused_variables)]\n    fn refs<'a>(&'a self, f: &mut dyn FnMut(&'a str, &'static [&'static str])) {{\n{all}    }}\n}}\n").unwrap();
        self.out.push_str(&s);
        Ty { rust: name, walk: Walk::Typed, optional: false }
    }
}

/// A handler trait for a tagged enum: one method per kind, which the engine
/// implements (a kind added in the registry is a method the engine must
/// write, or the build stops); a kind marked `x-in-game: not made` defaults
/// to `not_made`. And `kind()`, its registry name, and `handle`, to dispatch.
fn handler(name: &str, variants: &[(String, String, String, bool)]) -> String {
    let mut t = format!("/// What the engine does with each kind of [`{name}`]: one method a kind, given\n/// that kind's figures.\npub trait {name}Handler {{\n    type Out;\n");
    if variants.iter().any(|v| v.3) {
        t.push_str("    /// A kind the game doesn't make yet.\n    fn not_made(&mut self, kind: &'static str) -> Self::Out;\n");
    }
    let mut arms = String::new();
    let mut kinds = String::new();
    for (kind, variant, inner, not_made) in variants {
        let method = snake(kind);
        if *not_made {
            writeln!(t, "    #[allow(unused_variables)]\n    fn {method}(&mut self, it: &{inner}) -> Self::Out {{\n        self.not_made({kind:?})\n    }}").unwrap();
        } else {
            writeln!(t, "    fn {method}(&mut self, it: &{inner}) -> Self::Out;").unwrap();
        }
        writeln!(arms, "            {name}::{variant}(x) => h.{method}(x),").unwrap();
        writeln!(kinds, "            {name}::{variant}(_) => {kind:?},").unwrap();
    }
    t.push_str("}\n\n");
    writeln!(t, "impl {name} {{\n    /// Its kind, as the registry names it.\n    pub fn kind(&self) -> &'static str {{\n        match self {{\n{kinds}        }}\n    }}\n\n    /// Hands it to `h`, by its kind.\n    pub fn handle<H: {name}Handler>(&self, h: &mut H) -> H::Out {{\n        match self {{\n{arms}        }}\n    }}\n}}\n").unwrap();
    t
}

/// The code that hands `expr`'s references to `f`, if it has any.
fn walker(w: &Walk, expr: &str) -> Option<String> {
    walk_at(w, expr, 0)
}

fn walk_at(w: &Walk, expr: &str, depth: usize) -> Option<String> {
    let pad = "    ".repeat(depth + 2);
    match w {
        Walk::None => None,
        Walk::Ref(kinds) => Some(format!("{pad}f({expr}.as_str(), &[{}]);\n", kinds.iter().map(|k| format!("{k:?}")).collect::<Vec<_>>().join(", "))),
        Walk::Typed => Some(format!("{pad}{expr}.refs(f);\n")),
        Walk::Each(inner) => {
            let x = format!("x{depth}");
            let body = walk_at(inner, &x, depth + 1)?;
            Some(format!("{pad}for {x} in {expr}.iter() {{\n{body}{pad}}}\n"))
        }
    }
}

/// A doc comment from a node's description.
fn doc(v: &Value, indent: &str) -> String {
    match v.get("description").and_then(Value::as_str) {
        Some(d) => d.lines().map(|l| format!("{indent}/// {}\n", l.trim())).collect(),
        None => String::new(),
    }
}

/// A Rust field name for property `k`, and the name to rename it from if it isn't the same.
fn field(k: &str) -> (&'static str, Option<String>) {
    let id = snake(k);
    let id = match id.as_str() {
        "type" | "use" | "where" | "yield" | "ref" | "mod" | "loop" | "match" | "move" | "self" | "static" | "struct" | "trait" | "in" | "as" | "fn" | "for" | "if" | "impl" | "let" | "mut" | "pub" | "return" | "box" | "crate" | "dyn" | "else" | "enum" | "extern" | "false" | "true" | "unsafe" | "while" | "async" | "await" | "const" | "continue" | "break" | "super" => format!("{id}_"),
        _ => id,
    };
    let rename = if id == k { None } else { Some(k.to_string()) };
    (Box::leak(id.into_boxed_str()), rename)
}

fn words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut w = String::new();
    let mut prev_lower = false;
    for c in s.chars() {
        if !c.is_alphanumeric() {
            if !w.is_empty() {
                out.push(std::mem::take(&mut w));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !w.is_empty() {
            out.push(std::mem::take(&mut w));
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        w.push(c);
    }
    if !w.is_empty() {
        out.push(w);
    }
    out
}

fn camel(s: &str) -> String {
    let mut out: String = words(s).iter().map(|w| { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()).unwrap_or_default() }).collect();
    if out.is_empty() {
        out = "Empty".into();
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'N');
    }
    out
}

fn snake(s: &str) -> String {
    // (A property all in capitals, as a star class's, stays as it is.)
    if s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
        return s.to_string();
    }
    words(s).iter().map(|w| w.to_lowercase()).collect::<Vec<_>>().join("_")
}

fn plural(s: &str) -> String {
    match s {
        "equipment" | "stock" | "seeding" | "dogma" => s.to_string(),
        _ if s.ends_with('y') && !s.ends_with("ey") => format!("{}ies", &s[..s.len() - 1]),
        _ if s.ends_with('s') || s.ends_with('x') || s.ends_with("ch") || s.ends_with("sh") => format!("{s}es"),
        _ => format!("{s}s"),
    }
}

fn singular(s: &str) -> String {
    if let Some(stem) = s.strip_suffix("ies") {
        format!("{stem}y")
    } else if s.ends_with("ss") || s.ends_with("sis") {
        s.to_string()
    } else if let Some(stem) = s.strip_suffix('s') {
        stem.to_string()
    } else {
        format!("{s}Item")
    }
}
