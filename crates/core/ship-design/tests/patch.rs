use universe_ship_design::{Document, emit, parse_source, patch};

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
