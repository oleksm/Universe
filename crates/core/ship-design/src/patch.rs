//! Conservative source bridge. Scalar edits preserve surrounding bytes; structural
//! edits retain comment-bearing source lines in an explicit canonical appendix.
use crate::{Document, Error, Limits, Result, SourceDocument};
use serde_json::Value;
use std::collections::BTreeSet;

const HEADER: &str = "# Source comments retained after structural YAML rewrite:";
const PREFIX: &str = "# Previous source: ";

/// Patch validated authoring data without silently losing comments.
///
/// Unchanged documents return exact source bytes. Single-line scalar replacements
/// preserve all other bytes. Unsupported edits use canonical YAML plus a labelled
/// comment appendix; inline comments retain their original source line as context.
/// This is not a full concrete-syntax tree: comment placement is retained only by
/// the scalar fast path. Every returned document is reparsed and compared to `new`.
pub fn patch(source: &SourceDocument, new: &Document) -> Result<String> {
    // Reparse rather than trusting public locations or a caller-mutated document.
    let original = crate::parse_source(&source.source)?;
    if original.document != source.document {
        return Err(Error::message(
            "",
            "source text and SourceDocument.document disagree",
        ));
    }
    let errors = crate::validate(new);
    if !errors.is_empty() {
        return Err(Error {
            diagnostics: errors,
        });
    }
    if new == &original.document {
        return Ok(source.source.clone());
    }
    let old_value =
        serde_json::to_value(&original.document).map_err(|e| Error::message("", e.to_string()))?;
    let new_value = serde_json::to_value(new).map_err(|e| Error::message("", e.to_string()))?;
    let mut changes = Vec::new();
    if scalar_changes(&old_value, &new_value, "", &mut changes) {
        let mut replacements = Vec::new();
        let mut supported = true;
        for (path, value) in changes {
            let Some(location) = original.locations.get(&path) else {
                supported = false;
                break;
            };
            let Some((start, end)) = scalar_span(&source.source, location.line, location.column)
            else {
                supported = false;
                break;
            };
            replacements.push((
                start,
                end,
                serde_json::to_string(&value).map_err(|e| Error::message(&path, e.to_string()))?,
            ));
        }
        replacements.sort_by_key(|r| r.0);
        if replacements.windows(2).any(|w| w[0].1 > w[1].0) {
            supported = false;
        }
        if supported {
            let mut text = source.source.clone();
            for (start, end, value) in replacements.into_iter().rev() {
                text.replace_range(start..end, &value);
            }
            if text.len() <= Limits::default().max_bytes
                && crate::parse(&text).is_ok_and(|d| d == *new)
            {
                return Ok(text);
            }
        }
    }
    let mut text = crate::emit(new)?;
    // Conservatively keep every line containing '#': this cannot omit a YAML
    // comment, and may additionally retain quoted/block-scalar hashes as context.
    // Previously archived lines remain unchanged, avoiding recursive prefixing.
    let mut preserved = Vec::new();
    for line in source.source.lines().filter(|line| line.contains('#')) {
        if line == HEADER {
            continue;
        }
        let line = if line.trim_start().starts_with('#') {
            line.to_owned()
        } else {
            format!("{PREFIX}{line}")
        };
        preserved.push(line);
    }
    if !preserved.is_empty() {
        text.push('\n');
        text.push_str(HEADER);
        text.push('\n');
        // Preserve multiplicity of existing comments. Avoid adding another copy
        // of a code-with-hash line that is already archived on a later rewrite.
        let archived: BTreeSet<_> = preserved
            .iter()
            .filter_map(|s| s.strip_prefix(PREFIX))
            .collect();
        let mut seen_context = BTreeSet::new();
        for line in &preserved {
            if let Some(context) = line.strip_prefix(PREFIX) {
                if archived.contains(context) && !seen_context.insert(context) {
                    continue;
                }
            }
            text.push_str(line);
            text.push('\n');
        }
    }
    if text.len() > Limits::default().max_bytes {
        return Err(Error::message(
            "",
            "patched source exceeds document byte limit; comments were not discarded",
        ));
    }
    let result = crate::parse(&text)?;
    if result != *new {
        return Err(Error::message(
            "",
            "patched source does not reproduce requested document",
        ));
    }
    Ok(text)
}
fn scalar_changes(old: &Value, new: &Value, path: &str, out: &mut Vec<(String, Value)>) -> bool {
    if old == new {
        return true;
    }
    match (old, new) {
        (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => {
            a.iter().all(|(key, value)| {
                scalar_changes(value, &b[key], &crate::yaml::pointer(path, key), out)
            })
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
            .iter()
            .zip(b)
            .enumerate()
            .all(|(i, (a, b))| scalar_changes(a, b, &format!("{path}/{i}"), out)),
        (a, b) if !a.is_object() && !a.is_array() && !b.is_object() && !b.is_array() => {
            out.push((path.into(), b.clone()));
            true
        }
        _ => false,
    }
}
/// Conservative single-line YAML scalar span. A final parse/equality check is
/// authoritative if plain scalar punctuation or continuation makes this ambiguous.
fn scalar_span(source: &str, line: usize, column: usize) -> Option<(usize, usize)> {
    let mut offset = 0;
    let row = source.split_inclusive('\n').nth(line.checked_sub(1)?)?;
    for s in source.split_inclusive('\n').take(line - 1) {
        offset += s.len();
    }
    let index = row
        .char_indices()
        .nth(column.checked_sub(1)?)
        .map(|p| p.0)?;
    let rest = &row[index..];
    let first = rest.chars().next()?;
    if matches!(first, '|' | '>' | '[' | '{' | '\n' | '\r') {
        return None;
    }
    if first == '\'' || first == '"' {
        let mut it = rest.char_indices().skip(1).peekable();
        while let Some((i, ch)) = it.next() {
            if first == '"' && ch == '\\' {
                it.next();
                continue;
            }
            if ch == first {
                if first == '\'' && it.peek().is_some_and(|p| p.1 == '\'') {
                    it.next();
                    continue;
                }
                return Some((offset + index, offset + index + i + ch.len_utf8()));
            }
            if ch == '\n' || ch == '\r' {
                return None;
            }
        }
        None
    } else {
        let mut end = rest.len();
        let mut previous = ' ';
        for (i, ch) in rest.char_indices() {
            if matches!(ch, '\n' | '\r' | ',' | ']' | '}') || ch == '#' && previous.is_whitespace()
            {
                end = i;
                break;
            }
            previous = ch;
        }
        let value = rest[..end].trim_end();
        if value.is_empty() {
            None
        } else {
            Some((offset + index, offset + index + value.len()))
        }
    }
}
