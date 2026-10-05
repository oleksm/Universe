//! Settlements' ground, as the registry records it (Local Administration, read
//! from `crate::registry`; see [`from_registry`]):
//! zones, parcels, streets, power lines, and facilities with their modules laid
//! out on their parcels. Facts only: the world holds what stands where; what is
//! made there is the economy's.
//!
//! Everything is in metres [east, north] of the settlement's position, which is
//! its spaceport's pad grid centre (the game's own `Spaceport::direction`).

use glam::DVec3;

/// A settlement with ground recorded, found by its system, body and name
/// (the spaceport's).
#[derive(Clone, Debug)]
pub struct Settlement {
    pub system: String,
    pub body: String,
    pub name: String,
    pub zones: Vec<Zone>,
    pub parcels: Vec<Parcel>,
    pub streets: Vec<Street>,
    pub power_lines: Vec<PowerLine>,
    pub facilities: Vec<Facility>,
}

/// Ground set aside for a use (port, industrial, commercial, civic, residential).
#[derive(Clone, Debug)]
pub struct Zone {
    pub name: String,
    pub use_: String,
    pub outline: Vec<(f64, f64)>,
}

/// A lot of land and who owns it (a Maker House company's key; empty:
/// vacant, the land office's to sell).
#[derive(Clone, Debug)]
pub struct Parcel {
    pub number: u32,
    pub owner: String,
    pub owner_name: String,
    pub outline: Vec<(f64, f64)>,
}

#[derive(Clone, Debug)]
pub struct Street {
    pub name: String,
    pub line: Vec<(f64, f64)>,
}

/// A power line and the most it can carry (W).
#[derive(Clone, Debug)]
pub struct PowerLine {
    pub name: String,
    pub capacity: f64,
    pub line: Vec<(f64, f64)>,
}

/// A facility on a parcel: what kind, and its modules where they stand.
#[derive(Clone, Debug)]
pub struct Facility {
    pub name: String,
    pub kind: String,
    pub parcel: u32,
    /// The most it can do, as the engine works it out from its modules:
    /// what each line makes (product, kg/s), the power it draws flat out
    /// (W), the power it can supply (W), what it can hold (kg).
    pub makes: Vec<(String, f64)>,
    pub draws: f64,
    pub supplies: f64,
    pub holds: f64,
    /// What it's built of: (industrial module, how many), in the order laid out.
    pub modules: Vec<(String, u32)>,
    /// Flat out: what it takes in, gives off and burns, each (name, the
    /// market category it's traded as or empty if none, kg/s).
    pub takes: Vec<(String, String, f64)>,
    pub gives: Vec<(String, String, f64)>,
    pub burns: Vec<(String, String, f64)>,
    pub blocks: Vec<Block>,
}

/// One industrial module standing on the ground: its centre, its footprint
/// (length and width, m), its height, and which way its length runs (0 east,
/// 90 north).
#[derive(Clone, Debug)]
pub struct Block {
    pub module: String,
    pub centre: (f64, f64),
    pub length: f64,
    pub width: f64,
    pub height: f64,
    pub heading: f64,
}

impl Block {
    /// Its half sizes along east and north (m).
    pub fn half_extent(&self) -> (f64, f64) {
        if self.heading.rem_euclid(180.0) == 90.0 { (self.width / 2.0, self.length / 2.0) } else { (self.length / 2.0, self.width / 2.0) }
    }
}

impl Settlement {
    /// (A parcel's owner is any Maker House company, not only the makers the
    /// game has as brands: not checked here.)
    pub fn check(&self) -> Result<(), String> {
        for f in &self.facilities {
            if let Some((what, ..)) = f.burns.iter().find(|(_, kind, _)| kind.is_empty()) {
                return Err(format!("{}: {} burns {what}, which nothing is traded as", self.name, f.name));
            }
            if !self.parcels.iter().any(|p| p.number == f.parcel) {
                return Err(format!("{}: {} stands on no parcel {}", self.name, f.name, f.parcel));
            }
        }
        Ok(())
    }
}

/// A point `(east, north)` metres from a settlement at `dir` (unit, the
/// body's frame) on a body of `radius`: its direction (unit), as the pads'
/// are placed (`spaceport::pad_direction`).
pub fn direction(dir: DVec3, radius: f64, at: (f64, f64)) -> DVec3 {
    let (north, east) = crate::spaceport::tangent(dir);
    (dir * radius + east * at.0 + north * at.1).normalize()
}

/// Is `p` inside the polygon `outline` (even-odd)?
pub fn inside(outline: &[(f64, f64)], p: (f64, f64)) -> bool {
    let mut odd = false;
    for k in 0..outline.len() {
        let (a, b) = (outline[k], outline[(k + 1) % outline.len()]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            odd = !odd;
        }
    }
    odd
}

/// An outline's area (m², shoelace).
pub fn area(outline: &[(f64, f64)]) -> f64 {
    (0..outline.len()).map(|k| { let (a, b) = (outline[k], outline[(k + 1) % outline.len()]); a.0 * b.1 - b.0 * a.1 }).sum::<f64>().abs() / 2.0
}

/// An industrial module (the SFO's, SFO 10): what facilities are built of.
#[derive(Clone, Debug)]
pub struct IndustrialModule {
    pub key: String,
    pub name: String,
    /// Its footprint and height (m).
    pub length: f64,
    pub width: f64,
    pub height: f64,
    /// Power it needs, and supplies (W).
    pub needs: f64,
    pub supplies: f64,
    /// What it holds (kg).
    pub holds: f64,
}

/// The ground between modules, and between them and a parcel's edges (m):
/// the same invented rule the registry's build lays facilities out by.
pub const LAYOUT_GAP: f64 = 20.0;

/// Modules on a parcel, as the registry lays them out: in rows, the first
/// along the side facing the nearest of `streets`, each row behind the
/// last, each module's length along its row, `LAYOUT_GAP` round each. Only
/// rectangles squared to east and north are laid out. The blocks, or why
/// they don't fit.
pub fn lay_out(outline: &[(f64, f64)], streets: &[Street], order: &[(&IndustrialModule, u32)]) -> Result<Vec<Block>, String> {
    let (w, e) = outline.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.0), m.1.max(p.0)));
    let (sth, nth) = outline.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.1), m.1.max(p.1)));
    if outline.len() != 4 || outline.iter().any(|p| (p.0 != w && p.0 != e) || (p.1 != sth && p.1 != nth)) {
        return Err("ONLY A RECTANGLE SQUARED TO EAST AND NORTH IS LAID OUT YET".into());
    }
    // (Its front: the side nearest a street; north if there's none.)
    let gap_to = |p: (f64, f64)| streets.iter().flat_map(|s| s.line.windows(2).map(move |w| segment_distance(p, w[0], w[1]))).fold(f64::MAX, f64::min);
    let sides = [("north", ((w + e) / 2.0, nth)), ("south", ((w + e) / 2.0, sth)), ("east", (e, (sth + nth) / 2.0)), ("west", (w, (sth + nth) / 2.0))];
    let front = if streets.is_empty() { "north" } else { sides.iter().min_by(|a, b| gap_to(a.1).total_cmp(&gap_to(b.1))).map_or("north", |s| s.0) };
    let ns = front == "north" || front == "south";
    let (along, deep) = if ns { (e - w, nth - sth) } else { (nth - sth, e - w) };
    let mut rows: Vec<(Vec<(&IndustrialModule, f64)>, f64)> = Vec::new();
    let (mut row, mut at, mut depth) = (Vec::new(), LAYOUT_GAP, 0.0f64);
    for &(m, n) in order {
        for _ in 0..n {
            if m.length > along - 2.0 * LAYOUT_GAP {
                return Err(format!("A {} ({:.0} M LONG) IS LONGER THAN THE PARCEL IS WIDE", m.name.to_uppercase(), m.length));
            }
            if !row.is_empty() && at + m.length > along - LAYOUT_GAP {
                rows.push((std::mem::take(&mut row), depth));
                (at, depth) = (LAYOUT_GAP, 0.0);
            }
            row.push((m, at));
            at += m.length + LAYOUT_GAP;
            depth = depth.max(m.width);
        }
    }
    if !row.is_empty() {
        rows.push((row, depth));
    }
    let mut blocks = Vec::new();
    let mut back = LAYOUT_GAP;
    for (row, d) in rows {
        for (m, a) in row {
            let (u, v) = (a + m.length / 2.0, back + m.width / 2.0);
            let centre = match front {
                "north" => (w + u, nth - v),
                "south" => (w + u, sth + v),
                "east" => (e - v, sth + u),
                _ => (w + v, sth + u),
            };
            blocks.push(Block { module: m.key.clone(), centre, length: m.length, width: m.width, height: m.height, heading: if ns { 0.0 } else { 90.0 } });
        }
        back += d + LAYOUT_GAP;
    }
    if back > deep + 1e-6 {
        return Err(format!("ITS MODULES NEED {back:.0} M OF DEPTH IN ROWS; THE PARCEL HAS {deep:.0} M"));
    }
    Ok(blocks)
}

/// How far `p` is from the segment `a`-`b` (m).
pub fn segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) };
    ((p.0 - (a.0 + t * dx)).powi(2) + (p.1 - (a.1 + t * dy)).powi(2)).sqrt()
}

/// Do two rectangles squared to east and north overlap (by more than touching)?
pub fn rects_overlap(a: &[(f64, f64)], b: &[(f64, f64)]) -> bool {
    let bounds = |o: &[(f64, f64)]| o.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |m, p| (m.0.min(p.0), m.1.min(p.1), m.2.max(p.0), m.3.max(p.1)));
    let (a, b) = (bounds(a), bounds(b));
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// The settlements with ground and the industrial modules, from the registry:
/// each settlement's zones, parcels, streets, power lines and facilities (its
/// records filed under its key), and what each facility can do at most,
/// worked out here from its modules (see [`line_most`]). Its modules laid out
/// on its parcel by [`lay_out`].
pub fn from_registry(reg: &crate::registry::Registry) -> (Vec<Settlement>, Vec<IndustrialModule>) {
    let industry: Vec<IndustrialModule> = reg
        .modules
        .iter()
        .map(|m| {
            let p = &m.physical;
            let needs = m.needs.power.or_else(|| m.recipes.first().and_then(|r| r.power)).or_else(|| m.throughput.as_ref().and_then(|t| t.power)).unwrap_or(0.0);
            IndustrialModule {
                key: m.identity.key.clone(),
                name: m.identity.name.clone(),
                length: p.length.unwrap_or(0.0),
                width: p.width.unwrap_or(0.0),
                height: p.height.unwrap_or(0.0),
                needs,
                supplies: m.generation.as_ref().map_or(0.0, |g| g.supplies),
                holds: m.capacity.holds.unwrap_or(0.0),
            }
        })
        .collect();
    let module_of = |k: &str| reg.module(k).unwrap_or_else(|| panic!("no {k}"));
    let industrial = |k: &str| industry.iter().find(|m| m.key == k).unwrap_or_else(|| panic!("no {k}"));
    let name = |k: &str| reg.name(k).unwrap_or(k).to_string();
    let kind_of = |item: &str| reg.traded_as(item).unwrap_or_default();
    // Records filed under a settlement: `zone.treistun.port-trethi.x` is Port Trethi's.
    let under = |key: &str, s: &str| key.split_once('.').is_some_and(|(_, rest)| rest.strip_prefix(s).is_some_and(|r| r.starts_with('.')));
    let pt = |p: &[f64; 2]| (p[0], p[1]);
    let mut out = Vec::new();
    for s in reg.settlements.iter().filter(|s| s.kind == crate::registry::SettlementKind::Settlement) {
        let place = s.identity.key.split_once('.').map_or("", |(_, r)| r);
        let parcels: Vec<_> = reg.parcels.iter().filter(|p| under(&p.identity.key, place)).collect();
        let zones: Vec<_> = reg.zones.iter().filter(|z| under(&z.identity.key, place)).collect();
        if parcels.is_empty() && zones.is_empty() {
            continue;
        }
        let at = s.at.as_deref().unwrap_or_else(|| panic!("{}: at no body", s.identity.key));
        let system = at.split('.').nth(1).map(|sys| name(&format!("system.{sys}"))).unwrap_or_default();
        let streets: Vec<Street> = reg.streets.iter().filter(|r| under(&r.identity.key, place)).map(|r| Street { name: r.identity.name.clone(), line: r.line.iter().map(pt).collect() }).collect();
        let parcel_of = |key: &str| parcels.iter().find(|p| p.identity.key == key).unwrap_or_else(|| panic!("no parcel {key}"));
        let facilities = reg
            .facilities
            .iter()
            .filter(|f| under(&f.identity.key, place))
            .map(|f| {
                let lines: Vec<LineMost> = f.lines.iter().map(|ln| line_most(reg, &f.identity.key, ln)).collect();
                let built: Vec<(&crate::registry::Module, u32)> = f.modules.iter().map(|m| (module_of(&m.module), m.count)).collect();
                let flow = |(item, rate): &(String, f64)| (name(item), kind_of(item), *rate);
                let listed: Vec<(String, u32)> = lines.iter().flat_map(|l| l.modules.clone()).chain(f.modules.iter().map(|m| (m.module.clone(), m.count))).collect();
                let parcel = parcel_of(&f.parcel);
                let street: Vec<Street> = parcel.address.as_ref().and_then(|a| reg.streets.iter().find(|r| r.identity.key == a.street)).map(|r| Street { name: r.identity.name.clone(), line: r.line.iter().map(pt).collect() }).into_iter().collect();
                let order: Vec<(&IndustrialModule, u32)> = listed.iter().map(|(m, n)| (industrial(m), *n)).collect();
                let blocks = lay_out(&parcel.outline.iter().map(pt).collect::<Vec<_>>(), &street, &order).unwrap_or_else(|e| panic!("{}: {e}", f.identity.key));
                Facility {
                    name: f.identity.name.clone(),
                    kind: f.kind.as_str().to_string(),
                    parcel: parcel.number,
                    makes: lines.iter().map(|l| (l.product.clone(), l.output)).collect(),
                    draws: lines.iter().map(|l| l.power).sum::<f64>(),
                    supplies: built.iter().map(|(m, n)| m.generation.as_ref().map_or(0.0, |g| g.supplies) * *n as f64).sum::<f64>(),
                    holds: built.iter().map(|(m, n)| m.capacity.holds.unwrap_or(0.0) * *n as f64).sum::<f64>(),
                    modules: listed,
                    takes: lines.iter().flat_map(|l| &l.supplies).map(flow).collect(),
                    gives: lines.iter().flat_map(|l| &l.made).chain(lines.iter().flat_map(|l| &l.by_products)).map(flow).collect(),
                    burns: built.iter().flat_map(|(m, n)| m.generation.iter().flat_map(|g| &g.burns).map(move |b| (b.item.clone(), b.rate * *n as f64))).map(|b| flow(&b)).collect(),
                    blocks,
                }
            })
            .collect();
        out.push(Settlement {
            system,
            body: name(at),
            name: s.identity.name.clone(),
            zones: zones.iter().map(|z| Zone { name: z.identity.name.clone(), use_: z.use_.as_str().to_string(), outline: z.outline.iter().map(pt).collect() }).collect(),
            parcels: parcels
                .iter()
                .map(|p| Parcel { number: p.number, owner: p.owner.clone().unwrap_or_default(), owner_name: p.owner.as_deref().map(name).unwrap_or_default(), outline: p.outline.iter().map(pt).collect() })
                .collect(),
            streets,
            power_lines: reg.power_lines.iter().filter(|l| under(&l.identity.key, place)).map(|l| PowerLine { name: l.identity.name.clone(), capacity: l.capacity, line: l.line.iter().map(pt).collect() }).collect(),
            facilities,
        });
    }
    (out, industry)
}

/// The most a line can do, flat out (kg/s, W): what it makes, at what rate,
/// what it then takes in and gives off, the power it draws, and its modules.
#[derive(Clone, Debug)]
pub struct LineMost {
    /// What it makes, by name (a shop line: the module's name: it works whatever it's given).
    pub product: String,
    pub output: f64,
    /// (item key, kg/s).
    pub made: Vec<(String, f64)>,
    pub supplies: Vec<(String, f64)>,
    pub by_products: Vec<(String, f64)>,
    pub power: f64,
    /// (module key, how many), in its order.
    pub modules: Vec<(String, u32)>,
    /// The module that holds the line to `output` (None: a shop line).
    pub tightest: Option<String>,
    /// Each module's share of the work, in its order.
    pub rows: Vec<LineRow>,
    /// What one step gives off and another takes on the same site (kg/s).
    pub reused: Vec<(String, f64)>,
    /// The ground its modules cover (m²).
    pub area: f64,
}

/// One module of a line at its most.
#[derive(Clone, Debug)]
pub struct LineRow {
    pub module: String,
    pub count: u32,
    /// What its count can put through, set as it is (kg/s; None: it makes nothing itself).
    pub can: Option<f64>,
    /// What the line asks of it flat out (kg/s).
    pub at_full: Option<f64>,
    /// `at_full` over `can`.
    pub used: Option<f64>,
    /// m², and W.
    pub area: f64,
    pub power: f64,
}

/// A line of `facility` at its most. A line that says what it makes: back
/// from that item through its modules' recipes, each module set to the
/// recipe that makes what the next one needs (a module with recipes and none
/// that leads there is a fault in the records); then forward, each module's
/// share of the work by what the next one takes, the line held to what its
/// tightest module lets through. What one step gives off and another takes
/// on the same site is used again. A line of shop modules (no recipes: they
/// work whatever they're given) does what its modules put through.
pub fn line_most(reg: &crate::registry::Registry, facility: &str, line: &crate::registry::Line) -> LineMost {
    let module = |k: &str| reg.module(k).unwrap_or_else(|| panic!("{facility}: no {k}"));
    let modules: Vec<(String, u32)> = line.modules.iter().map(|m| (m.module.clone(), m.count)).collect();
    let Some(target) = &line.makes else {
        let rate: f64 = line.modules.iter().map(|m| module(&m.module).throughput.as_ref().map_or(0.0, |t| t.rate) * m.count as f64).sum();
        let power: f64 = line.modules.iter().map(|m| module(&m.module).throughput.as_ref().and_then(|t| t.power).unwrap_or(0.0) * m.count as f64).sum();
        let product = line.modules.iter().map(|m| module(&m.module).identity.name.clone()).collect::<Vec<_>>().join(", ");
        let rows = line
            .modules
            .iter()
            .map(|m| {
                let md = module(&m.module);
                let t = md.throughput.as_ref();
                let can = t.map(|t| t.rate * m.count as f64);
                LineRow { module: m.module.clone(), count: m.count, can, at_full: can, used: can.map(|_| 1.0), area: footprint(md) * m.count as f64, power: t.and_then(|t| t.power).unwrap_or(0.0) * m.count as f64 }
            })
            .collect::<Vec<_>>();
        let area = rows.iter().map(|r| r.area).sum();
        return LineMost { product, output: rate, made: Vec::new(), supplies: Vec::new(), by_products: Vec::new(), power, modules, tightest: None, rows, reused: Vec::new(), area };
    };
    // Back from what it makes: the recipe each module is set to.
    let steps: Vec<&str> = {
        let mut s: Vec<&str> = Vec::new();
        for m in &line.modules {
            if !s.contains(&m.module.as_str()) {
                s.push(&m.module);
            }
        }
        s
    };
    let mut need: Vec<&str> = vec![target];
    let mut chosen: Vec<Option<&crate::registry::ModuleRecipe>> = vec![None; steps.len()];
    for (i, s) in steps.iter().enumerate().rev() {
        let m = module(s);
        match m.recipes.iter().find(|r| need.contains(&r.makes.as_str())) {
            Some(r) => {
                need.extend(r.inputs.iter().filter_map(|x| x.item.as_deref()));
                chosen[i] = Some(r);
            }
            None if !m.recipes.is_empty() => panic!("{facility}: {} has no recipe that leads to {target}", m.identity.name),
            None => {}
        }
    }
    let plan = |output: f64| {
        let making: Vec<usize> = (0..steps.len()).filter(|&i| chosen[i].is_some_and(|r| r.rate.is_some())).collect();
        let made_by = |item: &str| making.iter().copied().find(|&i| chosen[i].is_some_and(|r| r.makes == item));
        let mut demand = vec![0.0; steps.len()];
        if let Some(&last) = making.last() {
            demand[last] = output;
        }
        let (mut supplies, mut by): (Vec<(String, f64)>, Vec<(String, f64)>) = (Vec::new(), Vec::new());
        let add = |list: &mut Vec<(String, f64)>, item: &str, v: f64| match list.iter_mut().find(|(k, _)| k == item) {
            Some(e) => e.1 += v,
            None => list.push((item.to_string(), v)),
        };
        for i in (0..steps.len()).rev() {
            let Some(r) = chosen[i] else { continue };
            let d = demand[i];
            for x in &r.inputs {
                let (Some(item), q) = (x.item.as_deref(), x.quantity.unwrap_or(0.0)) else { continue };
                match made_by(item) {
                    Some(src) if src < i => demand[src] += d * q,
                    _ => add(&mut supplies, item, d * q),
                }
            }
            for x in &r.outputs {
                let (Some(item), q) = (x.item.as_deref(), x.quantity.unwrap_or(0.0)) else { continue };
                add(&mut by, item, d * q);
            }
        }
        // (What is given off and needed on the same site is used again.)
        let mut reused = Vec::new();
        for (item, v) in supplies.iter_mut() {
            if let Some(b) = by.iter_mut().find(|(k, _)| k == item) {
                let used = v.min(b.1);
                *v -= used;
                b.1 -= used;
                reused.push((item.clone(), used));
            }
        }
        let powers: Vec<f64> = (0..steps.len())
            .map(|i| match chosen[i] {
                Some(r) if r.rate.is_some() => r.power.unwrap_or(0.0) * demand[i] / r.rate.unwrap(),
                _ => module(steps[i]).needs.power.unwrap_or(0.0),
            })
            .collect();
        (demand, supplies, by, powers, reused)
    };
    let has = |s: &str| line.modules.iter().filter(|m| m.module == s).map(|m| m.count).sum::<u32>() as f64;
    let (unit, ..) = plan(1.0);
    let (most, tightest) = (0..steps.len())
        .filter_map(|i| Some((chosen[i]?.rate?, unit[i], i)).filter(|(_, d, _)| *d > 0.0).map(|(rate, d, i)| (has(steps[i]) * rate / d, Some(steps[i].to_string()))))
        .fold((f64::INFINITY, None), |a, b| if b.0 < a.0 { b } else { a });
    let most = if most.is_finite() { most } else { 0.0 };
    let (demand, supplies, by, powers, reused) = plan(most);
    let rows: Vec<LineRow> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let n = has(s);
            let can = chosen[i].and_then(|r| r.rate).map(|rate| rate * n);
            let at_full = can.map(|_| demand[i]);
            LineRow { module: s.to_string(), count: n as u32, can, at_full, used: can.zip(at_full).map(|(c, a)| if c > 0.0 { a / c } else { 0.0 }), area: footprint(module(s)) * n, power: powers[i] }
        })
        .collect();
    let keep = |l: Vec<(String, f64)>| l.into_iter().filter(|(_, v)| *v > 1e-9).collect::<Vec<_>>();
    let area = rows.iter().map(|r| r.area).sum();
    LineMost { product: reg.name(target).unwrap_or(target).to_string(), output: most, made: vec![(target.clone(), most)], supplies: keep(supplies), by_products: keep(by), power: powers.iter().sum(), modules, tightest, rows, reused: keep(reused), area }
}

/// A module's footprint (m²).
fn footprint(m: &crate::registry::Module) -> f64 {
    m.physical.length.unwrap_or(0.0) * m.physical.width.unwrap_or(0.0)
}
