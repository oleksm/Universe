//! Order: the law an administration keeps and the insurer's terms, as the
//! registry has them (`org.<administration>.law`, `org.<insurer>.insurance`).
//! What follows from them (an offence answered, a loss paid) is the
//! services' and the sim's.

use crate::registry::{registry, Org, OrgInsurance, OrgLaw};

/// The law in system `system` (its galaxy index): the law of the administration that
/// `administers` its record. None: no law there.
pub fn law(system: usize) -> Option<&'static OrgLaw> {
    static BY_SYSTEM: std::sync::OnceLock<std::collections::HashMap<usize, &'static OrgLaw>> = std::sync::OnceLock::new();
    BY_SYSTEM
        .get_or_init(|| {
            let reg = registry();
            reg.orgs
                .iter()
                .filter_map(|o| Some((reg.system(o.administers.as_deref()?)?.identity.index? as usize, o.law.as_ref()?)))
                .collect()
        })
        .get(&system)
        .copied()
}

/// An offence's name, as the registry writes it ("reckless flying").
pub fn offence_name(o: crate::registry::Offence) -> String {
    serde_json::to_value(o).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| format!("{o:?}"))
}

/// The insurer, and its terms. (One so far, Treistun Mutual: when there
/// are more, a pilot's is the one that took it on.)
pub fn insurer() -> Option<(&'static Org, &'static OrgInsurance)> {
    registry().orgs.iter().find_map(|o| Some((o, o.insurance.as_ref()?)))
}
