//! Order: the law an administration keeps and the insurer's terms, as the
//! registry has them (`org.<administration>.law`, `org.<insurer>.insurance`).
//! What follows from them (an offence answered, a loss paid) is the
//! services' and the sim's.

use crate::registry::{registry, Org, OrgInsurance, OrgLaw};

/// The law in the system named `system`: its administration's (an
/// administration is named for the system it administers). None: no law there.
pub fn law(system: &str) -> Option<&'static OrgLaw> {
    registry().orgs.iter().find(|o| o.law.is_some() && o.identity.name.eq_ignore_ascii_case(system)).and_then(|o| o.law.as_ref())
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
