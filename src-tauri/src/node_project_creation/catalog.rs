//! Compile-time catalog: no network requests, assets or initializers are loaded.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub revision: String,
    pub templates: Vec<Template>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Template {
    pub id: String,
    pub revision: String,
    pub title: String,
    pub language: String,
    pub node_range: String,
    pub package_manager: PackageManager,
    pub qualification: String,
    pub lockfile_status: String,
    pub intended_files: Vec<String>,
    pub scripts: BTreeMap<String, String>,
    pub dependencies: Vec<Dependency>,
    pub provenance: Provenance,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageManager {
    pub name: String,
    pub version_range: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub development: bool,
    pub node_range: Option<String>,
    pub license: String,
    pub metadata_url: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub kind: String,
    pub reference_url: String,
    pub upstream_revision: Option<String>,
    pub license: String,
    pub note: String,
}
pub fn catalog() -> Catalog {
    serde_json::from_str(include_str!("catalog.json")).expect("tested built-in catalog")
}
pub fn revision() -> String {
    super::digest(include_bytes!("catalog.json"))
}
