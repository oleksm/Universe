//! Physical handling units, stamped at sealing (docs/packing-contract.md).
//! Quantities and lots remain the inventory's source of truth. These units describe the
//! packaging around those kilograms; opening one removes its tare and ends its label.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use universe_world::registry::{Batch, Label, LabelBarcode};

pub type Units = VecDeque<Unit>;
pub type Sequences = BTreeMap<String, u16>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    pub batch: Batch,
    pub product: String,
    pub brand: String,
    pub template: String,
    pub net_mass: f64,
    /// Capacities and tare at packing: subsequent catalogue changes do not rewrite history.
    pub capacity: f64,
    pub tare: f64,
    pub volume: f64,
    /// None while filling. Once sealed the label is immutable.
    pub label: Option<Label>,
}

#[derive(Clone, Debug)]
pub struct Spec {
    pub template: String,
    pub capacity: f64,
    pub tare: f64,
    pub volume: f64,
}

impl Spec {
    /// Use the same registry-derived stowage density as the cargo simulation (kg/m³).
    pub fn for_stock(key: &str, density: f64) -> Option<Self> {
        let reg = universe_world::registry::registry();
        let template = reg.stock(key)?.package.as_ref()?;
        let p = reg.package(template)?;
        let capacity = p.holds.mass.min(p.holds.volume * density);
        let s = Self {
            template: template.clone(),
            capacity,
            tare: p.physical.mass,
            volume: p.physical.length * p.physical.width * p.physical.height,
        };
        (density.is_finite() && density > 0.0 && s.valid()).then_some(s)
    }

    fn valid(&self) -> bool {
        [self.capacity, self.tare, self.volume]
            .iter()
            .all(|x| x.is_finite() && *x > 0.0)
    }
}

impl Unit {
    pub fn gross_mass(&self) -> f64 {
        self.net_mass + self.tare
    }

    /// False means this lot has exhausted its four-digit suffix; never wrap it.
    pub fn seal(&mut self, at: f64, sequences: &mut Sequences) -> bool {
        if self.label.is_some() {
            return true;
        }
        if !at.is_finite() || at < self.batch.made_on || self.net_mass <= 0.0 {
            return false;
        }
        let last = sequences.entry(self.batch.lot.clone()).or_default();
        if *last >= 9999 {
            return false;
        }
        *last += 1;
        self.label = Some(Label {
            brand: self.brand.clone(),
            product: self.product.clone(),
            lot: self.batch.lot.clone(),
            serial: format!("{}-{:04}", self.batch.lot, *last),
            made_at: self.batch.made_at.clone(),
            made_on: self.batch.made_on,
            packed_on: at,
            net_mass: self.net_mass,
            gross_mass: self.gross_mass(),
            package: self.template.clone(),
            barcode: Some(LabelBarcode::Code128),
        });
        true
    }
}

/// Fill at most one lot. Returns kilograms still unpacked when suffix 9999 closes the lot.
/// The caller opens a new batch for those kilograms. Invalid input is left untouched.
pub fn fill(
    units: &mut Units,
    sequences: &mut Sequences,
    spec: &Spec,
    batch: &Batch,
    product: &str,
    brand: &str,
    kg: f64,
    at: f64,
) -> f64 {
    if !spec.valid() || !kg.is_finite() || kg <= 0.0 || !at.is_finite() || at < batch.made_on {
        return kg;
    }
    let mut left = kg;
    while left > 0.0 {
        if sequences.get(&batch.lot).copied().unwrap_or(0) >= 9999 {
            break;
        }
        let open = units.iter().position(|u| {
            u.label.is_none()
                && u.batch.lot == batch.lot
                && u.product == product
                && u.template == spec.template
                && u.brand == brand
        });
        let i = open.unwrap_or_else(|| {
            units.push_back(Unit {
                batch: batch.clone(),
                product: product.into(),
                brand: brand.into(),
                template: spec.template.clone(),
                net_mass: 0.0,
                capacity: spec.capacity,
                tare: spec.tare,
                volume: spec.volume,
                label: None,
            });
            units.len() - 1
        });
        let u = &mut units[i];
        let room = (u.capacity - u.net_mass).max(0.0);
        let add = left.min(room);
        u.net_mass += add;
        left -= add;
        if add == room {
            if !u.seal(at, sequences) {
                break;
            }
        } else {
            break;
        }
    }
    left
}

/// Close a lot, including its part-full unit. False leaves an exhausted unit unlabelled.
pub fn close(units: &mut Units, lot: &str, at: f64, sequences: &mut Sequences) -> bool {
    let mut ok = true;
    for u in units.iter_mut().filter(|u| u.batch.lot == lot) {
        ok &= u.seal(at, sequences);
    }
    ok
}

/// Remove packaging for the lot pieces taken from an inventory. Whole sealed units travel;
/// a split or unsealed unit is opened and BOTH portions become loose, retaining their lot.
/// Loose stock is consumed before packages within each lot.
pub fn take(units: &mut Units, before: &crate::lots::Lots, pieces: &[(Batch, f64)]) -> Vec<Unit> {
    let mut out = Vec::new();
    for (batch, kg) in pieces {
        let packed: f64 = units
            .iter()
            .filter(|u| u.batch.lot == batch.lot)
            .map(|u| u.net_mass)
            .sum();
        let total: f64 = before
            .iter()
            .filter(|(b, _)| b.lot == batch.lot)
            .map(|(_, k)| k)
            .sum();
        let mut want = (*kg - (total - packed).max(0.0)).max(0.0);
        // Inventory sums and per-unit sums take different floating-point paths.
        // An ulp-sized remainder after a whole unit must not open the next shell.
        // Scale to the quantities themselves so genuinely tiny fills still move.
        let roundoff = 64.0 * f64::EPSILON * kg.abs().max(total.abs()).max(packed.abs());
        let mut i = 0;
        while want > roundoff && i < units.len() {
            if units[i].batch.lot != batch.lot {
                i += 1;
                continue;
            }
            let u = units.remove(i).unwrap();
            if want + roundoff >= u.net_mass {
                want = (want - u.net_mass).max(0.0);
                if u.label.is_some() {
                    out.push(u);
                }
            } else {
                // Remainder is already accounted for in the inventory's lot kilograms.
                want = 0.0;
            }
        }
    }
    out
}

/// Same selection as take, without opening or moving any units.
pub fn preview(units: &Units, lots: &crate::lots::Lots, kg: f64) -> Vec<Unit> {
    let mut selected_lots = lots.clone();
    let pieces = crate::lots::take(&mut selected_lots, kg);
    take(&mut units.clone(), lots, &pieces)
}

pub fn extra_volume(units: &[Unit], density: f64) -> f64 {
    units
        .iter()
        .map(|u| (u.volume - u.net_mass / density).max(0.0))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn batch() -> Batch {
        Batch {
            lot: "HHI-000001".into(),
            made_at: "facility.test".into(),
            made_on: 10.0,
            heat: None,
        }
    }
    fn spec() -> Spec {
        Spec {
            template: "package.test".into(),
            capacity: 100.0,
            tare: 7.0,
            volume: 1.0,
        }
    }
    fn filled(kg: f64) -> (Units, Sequences) {
        let (mut u, mut s) = (Units::new(), Sequences::new());
        assert_eq!(
            fill(
                &mut u,
                &mut s,
                &spec(),
                &batch(),
                "stock.test",
                "org.test",
                kg,
                12.0
            ),
            0.0
        );
        (u, s)
    }
    #[test]
    fn accrual_and_partial_lot_close() {
        let (mut u, mut s) = filled(40.0);
        assert!(u[0].label.is_none());
        fill(
            &mut u,
            &mut s,
            &spec(),
            &batch(),
            "stock.test",
            "org.test",
            80.0,
            15.0,
        );
        assert_eq!(u.len(), 2);
        assert_eq!(u[0].label.as_ref().unwrap().serial, "HHI-000001-0001");
        close(&mut u, &batch().lot, 20.0, &mut s);
        let l = u[1].label.as_ref().unwrap();
        assert_eq!(
            (l.net_mass, l.gross_mass, l.made_on, l.packed_on),
            (20.0, 27.0, 10.0, 20.0)
        );
        close(&mut u, &batch().lot, 30.0, &mut s);
        assert_eq!(s[&batch().lot], 2);
        assert_eq!(u[1].label.as_ref().unwrap().packed_on, 20.0);
    }
    #[test]
    fn whole_transfer_keeps_identity_split_ends_it() {
        let (mut u, _) = filled(200.0);
        let before = [(batch(), 200.0)].into();
        let out = take(&mut u, &before, &[(batch(), 100.0)]);
        assert_eq!(out[0].label.as_ref().unwrap().serial, "HHI-000001-0001");
        assert_eq!(u[0].label.as_ref().unwrap().serial, "HHI-000001-0002");
        assert!(take(&mut u, &[(batch(), 100.0)].into(), &[(batch(), 1.0)]).is_empty());
        assert!(u.is_empty());
    }
    #[test]
    fn whole_transfer_roundoff_does_not_open_next_fill() {
        let (mut u, _) = filled(125.0);
        let moved = take(&mut u, &[(batch(), 125.0)].into(), &[(batch(), 100.0 + 1e-13)]);
        assert_eq!(moved.len(), 1);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].net_mass, 25.0);
        assert!(u[0].label.is_none());
        let (mut u, _) = filled(125.0);
        let moved = take(&mut u, &[(batch(), 125.0)].into(), &[(batch(), 100.0 - 1e-13)]);
        assert_eq!(moved.len(), 1);
        assert_eq!(u.len(), 1);
    }

    #[test]
    fn loose_first_and_no_mixed_lots() {
        let (mut u, mut s) = filled(100.0);
        assert!(take(&mut u, &[(batch(), 150.0)].into(), &[(batch(), 50.0)]).is_empty());
        assert_eq!(u.len(), 1);
        let mut b = batch();
        b.lot = "HHI-000002".into();
        fill(
            &mut u,
            &mut s,
            &spec(),
            &b,
            "stock.test",
            "org.test",
            60.0,
            20.0,
        );
        assert_eq!(u.len(), 2);
        assert_eq!(u[1].net_mass, 60.0);
    }
    #[test]
    fn exhaustion_never_wraps_and_repacking_never_reuses() {
        let (mut u, mut s) = filled(100.0);
        take(&mut u, &[(batch(), 100.0)].into(), &[(batch(), 1.0)]);
        fill(
            &mut u,
            &mut s,
            &spec(),
            &batch(),
            "stock.test",
            "org.test",
            99.0,
            20.0,
        );
        close(&mut u, &batch().lot, 20.0, &mut s);
        assert_eq!(u[0].label.as_ref().unwrap().serial, "HHI-000001-0002");
        s.insert(batch().lot, 9998);
        assert_eq!(
            fill(
                &mut u,
                &mut s,
                &spec(),
                &batch(),
                "stock.test",
                "org.test",
                200.0,
                30.0
            ),
            100.0
        );
        assert_eq!(s[&batch().lot], 9999);
        assert!(
            u.back()
                .unwrap()
                .label
                .as_ref()
                .unwrap()
                .serial
                .ends_with("9999")
        );
    }
}
