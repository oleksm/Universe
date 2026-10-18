//! Typed authoring document. Registry facts and derived performance are external.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
fn required_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Module {
    Equipment(Equipment),
    HullHold(Hold),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pose {
    pub position_m: [f64; 3],
    pub rotation_xyzw: [f64; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Hull {
    #[schemars(length(min = 1))]
    pub record: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = "^[a-f0-9]{64}$"))]
    pub model_sha256: Option<String>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum EquipmentKind {
    #[serde(rename = "equipment")]
    Equipment,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Equipment {
    pub kind: EquipmentKind,
    #[schemars(length(min = 1))]
    pub record: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub slot: Option<String>,
    pub pose: Pose,
    pub envelope_m: [f64; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub notes: Option<String>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum HoldKind {
    #[serde(rename = "hull_hold")]
    HullHold,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Hold {
    pub kind: HoldKind,
    pub pose: Pose,
    pub envelope_m: [f64; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub position_m: [f64; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub hull_anchor: Option<String>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum ProfileShape {
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "round")]
    Round,
    #[serde(rename = "square")]
    Square,
    #[serde(rename = "hex")]
    Hex,
    #[serde(rename = "oct")]
    Oct,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum ProfilePlacement {
    #[serde(rename = "floor")]
    Floor,
    #[serde(rename = "center")]
    Center,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub shape: ProfileShape,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub width_m: f64,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub height_m: f64,
    pub placement: ProfilePlacement,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Path {
    #[schemars(length(min = 1))]
    pub from: String,
    #[schemars(length(min = 1))]
    pub to: String,
    pub profile: Profile,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub paths: Vec<String>,
    pub walled: bool,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum HatchShape {
    #[serde(rename = "round")]
    Round,
    #[serde(rename = "square")]
    Square,
    #[serde(rename = "hex")]
    Hex,
    #[serde(rename = "oct")]
    Oct,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum HatchSlide {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "side")]
    Side,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Hatch {
    pub shape: HatchShape,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub width_m: f64,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub height_m: f64,
    pub slide: HatchSlide,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum SideDoorSide {
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "right")]
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SideDoor {
    #[schemars(length(min = 1))]
    pub path: String,
    #[schemars(range(min = 0, max = 1))]
    pub fraction: f64,
    pub side: SideDoorSide,
    pub hatch: Hatch,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum PathEndEnd {
    #[serde(rename = "from")]
    From,
    #[serde(rename = "to")]
    To,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum PathEndState {
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "door")]
    Door,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathEnd {
    #[schemars(length(min = 1))]
    pub path: String,
    pub end: PathEndEnd,
    pub state: PathEndState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hatch: Option<Hatch>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Beam {
    pub from_m: [f64; 3],
    pub to_m: [f64; 3],
    #[schemars(length(min = 1))]
    pub stock: String,
    pub pinned: [bool; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Plate {
    pub min_xz_m: [f64; 2],
    pub max_xz_m: [f64; 2],
    pub height_m: f64,
    #[schemars(length(min = 1))]
    pub stock: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WallDoor {
    #[schemars(range(min = 0))]
    pub distance_m: f64,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub width_m: f64,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub height_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Wall {
    #[schemars(length(min = 2))]
    pub points_xz_m: Vec<[f64; 2]>,
    pub bulges_m: Vec<f64>,
    pub doors: BTreeMap<String, WallDoor>,
    pub rail: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ladder {
    pub at_xz_m: [f64; 2],
    #[schemars(length(min = 1))]
    pub to_deck: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stair {
    pub from_xz_m: [f64; 2],
    pub to_xz_m: [f64; 2],
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub width_m: f64,
    #[schemars(length(min = 1))]
    pub to_deck: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Deck {
    pub floor_m: f64,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub headroom_m: f64,
    pub floors: BTreeMap<String, Vec<[f64; 2]>>,
    pub walls: BTreeMap<String, Wall>,
    pub ladders: BTreeMap<String, Ladder>,
    pub stairs: BTreeMap<String, Stair>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum Format {
    #[serde(rename = "freefall.ship")]
    FreefallShip,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum Version {
    #[serde(rename = "1-draft")]
    V1Draft,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    #[schemars(length(min = 1))]
    pub id: String,
    #[schemars(length(min = 1))]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub notes: Option<String>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum Coordinates {
    #[serde(rename = "right-handed-x-right-y-up-z-aft-metres")]
    RightHandedXRightYUpZAftMetres,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum PlacementConvention {
    #[serde(rename = "studio-centered-v1")]
    StudioCenteredV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0))]
    pub gravity_m_s2: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0))]
    pub delta_v_m_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0))]
    pub cargo_kg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0))]
    pub crew: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0))]
    pub trip_s: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Access {
    pub points: BTreeMap<String, Point>,
    pub paths: BTreeMap<String, Path>,
    pub groups: BTreeMap<String, Group>,
    pub side_doors: BTreeMap<String, SideDoor>,
    pub ends: BTreeMap<String, PathEnd>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Structure {
    pub members: BTreeMap<String, Beam>,
    pub plates: BTreeMap<String, Plate>,
    pub landing_pads: BTreeMap<String, [f64; 3]>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum Layer {
    #[serde(rename = "hull")]
    Hull,
    #[serde(rename = "grid_measures")]
    GridMeasures,
    #[serde(rename = "points")]
    Points,
    #[serde(rename = "points.entry")]
    PointsEntry,
    #[serde(rename = "points.dash")]
    PointsDash,
    #[serde(rename = "points.windows")]
    PointsWindows,
    #[serde(rename = "points.service")]
    PointsService,
    #[serde(rename = "points.mining")]
    PointsMining,
    #[serde(rename = "points.mounts_engines")]
    PointsMountsEngines,
    #[serde(rename = "points.laid")]
    PointsLaid,
    #[serde(rename = "points.names")]
    PointsNames,
    #[serde(rename = "access")]
    Access,
    #[serde(rename = "access.lines")]
    AccessLines,
    #[serde(rename = "access.outlines")]
    AccessOutlines,
    #[serde(rename = "access.walls")]
    AccessWalls,
    #[serde(rename = "access.hatches")]
    AccessHatches,
    #[serde(rename = "clashes")]
    Clashes,
    #[serde(rename = "decks_2d")]
    Decks2d,
    #[serde(rename = "decks_2d.floors")]
    Decks2dFloors,
    #[serde(rename = "decks_2d.walls")]
    Decks2dWalls,
    #[serde(rename = "hollow_map")]
    HollowMap,
    #[serde(rename = "reach")]
    Reach,
    #[serde(rename = "modules")]
    Modules,
    #[serde(rename = "frame")]
    Frame,
    #[serde(rename = "pressure")]
    Pressure,
    #[serde(rename = "decks")]
    Decks,
    #[serde(rename = "wiring")]
    Wiring,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub target_m: [f64; 3],
    pub yaw_rad: f64,
    pub pitch_rad: f64,
    #[schemars(extend("exclusiveMinimum" = 0))]
    pub distance_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct View {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<BTreeMap<Layer, bool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_plane_m: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Camera>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum DependenciesPolicy {
    #[serde(rename = "compatible")]
    Compatible,
    #[serde(rename = "locked")]
    Locked,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelLock {
    #[schemars(length(min = 1))]
    pub version: String,
    #[schemars(regex(pattern = "^[a-f0-9]{64}$"))]
    pub manifest_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Dependencies {
    pub policy: DependenciesPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub registry_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<BTreeMap<String, ModelLock>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Document {
    #[serde(rename = "$schema")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub schema: Option<String>,
    pub format: Format,
    pub version: Version,
    pub design: Identity,
    pub coordinates: Coordinates,
    pub placement_convention: PlacementConvention,
    #[serde(deserialize_with = "required_option")]
    #[schemars(with = "Option<Hull>", required)]
    pub hull: Option<Hull>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements: Option<Requirements>,
    pub modules: BTreeMap<String, Module>,
    pub access: Access,
    pub structure: Structure,
    pub decks: BTreeMap<String, Deck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<View>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Dependencies>,
}
