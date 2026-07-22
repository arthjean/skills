use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MANIFEST_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/catalog-manifest.json"));

#[derive(Clone, Copy)]
pub struct EmbeddedFile {
    pub path: &'static str,
    pub bytes: &'static [u8],
    pub mode: u32,
    pub sha256: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/embedded-catalog.rs"));

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub schema_version: u16,
    pub catalog_sha256: String,
    pub files_total: usize,
    pub assets: Vec<Asset>,
    pub provider_contracts: Vec<ProviderContract>,
    pub external_capabilities: Vec<ExternalCapability>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Asset {
    pub name: String,
    pub relative_path: String,
    pub kind: AssetKind,
    pub source_type: SourceType,
    pub provider: Option<Provider>,
    pub size: u64,
    pub files: Vec<FileRecord>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Skill,
    Agent,
    Support,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Directory,
    File,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Claude,
    Codex,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileRecord {
    pub relative_path: String,
    pub size: u64,
    pub mode: u32,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderContract {
    pub provider: Provider,
    pub validated_version: String,
    pub models: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExternalCapability {
    pub command: String,
    pub required: bool,
}

pub struct Catalog {
    manifest: Manifest,
}

impl Catalog {
    pub fn load() -> Result<Self, CatalogError> {
        let manifest = serde_json::from_str(MANIFEST_JSON)
            .map_err(|error| CatalogError(format!("embedded manifest is invalid: {error}")))?;
        verify(&manifest, EMBEDDED_FILES)?;
        Ok(Self { manifest })
    }

    pub fn skill_count(&self) -> usize {
        self.manifest
            .assets
            .iter()
            .filter(|asset| asset.kind == AssetKind::Skill)
            .count()
    }

    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn embedded_file(&self, relative_path: &str) -> Option<EmbeddedFile> {
        EMBEDDED_FILES
            .iter()
            .find(|file| file.path == relative_path)
            .copied()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct CatalogError(String);

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CatalogError {}

fn verify(manifest: &Manifest, embedded: &[EmbeddedFile]) -> Result<(), CatalogError> {
    if manifest.schema_version != 1 {
        return Err(CatalogError(format!(
            "unsupported embedded manifest schema {}",
            manifest.schema_version
        )));
    }
    let records = manifest
        .assets
        .iter()
        .flat_map(|asset| asset.files.iter())
        .map(|file| (file.relative_path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    if records.len() != manifest.files_total || embedded.len() != manifest.files_total {
        return Err(CatalogError(
            "embedded file count differs from the manifest".to_owned(),
        ));
    }

    for file in embedded {
        let Some(record) = records.get(file.path) else {
            return Err(CatalogError(format!(
                "{}: embedded file is absent from manifest",
                file.path
            )));
        };
        let actual_size = u64::try_from(file.bytes.len())
            .map_err(|error| CatalogError(format!("{}: size overflow: {error}", file.path)))?;
        let actual_hash = format!("{:x}", Sha256::digest(file.bytes));
        if actual_size != record.size
            || file.mode != record.mode
            || file.sha256 != record.sha256
            || actual_hash != record.sha256
        {
            return Err(CatalogError(format!(
                "{}: embedded metadata or bytes differ",
                file.path
            )));
        }
    }
    if hash_embedded(embedded) != manifest.catalog_sha256 {
        return Err(CatalogError(
            "catalog fingerprint differs from embedded files".to_owned(),
        ));
    }
    Ok(())
}

fn hash_embedded(files: &[EmbeddedFile]) -> String {
    let mut digest = Sha256::new();
    for file in files {
        digest.update(file.path.as_bytes());
        digest.update([0]);
        digest.update(file.sha256.as_bytes());
        digest.update([0]);
        digest.update(file.mode.to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coverage_tests {
    use super::CatalogError;

    #[test]
    fn catalog_error_display_preserves_the_verifier_detail() {
        let error = CatalogError("catalog mismatch".to_owned());
        assert_eq!(error.to_string(), "catalog mismatch");
    }
}
