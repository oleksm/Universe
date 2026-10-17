//! A document becomes an owned definition for the real world simulator.
//! No integration, resource consumption or contact response belongs here.
use super::*;
use universe_engine::glam::{DMat3, DVec3};
use universe_sim::world::{
    content::content,
    modules::Module,
    shape::Shape,
    ship::{ClassSpec, HullFrame, Slot},
};

fn corners(lo: Vec3, hi: Vec3) -> Vec<Vec3> {
    (0..8)
        .map(|n| {
            Vec3::new(
                if n & 1 == 0 { lo.x } else { hi.x },
                if n & 2 == 0 { lo.y } else { hi.y },
                if n & 4 == 0 { lo.z } else { hi.z },
            )
        })
        .collect()
}

// Resolve the registry's default push just as placed_module does. Block's
// fallback only sees b.push and can otherwise leave a forward engine upright.
fn orientation(b: &Block, f: &Fitted) -> universe_engine::glam::Quat {
    b.rotation
        .unwrap_or_else(|| crate::equipment_visual::rotation(push_of(b, f).map(|p| p.0)))
}

pub(super) fn compile(
    i: &Interior,
    craft: &crate::test_drive::Craft,
) -> Result<Arc<ClassSpec>, String> {
    if !craft.mass.is_finite() || craft.mass <= 0. {
        return Err("The design needs physical mass before it can be simulated".into());
    }
    // Ship has one consumable fuel pool. Never substitute a rocket's reaction
    // mass for its separate reactor fuel, or merge different tank contents.
    let mut fuels = std::collections::BTreeSet::new();
    let mut actuator_records = Vec::new();
    for b in &i.plan.blocks {
        let f = i
            .fit
            .iter()
            .find(|f| f.id == kind(&b.id))
            .ok_or_else(|| format!("No registry equipment record for {}", b.id))?;
        let Some((device, fields, _)) = figures(f) else {
            if push_of(b, f).is_some() {
                return Err(format!("{} has no actuator figures", f.key));
            }
            continue;
        };
        if device == "power_plant"
            || (push_of(b, f).is_some() && !fields.contains_key("full_power"))
        {
            let mut has_fuel = false;
            for key in ["propellant", "burns"] {
                if let Some(fuel) = fields
                    .get(key)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    fuels.insert(fuel.to_string());
                    has_fuel = true;
                }
            }
            if !has_fuel {
                return Err(format!("{} has no fuel or propellant record", f.key));
            }
        }
        if push_of(b, f).is_some() {
            actuator_records.push((f.key.clone(), fields));
        }
    }
    if fuels.len() > 1 {
        return Err(format!(
            "Shared ship runtime has one fuel pool; this design consumes {}",
            fuels.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    let fuel = fuels.into_iter().next();
    if actuator_records.len() != craft.actuators.len() {
        return Err("Authored actuators and registry devices do not match".into());
    }
    let mut parts = Vec::new();
    let mut nodes = Vec::new();
    let mut slots = Vec::new();
    let mut fit = Vec::new();
    let mut fitted_mass = 0.;
    let mut authored_placements = Vec::new();
    let mut dry_mass = 0.;
    let mut dry_first = DVec3::ZERO;
    let mut dry_terms = Vec::new();
    let origin = craft.visual_origin.as_dvec3();
    let mut fuel_first = DVec3::ZERO;
    let mut fuel_mass = 0.;
    for b in &i.plan.blocks {
        let Some(f) = i.fit.iter().find(|f| f.id == kind(&b.id)) else {
            return Err(format!("No registry equipment record for {}", b.id));
        };
        let rotation = orientation(b, f);
        if !rotation.is_finite()
            || !b.at.is_finite()
            || !b.size.is_finite()
            || b.size.min_element() <= 0.
        {
            return Err(format!("{} has invalid authored geometry", f.key));
        }
        let vertices = corners(-b.size * 0.5, b.size * 0.5)
            .into_iter()
            .map(|p| (b.at + rotation * p).as_dvec3())
            .collect();
        parts.push(vertices);
        let at = b.at.as_dvec3() - origin;
        if !f.mass.is_finite() || f.mass < 0. || !f.load.is_finite() || f.load < 0. {
            return Err(format!("{} has invalid physical mass or contents", f.key));
        }
        let mut ballast = f.load;
        dry_mass += f.mass;
        dry_first += at * f.mass;
        dry_terms.push((
            at,
            f.mass,
            b.size.as_dvec3(),
            rotation.as_dquat().normalize(),
        ));
        if let Some(h) =
            content()
                .handle::<Module>(&f.key)
                .filter(|h| match &content().get(*h).does {
                    universe_sim::world::modules::Does::Tank { holds, .. } => {
                        fuel.as_deref().is_some_and(|fuel| fuel == holds.as_str())
                    }
                    _ => true,
                })
        {
            let module = content().get(h);
            let slot = format!("part_{}", b.uid.0);
            nodes.push((format!("mount_{slot}"), b.at.as_dvec3(), DVec3::Y));
            slots.push(Slot {
                name: slot.clone(),
                kind: module.does.slot(),
                size: module.size,
                mount: None,
            });
            authored_placements.push(universe_sim::world::ship::Placed {
                slot: slot.clone(),
                module: h,
                at,
                half: b.size.as_dvec3() * 0.5,
            });
            fit.push((slot, h));
            fitted_mass += module.mass;
        }
        if let Some((kind, fields, _)) = figures(f) {
            if kind == "tank" && f.load > 0. {
                let holds = fields.get("holds").and_then(|v| v.as_str()).unwrap_or("");
                let burns = fuel.as_deref().is_some_and(|fuel| fuel == holds);
                if burns {
                    fuel_mass += f.load;
                    fuel_first += at * f.load;
                    ballast = 0.;
                }
            }
        }
        // Non-propellant contents are fixed payload in this isolated trial.
        if ballast > 0. {
            dry_mass += ballast;
            dry_first += at * ballast;
            dry_terms.push((
                at,
                ballast,
                b.size.as_dvec3(),
                rotation.as_dquat().normalize(),
            ));
        }
    }
    for beam in &i.plan.beams {
        let stock = stocks()
            .iter()
            .find(|s| s.key == beam.stock)
            .ok_or_else(|| format!("Unknown stock {}", beam.stock))?;
        let axis = beam.b - beam.a;
        let length = axis.length();
        if length <= 0. {
            continue;
        }
        let rotation = universe_engine::glam::Quat::from_rotation_arc(Vec3::Z, axis / length);
        let radius = stock.section.reach() as f32;
        let half = Vec3::new(radius, radius, length * 0.5);
        let centre = (beam.a + beam.b) * 0.5;
        parts.push(
            corners(-half, half)
                .into_iter()
                .map(|p| (centre + rotation * p).as_dvec3())
                .collect(),
        );
        let mass = stock.per_metre * f64::from(length);
        let at = centre.as_dvec3() - origin;
        dry_mass += mass;
        dry_first += at * mass;
        dry_terms.push((at, mass, (half * 2.).as_dvec3(), rotation.as_dquat()));
    }
    for plate in &i.plan.plates {
        let stock = plate_stocks()
            .iter()
            .find(|s| s.key == plate.stock)
            .ok_or_else(|| format!("Unknown plate {}", plate.stock))?;
        let lo = Vec3::new(plate.lo.x, plate.y - deck_depth(plate), plate.lo.y);
        let hi = Vec3::new(plate.hi.x, plate.y, plate.hi.y);
        parts.push(corners(lo, hi).into_iter().map(|p| p.as_dvec3()).collect());
        let mass = stock.per_square_metre * f64::from((hi.x - lo.x).abs() * (hi.z - lo.z).abs());
        let at = ((lo + hi) * 0.5).as_dvec3() - origin;
        dry_mass += mass;
        dry_first += at * mass;
        dry_terms.push((
            at,
            mass,
            (hi - lo).as_dvec3(),
            universe_engine::glam::DQuat::IDENTITY,
        ));
    }
    for (at, diameter, node) in super::nodes(&i.plan.beams, &roots(&i.plan, &i.fit)) {
        if let Some(node) = node {
            let p = at.as_dvec3() - origin;
            dry_mass += node.mass;
            dry_first += p * node.mass;
            dry_terms.push((
                p,
                node.mass,
                DVec3::splat(f64::from(diameter)),
                universe_engine::glam::DQuat::IDENTITY,
            ));
        }
    }
    let native = i.spec().map(|spec| {
        let shape = spec.shape();
        for range in &shape.part_points {
            parts.push(shape.mesh.points[range.clone()].to_vec());
        }
        let at = shape.solid.centroid - origin;
        dry_mass += spec.frame_mass;
        dry_first += at * spec.frame_mass;
        (
            at,
            spec.frame_mass,
            shape.solid.inertia * (spec.frame_mass / shape.solid.volume),
        )
    });
    if parts.is_empty() {
        return Err("The design has no collision geometry".into());
    }
    let mut shape = Shape::from_parts_at(format!("draft:{}", i.plan.hull), parts, nodes, origin)?;
    for (n, leg) in craft.legs.iter().enumerate() {
        shape.nodes.push(universe_sim::world::shape::Node {
            name: format!("gear_{n}"),
            role: universe_sim::world::shape::Role::Gear,
            at: leg.foot,
            dir: DVec3::NEG_Y,
        });
    }
    let radius = shape
        .mesh
        .points
        .iter()
        .map(|p| p.length())
        .fold(0., f64::max);
    if !dry_mass.is_finite()
        || dry_mass <= 0.
        || !fuel_mass.is_finite()
        || ((dry_mass + fuel_mass) - craft.mass).abs() > craft.mass * 1e-9
    {
        return Err("Compiled dry mass and fuel do not match the authored craft mass".into());
    }
    if fitted_mass > dry_mass + craft.mass * 1e-9 {
        return Err("Playable module mass exceeds the authored dry mass".into());
    }
    if !origin.is_finite()
        || !craft.area.is_finite()
        || craft.area <= 0.
        || !craft.steady.is_finite()
        || craft.steady < 0.
        || !craft.plants.is_finite()
        || craft.plants < 0.
    {
        return Err("Invalid authored origin, drag area or power figures".into());
    }
    let frame = HullFrame {
        frame_mass: (dry_mass - fitted_mass).max(0.),
        price: 0.,
        slots,
        nozzles: Vec::new(),
        radius,
        drag_area: craft.area,
        // This prototype has no certified hull damage budget. This is an
        // explicitly unrated collision test, not a structural qualification.
        hull_strength: f64::INFINITY,
        bay: (0., 0.),
    };
    let mut spec = ClassSpec::assemble_uncommissioned_owned(
        format!("draft:{}", i.plan.hull),
        i.plan.hull.clone(),
        Arc::new(shape),
        frame,
        fit,
    )?;
    // Preserve authored component locations; the commissioning fitter may search
    // for spare space, while an editor must simulate exactly what was placed.
    spec.placed = authored_placements;
    spec.crowded.clear();
    spec.dry_mass = dry_mass;
    spec.dry_com = dry_first / dry_mass.max(f64::MIN_POSITIVE);
    spec.dry_inertia = DMat3::ZERO;
    for (at, mass, size, rotation) in dry_terms {
        let r = at - spec.dry_com;
        let own = DMat3::from_diagonal(
            DVec3::new(
                size.y * size.y + size.z * size.z,
                size.x * size.x + size.z * size.z,
                size.x * size.x + size.y * size.y,
            ) * (mass / 12.),
        );
        let q = DMat3::from_quat(rotation);
        spec.dry_inertia += q * own * q.transpose()
            + (DMat3::IDENTITY * r.length_squared() - DMat3::from_cols(r * r.x, r * r.y, r * r.z))
                * mass;
    }
    if let Some((at, mass, own)) = native {
        let r = at - spec.dry_com;
        spec.dry_inertia += own
            + (DMat3::IDENTITY * r.length_squared() - DMat3::from_cols(r * r.x, r * r.y, r * r.z))
                * mass;
    }
    spec.fuel_capacity = fuel_mass;
    spec.fuel = fuel.unwrap_or_default();
    spec.tank_at = if fuel_mass > 0. {
        fuel_first / fuel_mass
    } else {
        spec.dry_com
    };
    spec.trim_cells = universe_sim::world::trim::cells(spec.shape(), spec.tank_at);
    spec.power_draw = craft.steady;
    // Plant efficiency comes from the shared assembler, so do not graft power
    // from an unrepresented device onto a different (or absent) plant.
    if (spec.power_output - craft.plants).abs() > craft.plants.max(1.) * 1e-9 {
        return Err("Authored power plants are not represented by the shared module fit".into());
    }
    if craft.plants > 0.
        && universe_sim::world::materials::material(&spec.fuel)
            .is_none_or(|m| m.energy <= 0. || !m.energy.is_finite())
    {
        return Err("The power plant's fuel has no usable registry energy figure".into());
    }
    spec.thrusters = craft
        .actuators
        .iter()
        .zip(&actuator_records)
        .enumerate()
        .map(|(n, (a, (key, fields)))| {
            use universe_sim::world::ship::{Thruster, ThrusterRole};
            let number = |field: &str| {
                fields
                    .get(field)
                    .and_then(|v| v.as_f64())
                    .filter(|v| v.is_finite() && *v > 0.)
                    .ok_or_else(|| format!("{key} needs a positive {field} for simulation"))
            };
            let efficiency = number("efficiency")?;
            let exhaust = number("exhaust")?;
            if efficiency > 1.
                || !a.at.is_finite()
                || !a.dir.is_finite()
                || (a.dir.length_squared() - 1.).abs() > 1e-6
                || !a.thrust.is_finite()
                || a.thrust <= 0.
            {
                return Err(format!("{key} has invalid authored thrust or efficiency"));
            }
            if matches!(a.kind, crate::test_drive::Kind::Fan { .. })
                != fields.contains_key("full_power")
            {
                return Err(format!("{key} actuator/device kind mismatch"));
            }
            Ok(Thruster {
                nozzle: format!("draft_{n}"),
                role: if a.dir.z < -0.5 {
                    ThrusterRole::Main
                } else if a.dir.y > 0.5 {
                    ThrusterRole::Lift
                } else {
                    ThrusterRole::Rcs
                },
                at: a.at,
                push: a.dir,
                thrust: a.thrust,
                exhaust,
                efficiency,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    // The same role projections and full-load turn envelope as ClassSpec::assemble,
    // recomputed after replacing its provisional mass layout and empty nozzles.
    use universe_sim::world::ship::ThrusterRole;
    let along = |role, dir: DVec3| {
        spec.thrusters
            .iter()
            .filter(|t| t.role == role)
            .map(|t| t.thrust * t.push.dot(dir).max(0.))
            .sum::<f64>()
    };
    let main = along(ThrusterRole::Main, DVec3::NEG_Z);
    let lift = along(ThrusterRole::Lift, DVec3::Y);
    let side = [DVec3::X, DVec3::NEG_X, DVec3::NEG_Y, DVec3::Z, DVec3::NEG_Z]
        .into_iter()
        .map(|dir| along(ThrusterRole::Rcs, dir))
        .fold(f64::INFINITY, f64::min);
    spec.main_thrust = main;
    spec.lift_thrust = lift;
    spec.rcs_thrust = side;
    let trim = universe_sim::world::trim::Trim::default();
    let com = universe_sim::world::trim::centre_of_mass(&spec, spec.fuel_capacity, 0., &trim);
    let inertia = universe_sim::world::trim::inertia(&spec, spec.fuel_capacity, 0., &trim);
    if !com.is_finite() || !inertia.is_finite() || inertia.determinant() <= 0. {
        return Err("Authored mass distribution has no finite, invertible inertia".into());
    }
    spec.turn_accel = universe_sim::world::thrusters::turn_envelope(
        &spec.thrusters,
        com,
        spec.dry_mass + spec.fuel_capacity,
        inertia,
    );
    Ok(Arc::new(spec))
}

/// Device limits stay individual: a warm pack cannot lend a cold pack its limit.
pub(super) fn devices(
    i: &Interior,
    craft: &crate::test_drive::Craft,
) -> Result<universe_sim::world::ship_devices::Fit, String> {
    use universe_sim::world::ship_devices::{Battery, Fan, Fit, LandingLeg};
    let mut fit = Fit::default();
    let mut thruster = 0;
    for b in &i.plan.blocks {
        let f = i
            .fit
            .iter()
            .find(|f| f.id == kind(&b.id))
            .ok_or_else(|| format!("No registry equipment record for {}", b.id))?;
        let Some((kind, fields, _)) = figures(f) else {
            continue;
        };
        let number = |key: &str| {
            fields
                .get(key)
                .and_then(|v| v.as_f64())
                .ok_or_else(|| format!("{} lacks {key}", f.key))
        };
        if push_of(b, f).is_some() {
            let actuator = craft
                .actuators
                .get(thruster)
                .ok_or("Missing authored actuator for device")?;
            if matches!(actuator.kind, crate::test_drive::Kind::Fan { .. })
                != fields.contains_key("full_power")
            {
                return Err(format!("{} actuator/device kind mismatch", f.key));
            }
            if fields.contains_key("full_power") {
                fit.fans.push(Fan {
                    thruster,
                    full_power: number("full_power")?,
                    min_density: number("min_density")?,
                    reference_density: universe_sim::physics::laws::EARTH_AIR_DENSITY,
                });
            }
            thruster += 1;
        }
        if kind == "battery" {
            let physical = &universe_sim::world::registry::registry()
                .equipment(&f.key)
                .ok_or_else(|| format!("Missing {}", f.key))?
                .physical;
            let heat = physical
                .specific_heat
                .ok_or_else(|| format!("{} needs a specific heat for simulation", f.key))?;
            let max_temperature = physical
                .operating_max_temperature
                .ok_or_else(|| format!("{} needs an operating temperature limit", f.key))?;
            fit.batteries.push(Battery {
                capacity: number("stores")?,
                rate: number("rate")?,
                efficiency: number("efficiency")?,
                heat_capacity: f.mass * heat,
                max_temperature,
            });
        }
        if let Some((holds, stroke, efficiency, sink_rate)) = f.gear {
            fit.legs.push(LandingLeg {
                foot: (b.at + orientation(b, f) * (Vec3::NEG_Y * b.size.y * 0.5)
                    - craft.visual_origin)
                    .as_dvec3(),
                holds,
                stroke,
                efficiency,
                sink_rate,
                spring: None,
                damper: None,
            });
        }
    }
    if thruster != craft.actuators.len() {
        return Err("Authored actuators and device indices do not match".into());
    }
    if fit.legs.len() != craft.legs.len() {
        return Err("Authored landing legs and device fit do not match".into());
    }
    fit.validate(craft.actuators.len())?;
    Ok(fit)
}
