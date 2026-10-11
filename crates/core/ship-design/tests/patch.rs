use universe_ship_design::{Document, emit, parse, parse_source, patch};

#[test]
fn unchanged_source_is_byte_exact_including_crlf_and_comments() {
    let source = format!(
        "# intro\r\n{}# final café\r\n",
        emit(&Document::new("test", "Ship"))
            .unwrap()
            .replace('\n', "\r\n")
    );
    let parsed = parse_source(&source).unwrap();
    assert_eq!(patch(&parsed, &parsed.document).unwrap(), source);
}
#[test]
fn scalar_edit_keeps_inline_comments_and_other_bytes() {
    let source = emit(&Document::new("test", "Old name"))
        .unwrap()
        .replace("\"Old name\"", "'Old name' # keep inline");
    let parsed = parse_source(&source).unwrap();
    let mut new = parsed.document.clone();
    new.design.name = "New # name: café".into();
    let patched = patch(&parsed, &new).unwrap();
    assert_eq!(
        patched,
        source.replace("'Old name'", "\"New # name: café\"")
    );
    assert_eq!(parse(&patched).unwrap(), new);
}
#[test]
fn vector_edit_preserves_flow_collection_and_comments() {
    let source = include_str!("fixtures/all-layers.ship.yaml");
    let old = parse_source(source).unwrap();
    let mut new = old.document.clone();
    match new.modules.get_mut("engine_left").unwrap() {
        universe_ship_design::Module::Equipment(e) => e.pose.position_m[0] = -3.25,
        _ => panic!(),
    }
    let patched = patch(&old, &new).unwrap();
    assert_eq!(patched, source.replace("[-2, 1, 3]", "[-3.25, 1, 3]"));
}
#[test]
fn structural_rewrite_retains_inline_full_line_and_block_hashes() {
    let source = emit(&Document::new("test", "Name # literal"))
        .unwrap()
        .replace("\"test\"", "\"test\" # identity rationale");
    let source = format!("# human heading\n{source}# human ending\n");
    let old = parse_source(&source).unwrap();
    let mut new = old.document.clone();
    new.design.notes = Some("Added field".into());
    let patched = patch(&old, &new).unwrap();
    assert_eq!(parse(&patched).unwrap(), new);
    for s in [
        "# human heading",
        "# human ending",
        "# identity rationale",
        "Name # literal",
        "Source comments retained",
    ] {
        assert!(patched.contains(s), "{s}");
    }
    // Another structural rewrite must not recursively prefix archived source.
    let old2 = parse_source(&patched).unwrap();
    new.design.notes = None;
    let again = patch(&old2, &new).unwrap();
    assert_eq!(parse(&again).unwrap(), new);
    assert!(!again.contains("# Previous source: # Previous source:"));
}
#[test]
fn multiline_scalar_falls_back_without_losing_comments() {
    let source = emit(&Document::new("test", "Ship"))
        .unwrap()
        .replace("\"Ship\"", "|\n    Ship # literal inside block");
    let source = format!("{source}# comment after block\n");
    let old = parse_source(&source).unwrap();
    let mut new = old.document.clone();
    new.design.name = "Renamed".into();
    let p = patch(&old, &new).unwrap();
    assert_eq!(parse(&p).unwrap(), new);
    assert!(p.contains("# literal inside block"));
    assert!(p.contains("# comment after block"));
}
#[test]
fn stale_source_or_invalid_new_document_is_rejected() {
    let mut old = parse_source(&emit(&Document::new("test", "Ship")).unwrap()).unwrap();
    let new = old.document.clone();
    old.document.design.name = "Stale".into();
    assert!(patch(&old, &new).is_err());
    let old = parse_source(&emit(&new).unwrap()).unwrap();
    let mut invalid = new;
    invalid.design.name = String::new();
    assert!(patch(&old, &invalid).is_err());
}
#[test]
fn quoted_scalar_with_escaped_quotes_and_comment_characters() {
    let mut doc = Document::new("test", "Quote \" # text");
    let source = emit(&doc).unwrap();
    let old = parse_source(&source).unwrap();
    doc.design.name = "Next".into();
    let p = patch(&old, &doc).unwrap();
    assert_eq!(parse(&p).unwrap(), doc);
    assert!(!p.contains("Source comments retained"));
}
