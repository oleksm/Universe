//! Optional physical devices on a compiled ship. Shared by every live Ship;
//! no editor policy and no second rigid-body integrator.
use crate::ship::{Authority, Controls, Ship, Thruster};
use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fan {
    /// Index in the compiled ClassSpec thruster list.
    pub thruster: usize,
    pub full_power: f64,
    pub min_density: f64,
    pub reference_density: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Battery {
    pub capacity: f64,
    pub rate: f64,
    pub efficiency: f64,
    /// Whole pack J/K and absolute thermal cutoff K, from its physical record.
    pub heat_capacity: f64,
    pub max_temperature: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LandingLeg {
    /// Same authored coordinate frame as the ClassSpec shape/COM.
    pub foot: DVec3,
    pub holds: f64,
    pub stroke: f64,
    pub efficiency: f64,
    pub sink_rate: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Fit {
    pub fans: Vec<Fan>,
    pub batteries: Vec<Battery>,
    pub legs: Vec<LandingLeg>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackState {
    pub energy: f64,
    pub temperature: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub fit: Fit,
    pub packs: Vec<PackState>,
    /// Current atmospheric density, sampled by the World before allocation.
    #[serde(default)]
    pub density: f64,
    /// Last bus demand/delivery for presentation; not another control input.
    #[serde(default)]
    pub demand_w: f64,
    #[serde(default)]
    pub delivered_w: f64,
    #[serde(skip)]
    pack_w: Vec<f64>,
    #[serde(skip)]
    pub(crate) plant_fuel_rate: f64,
    #[serde(skip)]
    pub(crate) rocket_fuel_rate: f64,
    #[serde(skip)]
    pub(crate) capacitor_w: f64,
    #[serde(skip)]
    authority_cache: Option<(Vec<u64>, Authority)>,
}
impl Fit {
    pub fn validate(&self, thrusters: usize) -> Result<(), String> {
        let positive = |v: f64| v.is_finite() && v > 0.;
        let mut seen = std::collections::HashSet::new();
        for f in &self.fans {
            if f.thruster >= thrusters
                || !seen.insert(f.thruster)
                || !positive(f.full_power)
                || !positive(f.reference_density)
                || !f.min_density.is_finite()
                || f.min_density < 0.
            {
                return Err("invalid or duplicate fan specification".into());
            }
        }
        for b in &self.batteries {
            if ![
                b.capacity,
                b.rate,
                b.efficiency,
                b.heat_capacity,
                b.max_temperature,
            ]
            .into_iter()
            .all(positive)
                || b.efficiency > 1.
            {
                return Err("invalid battery specification".into());
            }
        }
        for l in &self.legs {
            if !l.foot.is_finite()
                || ![l.holds, l.stroke, l.efficiency, l.sink_rate]
                    .into_iter()
                    .all(positive)
                || l.efficiency > 1.
            {
                return Err("invalid landing leg specification".into());
            }
        }
        Ok(())
    }
    /// Existing live landing policy: equal leg loading, conservatively limited
    /// by the weakest leg and shortest effective stroke. This is not a spring solver.
    pub fn landing(&self) -> Option<crate::legs::Legs> {
        if self.legs.is_empty() {
            return None;
        }
        Some(crate::legs::Legs {
            force: self
                .legs
                .iter()
                .map(|l| l.holds)
                .fold(f64::INFINITY, f64::min)
                * self.legs.len() as f64,
            stroke: self
                .legs
                .iter()
                .map(|l| l.stroke * l.efficiency)
                .fold(f64::INFINITY, f64::min),
            efficiency: 1.,
            designed: self
                .legs
                .iter()
                .map(|l| l.sink_rate)
                .fold(f64::INFINITY, f64::min),
        })
    }
}
impl State {
    pub fn new(fit: Fit, thrusters: usize, charge: f64, temperature: f64) -> Result<Self, String> {
        fit.validate(thrusters)?;
        if !charge.is_finite()
            || !(0.0..=1.0).contains(&charge)
            || !temperature.is_finite()
            || temperature <= 0.
        {
            return Err("invalid initial battery state".into());
        }
        let packs = fit
            .batteries
            .iter()
            .map(|b| PackState {
                energy: b.capacity * charge,
                temperature,
            })
            .collect();
        Ok(Self {
            fit,
            packs,
            density: 0.,
            demand_w: 0.,
            delivered_w: 0.,
            pack_w: Vec::new(),
            plant_fuel_rate: 0.,
            rocket_fuel_rate: 0.,
            capacitor_w: 0.,
            authority_cache: None,
        })
    }
    /// Per-ship, bounded to one exact solver input. Spin, time, battery energy
    /// and temperature are not authority inputs; their limits remain in plan().
    /// Mass, COM, full inertia and every effective actuator column/role are.
    fn authority(&mut self, ts: &[Thruster], body: crate::flight_control::Body) -> Authority {
        let key: Vec<u64> = std::iter::once(body.mass)
            .chain(body.com.to_array()).chain(body.inertia.to_cols_array())
            .map(f64::to_bits)
            .chain(ts.iter().flat_map(|t| t.at.to_array().into_iter()
                .chain(t.push.to_array()).chain([t.thrust]).map(f64::to_bits)
                .chain(std::iter::once(t.role as u64))))
            .collect();
        if let Some((old, result)) = &self.authority_cache {
            if *old == key { return *result; }
        }
        let _scope = universe_prof::scope("sim/devices/authority");
        let result = authority(ts, body);
        self.authority_cache = Some((key, result));
        result
    }

    pub fn is_fan(&self, index: usize) -> bool {
        self.fit.fans.iter().any(|f| f.thruster == index)
    }
    fn available(&self, dt: f64) -> Vec<f64> {
        self.fit
            .batteries
            .iter()
            .zip(&self.packs)
            .map(|(b, p)| {
                if p.temperature >= b.max_temperature || p.energy <= 0. {
                    return 0.;
                }
                let heat_per_bus = 1. / b.efficiency - 1.;
                let thermal = if heat_per_bus > 0. {
                    (b.max_temperature - p.temperature) * b.heat_capacity / (dt * heat_per_bus)
                } else {
                    f64::INFINITY
                };
                b.rate
                    .min(p.energy * b.efficiency / dt)
                    .min(thermal)
                    .max(0.)
            })
            .collect()
    }
    fn consume(&mut self, dt: f64) {
        for ((b, p), w) in self
            .fit
            .batteries
            .iter()
            .zip(&mut self.packs)
            .zip(&self.pack_w)
        {
            let used = (w * dt / b.efficiency).min(p.energy).max(0.);
            p.energy -= used;
            p.temperature += used * (1. - b.efficiency) / b.heat_capacity;
        }
    }
    fn effective(&self, ts: &mut [Thruster], fuel: f64, powered: bool) {
        for (i, t) in ts.iter_mut().enumerate() {
            if let Some(f) = self.fit.fans.iter().find(|f| f.thruster == i) {
                let factor = if powered && self.density.is_finite() && self.density >= f.min_density
                {
                    (self.density / f.reference_density).cbrt().clamp(0., 1.)
                } else {
                    0.
                };
                t.thrust *= factor;
            } else if fuel <= 0. || !powered {
                t.thrust = 0.;
            }
        }
    }
    /// Limit actual fractions by finite fuel, bus power, pack energy and thermal
    /// headroom. Energy is debited only for spans actually integrated by Devices.
    fn plan(
        &mut self,
        ts: &[Thruster],
        jets: &mut [f64],
        dt: f64,
        fuel: f64,
        plant: f64,
        per_kg: f64,
        steady: f64,
        capacitor_w: f64,
    ) {
        for (t, j) in ts.iter().zip(jets.iter_mut()) {
            if t.thrust <= 0. {
                *j = 0.;
            }
        }
        let rocket_rate: f64 = ts
            .iter()
            .zip(jets.iter())
            .enumerate()
            .filter(|(i, _)| !self.is_fan(*i))
            .map(|(_, (t, j))| {
                if t.exhaust > 0. {
                    t.thrust * j / t.exhaust
                } else {
                    0.
                }
            })
            .sum();
        let fuel_factor = if rocket_rate > 0. {
            (fuel / (rocket_rate * dt)).clamp(0., 1.)
        } else {
            1.
        };
        for (i, j) in jets.iter_mut().enumerate() {
            if !self.is_fan(i) {
                *j *= fuel_factor;
            }
        }
        self.rocket_fuel_rate = rocket_rate * fuel_factor;
        let remaining_rate = (fuel / dt - self.rocket_fuel_rate).max(0.);
        let plant = if per_kg > 0. {
            plant.min(remaining_rate * per_kg)
        } else {
            0.
        };
        let available = self.available(dt);
        let bus = plant + available.iter().sum::<f64>();
        let wanted: f64 = self
            .fit
            .fans
            .iter()
            .map(|f| f.full_power * jets[f.thruster].powf(1.5))
            .sum();
        let factor = if wanted > 0. {
            ((bus - steady).max(0.) / wanted).min(1.).powf(2. / 3.)
        } else {
            1.
        };
        for f in &self.fit.fans {
            jets[f.thruster] *= factor;
        }
        self.demand_w = steady + wanted;
        self.delivered_w = steady.min(bus) + wanted * factor.powf(1.5);
        let from_plant = self.delivered_w.min(plant);
        self.capacitor_w = capacitor_w.min((plant - from_plant).max(0.));
        self.plant_fuel_rate = if per_kg > 0. {
            (from_plant + self.capacitor_w) / per_kg
        } else {
            0.
        };
        let from_pack = (self.delivered_w - from_plant).max(0.);
        let total = available.iter().sum::<f64>();
        self.pack_w = available
            .iter()
            .map(|a| {
                if total > 0. {
                    a * from_pack / total
                } else {
                    0.
                }
            })
            .collect();
    }
}

fn authority(ts: &[Thruster], body: crate::flight_control::Body) -> Authority {
    let straight = crate::thrusters::STRAIGHT
        * crate::thrusters::turn_envelope(ts, body.com, body.mass, body.inertia).min_element();
    let push = |d: DVec3, main: bool| {
        let full = ts.iter().map(|t| (t.push.dot(d) * t.thrust).max(0.)).sum();
        crate::thrusters::balanced_with(
            ts,
            body.com,
            body.mass,
            body.inertia,
            d,
            full,
            straight,
            main,
        )
    };
    Authority {
        main: push(DVec3::NEG_Z, true),
        lift: push(DVec3::Y, false),
        side: [DVec3::X, DVec3::NEG_X, DVec3::NEG_Y, DVec3::Z, DVec3::NEG_Z]
            .into_iter()
            .map(|d| push(d, false))
            .fold(f64::INFINITY, f64::min),
    }
}

impl Ship {
    pub fn configure_devices(
        &mut self,
        fit: Fit,
        charge: f64,
        temperature: f64,
    ) -> Result<(), String> {
        self.devices = Some(State::new(
            fit,
            self.spec().thrusters.len(),
            charge,
            temperature,
        )?);
        Ok(())
    }
    pub(crate) fn drive_devices(&mut self, turn: Option<&Controls>, dt: f64, push: bool) {
        if !dt.is_finite() || dt <= 0. {
            self.applied = (DVec3::ZERO, DVec3::ZERO);
            self.jets.fill(0.);
            let s = self.devices.as_mut().unwrap();
            s.pack_w.fill(0.);
            s.plant_fuel_rate = 0.;
            s.rocket_fuel_rate = 0.;
            s.capacitor_w = 0.;
            return;
        }
        let mut state = self.devices.take().unwrap();
        let spec = self.spec();
        let mut ts = crate::trim::thrusters(spec, &self.trim).into_owned();
        state.effective(&mut ts, self.fuel, self.powered);
        let per_kg =
            crate::materials::material(&spec.fuel).map_or(0., |m| m.energy) * spec.plant_efficiency;
        let cap_w = if self.powered && !self.hyperdrive {
            ((spec.capacitor_capacity - self.energy).max(0.) / dt).min(spec.capacitor_rate)
        } else {
            0.
        };
        let (output, draw, turn_rate, roll_rate) = (
            spec.power_output,
            spec.power_draw,
            spec.turn_rate,
            spec.roll_rate,
        );
        let body = crate::flight_control::Body {
            mass: self.mass(),
            com: self.centre_of_mass(),
            inertia: self.inertia(),
            spin: self.angular_velocity,
        };
        let want = turn.filter(|_| !self.manual).map(|t| {
            crate::flight_control::rates(
                DVec3::new(t.pitch, t.yaw, t.roll),
                DVec3::new(turn_rate, turn_rate, roll_rate),
            )
        });
        self.jets.resize(ts.len(), 0.);
        if self.manual && push {
            crate::flight_control::manual(&ts, body, self.held, self.powered, dt, &mut self.jets);
        } else {
            // No translational request needs no envelope. Angular control still
            // uses the live allocator below. Never solve disabled hardware.
            let a = if self.powered && push && (self.throttle > 0. || self.rcs != DVec3::ZERO) {
                state.authority(&ts, body)
            } else {
                Authority { main: 0., lift: 0., side: 0. }
            };
            let main = ts
                .iter()
                .filter(|t| t.role == crate::ship::ThrusterRole::Main)
                .map(|t| t.thrust * (-t.push.z).max(0.))
                .sum();
            let (force, level) =
                crate::flight_control::force_demand(self.throttle, self.rcs, a, main, push);
            crate::flight_control::step(
                &ts,
                body,
                crate::flight_control::Demand {
                    force,
                    want_rates: want,
                    drive: Some(level),
                    enabled: self.powered,
                },
                dt,
                &mut self.jets,
            );
        }
        // Only committed, post-limit torque enters the existing angular integrator.
        state.plan(
            &ts,
            &mut self.jets,
            dt,
            self.fuel,
            if self.powered { output } else { 0. },
            per_kg,
            if self.powered { draw } else { 0. },
            cap_w,
        );
        let (mut force, mut torque) = (DVec3::ZERO, DVec3::ZERO);
        for (t, j) in ts.iter().zip(&self.jets) {
            let f = t.push * t.thrust * j;
            force += f;
            torque += (t.at - body.com).cross(f);
        }
        self.applied = (force, torque);
        self.angular_velocity =
            crate::flight_control::integrate_rates(body.inertia, body.spin, torque, want, dt);
        self.orientation =
            (self.orientation * DQuat::from_scaled_axis(self.angular_velocity * dt)).normalize();
        self.devices = Some(state);
    }
    pub(crate) fn consume_devices(&mut self, dt: f64) {
        if let Some(state) = &mut self.devices {
            state.consume(dt);
        }
    }
}

#[cfg(test)]
mod authority_tests {
    use super::*;
    #[test]
    fn exact_authority_cache_tracks_physical_inputs_not_spin() {
        use crate::flight_control::Body;
        use crate::ship::ThrusterRole;
        let mut state = State::new(Fit::default(), 4, 1., 293.).unwrap();
        let mut ts: Vec<_> = [(-2.,-2.),(-2.,2.),(2.,-2.),(2.,2.)].into_iter().map(|(x,z)| Thruster {
            nozzle: "test".into(), at:DVec3::new(x,0.,z), push:DVec3::Y,
            thrust:5000., role:ThrusterRole::Lift, exhaust:0., efficiency:1.,
        }).collect();
        let mut b = Body { mass:1000., com:DVec3::ZERO, inertia:glam::DMat3::IDENTITY*4000., spin:DVec3::ZERO };
        let cold = std::time::Instant::now();
        let expected = authority(&ts,b);
        let cold = cold.elapsed();
        assert_eq!(state.authority(&ts,b),expected);
        let warm = std::time::Instant::now();
        for _ in 0..1000 { assert_eq!(state.authority(&ts,b),expected); }
        println!("authority cold={cold:?};1000 exact hits={:?}",warm.elapsed());
        let old = state.authority_cache.clone().unwrap().0;
        b.spin = DVec3::ONE;
        assert_eq!(state.authority(&ts,b),expected);
        assert_eq!(state.authority_cache.as_ref().unwrap().0,old);
        for case in 0..6 {
            let prior=state.authority_cache.as_ref().unwrap().0.clone();
            match case {
                0=>b.mass+=1., 1=>b.com.x+=0.01, 2=>b.inertia.x_axis.x+=1.,
                3=>ts[0].thrust*=0.9, 4=>ts[0].at.z+=0.01,
                _=>ts[0].role=ThrusterRole::Rcs,
            }
            assert_eq!(state.authority(&ts,b),authority(&ts,b));
            assert_ne!(state.authority_cache.as_ref().unwrap().0,prior);
        }
    }
}
