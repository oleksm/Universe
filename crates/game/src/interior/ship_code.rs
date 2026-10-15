//! Boundary between the versioned ship document and the legacy Studio projection.
//! Never imports registry facts or derived simulation results from a document.
use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use universe_ship_design as code;
use universe_sim::world::deckplan as deck;

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Authored {
    document: code::Document,
    names: BTreeMap<Id, String>,
    #[serde(default)]
    source: String,
    #[serde(default)]
    baseline: Value,
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(from = "PadRepr")]
pub(super) struct Pad {
    pub at: Vec3,
    pub uid: Id,
}
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum PadRepr {
    Named {
        at: Vec3,
        #[serde(default)]
        uid: Id,
    },
    Legacy(Vec3),
}
impl From<PadRepr> for Pad {
    fn from(v: PadRepr) -> Self {
        match v {
            PadRepr::Named { at, uid } => Self { at, uid },
            PadRepr::Legacy(at) => Self {
                at,
                uid: Id::default(),
            },
        }
    }
}

const LAYER_KEYS: [&str; 26] = [
    "hull",
    "grid_measures",
    "points",
    "points.entry",
    "points.dash",
    "points.windows",
    "points.service",
    "points.mining",
    "points.mounts_engines",
    "points.laid",
    "points.names",
    "access",
    "access.lines",
    "access.outlines",
    "access.walls",
    "access.hatches",
    "clashes",
    "decks_2d",
    "decks_2d.floors",
    "decks_2d.walls",
    "hollow_map",
    "reach",
    "modules",
    "frame",
    "pressure",
    "decks",
];
fn id(prefix: &str, n: usize) -> String {
    format!("{prefix}_{:06}", n + 1)
}
fn v3(p: [f64; 3]) -> Vec3 {
    Vec3::from_array(p.map(|x| x as f32))
}
fn v2(p: [f64; 2]) -> Vec2 {
    Vec2::from_array(p.map(|x| x as f32))
}
fn value<T: serde::Serialize>(v: T) -> Value {
    serde_json::to_value(v).expect("finite validated authoring value")
}
fn object<T>(items: impl Iterator<Item = (String, T)>, f: impl Fn(T) -> Value) -> Value {
    Value::Object(items.map(|(k, v)| (k, f(v))).collect())
}
fn shape(s: Section) -> &'static str {
    match s {
        Section::Line => "line",
        Section::Round => "round",
        Section::Square => "square",
        Section::Hex => "hex",
        Section::Oct => "oct",
    }
}
fn profile(p: Profile) -> Value {
    json!({"shape":shape(p.section),"width_m":p.width,"height_m":p.height,"placement":if p.stand {"floor"}else{"center"}})
}
fn hatch(h: Hatch) -> Value {
    json!({"shape":shape(h.section),"width_m":h.width,"height_m":h.height,"slide":match h.slide {Slide::None=>"none",Slide::Up=>"up",Slide::Side=>"side"}})
}
fn section(s: code::ProfileShape) -> Section {
    match s {
        code::ProfileShape::Line => Section::Line,
        code::ProfileShape::Round => Section::Round,
        code::ProfileShape::Square => Section::Square,
        code::ProfileShape::Hex => Section::Hex,
        code::ProfileShape::Oct => Section::Oct,
    }
}
fn read_hatch(h: &code::Hatch) -> Hatch {
    Hatch {
        section: match h.shape {
            code::HatchShape::Round => Section::Round,
            code::HatchShape::Square => Section::Square,
            code::HatchShape::Hex => Section::Hex,
            code::HatchShape::Oct => Section::Oct,
        },
        width: h.width_m as f32,
        height: h.height_m as f32,
        slide: match h.slide {
            code::HatchSlide::None => Slide::None,
            code::HatchSlide::Up => Slide::Up,
            code::HatchSlide::Side => Slide::Side,
        },
    }
}

fn deck_value(d: &deck::Deck, n: usize, plan: &deck::DeckPlan) -> Value {
    let target = |t: Option<Id>| {
        id(
            "deck",
            t.and_then(|id| plan.decks.iter().position(|d| d.uid == id))
                .unwrap_or(n + 1),
        )
    };
    json!({"floor_m":d.floor,"headroom_m":d.headroom,
        "floors":object(d.planes.iter().enumerate().map(|(n,p)|(id("floor",n),p)), |p|value(&p.outline)),
        "walls":object(d.walls.iter().enumerate().map(|(n,w)|(id("wall",n),w)), |w|json!({"points_xz_m":w.points,"bulges_m":(0..w.points.len().saturating_sub(1)).map(|n|w.bulges.get(n).copied().unwrap_or(0.)).collect::<Vec<_>>(),"rail":w.rail,"doors":object(w.doors.iter().enumerate().map(|(n,d)|(id("door",n),d)),|d|json!({"distance_m":d.at,"width_m":d.width,"height_m":d.height}))})),
        "ladders":object(d.ladders.iter().enumerate().map(|(n,l)|(id("ladder",n),l)),|l|json!({"at_xz_m":l.at,"to_deck":target(l.target)})),
        "stairs":object(d.stairs.iter().enumerate().map(|(n,s)|(id("stair",n),s)),|s|json!({"from_xz_m":s.from,"to_xz_m":s.to,"width_m":s.width,"to_deck":target(s.target)}))})
}

/// Export every authored collection, regardless of display visibility.
fn generated_raw(i: &Interior) -> Result<code::Document, String> {
    let p = &i.plan;
    let hull_key = p.on.as_deref().unwrap_or(&p.hull);
    if !hull_key.is_empty() && i.spec().is_none() {
        return Err(format!(
            "unresolved legacy hull {hull_key}; open its installed model before export"
        ));
    }
    let points = object(
        p.points
            .iter()
            .enumerate()
            .map(|(n, p)| (id("point", n), p)),
        |p| {
            let mut v = json!({"position_m":p.at});
            if let Some(name) = &p.name {
                v["name"] = value(name)
            }
            v
        },
    );
    let modules: Result<BTreeMap<_,_>,String> = p.blocks.iter().enumerate().map(|(n,b)| {
        let f=i.fit.iter().find(|f| f.id==kind(&b.id));
        let q=b.rotation.unwrap_or_else(||crate::equipment_visual::rotation(f.and_then(|f|push_of(b,f).map(|v|v.0))));
        let mut v=json!({"kind":"hull_hold","pose":{"position_m":b.at,"rotation_xyzw":q},"envelope_m":b.size});
        if b.id!="HOLD" {
            let f=f.ok_or_else(||format!("module {} has no registry record; cannot export it as equipment",b.id))?;
            v["kind"]=json!("equipment");v["record"]=value(&f.key);
            if i.spec().is_some() {v["slot"]=value(kind(&b.id));}
        }
        Ok((id("module",n),v))
    }).collect();
    let decks = i.decks_now.as_ref();
    let mut root = json!({"format":"freefall.ship","version":"1-draft","design":{"id":p.hull,"name":p.hull},
        "coordinates":"right-handed-x-right-y-up-z-aft-metres","placement_convention":"studio-centered-v1",
        "hull":match i.spec() { Some(h) => Some(json!({"record":universe_sim::world::registry::registry().hulls.iter().find(|r|h.visual.as_deref().map_or(r.identity.key==h.key, |v|r.model.as_deref()==Some(v))).ok_or("hull has no registry identity")?.identity.key})), None => None },
        "modules":modules?,"access":{"points":points,
            "paths":object(p.lines.iter().enumerate().map(|(n,l)|(id("path",n),l)),|&PathLine(a,b,pr,_)|json!({"from":id("point",a),"to":id("point",b),"profile":profile(pr)})),
            "groups":object(p.groups.iter().enumerate().map(|(n,g)|(id("group",n),g)),|g|json!({"paths":g.lines.iter().map(|&n|id("path",n)).collect::<Vec<_>>(),"walled":g.walled})),
            "side_doors":object(p.doors.iter().enumerate().map(|(n,d)|(id("door",n),d)),|d|json!({"path":id("path",d.0),"fraction":d.1,"side":if d.2 {"right"}else{"left"},"hatch":hatch(d.3)})),
            "ends":object(p.ends.iter().enumerate().map(|(n,e)|(id("end",n),e)),|e|{let mut v=json!({"path":id("path",e.0),"end":if e.1==0 {"from"}else{"to"},"state":match e.2 {End::Closed=>"closed",End::Open=>"open",End::Door=>"door"}});if e.2==End::Door {v["hatch"]=hatch(e.3)}v})},
        "structure":{
            "members":object(p.beams.iter().enumerate().map(|(n,b)|(id("beam",n),b)),|b|json!({"from_m":b.a,"to_m":b.b,"stock":b.stock,"pinned":b.pinned})),
            "plates":object(p.plates.iter().enumerate().map(|(n,p)|(id("plate",n),p)),|p|json!({"min_xz_m":p.lo,"max_xz_m":p.hi,"height_m":p.y,"stock":p.stock})),
            "landing_pads":object(p.pads.iter().enumerate().map(|(n,p)|(id("pad",n),p)),|p|value(p.at))},
        "decks":object(decks.into_iter().flat_map(|d|d.decks.iter()).enumerate().map(|(n,d)|(id("deck",n),(n,d))),|(n,d)|deck_value(d,n,decks.unwrap())),
        "view":{"layers":LAYER_KEYS.iter().enumerate().map(|(n,k)|(k.to_string(),!i.hidden[n])).collect::<BTreeMap<_,_>>()}});
    let mut requirements = serde_json::Map::new();
    if let Some(g) = p.gravity {
        requirements.insert("gravity_m_s2".into(), value(g));
    }
    if let Some(t) = &p.targets {
        for (k, v) in [
            ("delta_v_m_s", t.delta_v),
            ("cargo_kg", t.cargo),
            ("crew", t.crew),
            ("trip_s", t.trip.map(|h| h * 3600.)),
        ] {
            if let Some(v) = v {
                requirements.insert(k.into(), value(v));
            }
        }
    }
    if !requirements.is_empty() {
        root["requirements"] = Value::Object(requirements);
    }
    if let Some(y) = i.plane {
        root["view"]["work_plane_m"] = value(y);
    }
    if let (Some(target), Some(distance)) = (i.target, i.distance) {
        root["view"]["camera"] =
            json!({"target_m":target,"yaw_rad":i.yaw,"pitch_rad":i.pitch,"distance_m":distance});
    }
    restore_names(&mut root, &i.plan, decks);
    let doc: code::Document = serde_json::from_value(root).map_err(|e| e.to_string())?;
    let problems = code::validate(&doc);
    if !problems.is_empty() {
        return Err(format!("design cannot be represented yet: {problems:?}"));
    }
    Ok(doc)
}

fn projection(doc: &code::Document) -> Result<(Plan, deck::DeckPlan), String> {
    if !safe_design_id(&doc.design.id) {
        return Err("design ID must be a name, not a filesystem path".into());
    }
    let problems = code::validate(doc);
    if !problems.is_empty() {
        return Err(format!("invalid ship document: {problems:?}"));
    }
    // All current 3D editor coordinates are f32. Reject overflow before mutation.
    fn range(v: &Value) -> bool {
        match v {
            Value::Number(n) => n.as_f64().is_none_or(|x| (x as f32).is_finite()),
            Value::Array(a) => a.iter().all(range),
            Value::Object(o) => o.values().all(range),
            _ => true,
        }
    }
    if !range(&value(doc)) {
        return Err("a value exceeds Studio's finite coordinate range".into());
    }
    if doc
        .dependencies
        .as_ref()
        .is_some_and(|d| d.policy == code::DependenciesPolicy::Locked)
        || doc.hull.as_ref().is_some_and(|h| h.model_sha256.is_some())
    {
        return Err(
            "this Studio importer cannot yet verify locked registry/model dependencies".into(),
        );
    }
    let on = if let Some(h) = &doc.hull {
        let reg = universe_sim::world::registry::registry();
        let record = reg
            .hulls
            .iter()
            .find(|r| r.identity.key == h.record)
            .ok_or_else(|| format!("unknown hull {}", h.record))?;
        let model = record
            .model
            .as_deref()
            .ok_or("hull has no installed model")?;
        let bytes = std::fs::read(model).map_err(|e| format!("{model}: {e}"))?;
        let handle = universe_sim::world::import::commission(&bytes, model)?;
        universe_sim::world::content::content()
            .get(handle)
            .key
            .clone()
    } else {
        String::new()
    };
    for (key, b) in &doc.structure.members {
        if !stocks().iter().any(|s| s.key == b.stock) {
            return Err(format!("member {key}: unavailable stock {}", b.stock));
        }
    }
    for (key, p) in &doc.structure.plates {
        if !plate_stocks().iter().any(|s| s.key == p.stock) {
            return Err(format!("plate {key}: unavailable stock {}", p.stock));
        }
    }
    if doc.access.points.values().any(|p| p.hull_anchor.is_some()) {
        return Err("named hull anchor binding is not implemented by Studio".into());
    }
    let points: BTreeMap<_, _> = doc
        .access
        .points
        .keys()
        .enumerate()
        .map(|(n, k)| (k.as_str(), n))
        .collect();
    let paths: BTreeMap<_, _> = doc
        .access
        .paths
        .keys()
        .enumerate()
        .map(|(n, k)| (k.as_str(), n))
        .collect();
    let mut p = Plan {
        hull: doc.design.id.clone(),
        on: Some(on),
        ..Default::default()
    };
    p.points = doc
        .access
        .points
        .values()
        .map(|p| Point {
            uid: Id::default(),
            at: v3(p.position_m),
            name: p.name.clone(),
        })
        .collect();
    p.lines = doc
        .access
        .paths
        .values()
        .map(|l| {
            PathLine(
                points[l.from.as_str()],
                points[l.to.as_str()],
                Profile {
                    section: section(l.profile.shape),
                    width: l.profile.width_m as f32,
                    height: l.profile.height_m as f32,
                    stand: l.profile.placement == code::ProfilePlacement::Floor,
                },
                Id::default(),
            )
        })
        .collect();
    p.groups = doc
        .access
        .groups
        .values()
        .map(|g| Group {
            uid: Id::default(),
            lines: g.paths.iter().map(|k| paths[k.as_str()]).collect(),
            walled: g.walled,
        })
        .collect();
    p.doors = doc
        .access
        .side_doors
        .values()
        .map(|d| {
            SideDoor(
                paths[d.path.as_str()],
                d.fraction as f32,
                d.side == code::SideDoorSide::Right,
                read_hatch(&d.hatch),
                Id::default(),
            )
        })
        .collect();
    p.ends = doc
        .access
        .ends
        .values()
        .map(|e| {
            EndSet(
                paths[e.path.as_str()],
                usize::from(e.end == code::PathEndEnd::To),
                match e.state {
                    code::PathEndState::Open => End::Open,
                    code::PathEndState::Closed => End::Closed,
                    code::PathEndState::Door => End::Door,
                },
                e.hatch.as_ref().map_or_else(Hatch::default, read_hatch),
                Id::default(),
            )
        })
        .collect();
    p.beams = doc
        .structure
        .members
        .values()
        .map(|b| Beam {
            uid: Id::default(),
            a: v3(b.from_m),
            b: v3(b.to_m),
            stock: b.stock.clone(),
            pinned: b.pinned,
        })
        .collect();
    p.plates = doc
        .structure
        .plates
        .values()
        .map(|v| Plate {
            uid: Id::default(),
            lo: v2(v.min_xz_m),
            hi: v2(v.max_xz_m),
            y: v.height_m as f32,
            stock: v.stock.clone(),
        })
        .collect();
    p.pads = doc
        .structure
        .landing_pads
        .values()
        .copied()
        .map(|v| Pad {
            at: v3(v),
            uid: Id::default(),
        })
        .collect();
    if let Some(r) = &doc.requirements {
        p.gravity = r.gravity_m_s2;
        p.targets = Some(Targets {
            delta_v: r.delta_v_m_s,
            cargo: r.cargo_kg,
            crew: r.crew,
            trip: r.trip_s.map(|s| s / 3600.),
        });
    }
    let fit = hull_of(&p).map_or_else(|| catalogue().to_vec(), fit_of);
    let mut used = std::collections::BTreeSet::new();
    for (n, (key, m)) in doc.modules.iter().enumerate() {
        let (bid, pose, size) = match m {
            code::Module::HullHold(h) => ("HOLD".to_string(), &h.pose, h.envelope_m),
            code::Module::Equipment(e) => {
                let bid = if p.on.as_deref() == Some("") {
                    if e.slot.is_some() {
                        return Err(format!("module {key}: a slot requires a hull"));
                    }
                    format!("{}#{}", e.record, n + 1)
                } else {
                    e.slot.clone().ok_or_else(|| {
                        format!("module {key}: a hull module needs its actual fit slot")
                    })?
                };
                if !fit.iter().any(|f| f.id == kind(&bid) && f.key == e.record) {
                    return Err(format!(
                        "module {key}: equipment {} is not available in this catalogue/slot",
                        e.record
                    ));
                }
                (bid, &e.pose, e.envelope_m)
            }
        };
        if !used.insert(bid.clone()) {
            return Err(format!("duplicate Studio slot {bid}"));
        }
        p.blocks.push(Block {
            uid: Id::default(),
            id: bid,
            at: v3(pose.position_m),
            size: v3(size),
            push: None,
            rotation: Some(
                glam::Quat::from_array(pose.rotation_xyzw.map(|x| x as f32)).normalize(),
            ),
        });
    }
    let mut ordered: Vec<_> = doc.decks.iter().collect();
    ordered.sort_by(|a, b| {
        a.1.floor_m
            .total_cmp(&b.1.floor_m)
            .then_with(|| a.0.cmp(b.0))
    });
    let mut decks = deck::DeckPlan {
        hull: p.hull.clone(),
        decks: Vec::new(),
    };
    let deck_ids: BTreeMap<_, _> = ordered
        .iter()
        .map(|(key, _)| ((*key).clone(), Id::default()))
        .collect();
    for (key, d) in &ordered {
        decks.decks.push(deck::Deck {
            uid: deck_ids[*key],
            floor: d.floor_m,
            headroom: d.headroom_m,
            planes: d
                .floors
                .values()
                .map(|p| {
                    p.iter()
                        .map(|&p| glam::DVec2::from_array(p))
                        .collect::<Vec<_>>()
                        .into()
                })
                .collect(),
            walls: d
                .walls
                .values()
                .map(|w| deck::Wall {
                    uid: Default::default(),
                    points: w
                        .points_xz_m
                        .iter()
                        .map(|&p| glam::DVec2::from_array(p))
                        .collect(),
                    bulges: w.bulges_m.clone(),
                    doors: w
                        .doors
                        .values()
                        .map(|d| deck::Door {
                            uid: Default::default(),
                            at: d.distance_m,
                            width: d.width_m,
                            height: d.height_m,
                        })
                        .collect(),
                    rail: w.rail,
                })
                .collect(),
            ladders: d
                .ladders
                .values()
                .map(|l| deck::Ladder {
                    uid: Default::default(),
                    target: Some(deck_ids[&l.to_deck]),
                    at: glam::DVec2::from_array(l.at_xz_m),
                })
                .collect(),
            stairs: d
                .stairs
                .values()
                .map(|s| deck::Stair {
                    uid: Default::default(),
                    target: Some(deck_ids[&s.to_deck]),
                    from: glam::DVec2::from_array(s.from_xz_m),
                    to: glam::DVec2::from_array(s.to_xz_m),
                    width: s.width_m,
                })
                .collect(),
        });
    }
    let span_ok = |a: Vec3, b: Vec3| {
        let d = a.distance(b);
        d.is_finite() && d > 0.
    };
    if p.blocks
        .iter()
        .any(|b| !b.size.cmpgt(Vec3::ZERO).all() || !b.size.length().is_finite())
        || p.lines.iter().any(|&PathLine(a, b, pr, _)| {
            !span_ok(p.points[a].at, p.points[b].at) || pr.width <= 0. || pr.height <= 0.
        })
        || p.beams.iter().any(|b| !span_ok(b.a, b.b))
        || p.plates.iter().any(|b| !b.hi.cmpgt(b.lo).all())
    {
        return Err("geometry collapses or overflows at Studio's f32 precision".into());
    }
    if doc
        .view
        .as_ref()
        .and_then(|v| v.camera.as_ref())
        .is_some_and(|c| c.distance_m as f32 <= 0.)
    {
        return Err("camera distance collapses at Studio's f32 precision".into());
    }
    bind_names(&mut p, &decks, doc);
    Ok((p, decks))
}

/// Names are attached to identities, never to current vector positions.
fn named(p: &Plan, prefix: &str, uid: Id) -> String {
    if let Some(name) = p.authored.as_ref().and_then(|a| a.names.get(&uid)) {
        return name.clone();
    }
    let mut name = format!("{prefix}_{:016x}", uid.0);
    while p
        .authored
        .as_ref()
        .is_some_and(|a| a.names.values().any(|n| n == &name))
    {
        name.push('_');
    }
    name
}
fn bind_names(p: &mut Plan, decks: &deck::DeckPlan, doc: &code::Document) {
    let mut names = BTreeMap::new();
    let mut bind = |ids: Vec<Id>, keys: Vec<String>| {
        for (uid, key) in ids.into_iter().zip(keys) {
            names.insert(uid, key);
        }
    };
    macro_rules! bind {
        ($items:expr,$keys:expr,$id:expr) => {
            bind(
                $items.iter().map($id).collect(),
                $keys.keys().cloned().collect(),
            );
        };
    }
    bind!(p.points, doc.access.points, |x: &Point| x.uid);
    bind!(p.lines, doc.access.paths, |x: &PathLine| x.3);
    bind!(p.groups, doc.access.groups, |x: &Group| x.uid);
    bind!(p.doors, doc.access.side_doors, |x: &SideDoor| x.4);
    bind!(p.ends, doc.access.ends, |x: &EndSet| x.4);
    bind!(p.blocks, doc.modules, |x: &Block| x.uid);
    bind!(p.beams, doc.structure.members, |x: &Beam| x.uid);
    bind!(p.plates, doc.structure.plates, |x: &Plate| x.uid);
    bind!(p.pads, doc.structure.landing_pads, |x: &Pad| x.uid);
    let mut ordered: Vec<_> = doc.decks.iter().collect();
    ordered.sort_by(|a, b| {
        a.1.floor_m
            .total_cmp(&b.1.floor_m)
            .then_with(|| a.0.cmp(b.0))
    });
    for (d, (key, src)) in decks.decks.iter().zip(ordered) {
        bind(vec![d.uid], vec![key.clone()]);
        bind!(d.planes, src.floors, |x: &deck::Floor| x.uid);
        bind!(d.walls, src.walls, |x: &deck::Wall| x.uid);
        bind!(d.ladders, src.ladders, |x: &deck::Ladder| x.uid);
        bind!(d.stairs, src.stairs, |x: &deck::Stair| x.uid);
        for (w, sw) in d.walls.iter().zip(src.walls.values()) {
            bind!(w.doors, sw.doors, |x: &deck::Door| x.uid);
        }
    }
    p.authored = Some(Arc::new(Authored {
        document: doc.clone(),
        names,
        source: String::new(),
        baseline: Value::Null,
    }));
}
fn rename_collection(root: &mut Value, path: &str, keys: &[String]) {
    let Some(map) = root.pointer_mut(path).and_then(Value::as_object_mut) else {
        return;
    };
    let old = std::mem::take(map);
    *map = old
        .into_values()
        .zip(keys)
        .map(|(v, k)| (k.clone(), v))
        .collect();
}
fn replace_ref(value: &mut Value, names: &BTreeMap<String, String>) {
    if let Some(name) = value.as_str().and_then(|s| names.get(s)) {
        *value = json!(name);
    }
}
fn restore_names(root: &mut Value, p: &Plan, decks: Option<&deck::DeckPlan>) {
    let points: Vec<_> = p.points.iter().map(|x| named(p, "point", x.uid)).collect();
    let paths: Vec<_> = p.lines.iter().map(|x| named(p, "path", x.3)).collect();
    let decknames: Vec<_> = decks
        .into_iter()
        .flat_map(|p| &p.decks)
        .map(|x| named(p, "deck", x.uid))
        .collect();
    let refs = |prefix: &str, names: &[String]| {
        names
            .iter()
            .enumerate()
            .map(|(n, k)| (id(prefix, n), k.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    let pr = refs("point", &points);
    let lr = refs("path", &paths);
    let dr = refs("deck", &decknames);
    for v in root["access"]["paths"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        replace_ref(&mut v["from"], &pr);
        replace_ref(&mut v["to"], &pr);
    }
    for v in root["access"]["groups"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        for k in v["paths"].as_array_mut().unwrap() {
            replace_ref(k, &lr);
        }
    }
    for collection in ["side_doors", "ends"] {
        for v in root["access"][collection]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            replace_ref(&mut v["path"], &lr);
        }
    }
    for v in root["decks"].as_object_mut().unwrap().values_mut() {
        for collection in ["ladders", "stairs"] {
            for c in v[collection].as_object_mut().unwrap().values_mut() {
                replace_ref(&mut c["to_deck"], &dr);
            }
        }
    }
    rename_collection(root, "/access/points", &points);
    rename_collection(root, "/access/paths", &paths);
    macro_rules! rename {
        ($path:expr,$items:expr,$prefix:expr,$id:expr) => {
            rename_collection(
                root,
                $path,
                &$items
                    .iter()
                    .map(|x| named(p, $prefix, ($id)(x)))
                    .collect::<Vec<_>>(),
            );
        };
    }
    rename!("/modules", p.blocks, "module", |x: &Block| x.uid);
    rename!("/access/groups", p.groups, "group", |x: &Group| x.uid);
    rename!("/access/side_doors", p.doors, "door", |x: &SideDoor| x.4);
    rename!("/access/ends", p.ends, "end", |x: &EndSet| x.4);
    rename!("/structure/members", p.beams, "beam", |x: &Beam| x.uid);
    rename!("/structure/plates", p.plates, "plate", |x: &Plate| x.uid);
    rename!("/structure/landing_pads", p.pads, "pad", |x: &Pad| x.uid);
    for (n, d) in decks.into_iter().flat_map(|d| &d.decks).enumerate() {
        let base = format!("/decks/{}", id("deck", n));
        for (wn, w) in d.walls.iter().enumerate() {
            rename!(
                &format!("{base}/walls/{}/doors", id("wall", wn)),
                w.doors,
                "door",
                |x: &deck::Door| x.uid
            );
        }
        rename!(
            &format!("{base}/floors"),
            d.planes,
            "floor",
            |x: &deck::Floor| x.uid
        );
        rename!(
            &format!("{base}/walls"),
            d.walls,
            "wall",
            |x: &deck::Wall| x.uid
        );
        rename!(
            &format!("{base}/ladders"),
            d.ladders,
            "ladder",
            |x: &deck::Ladder| x.uid
        );
        rename!(
            &format!("{base}/stairs"),
            d.stairs,
            "stair",
            |x: &deck::Stair| x.uid
        );
    }
    rename_collection(root, "/decks", &decknames);
}

/// Keep authored precision and metadata where the visual projection is unchanged.
/// Missing edited members are actual deletions, not a request to resurrect them.
fn merge_authored(original: &Value, baseline: &Value, edited: &Value) -> Value {
    if baseline == edited {
        return original.clone();
    }
    if let (Some(old), Some(base), Some(now)) =
        (original.as_array(), baseline.as_array(), edited.as_array())
    {
        if matches!(old.len(), 2 | 3)
            && old.iter().all(Value::is_number)
            && old.len() == base.len()
            && base.len() == now.len()
        {
            return Value::Array(
                old.iter()
                    .zip(base)
                    .zip(now)
                    .map(|((o, b), n)| merge_authored(o, b, n))
                    .collect(),
            );
        }
    }
    if let (Some(old), Some(base), Some(now)) = (
        original.as_object(),
        baseline.as_object(),
        edited.as_object(),
    ) {
        let mut out = old.clone();
        for key in base.keys() {
            if !now.contains_key(key) {
                out.remove(key);
            }
        }
        for (key, value) in now {
            let result = match (old.get(key), base.get(key)) {
                (Some(old), Some(base)) => merge_authored(old, base, value),
                (None, Some(base)) if base == value => continue,
                _ => value.clone(),
            };
            out.insert(key.clone(), result);
        }
        Value::Object(out)
    } else {
        edited.clone()
    }
}
fn generated(i: &Interior) -> Result<code::Document, String> {
    let raw = generated_raw(i)?;
    let Some(a) = i.plan.authored.as_ref().filter(|a| !a.baseline.is_null()) else {
        return Ok(raw);
    };
    let out = merge_authored(&value(&a.document), &a.baseline, &value(raw));
    let doc: code::Document = serde_json::from_value(out).map_err(|e| e.to_string())?;
    let errors = code::validate(&doc);
    if !errors.is_empty() {
        return Err(format!("{errors:?}"));
    }
    Ok(doc)
}

fn read_source(path: &std::path::Path) -> Result<String, String> {
    use std::io::Read;
    let limit = code::Limits::default().max_bytes;
    let mut text = String::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > limit {
        return Err(format!("ship source exceeds parser limit of {limit} bytes"));
    }
    Ok(text)
}

pub(crate) fn check_path(path: &std::path::Path) -> Result<(), String> {
    let doc = code::parse(&read_source(path)?).map_err(|e| e.to_string())?;
    projection(&doc).map(|_| ())
}

#[derive(Clone, PartialEq)]
pub(super) struct ViewState {
    hidden: [bool; 26],
    yaw: f32,
    pitch: f32,
    distance: Option<f32>,
    target: Option<Vec3>,
    plane: Option<f32>,
}
#[derive(Clone)]
pub(super) struct Snapshot {
    pub(super) plan: Plan,
    decks: Option<deck::DeckPlan>,
    view: ViewState,
    source_path: Option<std::path::PathBuf>,
    saved_source: Option<String>,
}
pub(super) struct CodePanel {
    base: String,
    editor: universe_engine::text_editor::TextEditor,
    clipboard: universe_engine::clipboard::SystemClipboard,
}
impl Interior {
    pub(super) fn view_state(&self) -> ViewState {
        ViewState {
            hidden: self.hidden,
            yaw: self.yaw,
            pitch: self.pitch,
            distance: self.distance,
            target: self.target,
            plane: self.plane,
        }
    }
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            plan: self.plan.clone(),
            decks: self.decks_now.clone(),
            view: self.view_state(),
            source_path: self.source_path.clone(),
            saved_source: self.saved_source.clone(),
        }
    }
    pub(super) fn restore(&mut self, s: Snapshot) {
        if self.plan.hull != s.plan.hull || self.plan.on != s.plan.on {
            self.lines = None;
            self.job = None;
            self.hollow_job = None;
        }
        self.plan = s.plan;
        self.decks_now = s.decks;
        self.load_decks = Some(self.decks_now.clone().unwrap_or_else(|| deck::DeckPlan {
            hull: self.plan.hull.clone(),
            ..Default::default()
        }));
        self.decks = None;
        if self.source_path != s.source_path {
            self.saved_source = s.saved_source;
        }
        self.source_path = s.source_path;
        self.code_draft = None;
        self.hidden = s.view.hidden;
        self.yaw = s.view.yaw;
        self.pitch = s.view.pitch;
        self.distance = s.view.distance;
        self.target = s.view.target;
        self.plane = s.view.plane;
        self.cancel();
        self.pick = None;
        self.more.clear();
        self.refit();
    }
    fn prepared(text: &str) -> Result<Self, String> {
        let doc = code::parse(text).map_err(|e| e.to_string())?;
        let (plan, decks) = projection(&doc)?;
        let mut i = Self::new();
        i.plan = plan;
        i.decks_now = (!decks.decks.is_empty()).then_some(decks.clone());
        i.load_decks = Some(decks);
        i.refit();
        if let Some(v) = &doc.view {
            if let Some(layers) = &v.layers {
                for (n, key) in LAYER_KEYS.iter().enumerate() {
                    let layer = serde_json::from_value::<code::Layer>(json!(key)).unwrap();
                    if let Some(visible) = layers.get(&layer) {
                        i.hidden[n] = !visible;
                    }
                }
            }
            i.plane = v.work_plane_m.map(|y| y as f32);
            if let Some(c) = &v.camera {
                i.target = Some(v3(c.target_m));
                i.yaw = c.yaw_rad as f32;
                i.pitch = c.pitch_rad as f32;
                i.distance = Some(c.distance_m as f32);
            }
        }
        let baseline = value(generated_raw(&i)?);
        let a = Arc::make_mut(i.plan.authored.as_mut().unwrap());
        a.source = text.into();
        a.baseline = baseline;
        Ok(i)
    }
    pub(super) fn apply_ship_text(&mut self, text: &str) -> Result<(), String> {
        let candidate = Self::prepared(text)?;
        let before = self.snapshot();
        let mut after = candidate.snapshot();
        after.source_path = self.source_path.clone();
        after.saved_source = self.saved_source.clone();
        self.restore(after);
        self.undo.push(before);
        self.redo.clear();
        self.code_panel = None;
        self.code_draft = None;
        self.note("SHIP CODE APPLIED - editable design; SAVE to keep it");
        Ok(())
    }
    pub(crate) fn open_ship_source(&mut self, path: &std::path::Path) -> Result<(), String> {
        let text = read_source(path)?;
        let candidate = Self::prepared(&text)?;
        let mut state = candidate.snapshot();
        state.source_path = Some(std::fs::canonicalize(path).map_err(|e| e.to_string())?);
        state.saved_source = Some(text.clone());
        self.restore(state);
        self.saved_source = Some(text);
        self.code_panel = None;
        self.code_draft = None;
        self.undo.clear();
        self.redo.clear();
        self.saved = Some(self.plan.clone());
        self.saved_decks = Some(self.decks_now.clone());
        self.note("SHIP CODE OPENED");
        Ok(())
    }
    fn source_text(&self) -> Result<String, String> {
        let doc = generated(self)?;
        if let Some(a) = &self.plan.authored {
            if !a.source.is_empty() {
                let source = code::parse_source(&a.source).map_err(|e| e.to_string())?;
                return code::patch(&source, &doc).map_err(|e| e.to_string());
            }
        }
        code::emit(&doc).map_err(|e| e.to_string())
    }
    pub(super) fn ship_file(key: &str) -> std::path::PathBuf {
        Self::file(key).with_file_name(format!("{key}.ship.yaml"))
    }
    pub(super) fn save_ship(&mut self) -> Result<std::path::PathBuf, String> {
        if self.code_draft.is_some() {
            return Err(
                "Unapplied code draft: open CODE, then APPLY or REVERT before saving".into(),
            );
        }
        let text = self.source_text()?;
        let path = if let Some(p) = &self.source_path {
            p.clone()
        } else {
            if !safe_design_id(&self.plan.hull) {
                return Err("unsafe design file name".into());
            }
            let base = Self::ship_file(&self.plan.hull);
            if !base.exists() {
                base
            } else {
                (2..)
                    .map(|n| Self::ship_file(&format!("{}-{n}", self.plan.hull)))
                    .find(|p| !p.exists())
                    .unwrap()
            }
        };
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        if self.source_path.is_some() {
            let disk = match std::fs::read_to_string(&path) {
                Ok(s) => Some(s),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.to_string()),
            };
            if disk != self.saved_source {
                return Err("Source file changed outside Studio; export your edits and reopen before saving".into());
            }
        }
        if self.source_path.is_some() {
            use std::io::Write;
            let tmp = path.with_extension(format!("yaml.{}.tmp", Id::default().0));
            let result = (|| -> std::io::Result<()> {
                let mut f = std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&tmp)?;
                f.write_all(text.as_bytes())?;
                f.sync_all()?;
                std::fs::rename(&tmp, &path)?;
                std::fs::File::open(parent)?.sync_all()
            })();
            let _ = std::fs::remove_file(&tmp);
            result.map_err(|e| e.to_string())?;
        } else {
            write_new(&path, &text)?;
        }
        // Keep runtime identities and their projection baseline; reparsing the source
        // on next edit is sufficient, without reassigning names by vector position.
        if let Some(a) = self.plan.authored.as_mut() {
            Arc::make_mut(a).source = text.clone();
        }
        self.saved_source = Some(text);
        self.source_path = Some(path.clone());
        self.saved = Some(self.plan.clone());
        self.saved_decks = Some(self.decks_now.clone());
        Ok(path)
    }
    pub(crate) fn open_code(&mut self) {
        match self.source_text() {
            Ok(text) => {
                let retained = self.code_draft.is_some();
                let mut editor = universe_engine::text_editor::TextEditor::new(
                    self.code_draft.take().unwrap_or_else(|| text.clone()),
                );
                if retained {
                    editor.set_error(Some("Retained draft: APPLY replaces the current ship; REVERT loads its latest visual edits".into()));
                }
                editor.focused = true;
                self.code_panel = Some(CodePanel {
                    base: text,
                    editor,
                    clipboard: Default::default(),
                });
            }
            Err(e) => self.note(&e),
        }
    }
    pub(super) fn code_input(&mut self, ctx: &Context) {
        use universe_engine::clipboard::ClipboardAccess;
        let input = &ctx.input;
        let ctrl = input.down(KeyCode::ControlLeft) || input.down(KeyCode::ControlRight);
        let click =
            |k| input.button_pressed(MouseButton::Left) && inside(code_action(k), input.cursor);
        if input.pressed(KeyCode::Escape) || click(4) {
            let panel = self.code_panel.as_ref().unwrap();
            self.code_draft =
                (panel.editor.text() != panel.base).then(|| panel.editor.text().into());
            self.code_panel = None;
            return;
        }
        if (ctrl && input.pressed(KeyCode::Enter)) || click(1) {
            let text = self.code_panel.as_ref().unwrap().editor.text().to_string();
            if let Err(e) = self.apply_ship_text(&text) {
                self.code_panel.as_mut().unwrap().editor.set_error(Some(e));
            }
            return;
        }
        if click(5) {
            match self.source_text() {
                Ok(text) => {
                    let p = self.code_panel.as_mut().unwrap();
                    p.base = text.clone();
                    p.editor.set_text(text);
                    p.editor.set_error(None);
                }
                Err(e) => self.code_panel.as_mut().unwrap().editor.set_error(Some(e)),
            }
            return;
        }
        let panel = self.code_panel.as_mut().unwrap();
        if click(0) {
            match panel.clipboard.read_text() {
                Ok(s) => {
                    panel.editor.select_all();
                    panel.editor.insert(&s);
                }
                Err(e) => panel.editor.set_error(Some(e.to_string())),
            }
            return;
        }
        if click(2) {
            if let Err(e) = panel.clipboard.write_text(panel.editor.text()) {
                panel.editor.set_error(Some(e.to_string()));
            }
            return;
        }
        if click(3) {
            let result = (|| -> Result<String, String> {
                let text = panel.editor.text();
                let doc = code::parse(text).map_err(|e| e.to_string())?;
                projection(&doc)?;
                let dir = crate::save::data_dir().join("freefall/exports");
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let path = dir.join(format!("ship-{}.ship.yaml", Id::default().0));
                write_new(&path, text)?;
                Ok(format!("EXPORTED {}", path.display()))
            })();
            panel.editor.set_error(Some(result.unwrap_or_else(|e| e)));
            return;
        }
        panel.editor.update(
            input,
            code_rect(ctx.hud_size.as_vec2()),
            &mut panel.clipboard,
        );
    }
}
fn code_action(k: usize) -> (Vec2, Vec2) {
    (Vec2::new(12. + k as f32 * 100., 30.), Vec2::new(96., 20.))
}
fn code_rect(size: Vec2) -> universe_engine::text_editor::EditorRect {
    universe_engine::text_editor::EditorRect::new(
        Vec2::new(12., 62.),
        (size - Vec2::new(24., 74.)).max(Vec2::splat(40.)),
    )
}
pub(super) fn code_button(size: Vec2) -> (Vec2, Vec2) {
    (Vec2::new(size.x - 106., 6.), Vec2::new(94., 20.))
}
pub(super) fn draw_code(frame: &mut Frame, panel: &CodePanel) {
    frame.hud_rect(Vec2::ZERO, frame.size(), PAPER);
    frame.text(Vec2::new(12., 10.), "SHIP YAML - APPLY: CTRL+ENTER", LABEL);
    for (k, name) in ["PASTE", "APPLY", "COPY", "EXPORT", "CLOSE", "REVERT"]
        .iter()
        .enumerate()
    {
        let (p, s) = code_action(k);
        frame.hud_rect(p, s, LABEL.scale(0.15));
        frame.text(p + Vec2::new(6., 5.), name, LABEL);
    }
    panel.editor.draw(frame, code_rect(frame.size()));
}

/// Never clobber an existing export. Write and sync a sibling temporary file,
/// then publish the complete file with an atomic no-replace hard link.
fn write_new(path: &std::path::Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let temp = parent.join(format!(
        ".ship-export-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    let mut created = false;
    let result = (|| -> std::io::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        created = true;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::hard_link(&temp, path)?;
        std::fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if created {
        let _ = std::fs::remove_file(temp);
    }
    result.map_err(|e| format!("{}: {e}", path.display()))
}

/// Headless authoring tools; the same parser/projector is used for Apply.
pub(crate) fn cli(args: &[String]) -> Option<Result<String, String>> {
    let k = args
        .iter()
        .position(|a| a == "--validate-ship" || a == "--export-ship")?;
    Some((|| {
        let input = args
            .get(k + 1)
            .filter(|s| !s.starts_with("--"))
            .ok_or("expected a ship YAML path or saved design name")?;
        if args[k] == "--validate-ship" {
            let text = read_source(std::path::Path::new(input))?;
            let doc = code::parse(&text).map_err(|e| e.to_string())?;
            projection(&doc)?;
            return Ok(format!(
                "VALID: {} ({} modules, {} decks); compatible with Studio\n",
                doc.design.name,
                doc.modules.len(),
                doc.decks.len()
            ));
        }
        if !safe_design_id(input) {
            return Err("expected a saved design ID, not a path".into());
        }
        let yaml = if Interior::ship_file(input).exists() {
            let mut i = Interior::new();
            i.open_ship_source(&Interior::ship_file(input))?;
            i.source_text()?
        } else {
            let path = Interior::file(input);
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            // Read exactly what was saved: no hull-anchor snapping or registry resize.
            let plan: Plan = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            let mut i = Interior::new();
            i.plan = Plan {
                hull: input.clone(),
                ..plan
            };
            if i.spec().is_none() && i.plan.on.as_deref() != Some("") {
                // Imported hull handles are process-local. Resolve registered models
                // through the existing importer before adapting a saved hull fit.
                for record in &universe_sim::world::registry::registry().hulls {
                    if let Some(model) = record.model.as_deref() {
                        if let Ok(bytes) = std::fs::read(model) {
                            let _ = universe_sim::world::import::commission(&bytes, model);
                            if i.spec().is_some() {
                                break;
                            }
                        }
                    }
                }
            }
            i.fit = i.spec().map_or_else(|| catalogue().to_vec(), fit_of);
            let dp = Interior::deck_file(input);
            if dp.exists() {
                i.decks_now = Some(
                    serde_json::from_str(&std::fs::read_to_string(dp).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?,
                );
            }
            let doc = generated(&i)?;
            code::emit(&doc).map_err(|e| e.to_string())?
        };
        if let Some(k) = args.iter().position(|a| a == "--output") {
            let out = args.get(k + 1).ok_or("--output requires a path")?;
            write_new(std::path::Path::new(out), &yaml)?;
            Ok(format!("EXPORTED {}\n", out))
        } else {
            Ok(yaml)
        }
    })())
}
fn safe_design_id(id: &str) -> bool {
    !id.is_empty() && id != "." && id != ".." && !id.contains(['/', '\\'])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example() -> code::Document {
        code::parse(include_str!(
            "../../../../docs/proposals/ship-code-v1/all-layers.ship.yaml"
        ))
        .unwrap()
    }
    fn projected(d: &code::Document) -> Interior {
        let (p, decks) = projection(d).unwrap();
        let mut i = Interior::new();
        i.plan = p;
        i.decks_now = Some(decks);
        i.fit = catalogue().to_vec();
        i
    }
    fn source() -> String {
        include_str!("../../../../docs/proposals/ship-code-v1/all-layers.ship.yaml").into()
    }

    #[test]
    fn apply_undo_restores_whole_document_and_decks() {
        let mut i = Interior::new();
        i.plan.hull = "before".into();
        i.plan.on = Some(String::new());
        let old = i.snapshot();
        i.apply_ship_text(&source()).unwrap();
        assert!(!i.plan.blocks.is_empty());
        assert!(i.decks_now.is_some());
        let applied = i.snapshot();
        let before = i.undo.pop().unwrap();
        i.restore(before);
        assert!(i.plan == old.plan);
        assert!(i.decks_now.is_none());
        i.restore(applied);
        assert_eq!(i.source_text().unwrap(), source());
        let current = i.snapshot();
        assert!(i.apply_ship_text("nonsense: true").is_err());
        assert!(i.plan == current.plan);
        assert_eq!(i.decks_now, current.decks);
    }

    #[test]
    fn save_reopen_is_one_document_with_decks_and_metadata() {
        let dir = std::env::temp_dir().join(format!(
            "ship-save-{}-{}",
            std::process::id(),
            Id::default().0
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("all.ship.yaml");
        let mut i = Interior::prepared(&source()).unwrap();
        i.source_path = Some(path.clone());
        i.plan.blocks[0].at.x -= 1.;
        i.hidden[22] = true;
        i.decks_now.as_mut().unwrap().decks[0].floor -= 0.25;
        let expected = generated(&i).unwrap();
        i.save_ship().unwrap();
        assert!(!i.unsaved());
        let mut reopened = Interior::new();
        reopened.open_ship_source(&path).unwrap();
        assert_eq!(generated(&reopened).unwrap(), expected);
        assert!(!reopened.unsaved());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("Ship code proposal")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn external_file_changes_are_not_overwritten() {
        let path =
            std::env::temp_dir().join(format!("ship-external-{}.ship.yaml", Id::default().0));
        std::fs::write(&path, source()).unwrap();
        let mut i = Interior::new();
        i.open_ship_source(&path).unwrap();
        i.plan.blocks[0].at.x += 2.;
        std::fs::write(&path, "external edit").unwrap();
        assert!(i.save_ship().is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "external edit");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn draft_only_changes_are_dirty_and_not_saved_as_applied() {
        let mut i = Interior::prepared(&source()).unwrap();
        i.saved = Some(i.plan.clone());
        i.saved_decks = Some(i.decks_now.clone());
        assert!(!i.unsaved());
        // Looking round is no change.
        i.yaw += 1.;
        i.hidden[0] = !i.hidden[0];
        assert!(!i.unsaved());
        i.code_draft = Some("invalid draft".into());
        assert!(i.unsaved());
        assert!(i.save_ship().is_err());
        i.open_code();
        assert_eq!(
            i.code_panel.as_ref().unwrap().editor.text(),
            "invalid draft"
        );
    }

    #[test]
    fn rejected_source_does_not_mutate_open_design() {
        let mut i = projected(&example());
        let before = i.plan.clone();
        let decks = i.decks_now.clone();
        let path =
            std::env::temp_dir().join(format!("invalid-ship-{}.ship.yaml", std::process::id()));
        std::fs::write(&path, "format: nope\n").unwrap();
        assert!(i.open_ship_source(&path).is_err());
        assert!(i.plan == before);
        assert_eq!(i.decks_now, decks);
        std::fs::remove_file(path).unwrap();
    }
}
