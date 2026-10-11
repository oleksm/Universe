// Frozen pre-extraction Ship::drive oracle. Keep independent of flight_control helpers.
use super::*;
fn legacy_drive(ship: &mut Ship, turn: Option<&Controls>, dt: f64, push: bool) {
        if ship.manual && push {
            return legacy_manual(ship, dt);
        }
        let s = ship.spec();
        let inertia = ship.inertia();
        let w = ship.angular_velocity;
        // The push: the main drive along the nose, the thrusters as set — as
        // much as they give without turning the ship (off balance, less: the
        // flight computer keeps it straight first).
        let c = ship.rcs.clamp(DVec3::splat(-1.0), DVec3::ONE);
        let a = if push { ship.authority() } else { Authority { main: 0.0, lift: 0.0, side: 0.0 } };
        let force = if push { DVec3::NEG_Z * (ship.throttle.clamp(0.0, 1.0) * a.main) + DVec3::new(c.x * a.side, c.y * if c.y > 0.0 { a.lift } else { a.side }, c.z * a.side) } else { DVec3::ZERO };
        // The turn: toward the rates asked, as fast as the span allows.
        let want = turn.map(|t| DVec3::new(t.pitch.clamp(-1.0, 1.0) * s.turn_rate, t.yaw.clamp(-1.0, 1.0) * s.turn_rate, t.roll.clamp(-1.0, 1.0) * s.roll_rate));
        let torque = want.map_or(DVec3::ZERO, |want| inertia * ((want - w) / dt.max(TURN_RESPONSE)));
        if ship.fuel <= 0.0 || (force == DVec3::ZERO && torque.length_squared() < 1e-6) {
            // (Nothing asked, or nothing to burn: every jet off.)
            ship.jets.iter_mut().for_each(|j| *j = 0.0);
            ship.applied = (DVec3::ZERO, DVec3::ZERO);
        } else {
            // The drive at the throttle (its nozzles together, as far as it
            // goes straight); the thrusters and lift do the rest.
            let level = if push { ship.throttle.clamp(0.0, 1.0) * a.main / s.main_thrust.max(1.0) } else { 0.0 };
            let thrusters = crate::trim::thrusters(s, &ship.trim);
            ship.applied = crate::thrusters::allocate_with(&thrusters, ship.centre_of_mass(), ship.mass(), inertia, Some(level), force, torque, &mut ship.jets);
        }
        // Turning: I ω̇ = τ. (The spin's own coupling, ω × Iω, is left out:
        // the flight computer holds against it when it turns the ship, and a
        // ship tumbling free keeps its spin about its own axes — near enough
        // for a hull this close to symmetric, and steady over the long steps
        // the planner takes.) Never past the rates asked in one span.
        let accel = inertia.inverse() * ship.applied.1;
        let mut next = w + accel * dt;
        if let Some(want) = want {
            let toward = |w: f64, n: f64, t: f64| if (t - w) * (t - n) < 0.0 { t } else { n };
            next = DVec3::new(toward(w.x, next.x, want.x), toward(w.y, next.y, want.y), toward(w.z, next.z, want.z));
        }
        ship.angular_velocity = next;
        ship.orientation = (ship.orientation * DQuat::from_scaled_axis(ship.angular_velocity * dt)).normalize();
    }


fn legacy_manual(ship: &mut Ship, dt: f64) {
        let s = ship.spec();
        let com = ship.centre_of_mass();
        ship.jets.resize(s.thrusters.len(), 0.0);
        let (mut force, mut torque) = (DVec3::ZERO, DVec3::ZERO);
        let thrusters = crate::trim::thrusters(s, &ship.trim);
        for (k, t) in thrusters.iter().enumerate() {
            let on = k < 64 && ship.held & (1 << k) != 0 && ship.fuel > 0.0;
            ship.jets[k] = if on { 1.0 } else { 0.0 };
            if on {
                let f = t.push * t.thrust;
                force += f;
                torque += (t.at - com).cross(f);
            }
        }
        ship.applied = (force, torque);
        ship.angular_velocity += ship.inertia().inverse() * torque * dt;
        ship.orientation = (ship.orientation * DQuat::from_scaled_axis(ship.angular_velocity * dt)).normalize();
    }

#[test]
fn live_drive_matches_frozen_controller_over_mixed_spans() {
    for manual in [false, true] {
        for fuelled in [false, true] {
            let mut actual = Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::from_rotation_y(0.7));
            actual.manual = manual;
            actual.cargo = actual.spec().hold_capacity * 0.37;
            if !fuelled { actual.fuel = 0.0; }
            actual.angular_velocity = DVec3::new(0.3, -0.2, 0.1);
            let mut reference = actual.clone();
            for k in 0..96 {
                let x = k as f64 * 0.37;
                let controls = Controls { pitch: x.sin()*1.4, yaw: x.cos()*1.3, roll: (x*2.).sin()*1.2, ..Default::default() };
                let turn = (k % 5 != 0).then_some(&controls);
                let push = k % 4 != 0;
                let dt = [0.0, 0.001, 0.016, 0.12, 0.5, 2.0][k % 6];
                for s in [&mut actual, &mut reference] {
                    s.throttle = x.cos()*0.8 + 0.5;
                    s.rcs = DVec3::new(x.sin()*1.3, x.cos(), (x*0.3).sin());
                    s.held = if k % 2 == 0 { 0x5555555555555555 } else { u64::MAX };
                }
                actual.drive(turn, dt, push);
                legacy_drive(&mut reference, turn, dt, push);
                assert_eq!(actual.jets, reference.jets, "jets manual={manual} fuel={fuelled} span={k}");
                assert_eq!(actual.applied, reference.applied, "force/torque span={k}");
                assert_eq!(actual.angular_velocity, reference.angular_velocity, "spin span={k}");
                assert_eq!(actual.orientation, reference.orientation, "orientation span={k}");
            }
        }
    }
}

#[test]
fn arbitrary_body_derating_and_realized_torque_use_same_integration() {
    use crate::flight_control::{self as fc, Body, Demand};
    let inertia = DMat3::from_cols(DVec3::new(40.,2.,1.),DVec3::new(2.,60.,3.),DVec3::new(1.,3.,80.));
    let body = Body { mass: 20., com: DVec3::new(0.3,0.2,-0.1), inertia, spin: DVec3::new(0.2,-0.1,0.4) };
    let actuator = Thruster { nozzle:"arbitrary".into(),role:ThrusterRole::Lift,at:DVec3::X,push:DVec3::Y,thrust:5.,exhaust:0.,efficiency:1. };
    let mut jets=vec![0.7];
    let demand=Demand { force:DVec3::Y*1e6,want_rates:Some(DVec3::ZERO),drive:None,enabled:true };
    let result=fc::step(&[actuator.clone()],body,demand,0.02,&mut jets);
    assert!(jets[0]>=0. && jets[0]<=1.);
    assert!(result.force.length()<=5.);
    assert_eq!(result.torque,(actuator.at-body.com).cross(result.force));
    // Resource limiting happens after allocation: integrate delivered torque, not requested torque.
    let realized=result.torque*0.25;
    let spin=fc::integrate_rates(inertia,body.spin,realized,None,0.02);
    assert_eq!(spin,body.spin+inertia.inverse()*realized*0.02);
    let off=fc::step(&[actuator],body,Demand{enabled:false,..demand},0.02,&mut jets);
    assert_eq!(jets,vec![0.]);assert_eq!(off.force,DVec3::ZERO);assert_eq!(off.spin,body.spin);
    assert_eq!(fc::rates(DVec3::new(-2.,0.5,4.),DVec3::new(1.,2.,3.)),DVec3::new(-1.,1.,3.));
    assert_eq!(fc::integrate_rates(DMat3::IDENTITY,DVec3::ZERO,DVec3::splat(10.),Some(DVec3::ONE),1.),DVec3::ONE);
}
