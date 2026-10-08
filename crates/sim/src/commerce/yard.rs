//! The yard's services: refits, hulls bought, repairs, the vending machine, trim.

use universe_protocol::ShipId;

use universe_world::Facility;


/// A module taken out at a refit fetches this share of its price.
pub const BUYBACK: f64 = 0.6;

impl crate::universe::Universe {
    /// Pilot `id` refits slot `slot` with `module` (None: empties it) where
    /// it's docked or landed. At a station, the module is brought in from
    /// outside the economy at its shipyard's price, and the one taken out
    /// fetches `BUYBACK` of its price. At a port with a market, the module is
    /// one lying in its warehouse, at the exchange's ask, and the one taken
    /// out goes into the warehouse at its bid. Refused, with the reason, if
    /// there's neither here, none in stock, the fit won't do (see
    /// `ClassSpec::assemble`), or it can't pay. The credits it cost.
    pub fn refit_as(&mut self, id: ShipId, slot: &str, module: Option<universe_world::content::Handle<universe_world::modules::Module>>) -> Result<f64, String> {
        use universe_services::{Asset, Party};
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        self.barred(id, system)?;
        let sys = self.world.system(system);
        let here = match universe_world::traffic::docked_at(&sys, ship) {
            Some(f @ Facility::Station(_)) => f,
            Some(f) if self.markets.has_market(system, f) => f,
            _ => return Err("REFIT DOCKED AT A STATION, OR LANDED AT A PORT'S MARKET".into()),
        };
        let c = universe_world::content::content();
        let mut fit = ship.fit.as_deref().cloned().unwrap_or_else(|| c.get(ship.class).fit.clone());
        let old = fit.iter().position(|(s, _)| s == slot);
        let taken = old.map(|i| fit[i].1);
        if taken == module {
            return Err("FITTED ALREADY".into());
        }
        match (old, module) {
            (Some(i), Some(m)) => fit[i].1 = m,
            (Some(i), None) => {
                fit.remove(i);
            }
            (None, Some(m)) => fit.push((slot.to_string(), m)),
            (None, None) => return Err("EMPTY ALREADY".into()),
        }
        let mut refitted = ship.clone();
        refitted.refit(fit)?;
        use universe_services::outfitter;
        let stocked = matches!(here, Facility::Spaceport(_));
        let item = |m: universe_world::content::Handle<universe_world::modules::Module>| universe_world::goods::item(&c.get(m).key);
        let (price, back) = if stocked {
            // (From the warehouse: what lies there, at the exchange's prices.)
            let price = match module {
                Some(m) => {
                    let q = item(m).and_then(|i| self.quote_for(here, i)).filter(|q| q.buy.is_some() && q.level >= 1.0).ok_or("NONE IN STOCK HERE")?;
                    q.buy.unwrap_or(0.0)
                }
                None => 0.0,
            };
            (price, taken.and_then(item).and_then(|i| self.quote_for(here, i)).map_or(0.0, |q| q.sell))
        } else {
            // (This station's shipyard's, if it carries it: brought in.)
            let price = match module {
                Some(m) => {
                    let o = outfitter::offer(self.world.galaxy.seed, &self.world.gate_links, system, here, c.get(m));
                    if !o.carried {
                        return Err("NOT CARRIED HERE".into());
                    }
                    o.price
                }
                None => 0.0,
            };
            (price, taken.map_or(0.0, |m| c.get(m).price * BUYBACK))
        };
        let cost = price - back;
        let (me, market) = (Party::Pilot(id), Party::Market(system, here));
        let cause = universe_protocol::Cause::Rules;
        self.ledger.transfer(me, market, Asset::Credits, cost, self.tick, cause)?;
        // (The module fitted comes with its mark, onto the ship; one taken out goes back with its own.)
        let mut marks: Vec<universe_world::registry::Mark> = refitted.marks.as_deref().cloned().unwrap_or_default();
        if stocked {
            if let Some(i) = module.and_then(item) {
                let (_, m, _) = self.markets.economy.take_marked(system, here, i, self.world.goods[i].mass);
                marks.extend(m);
            }
            if let Some(i) = taken.and_then(item) {
                let key = &self.world.goods[i].key;
                let back: Vec<_> = marks.iter().position(|m| &m.design == key).map(|k| marks.remove(k)).into_iter().collect();
                self.markets.economy.put_marked(system, here, i, self.world.goods[i].mass, back, Vec::new());
            }
        }
        let mut refitted = refitted;
        refitted.marks = (!marks.is_empty()).then(|| std::sync::Arc::new(marks));
        self.vessels[id].ship = refitted;
        Ok(cost)
    }

    /// The player refits slot `slot` (see `refit_as`); the cockpit is told.
    pub fn refit(&mut self, slot: &str, module: Option<universe_world::content::Handle<universe_world::modules::Module>>) -> Result<f64, String> {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Refit { slot: slot.to_string(), module }));
        let r = self.refit_as(crate::combat::PLAYER, slot, module);
        let c = universe_world::content::content();
        let e = match &r {
            Ok(credits) => universe_avionics::Event::Refitted { slot: slot.to_string(), module: module.map(|m| c.get(m).name.clone()), credits: *credits },
            Err(reason) => universe_avionics::Event::Refused { reason: format!("REFIT: {reason}") },
        };
        self.events.push(e);
        r
    }
}

impl crate::universe::Universe {
    /// What hull `hull` costs pilot `id` where it's docked or landed, and what
    /// its ship fetches in trade: at a station, the frame and its stock fit
    /// at its prices (brought in); at a port with a market, a frame lying in
    /// its warehouse at the exchange's ask, its stock fit brought in. The old
    /// ship's frame and modules at `BUYBACK`.
    pub fn hull_offer(&self, id: ShipId, hull: universe_world::ship::Hull) -> Result<(f64, f64), String> {
        use universe_services::outfitter;
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        let sys = self.world.system(system);
        let here = match universe_world::traffic::docked_at(&sys, ship) {
            Some(f @ Facility::Station(_)) => f,
            Some(f) if self.markets.has_market(system, f) => f,
            _ => return Err("BUY A SHIP DOCKED AT A STATION, OR LANDED AT A PORT'S MARKET".into()),
        };
        let c = universe_world::content::content();
        let spec = c.get(hull);
        let frame = match here {
            Facility::Spaceport(_) => self.frame_in_stock(system, here, hull).ok_or("NO SUCH HULL IN STOCK HERE")?.1,
            _ => spec.frame.price,
        };
        let price = frame + spec.fit.iter().map(|(_, m)| outfitter::offer(self.world.galaxy.seed, &self.world.gate_links, system, here, c.get(*m)).price).sum::<f64>();
        let old = ship.spec();
        let trade_in = BUYBACK * (old.frame.price + old.fit.iter().map(|(_, m)| c.get(*m).price).sum::<f64>());
        Ok((price, trade_in))
    }

    /// Pilot `id` buys hull `hull` (with its stock fit), trading in its ship,
    /// at the station it's docked at: built from the place's metals (the
    /// frame) and its modules' own materials; its cargo moves over (if the
    /// new hold takes it) and its fuel (up to the new tank). The credits it cost.
    pub fn buy_hull_as(&mut self, id: ShipId, hull: universe_world::ship::Hull) -> Result<f64, String> {
        use universe_services::{Asset, Party};
        let (price, trade_in) = self.hull_offer(id, hull)?;
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        self.barred(id, system)?;
        if ship.class == hull && ship.fit.is_none() {
            return Err("THAT'S THE SHIP YOU HAVE".into());
        }
        let sys = self.world.system(system);
        let here = universe_world::traffic::docked_at(&sys, ship).expect("docked (checked)");
        let c = universe_world::content::content();
        let spec = c.get(hull);
        if ship.cargo + ship.hopper > spec.hold_capacity + 1e-6 {
            return Err(format!("ITS HOLD TAKES {:.0} T, THERE'S {:.1} T IN YOURS", spec.hold_capacity / 1000.0, (ship.cargo + ship.hopper) / 1000.0));
        }
        // (At a station, what building it takes is brought in from outside the economy, for now.)
        let cost = price - trade_in;
        let (me, market) = (Party::Pilot(id), Party::Market(system, here));
        self.ledger.transfer(me, market, Asset::Credits, cost, self.tick, universe_protocol::Cause::Rules)?;
        // (The hull bought comes with its mark.)
        let mut hull_marks = Vec::new();
        if let Some((item, _)) = self.frame_in_stock(system, here, hull) {
            hull_marks = self.markets.economy.take_marked(system, here, item, self.world.goods[item].mass).1;
        }
        // The new ship where the old one stood, with its cargo and fuel.
        let old = self.ship_by_id(id).expect("there").2.clone();
        let mut new = old.clone();
        new.class = hull;
        new.fit = None;
        new.marks = (!hull_marks.is_empty()).then(|| std::sync::Arc::new(hull_marks));
        // (A new hull: untrimmed.)
        new.trim = Default::default();
        new.refresh();
        new.fuel = old.fuel.min(spec.fuel_capacity);
        new.hull = 1.0;
        new.jets.clear();
        // (On its own feet: the new hull may stand taller or lower.)
        self.world.resettle(system, &mut new);
        self.vessels[id].ship = new;
        Ok(cost)
    }

    /// A frame of `hull` lying in the warehouse at `here` (a port's market):
    /// its stock item and the exchange's ask. (A hull the game imported is the
    /// registry's that names its model.)
    fn frame_in_stock(&self, system: usize, here: Facility, hull: universe_world::ship::Hull) -> Option<(usize, f64)> {
        let spec = universe_world::content::content().get(hull);
        let key = universe_world::ship::hull_record(spec)?.identity.key.clone();
        let item = universe_world::goods::item(&key)?;
        let place = self.markets.economy.place(system, here)?;
        let ask = place.price(&self.world.goods[item]).ask?;
        Some((item, ask))
    }

    /// The player buys a hull (see `buy_hull_as`); the cockpit is told.
    pub fn buy_hull(&mut self, hull: universe_world::ship::Hull) -> Result<f64, String> {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::BuyHull(hull)));
        let r = self.buy_hull_as(crate::combat::PLAYER, hull);
        let name = universe_world::content::content().get(hull).name.clone();
        self.events.push(match &r {
            Ok(credits) => universe_avionics::Event::BoughtShip { name, credits: *credits },
            Err(reason) => universe_avionics::Event::Refused { reason: format!("SHIPYARD: {reason}") },
        });
        r
    }
}

/// A whole hull's repair costs this share of its frame's price.
pub const REPAIR_PRICE: f64 = 0.3;

impl crate::universe::Universe {
    /// Pilot `id`'s hull mended at the station it's docked at, as far as its
    /// credits go: `REPAIR_PRICE` of the frame's price for a whole hull.
    /// What it cost, and how far it's mended now.
    pub fn repair(&mut self, id: ShipId) -> Result<(f64, f64), String> {
        use universe_services::{Asset, Party};
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        self.barred(id, system)?;
        let sys = self.world.system(system);
        let Some(here @ Facility::Station(_)) = universe_world::traffic::docked_at(&sys, ship) else { return Err("REPAIRS DOCKED AT A STATION".into()) };
        let missing = 1.0 - ship.hull;
        if missing <= 1e-9 {
            return Err("THE HULL IS SOUND".into());
        }
        let spec = ship.spec();
        let full_price = spec.frame.price * REPAIR_PRICE;
        // As much as the credits allow (the metals brought in from outside the economy, for now).
        let credits = self.ledger.credits(Party::Pilot(id)).max(0.0);
        let part = missing.min(credits / full_price);
        if part <= 1e-6 {
            return Err("NO CREDITS FOR REPAIRS".into());
        }
        let cost = part * full_price;
        self.ledger.transfer(Party::Pilot(id), Party::Market(system, here), Asset::Credits, cost, self.tick, universe_protocol::Cause::Rules)?;
        let hull = &mut self.vessels[id].ship;
        hull.hull = (hull.hull + part).min(1.0);
        Ok((cost, hull.hull))
    }

    /// The player's hull mended (see `repair`); the cockpit is told.
    pub fn repair_player(&mut self) {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Repair));
        let e = match self.repair(crate::combat::PLAYER) {
            Ok((credits, hull)) => universe_avionics::Event::Repaired { credits, hull },
            Err(reason) => universe_avionics::Event::Refused { reason: format!("REPAIR: {reason}") },
        };
        self.events.push(e);
    }

    /// A ship's value: its frame and what's fitted, at list prices.
    pub fn ship_value(ship: &universe_world::Ship) -> f64 {
        let c = universe_world::content::content();
        let s = ship.spec();
        s.frame.price + s.fit.iter().map(|(_, m)| c.get(*m).price).sum::<f64>()
    }
}

impl crate::universe::Universe {
    /// The player, on foot by a spaceport's vending machine, buys item
    /// `item` of it (`spaceport::VENDING`): paid to the port's market.
    pub fn vend(&mut self, item: usize) {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Vend(item)));
        use universe_services::{Asset, Party};
        let e = match (self.pilot_reach(), universe_world::spaceport::VENDING.get(item)) {
            (Some(universe_world::crew::Reach::Vending(port)), Some(&(what, price, note))) => {
                let market = Party::Market(self.vessels[crate::combat::PLAYER].system, Facility::Spaceport(port));
                match self.ledger.transfer(Party::Pilot(crate::combat::PLAYER), market, Asset::Credits, price, self.tick, universe_protocol::Cause::Rules) {
                    Ok(_) => universe_avionics::Event::Vended { what: what.into(), credits: price, note: note.into() },
                    Err(_) => universe_avionics::Event::Refused { reason: "THE MACHINE WANTS CREDITS YOU HAVEN'T GOT".into() },
                }
            }
            _ => universe_avionics::Event::Refused { reason: "NO VENDING MACHINE WITHIN REACH".into() },
        };
        self.events.push(e);
    }
}

impl crate::universe::Universe {
    /// The player's ship trimmed (see `world::trim`): at a station's
    /// shipyard, where its fuel can be pumped and its drive set.
    pub fn set_trim(&mut self, trim: universe_world::trim::Trim) -> Result<(), String> {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Trim(trim.clone())));
        let sys = self.ship_system();
        let r = match universe_world::traffic::docked_at(&sys, &self.vessels[crate::combat::PLAYER].ship) {
            Some(Facility::Station(_)) => {
                self.vessels[crate::combat::PLAYER].ship.trim = trim.clamped();
                Ok(())
            }
            _ => Err("TRIM DOCKED AT A STATION".to_string()),
        };
        self.events.push(match &r {
            Ok(()) => universe_avionics::Event::Trimmed,
            Err(reason) => universe_avionics::Event::Refused { reason: reason.clone() },
        });
        r
    }
}
