//! Boundary between the versioned ship document and the legacy Studio projection.
//! Never imports registry facts or derived simulation results from a document.
use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use universe_ship_design as code;
use universe_sim::world::deckplan as deck;

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

fn deck_value(d: &deck::Deck, n: usize) -> Value {
    json!({"floor_m":d.floor,"headroom_m":d.headroom,
        "floors":object(d.planes.iter().enumerate().map(|(n,p)|(id("floor",n),p)), value),
        "walls":object(d.walls.iter().enumerate().map(|(n,w)|(id("wall",n),w)), |w|json!({"points_xz_m":w.points,"bulges_m":(0..w.points.len().saturating_sub(1)).map(|n|w.bulges.get(n).copied().unwrap_or(0.)).collect::<Vec<_>>(),"rail":w.rail,"doors":object(w.doors.iter().enumerate().map(|(n,d)|(id("door",n),d)),|d|json!({"distance_m":d.at,"width_m":d.width,"height_m":d.height}))})),
        "ladders":object(d.ladders.iter().enumerate().map(|(n,l)|(id("ladder",n),l)),|l|json!({"at_xz_m":l.at,"to_deck":id("deck",n+1)})),
        "stairs":object(d.stairs.iter().enumerate().map(|(n,s)|(id("stair",n),s)),|s|json!({"from_xz_m":s.from,"to_xz_m":s.to,"width_m":s.width,"to_deck":id("deck",n+1)}))})
}

/// Export every authored collection, regardless of display visibility.
fn generated(i: &Interior) -> Result<code::Document, String> {
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
    let decks = i.load_decks.as_ref().or(i.decks_now.as_ref());
    let mut root = json!({"format":"freefall.ship","version":"1-draft","design":{"id":p.hull,"name":p.hull},
        "coordinates":"right-handed-x-right-y-up-z-aft-metres","placement_convention":"studio-centered-v1",
        "hull":match i.spec() { Some(h) => Some(json!({"record":universe_sim::world::registry::registry().hulls.iter().find(|r|h.visual.as_deref().map_or(r.identity.key==h.key, |v|r.model.as_deref()==Some(v))).ok_or("hull has no registry identity")?.identity.key})), None => None },
        "modules":modules?,"access":{"points":points,
            "paths":object(p.lines.iter().enumerate().map(|(n,l)|(id("path",n),l)),|&(a,b,pr)|json!({"from":id("point",a),"to":id("point",b),"profile":profile(pr)})),
            "groups":object(p.groups.iter().enumerate().map(|(n,g)|(id("group",n),g)),|g|json!({"paths":g.lines.iter().map(|&n|id("path",n)).collect::<Vec<_>>(),"walled":g.walled})),
            "side_doors":object(p.doors.iter().enumerate().map(|(n,d)|(id("door",n),d)),|d|json!({"path":id("path",d.0),"fraction":d.1,"side":if d.2 {"right"}else{"left"},"hatch":hatch(d.3)})),
            "ends":object(p.ends.iter().enumerate().map(|(n,e)|(id("end",n),e)),|e|{let mut v=json!({"path":id("path",e.0),"end":if e.1==0 {"from"}else{"to"},"state":match e.2 {End::Closed=>"closed",End::Open=>"open",End::Door=>"door"}});if e.2==End::Door {v["hatch"]=hatch(e.3)}v})},
        "structure":{
            "members":object(p.beams.iter().enumerate().map(|(n,b)|(id("beam",n),b)),|b|json!({"from_m":b.a,"to_m":b.b,"stock":b.stock,"pinned":b.pinned})),
            "plates":object(p.plates.iter().enumerate().map(|(n,p)|(id("plate",n),p)),|p|json!({"min_xz_m":p.lo,"max_xz_m":p.hi,"height_m":p.y,"stock":p.stock})),
            "landing_pads":object(p.pads.iter().enumerate().map(|(n,p)|(id("pad",n),p)),value)},
        "decks":object(decks.into_iter().flat_map(|d|d.decks.iter()).enumerate().map(|(n,d)|(id("deck",n),(n,d))),|(n,d)|deck_value(d,n)),
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
        return Err("named hull anchor binding is not implemented by this preview".into());
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
            at: v3(p.position_m),
            name: p.name.clone(),
        })
        .collect();
    p.lines = doc
        .access
        .paths
        .values()
        .map(|l| {
            (
                points[l.from.as_str()],
                points[l.to.as_str()],
                Profile {
                    section: section(l.profile.shape),
                    width: l.profile.width_m as f32,
                    height: l.profile.height_m as f32,
                    stand: l.profile.placement == code::ProfilePlacement::Floor,
                },
            )
        })
        .collect();
    p.groups = doc
        .access
        .groups
        .values()
        .map(|g| Group {
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
            )
        })
        .collect();
    p.beams = doc
        .structure
        .members
        .values()
        .map(|b| Beam {
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
        .map(v3)
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
    for (n, (key, d)) in ordered.iter().enumerate() {
        let next = ordered.get(n + 1).map(|(k, _)| k.as_str());
        if d.ladders
            .values()
            .map(|l| l.to_deck.as_str())
            .chain(d.stairs.values().map(|s| s.to_deck.as_str()))
            .any(|k| Some(k) != next)
        {
            return Err(format!(
                "deck {key}: Studio currently supports stairs/ladders only to the next higher deck"
            ));
        }
        decks.decks.push(deck::Deck {
            floor: d.floor_m,
            headroom: d.headroom_m,
            planes: d
                .floors
                .values()
                .map(|p| p.iter().map(|&p| glam::DVec2::from_array(p)).collect())
                .collect(),
            walls: d
                .walls
                .values()
                .map(|w| deck::Wall {
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
                    at: glam::DVec2::from_array(l.at_xz_m),
                })
                .collect(),
            stairs: d
                .stairs
                .values()
                .map(|s| deck::Stair {
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
        || p.lines.iter().any(|&(a, b, pr)| {
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
    Ok((p, decks))
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

/// Source previews are isolated from saved designs. Applying a new document is
/// transactional: parsing, compatibility checks and projection finish first.
impl Interior {
    pub(crate) fn open_ship_source(&mut self, path: &std::path::Path) -> Result<(), String> {
        let text = read_source(path)?;
        let doc = code::parse(&text).map_err(|e| e.to_string())?;
        let (plan, decks) = projection(&doc)?;
        self.start(&plan.hull, None, plan.on.clone());
        self.plan = plan;
        self.code_preview = Some(doc);
        self.hidden = [false; 26];
        self.target = None;
        self.distance = None;
        self.saved = None;
        self.saved_decks = None;
        self.decks_now = (!decks.decks.is_empty()).then_some(decks.clone());
        self.load_decks = Some(decks);
        self.refit();
        if let Some(v) = self.code_preview.as_ref().and_then(|d| d.view.as_ref()) {
            if let Some(layers) = &v.layers {
                for (n, key) in LAYER_KEYS.iter().enumerate() {
                    let layer =
                        serde_json::from_value::<code::Layer>(json!(key)).expect("known layer");
                    if let Some(visible) = layers.get(&layer) {
                        self.hidden[n] = !visible;
                    }
                }
            }
            self.plane = v.work_plane_m.map(|y| y as f32);
            if let Some(c) = &v.camera {
                self.target = Some(v3(c.target_m));
                self.yaw = c.yaw_rad as f32;
                self.pitch = c.pitch_rad as f32;
                self.distance = Some(c.distance_m as f32);
            }
        }
        self.message = Some((
            "YAML PREVIEW: SOURCE UNCHANGED; SAVING VISUAL EDITS IS NOT YET SUPPORTED".into(),
            12.,
        ));
        Ok(())
    }
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

/// Headless authoring tools; the same parser/projector is used for preview.
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
                "VALID: {} ({} modules, {} decks); compatible with Studio preview\n",
                doc.design.name,
                doc.modules.len(),
                doc.decks.len()
            ));
        }
        if !safe_design_id(input) {
            return Err("expected a saved design ID, not a path".into());
        }
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
        let yaml = code::emit(&doc).map_err(|e| e.to_string())?;
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
    #[test]
    fn all_authored_layers_survive_projection_and_export() {
        let doc = example();
        let mut i = projected(&doc);
        i.hidden = [true; 26];
        let exported = generated(&i).unwrap();
        let (p, decks) = projection(&exported).unwrap();
        assert!(p == i.plan, "native projection must survive import/export");
        assert_eq!(decks, i.decks_now.unwrap());
        assert_eq!(exported.modules.len(), doc.modules.len());
        assert_eq!(exported.access.points.len(), doc.access.points.len());
        assert_eq!(
            exported.structure.members.len(),
            doc.structure.members.len()
        );
        assert_eq!(exported.structure.plates.len(), doc.structure.plates.len());
        assert_eq!(
            exported.structure.landing_pads.len(),
            doc.structure.landing_pads.len()
        );
        assert_eq!(exported.view.unwrap().layers.unwrap().len(), 26);
    }
    #[test]
    fn full_roll_is_shared_by_design_walk_and_drive() {
        let mut i = projected(&example());
        let q = glam::Quat::from_euler(glam::EulerRot::XYZ, 0.41, 0.72, -0.28);
        i.plan.blocks[0].rotation = Some(q);
        let f = i
            .fit
            .iter()
            .find(|f| f.id == kind(&i.plan.blocks[0].id))
            .unwrap();
        assert!(
            push_of(&i.plan.blocks[0], f)
                .unwrap()
                .0
                .abs_diff_eq(q * Vec3::Y, 1e-6)
        );
        let design = placed_equipment(&i);
        let walk = i.stand_up();
        let drive = craft(&i);
        assert!(
            design[0]
                .transform
                .abs_diff_eq(walk.equipment[0].transform, 1e-6)
        );
        assert!(
            design[0]
                .transform
                .abs_diff_eq(drive.equipment[0].transform, 1e-6)
        );
        let out = generated(&i).unwrap();
        let (p, _) = projection(&out).unwrap();
        assert!(p.blocks[0].rotation.unwrap().abs_diff_eq(q, 1e-6));
        for mirror in 1..=3 {
            for n in 0..images(Vec3::ONE, mirror).len() {
                let r = mirror_rotation(q, mirror, n);
                assert!((r.length() - 1.).abs() < 1e-6);
                assert!((r * Vec3::Y).abs_diff_eq(image_dir(q * Vec3::Y, mirror, n), 1e-6));
            }
        }
    }
    #[test]
    fn rejects_unsupported_connections_and_missing_equipment() {
        let mut d = example();
        if let code::Module::Equipment(e) = d.modules.values_mut().next().unwrap() {
            e.record = "equipment.missing".into();
        }
        assert!(projection(&d).is_err());
        let mut d = example();
        let last = d
            .decks
            .values()
            .map(|d| d.floor_m)
            .fold(f64::NEG_INFINITY, f64::max);
        let mut extra = d.decks.values().last().unwrap().clone();
        extra.floor_m = last + 10.;
        extra.ladders.clear();
        extra.stairs.clear();
        d.decks.insert("extra".into(), extra);
        let mut changed = false;
        for deck in d.decks.values_mut() {
            for l in deck.ladders.values_mut() {
                l.to_deck = "extra".into();
                changed = true;
            }
        }
        assert!(changed);
        assert!(projection(&d).is_err());
    }
    #[test]
    fn export_is_atomic_and_does_not_overwrite_existing_file() {
        let dir = std::env::temp_dir().join(format!("ship-export-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("design.ship.yaml");
        let _ = std::fs::remove_file(&path);
        write_new(&path, "first").unwrap();
        assert!(write_new(&path, "second").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
        std::fs::remove_dir_all(dir).unwrap();
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
