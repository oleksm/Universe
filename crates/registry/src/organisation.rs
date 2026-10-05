//! `org.*`: an organisation (`standards/organisation.schema.yaml`): a company,
//! a standards body or a system's administration.

use serde::{Deserialize, Serialize};

use crate::Address;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Organisation {
    pub identity: OrgIdentity,
    pub kind: OrgKind,
    #[serde(default)]
    pub address: Option<Address>,
    /// One line, as the game shows it.
    #[serde(default)]
    pub note: Option<String>,
    /// What it is, in paragraphs.
    #[serde(default)]
    pub about: Vec<String>,
    /// Its story, in paragraphs.
    #[serde(default)]
    pub story: Vec<String>,
    /// A company's short trading name.
    #[serde(default)]
    pub ticker: Option<String>,
    /// What kind of company it is (left out: a maker).
    #[serde(default)]
    pub business: Option<Business>,
    #[serde(default)]
    pub who: Option<String>,
    #[serde(default)]
    pub what: Option<String>,
    /// A standards body's letters, before each of its standards' numbers.
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub seat: Option<String>,
    #[serde(default)]
    pub form: Option<Form>,
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub details: Option<Details>,
    /// The companies that founded it, by key.
    #[serde(default)]
    pub founded_by: Vec<String>,
    /// An administration's zoning code: what each kind of zone permits.
    #[serde(default)]
    pub zoning: Vec<ZoneRule>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrgIdentity {
    pub key: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrgKind {
    Company,
    StandardsBody,
    Administration,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Business {
    Maker,
    Warehousing,
    Exchange,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Form {
    Consortium,
    Independent,
    Authority,
    Corporation,
    Players,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Details {
    #[serde(default)]
    pub text: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneRule {
    #[serde(rename = "use")]
    pub zone: ZoneUse,
    pub permits: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ZoneUse {
    Port,
    Industrial,
    Commercial,
    Civic,
    Residential,
}

impl Organisation {
    /// Whether it makes things: a company that is a maker (or says nothing of
    /// its business). Only makers' names go on products.
    pub fn is_maker(&self) -> bool {
        self.kind == OrgKind::Company && self.business.is_none_or(|b| b == Business::Maker)
    }
}
