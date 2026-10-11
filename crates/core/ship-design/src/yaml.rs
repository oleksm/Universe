use crate::{Diagnostic, Error, Limits, Location};
use serde_json::{Map, Number, Value};
use std::collections::BTreeMap;
use yaml_rust2::{
    parser::{Event, Parser},
    scanner::{Marker, Scanner, TScalarStyle, TokenType},
};

pub(crate) fn pointer(parent: &str, key: &str) -> String {
    format!("{parent}/{}", key.replace('~', "~0").replace('/', "~1"))
}
fn location(m: Marker) -> Location {
    Location {
        line: m.line(),
        column: m.col() + 1,
    }
}
fn fail(path: &str, m: Marker, message: impl Into<String>) -> Error {
    Error {
        diagnostics: vec![Diagnostic {
            path: path.into(),
            message: message.into(),
            location: Some(location(m)),
        }],
    }
}
fn scan_error(e: yaml_rust2::scanner::ScanError) -> Error {
    fail("", *e.marker(), e.to_string())
}

struct Reader<'a> {
    parser: Parser<std::str::Chars<'a>>,
    locations: BTreeMap<String, Location>,
    nodes: usize,
    limits: Limits,
}
impl Reader<'_> {
    fn next(&mut self) -> Result<(Event, Marker), Error> {
        self.parser.next_token().map_err(scan_error)
    }
    fn node(
        &mut self,
        event: Event,
        mark: Marker,
        path: &str,
        depth: usize,
    ) -> Result<Value, Error> {
        self.nodes += 1;
        if depth > self.limits.max_depth || self.nodes > self.limits.max_nodes {
            return Err(fail(path, mark, "document nesting or node limit exceeded"));
        }
        self.locations.insert(path.into(), location(mark));
        match event {
            Event::Scalar(text, style, anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err(fail(
                        path,
                        mark,
                        "anchors and explicit tags are outside ship YAML profile v1",
                    ));
                }
                if text.len() > self.limits.max_scalar_bytes {
                    return Err(fail(path, mark, "scalar byte limit exceeded"));
                }
                scalar(&text, style == TScalarStyle::Plain).map_err(|s| fail(path, mark, s))
            }
            Event::MappingStart(anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err(fail(
                        path,
                        mark,
                        "anchors and explicit tags are outside ship YAML profile v1",
                    ));
                }
                let mut values = Map::new();
                loop {
                    let (event, m) = self.next()?;
                    if event == Event::MappingEnd {
                        break;
                    }
                    let key = match self.node(event, m, path, depth + 1)? {
                        Value::String(k) => k,
                        _ => return Err(fail(path, m, "mapping keys must be strings")),
                    };
                    self.locations.insert(path.into(), location(mark));
                    let child = pointer(path, &key);
                    if key == "<<" {
                        return Err(fail(
                            &child,
                            m,
                            "merge keys are outside ship YAML profile v1",
                        ));
                    }
                    if values.contains_key(&key) {
                        return Err(fail(&child, m, "duplicate mapping key"));
                    }
                    let (e, loc) = self.next()?;
                    values.insert(key, self.node(e, loc, &child, depth + 1)?);
                }
                Ok(Value::Object(values))
            }
            Event::SequenceStart(anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err(fail(
                        path,
                        mark,
                        "anchors and explicit tags are outside ship YAML profile v1",
                    ));
                }
                let mut values = Vec::new();
                loop {
                    let (e, m) = self.next()?;
                    if e == Event::SequenceEnd {
                        break;
                    }
                    let child = pointer(path, &values.len().to_string());
                    values.push(self.node(e, m, &child, depth + 1)?);
                }
                Ok(Value::Array(values))
            }
            Event::Alias(_) => Err(fail(path, mark, "aliases are outside ship YAML profile v1")),
            _ => Err(fail(path, mark, "expected a YAML value")),
        }
    }
}

// Versioned YAML 1.2 core scalars. No YAML 1.1 yes/no/on/off coercion.
fn scalar(s: &str, plain: bool) -> Result<Value, &'static str> {
    if !plain {
        return Ok(Value::String(s.into()));
    }
    match s {
        "" | "~" | "null" | "Null" | "NULL" => return Ok(Value::Null),
        "true" | "True" | "TRUE" => return Ok(Value::Bool(true)),
        "false" | "False" | "FALSE" => return Ok(Value::Bool(false)),
        ".nan" | ".NaN" | ".NAN" | ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF"
        | "-.inf" | "-.Inf" | "-.INF" => return Err("numbers must be finite"),
        _ => {}
    }
    let unsigned = s
        .strip_prefix('+')
        .or_else(|| s.strip_prefix('-'))
        .unwrap_or(s);
    let radix = if unsigned.starts_with("0x") {
        16
    } else if unsigned.starts_with("0o") {
        8
    } else {
        10
    };
    let digits = if radix == 10 {
        unsigned
    } else {
        &unsigned[2..]
    };
    if (radix == 10 || unsigned == s)
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_digit(radix))
    {
        let magnitude = u64::from_str_radix(digits, radix)
            .map_err(|_| "integer outside supported 64-bit range")?;
        let n = if s.starts_with('-') {
            Number::from(
                i64::try_from(-(magnitude as i128))
                    .map_err(|_| "integer outside supported 64-bit range")?,
            )
        } else {
            Number::from(magnitude)
        };
        return Ok(Value::Number(n));
    }
    if s.chars().any(|c| c.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e' | 'E'))
    {
        if let Ok(v) = s.parse::<f64>() {
            return Number::from_f64(v)
                .map(Value::Number)
                .ok_or("numbers must be finite");
        }
    }
    Ok(Value::String(s.into()))
}

pub(crate) fn read(
    source: &str,
    limits: Limits,
) -> Result<(Value, BTreeMap<String, Location>), Error> {
    if source.len() > limits.max_bytes {
        return Err(Error::message("", "document byte limit exceeded"));
    }
    // Check directives using tokens, not a regex that would mistake quoted text/comments.
    let mut scanner = Scanner::new(source.chars());
    while let Some(token) = scanner.next_token().map_err(scan_error)? {
        match token.1 {
            TokenType::VersionDirective(1, 2) => {}
            TokenType::VersionDirective(..) => {
                return Err(fail("", token.0, "only YAML 1.2 directives are supported"));
            }
            TokenType::TagDirective(..) => {
                return Err(fail(
                    "",
                    token.0,
                    "tag directives are outside ship YAML profile v1",
                ));
            }
            _ => {}
        }
    }
    let mut r = Reader {
        parser: Parser::new(source.chars()),
        locations: BTreeMap::new(),
        nodes: 0,
        limits,
    };
    let (e, m) = r.next()?;
    if e != Event::StreamStart {
        return Err(fail("", m, "expected YAML stream"));
    }
    let (e, m) = r.next()?;
    if e != Event::DocumentStart {
        return Err(fail("", m, "expected exactly one YAML document"));
    }
    let (e, m) = r.next()?;
    let value = r.node(e, m, "", 0)?;
    let (e, m) = r.next()?;
    if e != Event::DocumentEnd {
        return Err(fail("", m, "expected document end"));
    }
    let (e, m) = r.next()?;
    if e != Event::StreamEnd {
        return Err(fail("", m, "expected exactly one YAML document"));
    }
    Ok((value, r.locations))
}

pub(crate) fn write(v: &Value) -> Result<String, Error> {
    fn convert(v: &Value) -> yaml_rust2::Yaml {
        use yaml_rust2::Yaml as Y;
        match v {
            Value::Null => Y::Null,
            Value::Bool(v) => Y::Boolean(*v),
            Value::String(v) => Y::String(v.clone()),
            Value::Number(n) => n
                .as_i64()
                .map(Y::Integer)
                .unwrap_or_else(|| Y::Real(n.to_string())),
            Value::Array(a) => Y::Array(a.iter().map(convert).collect()),
            Value::Object(o) => Y::Hash(
                o.iter()
                    .map(|(k, v)| (Y::String(k.clone()), convert(v)))
                    .collect(),
            ),
        }
    }
    let mut out = String::from("%YAML 1.2\n");
    yaml_rust2::YamlEmitter::new(&mut out)
        .dump(&convert(v))
        .map_err(|e| Error::message("", e.to_string()))?;
    out.push('\n');
    Ok(out)
}
