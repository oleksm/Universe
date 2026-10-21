//! A works laid from its records (SFO 24 to 26, `docs/proposals/factory-kit-assembly.md` on fso): no layout
//! model. A layout's units stand at their bays, each run is laid along its link's path (straights, an elbow
//! at each corner, its supports by the kit's rule), and a joint (the run's connector, or the port's) is drawn
//! at every port a run ends on or taps. All in the layout's frame: metres from its middle at ground level, x
//! along, y across, z up. What draws each run and joint is its family's template (`template:`).

use glam::{DQuat, DVec3};

use crate::registry::{registry, Connector, Layout, Port, PortMedium, Run, Template, TemplateDraws, Unit, Visual};

/// A unit at its bay.
pub struct Placed {
    pub bay: String,
    pub unit: &'static Unit,
    /// The middle of the bay on its floor.
    pub at: DVec3,
    /// Its turn about z from the layout's x (rad).
    pub turn: f64,
}

impl Placed {
    /// A point in the unit's frame, in the layout's.
    pub fn point(&self, local: [f64; 3]) -> DVec3 {
        self.at + self.dir(local)
    }

    /// A direction in the unit's frame, in the layout's.
    pub fn dir(&self, local: [f64; 3]) -> DVec3 {
        DQuat::from_rotation_z(self.turn) * DVec3::from(local)
    }
}

/// A corner a run turns at.
pub struct Elbow {
    pub at: DVec3,
    /// The leg coming in and the leg going out, unit vectors.
    pub before: DVec3,
    pub after: DVec3,
    /// How far it turns (rad).
    pub angle: f64,
}

/// A run laid along its link.
pub struct Laid {
    /// The link's name in its layout.
    pub name: String,
    pub run: &'static Run,
    /// What draws it (none: the run names no template, or one the registry lacks).
    pub template: Option<&'static Template>,
    /// Circuits side by side on the path (cables).
    pub count: u32,
    /// From, the corners, to.
    pub path: Vec<DVec3>,
    pub elbows: Vec<Elbow>,
    /// Where each support's head meets the run.
    pub supports: Vec<DVec3>,
    /// Its blind ends (`cap`, the run's welded plate): where, and which way out of the run.
    pub caps: Vec<(DVec3, DVec3)>,
}

impl Laid {
    /// Its length along the path (m).
    pub fn length(&self) -> f64 {
        self.path.windows(2).map(|w| w[0].distance(w[1])).sum()
    }
}

/// A joint at a port: the connector, where its mating plane lies and which way the port faces.
pub struct Joint {
    pub connector: &'static Connector,
    pub template: Option<&'static Template>,
    pub at: DVec3,
    /// The port's facing, out of its thing (unit vector).
    pub facing: DVec3,
    /// The run it joins, by its place in `Plant::runs`.
    pub run: usize,
    /// The end it is: `<bay>:<port>` or `edge:<port>`.
    pub end: String,
}

/// A layout laid out: what to draw, and what in its records doesn't join up.
pub struct Plant {
    pub layout: &'static Layout,
    pub units: Vec<Placed>,
    pub runs: Vec<Laid>,
    pub joints: Vec<Joint>,
    /// Ends and runs the records name that aren't there, and run ends away from the ports they name.
    pub faults: Vec<String>,
}

/// The farthest a run's end may lie from the port it names (m) before it is said.
const AT_PORT: f64 = 0.05;

/// The layout `key` laid out. None: no such layout.
pub fn lay(key: &str) -> Option<Plant> {
    let reg = registry();
    let layout = reg.layout(key)?;
    let mut faults = Vec::new();
    let units: Vec<Placed> = layout
        .bays
        .iter()
        .filter_map(|b| {
            let Some(unit) = reg.unit(&b.unit) else {
                faults.push(format!("bay {}: no unit {}", b.id, b.unit));
                return None;
            };
            Some(Placed { bay: b.id.clone(), unit, at: DVec3::from(b.at), turn: b.turn.map_or(0.0, |t| t.rad()) })
        })
        .collect();
    // A port by its end's name, where it lies and faces in the layout.
    let port = |end: &str| -> Option<(&'static Port, DVec3, DVec3)> {
        let (place, id) = end.split_once(':')?;
        if place == "edge" {
            let p = layout.ports.iter().find(|p| p.id == id)?;
            return Some((p, DVec3::from(p.at), DVec3::from(p.facing).normalize_or_zero()));
        }
        let u = units.iter().find(|u| u.bay == place)?;
        let p = u.unit.ports.iter().find(|p| p.id == id)?;
        Some((p, u.point(p.at), u.dir(p.facing).normalize_or_zero()))
    };
    let mut runs = Vec::new();
    let mut joints = Vec::new();
    for (i, link) in layout.links.iter().enumerate() {
        let name = link.name.clone().unwrap_or_else(|| format!("link {i}"));
        let Some(key) = &link.run else { continue };
        let Some(run) = reg.run(key) else {
            faults.push(format!("{name}: no run {key}"));
            continue;
        };
        let path: Vec<DVec3> = std::iter::once(link.from).chain(link.via.iter().copied()).chain(std::iter::once(link.to)).map(DVec3::from).collect();
        let index = runs.len();
        let mut caps = Vec::new();
        // (A joint at each end that is a port, the port's own connector where it names one, and at each tap.)
        for (k, end) in link.ends.iter().enumerate() {
            if end == "cap" {
                let (at, inner) = if k == 0 { (path[0], path[1]) } else { (path[path.len() - 1], path[path.len() - 2]) };
                caps.push((at, (at - inner).normalize_or_zero()));
                continue;
            }
            let Some((place, _)) = end.split_once(':') else {
                faults.push(format!("{name}: an end {end:?}"));
                continue;
            };
            if matches!(place, "tee" | "onto" | "jacket") {
                continue;
            }
            let Some((p, at, facing)) = port(end) else {
                faults.push(format!("{name}: no port {end}"));
                continue;
            };
            let laid_end = if k == 0 { path[0] } else { path[path.len() - 1] };
            let gap = laid_end.distance(at);
            if gap > AT_PORT && !p.connector.as_deref().and_then(|c| reg.connector(c)).is_some_and(stands_off) {
                faults.push(format!("{name}: its end lies {gap:.2} m from {end}"));
            }
            joints.extend(joint(p, &run.connector, at, facing, index, end));
        }
        for end in &link.taps {
            match port(end) {
                Some((p, at, facing)) => joints.extend(joint(p, &run.connector, at, facing, index, end)),
                None => faults.push(format!("{name}: no tapped port {end}")),
            }
        }
        let (elbows, supports) = fittings(run, &path);
        runs.push(Laid { name, run, template: run.template.as_deref().and_then(|t| reg.template(t)), count: link.count.unwrap_or(1), path, elbows, supports, caps });
    }
    Some(Plant { layout, units, runs, joints, faults })
}

/// The joint at port `p`: its own connector, else the run's. None for a load port (its bay joins it, not a
/// run) or a connector the registry lacks.
fn joint(p: &Port, run_connector: &str, at: DVec3, facing: DVec3, run: usize, end: &str) -> Option<Joint> {
    if p.medium == PortMedium::Load {
        return None;
    }
    let reg = registry();
    let connector = reg.connector(p.connector.as_deref().unwrap_or(run_connector))?;
    Some(Joint { connector, template: connector.template.as_deref().and_then(|t| reg.template(t)), at, facing, run, end: end.to_string() })
}

/// Joints whose run's end stands clear of the port by design: a hood over a moving outlet, a belt's head over
/// what it drops into.
fn stands_off(c: &Connector) -> bool {
    use crate::registry::ConnectorKind::*;
    matches!(c.kind, Hood | BeltTransfer | Chute)
}

/// A run's elbows and supports along `path`, as the kit lays them (`tools/standards/plant_kit.py`, `lay`): an
/// elbow at each corner of more than a degree; on each leg rising more than half its length one support at
/// its top, on any other leg one every spacing (at least one), evenly along it.
fn fittings(run: &Run, path: &[DVec3]) -> (Vec<Elbow>, Vec<DVec3>) {
    let mut elbows = Vec::new();
    for w in path.windows(3) {
        let (before, after) = ((w[1] - w[0]).normalize_or_zero(), (w[2] - w[1]).normalize_or_zero());
        let angle = before.dot(after).clamp(-1.0, 1.0).acos();
        if angle > 1f64.to_radians() {
            elbows.push(Elbow { at: w[1], before, after, angle });
        }
    }
    let mut supports = Vec::new();
    if let Some(s) = &run.support {
        for w in path.windows(2) {
            let leg = w[0].distance(w[1]);
            if leg < 1e-9 {
                continue;
            }
            if (w[1].z - w[0].z).abs() > 0.5 * leg {
                supports.push(if w[1].z > w[0].z { w[1] } else { w[0] });
            } else {
                let k = ((leg / s.spacing - 1e-9).ceil() as usize).max(1);
                supports.extend((0..k).map(|i| w[0].lerp(w[1], (i as f64 + 0.5) / k as f64)));
            }
        }
    }
    (elbows, supports)
}

/// How a template's model is had: its own, built at its reference member (joint axis +Y), or until it has one
/// the installed model of a member (`model_of`, built in the older kit frame: the run's axis along +X).
pub enum TemplateModel {
    Own { visual: &'static Visual, reference: Option<&'static str> },
    Of { visual: &'static Visual, member: &'static str },
}

/// The model `template` is drawn with. None: a sweep, or nothing installed yet (draw its box).
pub fn template_model(template: &'static Template) -> Option<TemplateModel> {
    if template.draws == TemplateDraws::Sweep {
        return None;
    }
    if let Some(visual) = &template.visual {
        return Some(TemplateModel::Own { visual, reference: template.reference.as_deref() });
    }
    let member = template.model_of.as_deref()?;
    let reg = registry();
    let visual = reg.run(member).and_then(|r| r.visual.as_ref()).or_else(|| reg.connector(member).and_then(|c| c.visual.as_ref()))?;
    Some(TemplateModel::Of { visual, member })
}

/// A connector's model scale against the member its template's model was built at: per axis, its length,
/// height and width over the member's (contract: X length, Y height, Z width). One where either lacks a figure.
pub fn joint_scale(connector: &Connector, member: &str) -> DVec3 {
    let Some(m) = registry().connector(member) else { return DVec3::ONE };
    let (a, b) = (&connector.physical, &m.physical);
    let ratio = |x: Option<f64>, y: Option<f64>| match (x, y) {
        (Some(x), Some(y)) if y > 0.0 => x / y,
        _ => 1.0,
    };
    DVec3::new(ratio(a.length, b.length), ratio(a.height, b.height), ratio(a.width, b.width))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trial layout lays from its records alone: every run's end at the port it names, every port end
    /// joined, every blind end capped, and its elbows and supports as many as the registry's generator counted;
    /// and every other layout lays without a fault.
    #[test]
    fn shaft_furnace_lays_from_records() {
        let plant = lay("layout.shaft-furnace-airless").expect("the trial layout");
        assert!(plant.faults.is_empty(), "{}", plant.faults.join("\n"));
        assert!(!plant.units.is_empty() && plant.runs.len() >= 40, "{} units, {} runs", plant.units.len(), plant.runs.len());
        let links: Vec<_> = plant.layout.links.iter().filter(|l| l.run.is_some()).collect();
        for (laid, link) in plant.runs.iter().zip(&links) {
            assert_eq!(laid.elbows.len() as u32, link.fittings.elbows.unwrap_or(0), "{}: elbows", laid.name);
            assert_eq!(laid.supports.len() as u32, link.fittings.supports.unwrap_or(0), "{}: supports", laid.name);
            assert!(laid.template.is_some(), "{}: no template", laid.name);
        }
        let caps = links.iter().flat_map(|l| &l.ends).filter(|e| *e == "cap").count();
        assert_eq!(plant.runs.iter().map(|r| r.caps.len()).sum::<usize>(), caps, "a cap at every blind end");
        let port_ends = links.iter().flat_map(|l| &l.ends).filter(|e| *e != "cap" && !e.starts_with("tee:") && !e.starts_with("onto:") && !e.starts_with("jacket:")).count();
        let taps: usize = links.iter().map(|l| l.taps.len()).sum();
        assert_eq!(plant.joints.len(), port_ends + taps, "a joint at every port end and tap");
        let faults: Vec<String> = registry().layouts.iter().filter_map(|l| lay(&l.identity.key)).flat_map(|p| p.faults.into_iter().map(move |f| format!("{}: {f}", p.layout.identity.key))).collect();
        assert!(faults.is_empty(), "every layout lays: {}", faults.join("\n"));
    }
}
