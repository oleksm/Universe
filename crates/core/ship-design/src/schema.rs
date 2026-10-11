use serde_json::{Value, json};
use std::sync::OnceLock;

/// Draft 2020-12 schema generated from the same types used by parse/emit, with
/// the v1 field constraints that cannot be expressed as plain Rust field types.
pub fn schema() -> Value {
    fn non_null(v: &mut Value) {
        if let Some(a) = v.get_mut("anyOf").and_then(Value::as_array_mut) {
            a.retain(|x| x.get("type") != Some(&json!("null")));
            if a.len() == 1 {
                let only = a.remove(0);
                v.as_object_mut().unwrap().remove("anyOf");
                v.as_object_mut()
                    .unwrap()
                    .extend(only.as_object().unwrap().clone());
            }
        }
        if let Some(a) = v.get_mut("type").and_then(Value::as_array_mut) {
            a.retain(|x| x != "null");
        }
    }
    fn visit(v: &mut Value) {
        match v {
            Value::Object(o) => {
                if o.get("type") == Some(&json!("object"))
                    && o.get("additionalProperties").is_some_and(Value::is_object)
                    && !o.contains_key("propertyNames")
                {
                    o.insert(
                        "propertyNames".into(),
                        json!({"pattern":"^[a-zA-Z][a-zA-Z0-9_.-]*$"}),
                    );
                }
                if let Some(props) = o.get_mut("properties").and_then(Value::as_object_mut) {
                    for (name, field) in props {
                        if name != "hull" {
                            non_null(field);
                        }
                        if name == "envelope_m" {
                            field["items"]["exclusiveMinimum"] = json!(0);
                        }
                    }
                }
                for child in o.values_mut() {
                    visit(child);
                }
            }
            Value::Array(a) => {
                for child in a {
                    visit(child);
                }
            }
            _ => {}
        }
    }
    let mut s =
        serde_json::to_value(schemars::schema_for!(crate::Document)).expect("schema serializes");
    visit(&mut s);
    s["$id"] = json!("urn:freefall:ship-code:1-draft");
    let hull = s["properties"]["hull"].take();
    s["properties"]["hull"] = json!({"anyOf":[hull,{"type":"null"}]});
    s["$defs"]["Deck"]["properties"]["floors"]["additionalProperties"]["minItems"] = json!(3);
    s["$defs"]["Group"]["properties"]["paths"]["items"]["minLength"] = json!(1);
    s["$defs"]["PathEnd"]["allOf"] = json!([{"if":{"properties":{"state":{"const":"door"}}},"then":{"required":["hatch"]},"else":{"not":{"required":["hatch"]}}}]);
    s
}

pub(crate) fn structural(value: &Value) -> Vec<crate::Diagnostic> {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    let validator = VALIDATOR.get_or_init(|| {
        jsonschema::validator_for(&schema()).expect("generated ship schema is valid")
    });
    validator
        .iter_errors(value)
        .take(256)
        .map(|e| crate::Diagnostic {
            path: e.instance_path.to_string(),
            message: e.to_string(),
            location: None,
        })
        .collect()
}
