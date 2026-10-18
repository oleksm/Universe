//! Mechanical attachment validation, shared by Studio and flight compilation.
use super::*;

/// Placement is not proof of attachment. Until saved mount instances and their
/// structural load paths can be resolved, every placed module is unverified.
/// In particular, neither a nearby joint nor room/deck overlap grants a mount.
pub(super) fn faults(plan: &Plan, fit: &[Fitted]) -> Vec<String> {
    let registry = universe_sim::world::registry::registry();
    plan.blocks.iter().map(|b| {
        let fitted = fit.iter().find(|f| f.id == kind(&b.id));
        let name = fitted.map_or_else(|| kind(&b.id).to_uppercase(), |f| f.name.clone());
        let required = fitted.and_then(|f| registry.equipment(&f.key))
            .and_then(|e| e.fits.as_deref());
        let requirement = required.map_or_else(
            || "COMPATIBLE MOUNT TYPE UNDECLARED".to_string(),
            |key| format!("REQUIRES {}", key.to_uppercase()),
        );
        format!("{name} [{}] NOT MOUNTED: NO VERIFIED ATTACHMENT; {requirement}", b.id)
    }).collect()
}

pub(super) fn validate(plan: &Plan, fit: &[Fitted]) -> Result<(), String> {
    let faults = faults(plan, fit);
    if faults.is_empty() { Ok(()) } else { Err(faults.join("\n")) }
}
