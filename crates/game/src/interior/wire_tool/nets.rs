//! Static contact topology, independent of model placement and electrical power.
//! Only saved wires and declared `joined` groups conduct here. Breakers and
//! converters remain devices for the runtime to resolve, never inferred joins.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use universe_ship_design::Wire;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Contact {
    pub module: String,
    pub port: String,
    pub contact: String,
}

#[derive(Debug)]
pub(super) struct Switch {
    pub module: String,
    pub group: usize,
    /// Separate poles, each containing only matching roles and lines. No
    /// connection is made until the runtime states which port breakers close.
    pub poles: Vec<Vec<Contact>>,
}

#[derive(Debug)]
pub(super) struct Conversion {
    pub module: String,
    pub group: usize,
    pub from: Vec<String>,
    pub to: Vec<String>,
}

#[derive(Debug)]
pub(super) struct Topology {
    /// Includes isolated contacts: a port existing is not evidence of a wire.
    pub nets: Vec<Vec<Contact>>,
    pub switches: Vec<Switch>,
    pub conversions: Vec<Conversion>,
}
impl Topology {
    pub fn pending(&self) -> Vec<String> {
        let mut out = Vec::new();
        for switch in &self.switches {
            out.push(format!(
                "SWITCH {}/{}: {} CONTACT POLES, BREAKER STATES NOT RESOLVED",
                switch.module,
                switch.group,
                switch.poles.len()
            ));
        }
        for converter in &self.conversions {
            out.push(format!(
                "CONVERTER {}/{}: {} INPUT / {} OUTPUT PORTS, POWER TRANSFER NOT RESOLVED",
                converter.module,
                converter.group,
                converter.from.len(),
                converter.to.len()
            ));
        }
        out
    }
}

type PortKey = (String, String);
type Signature = (String, String);

struct Graph {
    contacts: BTreeMap<Contact, (String, String, Option<f64>, Signature)>,
    parent: BTreeMap<Contact, Contact>,
}
impl Graph {
    fn root(&mut self, c: &Contact) -> Contact {
        let mut root = c.clone();
        while self.parent[&root] != root {
            root = self.parent[&root].clone();
        }
        let mut child = c.clone();
        while child != root {
            child = self.parent.insert(child, root.clone()).unwrap();
        }
        root
    }
    fn join(&mut self, a: &Contact, b: &Contact) {
        let (a, b) = (self.root(a), self.root(b));
        // Stable representatives make the result independent of wire order.
        if a < b {
            self.parent.insert(b, a);
        } else if b < a {
            self.parent.insert(a, b);
        }
    }
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn electrical(p: &Value) -> bool {
    matches!(s(p, "medium"), "power" | "signal")
}

/// Records are keyed by authored equipment instance, not catalogue product.
/// Errors withhold the entire topology. Cable/connector ratings, geometry,
/// source availability and load flow are separate checks, not certified here.
pub(super) fn compile(
    records: &BTreeMap<String, Value>,
    wires: &BTreeMap<String, Wire>,
) -> Result<Topology, Vec<String>> {
    let mut faults = Vec::new();
    let mut ports = BTreeMap::<PortKey, &Value>::new();
    let mut graph = Graph {
        contacts: BTreeMap::new(),
        parent: BTreeMap::new(),
    };
    for (module, record) in records {
        for p in record["ports"].as_array().into_iter().flatten() {
            let port = s(p, "id").to_string();
            if ports.insert((module.clone(), port.clone()), p).is_some() {
                faults.push(format!("PORT {module}/{port}: DUPLICATE DECLARATION"));
            }
            if !electrical(p) {
                continue;
            }
            let pins = p["pins"].as_array();
            if port.is_empty() || s(p, "channel").is_empty() || pins.is_none_or(Vec::is_empty) {
                faults.push(format!("PORT {module}/{port}: CHANNEL OR CONTACTS MISSING"));
            }
            if !matches!(s(p, "flow"), "in" | "out" | "both")
                || (s(p, "medium") == "power"
                    && !p["voltage"]
                        .as_f64()
                        .is_some_and(|v| v > 0.0 && v.is_finite()))
            {
                faults.push(format!(
                    "PORT {module}/{port}: DIRECTION OR VOLTAGE MISSING"
                ));
            }
            for pin in pins.into_iter().flatten() {
                if s(pin, "role") == "spare" {
                    continue;
                }
                let contact = Contact {
                    module: module.clone(),
                    port: port.clone(),
                    contact: s(pin, "contact").into(),
                };
                let info = (
                    s(p, "medium").into(),
                    s(p, "channel").into(),
                    p["voltage"].as_f64(),
                    (s(pin, "role").into(), s(pin, "line").into()),
                );
                if contact.contact.is_empty()
                    || s(pin, "role").is_empty()
                    || graph.contacts.insert(contact.clone(), info).is_some()
                {
                    faults.push(format!(
                        "PORT {module}/{port}: INVALID OR DUPLICATE CONTACT"
                    ));
                }
                graph.parent.insert(contact.clone(), contact);
            }
        }
    }
    let mut occupied = BTreeMap::<PortKey, &str>::new();
    for (id, w) in wires {
        let prefix = format!("WIRE {id}");
        let mut ends = Vec::new();
        for end in [&w.from, &w.to] {
            let key = (end.module.clone(), end.port.clone());
            if let Some(first) = occupied.insert(key.clone(), id) {
                faults.push(format!(
                    "{prefix}: PORT {}/{} ALREADY USED BY WIRE {first}",
                    key.0, key.1
                ));
            }
            match ports.get(&key).filter(|p| electrical(p)) {
                Some(p) => ends.push(*p),
                None => faults.push(format!(
                    "{prefix}: ELECTRICAL PORT {}/{} UNRESOLVED",
                    key.0, key.1
                )),
            }
        }
        if ends.len() != 2 {
            continue;
        }
        let (a, b) = (ends[0], ends[1]);
        let flow = (s(a, "flow"), s(b, "flow"));
        if s(a, "medium") != s(b, "medium")
            || s(a, "channel") != s(b, "channel")
            || s(a, "key") != s(b, "key")
            || flow == ("in", "in")
            || flow == ("out", "out")
            || (s(a, "medium") == "power" && a["voltage"].as_f64() != b["voltage"].as_f64())
        {
            faults.push(format!(
                "{prefix}: PORT SERVICES, KEY, VOLTAGE OR DIRECTION DIFFER"
            ));
            continue;
        }
        let mut assigned = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        if w.pins.is_empty() {
            faults.push(format!("{prefix}: CONTACT MAP EMPTY"));
        }
        for pin in &w.pins {
            let a = Contact {
                module: w.from.module.clone(),
                port: w.from.port.clone(),
                contact: pin.from.clone(),
            };
            let b = Contact {
                module: w.to.module.clone(),
                port: w.to.port.clone(),
                contact: pin.to.clone(),
            };
            if pin.conductor.is_empty()
                || !assigned.0.insert(&pin.conductor)
                || !assigned.1.insert(&pin.from)
                || !assigned.2.insert(&pin.to)
            {
                faults.push(format!(
                    "{prefix}: CONDUCTOR OR CONTACT ASSIGNED MORE THAN ONCE"
                ));
                continue;
            }
            match (graph.contacts.get(&a), graph.contacts.get(&b)) {
                (Some(x), Some(y)) if x == y => graph.join(&a, &b),
                _ => faults.push(format!(
                    "{prefix}: CONTACT {}/{} UNRESOLVED OR ROLES/LINES DIFFER",
                    pin.from, pin.to
                )),
            }
        }
        for (end, assigned) in [(&w.from, &assigned.1), (&w.to, &assigned.2)] {
            if graph.contacts.keys().any(|c| {
                c.module == end.module && c.port == end.port && !assigned.contains(&c.contact)
            }) {
                faults.push(format!(
                    "{prefix}: PORT {}/{} HAS UNASSIGNED CONTACTS",
                    end.module, end.port
                ));
            }
        }
    }
    let mut switches = Vec::new();
    let mut conversions = Vec::new();
    for (module, record) in records {
        for (group, link) in record["internal"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let prefix = format!("INTERNAL {module}/{group}");
            match s(link, "kind") {
                "joined" | "switched" => {
                    let ids = port_ids(&ports, module, link, "ports", &prefix, &mut faults);
                    let mut poles = BTreeMap::<Signature, Vec<Contact>>::new();
                    for (c, (_, _, _, signature)) in &graph.contacts {
                        if c.module == *module && ids.contains(&c.port) {
                            poles.entry(signature.clone()).or_default().push(c.clone());
                        }
                    }
                    for pole in poles.values() {
                        let Some(first) = pole.first() else { continue };
                        for c in &pole[1..] {
                            if graph.contacts[first] != graph.contacts[c] {
                                faults.push(format!("{prefix}: JOIN CROSSES SERVICES OR CHANNELS"));
                            } else if s(link, "kind") == "joined" {
                                graph.join(first, c);
                            }
                        }
                    }
                    if s(link, "kind") == "switched" {
                        switches.push(Switch {
                            module: module.clone(),
                            group,
                            poles: poles.into_values().collect(),
                        });
                    }
                }
                "converts" => {
                    let from = port_ids(&ports, module, link, "from", &prefix, &mut faults);
                    let to = port_ids(&ports, module, link, "to", &prefix, &mut faults);
                    if from.iter().any(|p| to.contains(p)) {
                        faults.push(format!(
                            "{prefix}: CONVERSION INPUT ALSO DECLARED AS OUTPUT"
                        ));
                    }
                    conversions.push(Conversion {
                        module: module.clone(),
                        group,
                        from,
                        to,
                    });
                }
                _ => faults.push(format!("{prefix}: UNKNOWN CONNECTION KIND")),
            }
        }
    }
    if !faults.is_empty() {
        return Err(faults);
    }
    let mut nets = BTreeMap::<Contact, Vec<Contact>>::new();
    for c in graph.contacts.keys().cloned().collect::<Vec<_>>() {
        nets.entry(graph.root(&c)).or_default().push(c);
    }
    Ok(Topology {
        nets: nets.into_values().collect(),
        switches,
        conversions,
    })
}

fn port_ids(
    ports: &BTreeMap<PortKey, &Value>,
    module: &str,
    link: &Value,
    field: &str,
    prefix: &str,
    faults: &mut Vec<String>,
) -> Vec<String> {
    let mut ids = Vec::new();
    for p in link[field].as_array().into_iter().flatten() {
        let id = p.as_str().unwrap_or("").to_string();
        if ids.contains(&id)
            || !ports
                .get(&(module.into(), id.clone()))
                .is_some_and(|p| electrical(p))
        {
            faults.push(format!(
                "{prefix}: {field} PORT {id} DUPLICATE OR UNRESOLVED"
            ));
        }
        ids.push(id);
    }
    if ids.is_empty() {
        faults.push(format!("{prefix}: {field} PORTS MISSING"));
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use universe_ship_design::{WireEnd, WirePin};

    fn port(id: &str, flow: &str) -> Value {
        json!({"id":id,"medium":"signal","channel":"can-a","flow":flow,
            "pins":[{"contact":"1","role":"signal-high"},
                {"contact":"2","role":"signal-low"},{"contact":"3","role":"spare"}]})
    }
    fn wire(from: (&str, &str), to: (&str, &str)) -> Wire {
        let end = |(module, port): (&str, &str)| WireEnd {
            module: module.into(),
            port: port.into(),
            connector: "connector.test".into(),
        };
        Wire {
            from: end(from),
            to: end(to),
            cable: "run.test".into(),
            waypoints_m: vec![],
            pins: ["1", "2"]
                .map(|c| WirePin {
                    conductor: c.into(),
                    from: c.into(),
                    to: c.into(),
                })
                .into(),
        }
    }
    fn records() -> BTreeMap<String, Value> {
        [
            ("source".into(), json!({"ports":[port("data", "both")]})),
            (
                "board".into(),
                json!({"ports":[port("a", "both"),port("b", "both")],
                "internal":[{"kind":"joined","ports":["a","b"]}]}),
            ),
            ("load".into(), json!({"ports":[port("data", "both")]})),
        ]
        .into()
    }
    fn wires() -> BTreeMap<String, Wire> {
        [
            ("first".into(), wire(("source", "data"), ("board", "a"))),
            ("second".into(), wire(("board", "b"), ("load", "data"))),
        ]
        .into()
    }
    fn connected(t: &Topology, a: (&str, &str, &str), b: (&str, &str, &str)) -> bool {
        let contact = |(module, port, contact): (&str, &str, &str)| Contact {
            module: module.into(),
            port: port.into(),
            contact: contact.into(),
        };
        t.nets
            .iter()
            .any(|n| n.contains(&contact(a)) && n.contains(&contact(b)))
    }

    #[test]
    fn joins_only_authored_contacts_and_declared_internal_bars() {
        let mut r = records();
        let w = wires();
        let t = compile(&r, &w).unwrap();
        assert_eq!(t.nets.len(), 2);
        assert!(connected(
            &t,
            ("source", "data", "1"),
            ("load", "data", "1")
        ));
        assert!(!connected(
            &t,
            ("source", "data", "1"),
            ("load", "data", "2")
        ));
        assert!(t.nets.iter().all(|n| n.len() == 4));
        let reordered = [
            ("a".into(), w["second"].clone()),
            ("z".into(), w["first"].clone()),
        ]
        .into();
        assert_eq!(t.nets, compile(&r, &reordered).unwrap().nets);
        r.get_mut("board").unwrap()["internal"] = json!([]);
        let t = compile(&r, &w).unwrap();
        assert_eq!(t.nets.len(), 4);
        assert!(!connected(
            &t,
            ("source", "data", "1"),
            ("load", "data", "1")
        ));
        let t = compile(&r, &BTreeMap::new()).unwrap();
        assert_eq!(
            t.nets.len(),
            8,
            "unwired contacts stay isolated; spare contacts are excluded"
        );
    }

    #[test]
    fn switches_and_converters_are_devices_not_copper_or_sources() {
        let mut r = records();
        r.get_mut("board").unwrap()["internal"] = json!([
            {"kind":"switched","ports":["a","b"]},
            {"kind":"converts","from":["a"],"to":["b"]}]);
        let t = compile(&r, &wires()).unwrap();
        assert_eq!(t.nets.len(), 4);
        assert!(!connected(
            &t,
            ("source", "data", "1"),
            ("load", "data", "1")
        ));
        assert_eq!(t.switches[0].module, "board");
        assert_eq!(t.switches[0].poles.len(), 2);
        assert_eq!(t.switches[0].poles[0].len(), 2);
        assert_eq!(t.conversions[0].from, ["a"]);
        assert_eq!(t.conversions[0].to, ["b"]);
        assert_eq!(t.pending().len(), 2);
    }

    #[test]
    fn invalid_saved_links_withhold_the_topology() {
        let r = records();
        let w = wires();
        let mut bad = w.clone();
        bad.insert("duplicate".into(), bad["first"].clone());
        assert!(
            compile(&r, &bad)
                .unwrap_err()
                .iter()
                .any(|f| f.contains("ALREADY USED"))
        );
        let mut bad = w.clone();
        bad.get_mut("first").unwrap().pins[0].to = "2".into();
        assert!(
            compile(&r, &bad).is_err(),
            "polarity/role reversal is no net"
        );
        let mut bad = w.clone();
        bad.get_mut("first").unwrap().pins.pop();
        assert!(
            compile(&r, &bad)
                .unwrap_err()
                .iter()
                .any(|f| f.contains("UNASSIGNED"))
        );
        let mut bad = w.clone();
        bad.get_mut("first").unwrap().to.port = "absent".into();
        assert!(
            compile(&r, &bad)
                .unwrap_err()
                .iter()
                .any(|f| f.contains("UNRESOLVED"))
        );
        let mut bad = r.clone();
        bad.get_mut("board").unwrap()["ports"][1]["channel"] = json!("can-b");
        assert!(
            compile(&bad, &BTreeMap::new())
                .unwrap_err()
                .iter()
                .any(|f| f.contains("CROSSES"))
        );
        let mut bad = r;
        bad.get_mut("board").unwrap()["internal"][0]["ports"] = json!(["a", "absent"]);
        assert!(
            compile(&bad, &w)
                .unwrap_err()
                .iter()
                .any(|f| f.contains("UNRESOLVED"))
        );
    }

    #[test]
    fn multi_line_contacts_never_join_other_lines() {
        let mut r = records();
        for p in r.get_mut("board").unwrap()["ports"].as_array_mut().unwrap() {
            p["pins"] = json!([{"contact":"1","role":"signal","line":"tx"},
                {"contact":"2","role":"signal","line":"rx"}]);
        }
        let t = compile(&r, &BTreeMap::new()).unwrap();
        assert!(connected(&t, ("board", "a", "1"), ("board", "b", "1")));
        assert!(!connected(&t, ("board", "a", "1"), ("board", "b", "2")));
        let mut bad = wires();
        bad.get_mut("second").unwrap().pins[0].conductor = "2".into();
        assert!(compile(&r, &bad).is_err());
    }

    #[test]
    fn power_nets_separate_polarity_and_reject_voltage_and_flow_mismatches() {
        let mut r = records();
        for record in r.values_mut() {
            for p in record["ports"].as_array_mut().unwrap() {
                p["medium"] = json!("power");
                p["channel"] = json!("dc-28v");
                p["voltage"] = json!(28.0);
                p["pins"] = json!([{"contact":"1","role":"plus"},{"contact":"2","role":"minus"}]);
            }
        }
        let w = wires();
        let t = compile(&r, &w).unwrap();
        assert_eq!(t.nets.len(), 2);
        assert!(connected(
            &t,
            ("source", "data", "1"),
            ("load", "data", "1")
        ));
        assert!(!connected(
            &t,
            ("source", "data", "1"),
            ("load", "data", "2")
        ));
        let mut bad = r.clone();
        bad.get_mut("board").unwrap()["ports"][1]["voltage"] = json!(270.0);
        assert!(
            compile(&bad, &BTreeMap::new()).is_err(),
            "an internal bar cannot bridge voltages"
        );
        bad = r.clone();
        bad.get_mut("load").unwrap()["ports"][0]["voltage"] = json!(270.0);
        assert!(
            compile(&bad, &w).is_err(),
            "a channel label cannot hide a voltage mismatch"
        );
        bad = r;
        bad.get_mut("source").unwrap()["ports"][0]["flow"] = json!("out");
        bad.get_mut("board").unwrap()["ports"][0]["flow"] = json!("out");
        assert!(
            compile(&bad, &w).is_err(),
            "two output-only ports cannot mate"
        );
    }

    #[test]
    fn published_electrical_internal_connections_resolve_without_model_files() {
        let reg = universe_sim::world::registry::registry();
        let mut checked = 0;
        for equipment in &reg.equipment {
            let record = serde_json::to_value(equipment).unwrap();
            if record["internal"].as_array().is_none_or(Vec::is_empty) {
                continue;
            }
            let key = equipment.identity.key.clone();
            let records = [(key.clone(), record)].into();
            let t = compile(&records, &BTreeMap::new()).unwrap_or_else(|f| panic!("{key}: {f:?}"));
            assert!(!t.nets.is_empty(), "{key}");
            checked += 1;
        }
        assert!(checked > 0);
    }
}
