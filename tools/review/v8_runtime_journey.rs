//! Standalone read-only harness linked to main's actual runtime rlibs.
//! Flight changes only through ShipCommands and World::step_ship; crew through Person::step.
use glam::DVec3;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};
use universe_avionics::{
    landing::{self, PadFrame},
    nav::Phase,
};
use universe_world::{
    Person, Place, Ship, ShipCommands, ShipState, WalkCommands, World, crew,
    worlds::{pgs::Surface, sha256},
};
fn emit(out: &mut impl Write, v: Value) {
    writeln!(out, "{}", v).unwrap();
}
fn crew_tick(
    w: &mut World,
    system: &mut usize,
    sys: &universe_world::StarSystem,
    ship: &mut Ship,
    person: &mut Person,
    c: WalkCommands,
    events: &mut Vec<universe_world::CrewEvent>,
) {
    let dt = 1. / 60.;
    let mut ship_events = Vec::new();
    w.step_ship(
        ship,
        system,
        &ShipCommands {
            power: Some(false),
            ..Default::default()
        },
        dt,
        1.,
        &mut ship_events,
    );
    assert!(
        ship_events
            .iter()
            .all(|e| matches!(e, universe_world::ShipEvent::SystemsOff)),
        "unexpected parked-ship event: {ship_events:?}"
    );
    let mut positions = Vec::new();
    sys.positions(w.time, &mut positions);
    person.step(sys, ship, w.time, &positions, &[], None, &c, dt, events);
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let hand: Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let dest = Path::new(&args[2]);
    fs::create_dir_all(dest)?;
    let mut trace = BufWriter::new(fs::File::create(dest.join("trace.jsonl"))?);
    let bytes = fs::read(Path::new(hand["package"].as_str().unwrap()).join("surface.pgs"))?;
    let hash = sha256(&bytes);
    assert_eq!(hash, hand["surface_sha256"]);
    assert_eq!(bytes.len() as u64, hand["bytes"].as_u64().unwrap());
    let started = Instant::now();
    let source = Arc::new(Surface::read(&bytes)?);
    drop(bytes);
    let key = hand["body_binding"]["key"].as_str().unwrap().to_string();
    let mut w = World::with_surfaces(
        universe_world::registry::registry().galaxy().unwrap().seed as u64,
        Arc::new([(key.clone(), source.clone())].into_iter().collect()),
    );
    let mut system = w.home_system;
    let sys = w.system(system);
    let body = sys
        .bodies
        .iter()
        .position(|b| b.key == key)
        .ok_or("body missing")?;
    let b = &sys.bodies[body];
    assert!(b.terrain.as_ref().unwrap().canonical_surface());
    w.set_time(6.3 * 3600.);
    let mut positions = Vec::new();
    sys.positions(w.time, &mut positions);
    let row = &hand["corridor"][60];
    let d = DVec3::from_array(serde_json::from_value(row["unit_direction"].clone())?);
    let up = b.rotation(w.time) * d;
    let tangent = b.rotation(w.time)
        * DVec3::from_array(serde_json::from_value(hand["site"]["east"].clone())?);
    let tangent = (tangent - up * tangent.dot(up)).normalize();
    let mode = args.get(3).map(String::as_str).unwrap_or("orbit");
    let initial_alt = if mode == "lowflight" {
        source.query(d)?.height_m + 1000.
    } else {
        100000.
    };
    let r = b.rail.radius + initial_alt;
    let circular = (b.rail.mu / r).sqrt();
    let pos = positions[body] + up * r;
    let vel = sys.velocity(body, w.time)
        + if mode == "lowflight" {
            b.angular_velocity().cross(up * r)
        } else {
            tangent * circular
        };
    let mut ship = Ship::new(
        pos,
        vel,
        universe_world::ship::facing(
            if mode == "lowflight" {
                tangent
            } else {
                -tangent
            },
            up,
        ),
    );
    let start_fuel = ship.fuel;
    let mut phase = if mode == "lowflight" {
        Phase::Descent
    } else {
        Phase::Approach
    };
    let mut events = Vec::new();
    let mut ticks = 0;
    let start_t = w.time;
    emit(
        &mut trace,
        json!({"stage":"spawn","time":w.time,"position":ship.position,"velocity":ship.velocity,"orientation":ship.orientation,"hash":hash,"radius":b.rail.radius,"source_radius":source.radius_m,"mode":mode,"altitude_datum":initial_alt,"circular_speed":circular,"ship":ship.spec().name,"fuel":ship.fuel,"main_accel":ship.main_accel(),"lift_accel":ship.lift_accel(),"ready_ms":started.elapsed().as_secs_f64()*1000.,"policy":"one explicit initial spawn; lowflight is a separate downstream test, not continuation of failed orbit; target PadFrame at canonical direction, no port/flattening/ATC mutation; no subsequent flight state resets"}),
    );
    let mut last_log = -100.;
    while ship.is_flying() && w.time - start_t < 12_000. && started.elapsed().as_secs() < 480 {
        sys.positions(w.time, &mut positions);
        let rot = b.rotation(w.time);
        let n = rot * d;
        let center = positions[body];
        let ground = b.surface_radius(d);
        let pad = PadFrame {
            body_center: center,
            body_velocity: sys.velocity(body, w.time),
            body_radius: b.rail.radius,
            mu: b.rail.mu,
            angular_velocity: b.angular_velocity(),
            pad: center + n * (ground + ship.rest_height()),
            up: n,
            rotation: rot,
            terrain_top: b.max_radius() - b.rail.radius,
        };
        let dir = rot.inverse() * (ship.position - center).normalize();
        let alt = (ship.position - center).length() - b.surface_radius(dir);
        let dt = if alt > 5000. { 0.1 } else { 0.02 };
        let gravity = universe_world::physics::gravity(&sys.bodies, ship.position, &positions);
        let c = landing::autopilot(&pad, &ship, gravity, phase, dt);
        phase = c.phase;
        let commands = if mode == "orbit" && w.time - start_t < 30. {
            ShipCommands {
                turn: Some(Default::default()),
                ..Default::default()
            }
        } else {
            c.commands()
        };
        let target_delta = ship.position - pad.pad;
        let target_h = target_delta.dot(pad.up);
        let target_horizontal = (target_delta - pad.up * target_h).length();
        let target_speed = (ship.velocity - pad.frame_velocity(ship.position)).length();
        let before = w.time;
        w.step_ship(&mut ship, &mut system, &commands, dt, 1., &mut events);
        ticks += 1;
        emit(
            &mut trace,
            json!({"stage":"flight","tick":ticks,"time":w.time,"dt":w.time-before,"commands":commands,"phase":phase,"position":ship.position,"velocity":ship.velocity,"orientation":ship.orientation,"fuel":ship.fuel,"hull":ship.hull,"state":ship.state,"events":events,"altitude_canonical_before":alt,"target_plane_height_before":target_h,"target_horizontal_before":target_horizontal,"target_speed_before":target_speed}),
        );
        events.clear();
        if w.time - start_t - last_log >= 20. {
            println!(
                "t={:.1} alt={:.1} phase={:?} fuel={:.1} state={:?}",
                w.time - start_t,
                alt,
                phase,
                ship.fuel,
                ship.state
            );
            last_log = w.time - start_t;
            trace.flush()?;
        }
        if matches!(ship.state, ShipState::Destroyed { .. }) {
            break;
        }
    }
    trace.flush()?;
    let landed = matches!(ship.state,ShipState::Landed{body:hit,..} if hit==body);
    let mut crew_events = Vec::new();
    let mut person = Person::default();
    let mut max_jump = 0.0_f64;
    let mut distance = 0.0_f64;
    let mut reboard = false;
    let mut exit = false;
    let mut all_dry = true;
    let mut step_reach = Value::Null;
    if landed {
        sys.positions(w.time, &mut positions);
        let foot_start = w.time;
        crew_tick(
            &mut w,
            &mut system,
            &sys,
            &mut ship,
            &mut person,
            WalkCommands {
                interact: true,
                ..Default::default()
            },
            &mut crew_events,
        );
        exit = crew_events
            .iter()
            .any(|e| matches!(e, universe_world::CrewEvent::SteppedOutside { .. }));
        emit(
            &mut trace,
            json!({"stage":"exit","time":w.time,"crew":person,"events":crew_events,"reach":format!("{:?}",person.reach(&sys,&ship,w.time,&positions))}),
        );
        crew_events.clear();
        let start = match person.place {
            Place::Outside { position, .. } => Some(position),
            _ => None,
        };
        for k in 0..1200 {
            let back = match (person.place, start) {
                (Place::Outside { position, .. }, Some(p)) => position.distance(p) > 0.1,
                _ => false,
            };
            let c = WalkCommands {
                forward: if k < 480 {
                    1.
                } else if k >= 600 && back {
                    -1.
                } else {
                    0.
                },
                jump: k == 510,
                ..Default::default()
            };
            crew_tick(
                &mut w,
                &mut system,
                &sys,
                &mut ship,
                &mut person,
                c,
                &mut crew_events,
            );
            if let Place::Outside { position, .. } = person.place {
                all_dry &= matches!(
                    source.query(position.normalize())?.water,
                    universe_world::worlds::pgs::Water::Dry
                );
                if (510..600).contains(&k) {
                    max_jump =
                        max_jump.max(position.length() - b.surface_radius(position.normalize()));
                }
                if let Some(p) = start {
                    distance = distance.max(p.distance(position));
                }
            }
            let contact = if let Place::Outside { position, .. } = person.place {
                let q = source.query(position.normalize())?;
                json!({"face":q.face,"canonical_height_m":q.height_m,"ground_clearance_m":position.length()-b.rail.radius-q.height_m,"dry":matches!(q.water,universe_world::worlds::pgs::Water::Dry)})
            } else {
                Value::Null
            };
            emit(
                &mut trace,
                json!({"contact":contact,"stage":"walk","tick":k,"time":w.time,"dt":1./60.,"commands":c,"crew":person,"events":crew_events}),
            );
            crew_events.clear();
        }
        sys.positions(w.time, &mut positions);
        step_reach = json!(format!(
            "{:?}",
            person.reach(&sys, &ship, w.time, &positions)
        ));
        crew_tick(
            &mut w,
            &mut system,
            &sys,
            &mut ship,
            &mut person,
            WalkCommands {
                interact: true,
                ..Default::default()
            },
            &mut crew_events,
        );
        reboard = matches!(person.place, Place::Seat | Place::Aboard { .. })
            || crew_events.iter().any(|e| {
                matches!(
                    e,
                    universe_world::CrewEvent::CameAboard | universe_world::CrewEvent::SatDown
                )
            });
        emit(
            &mut trace,
            json!({"stage":"reboard_attempt","crew":person,"events":crew_events,"reach_before":step_reach,"crew_elapsed_seconds":w.time-foot_start,"inside_hatch":crew::inside_hatch(&ship,crew::ramp_angle(&sys,&ship))}),
        );
    }
    let final_time = w.time;
    let mut live = universe_sim::universe::Universe::from_world(w);
    live.vessels[universe_sim::PLAYER].ship = ship.clone();
    live.vessels[universe_sim::PLAYER].system = system;
    live.crew = person;
    let plan = universe_world::deckplan::DeckPlan {
        hull: ship.spec().key.clone(),
        decks: Vec::new(),
    };
    live.set_layout(&plan);
    let layout_present = live.interiors.layouts.contains_key(&plan.hull);
    live.events.clear();
    if landed {
        live.walk(
            &WalkCommands {
                interact: true,
                ..Default::default()
            },
            1. / 60.,
        );
    }
    let live_check = json!({"same_world_and_ship_state":true,"fresh_isolated_game_plan":plan,"shape_walk_present":ship.spec().shape().walk.is_some(),"layout_present_after_normal_set_layout":layout_present,"crew_after_live_walk":live.crew,"events":live.events,"reboard":matches!(live.crew.place,Place::Seat|Place::Aboard{..})});
    emit(
        &mut trace,
        json!({"stage":"live_universe_reboard_check","result":live_check}),
    );
    let result = json!({"hash":hash,"base":"0c484a56","runtime":"actual main rlibs; command harness, not GUI or render validation","start_mode":mode,"initial_orbit":if mode=="orbit"{json!({"radius_m":r,"speed_m_s":circular,"seconds_coasted":30})}else{Value::Null},"simulated_total_seconds":final_time-start_t,"ticks":ticks,"wall_seconds":started.elapsed().as_secs_f64(),"landed":landed,"exit":exit,"all_walk_samples_dry":all_dry,"live_reboard_check":live_check,"walk_max_distance_m":distance,"jump_max_ground_clearance_m":max_jump,"reboard":reboard,"reach_before_reboard":step_reach,"final_ship_state":ship.state,"fuel_used":start_fuel-ship.fuel,"remaining_fuel":ship.fuel,"hull":ship.hull,"source_ready_before_first_tick":true,"state_resets_after_spawn":0,"render_contact_gate":"owned by engine; not measured here","return_orbit_and_eviction":"not attempted"});
    fs::write(
        dest.join("result.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    trace.flush()?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
