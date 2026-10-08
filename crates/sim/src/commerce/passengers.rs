//! Passage: people waiting to leave a place booked onto a pilot's seats, and landed where they're bound.

use universe_protocol::ShipId;

use universe_world::Facility;


/// A passenger's fare (credits): this much, and this much more for each
/// gate on the way (paid on arrival by the settlement fund of the place
/// they settle at).
pub const FARE: f64 = 40.0;
pub const FARE_PER_GATE: f64 = 120.0;

/// Passage booked by people waiting to leave a place: where to, how many,
/// what each pays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Booking {
    pub system: usize,
    pub to: Facility,
    pub people: u32,
    pub fare: f64,
}

impl crate::universe::Universe {
    pub(super) fn ship_mut_by_id(&mut self, id: ShipId) -> Option<&mut universe_world::Ship> {
        self.vessels.get_mut(id).map(|v| &mut v.ship)
    }

    /// The passage booked from `market` in `system`: its people waiting to
    /// leave, each bound for a place that would have them (fed, with room)
    /// in this system or one a gate away, shared out by the room there.
    pub fn bookings(&self, system: usize, market: Facility) -> Vec<Booking> {
        let Some(here) = self.markets.economy.place(system, market) else { return Vec::new() };
        let waiting = (here.waiting * 1000.0).floor();
        if waiting < 1.0 {
            return Vec::new();
        }
        let near = |s: usize| s == system || self.world.gate_links.iter().any(|&(a, b)| (a == system && b == s) || (b == system && a == s));
        let homes: Vec<(&universe_services::economy::Place, f64)> = self
            .markets
            .economy
            .places
            .iter()
            .filter(|p| near(p.system) && !(p.system == system && p.facility == market) && p.welcomes())
            .map(|p| (p, (p.founded * universe_services::economy::ROOM - p.population).max(0.0)))
            .filter(|(_, room)| *room > 0.0)
            .collect();
        let rooms: f64 = homes.iter().map(|h| h.1).sum();
        homes
            .iter()
            .filter_map(|(p, room)| {
                let people = (waiting * room / rooms).floor() as u32;
                (people > 0).then_some(Booking { system: p.system, to: p.facility, people, fare: FARE + if p.system == system { 0.0 } else { FARE_PER_GATE } })
            })
            .collect()
    }

    /// Pilot `id`, docked at `market`, takes aboard people who've booked
    /// passage to `to` (system, market): as many as there are and its
    /// cabins seat. How many.
    pub fn board_passengers(&mut self, id: ShipId, market: Facility, to: (usize, Facility)) -> Result<u32, String> {
        let Some((_, system, _)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        let sys = self.system(system);
        let ship = self.ship_by_id(id).expect("there").2;
        if universe_world::traffic::docked_at(&sys, ship) != Some(market) {
            return Err("BOARD PASSENGERS DOCKED OR LANDED".into());
        }
        if ship.spec().seats == 0 {
            return Err("NO PASSENGER CABIN FITTED".into());
        }
        if ship.passengers > 0 && ship.bound_for != Some(to) {
            return Err("THE PASSENGERS ABOARD ARE BOUND ELSEWHERE".into());
        }
        let seats = ship.passenger_room();
        let Some(b) = self.bookings(system, market).into_iter().find(|b| (b.system, b.to) == to) else { return Err("NOBODY BOOKED FOR THERE".into()) };
        let n = b.people.min(seats);
        if n == 0 {
            return Err("EVERY SEAT TAKEN".into());
        }
        if let Some(place) = self.markets.economy.place_mut(system, market) {
            place.depart(n as f64 / 1000.0);
        }
        let ship = self.ship_mut_by_id(id).expect("there");
        ship.passengers += n;
        ship.bound_for = Some(to);
        ship.fare = b.fare;
        Ok(n)
    }

    /// Pilot `id`, docked at `market`, lands its passengers where they
    /// booked passage to (if it still has room for them): their fares paid
    /// by its settlement fund. The fares.
    pub fn land_passengers(&mut self, id: ShipId, market: Facility) -> Result<f64, String> {
        use universe_services::{Asset, Party};
        let Some((_, system, _)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        let sys = self.system(system);
        let ship = self.ship_by_id(id).expect("there").2;
        if universe_world::traffic::docked_at(&sys, ship) != Some(market) {
            return Err("LAND PASSENGERS DOCKED OR LANDED".into());
        }
        let (n, fare) = (ship.passengers, ship.fare);
        if n == 0 {
            return Err("NO PASSENGERS ABOARD".into());
        }
        if ship.bound_for != Some((system, market)) {
            return Err("THEY BOOKED PASSAGE ELSEWHERE".into());
        }
        let Some(place) = self.markets.economy.place_mut(system, market) else { return Err("NOBODY LIVES HERE".into()) };
        place.arrive(n as f64 / 1000.0);
        let paid = fare * n as f64;
        self.ledger.transfer(Party::World, Party::Pilot(id), Asset::Credits, paid, self.tick, universe_protocol::Cause::Rules).map_err(|e| format!("{e:?}"))?;
        let ship = self.ship_mut_by_id(id).expect("there");
        ship.passengers = 0;
        ship.bound_for = None;
        ship.fare = 0.0;
        Ok(paid)
    }

    /// The player, docked: lands the passengers aboard where they're bound
    /// (if this is it), or boards those booked for `to`.
    pub fn passengers(&mut self, to: Option<(usize, Facility)>) {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Passengers(to)));
        let sys = self.ship_system();
        let Some(market) = universe_world::traffic::docked_at(&sys, &self.vessels[crate::combat::PLAYER].ship) else {
            self.events.push(universe_avionics::Event::Refused { reason: "PASSENGERS DOCKED OR LANDED".into() });
            return;
        };
        let e = match to {
            None => {
                let n = self.vessels[crate::combat::PLAYER].ship.passengers;
                match self.land_passengers(crate::combat::PLAYER, market) {
                    Ok(credits) => universe_avionics::Event::PassengersLanded { count: n, credits },
                    Err(reason) => universe_avionics::Event::Refused { reason },
                }
            }
            Some(to) => match self.board_passengers(crate::combat::PLAYER, market, to) {
                Ok(count) => universe_avionics::Event::PassengersBoarded { count },
                Err(reason) => universe_avionics::Event::Refused { reason },
            },
        };
        self.events.push(e);
    }
}
