use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CatalogManifest {
    pub schema_version: u16,
    pub catalog_sha256: String,
    pub files_total: usize,
    pub assets: Vec<AssetManifest>,
    pub provider_contracts: Vec<ProviderContractSummary>,
    pub external_capabilities: Vec<ExternalCapability>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AssetManifest {
    pub name: String,
    pub relative_path: String,
    pub kind: AssetKind,
    pub source_type: SourceType,
    pub provider: Option<Provider>,
    pub size: u64,
    pub files: Vec<FileManifest>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Skill,
    Agent,
    Support,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Directory,
    File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Claude,
    Codex,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FileManifest {
    pub relative_path: String,
    pub size: u64,
    pub mode: u32,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct SourceFile {
    pub manifest: FileManifest,
    pub source_path: PathBuf,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderContractSummary {
    pub provider: Provider,
    pub validated_version: String,
    pub models: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExternalCapability {
    pub command: String,
    pub required: bool,
}

#[derive(Debug, Deserialize)]
pub struct ContractFixture {
    pub schema_version: u16,
    pub provider: String,
    pub provider_version: String,
    pub format: String,
    pub required_fields: Vec<String>,
    #[serde(default)]
    pub optional_fields: Vec<String>,
    pub models: Vec<String>,
    #[serde(default)]
    pub efforts: Vec<String>,
    #[serde(default)]
    pub permission_modes: Vec<String>,
    #[serde(default)]
    pub colors: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub permission_sections: Vec<String>,
    #[serde(default)]
    pub mcp_fields: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogInventory {
    pub schema_version: u16,
    pub skills: Vec<String>,
    pub claude_agents: Vec<String>,
    pub codex_agents: Vec<String>,
    pub claude_support: Vec<String>,
}

pub struct ScannedCatalog {
    pub assets: Vec<AssetManifest>,
    pub files: Vec<SourceFile>,
}

pub struct GeneratedCatalog {
    pub manifest_json: String,
    pub embedded_source: String,
    pub source_paths: Vec<PathBuf>,
}
