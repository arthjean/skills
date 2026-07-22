mod model;
mod path_policy;
mod providers;
mod scanner;

use std::path::Path;

use sha2::{Digest, Sha256};

use model::{CatalogManifest, ExternalCapability, GeneratedCatalog};

pub fn generate(repo_root: &Path) -> Result<GeneratedCatalog, String> {
    let mut scanned = scanner::scan(repo_root)?;
    let validation = providers::validate(repo_root, &scanned)?;
    providers::validate_internal_references(&scanned.files)?;
    let source_paths = scanned
        .files
        .iter()
        .map(|file| file.source_path.clone())
        .collect();

    let catalog_sha256 = catalog_hash(&scanned.files);
    let manifest = CatalogManifest {
        schema_version: 1,
        catalog_sha256,
        files_total: scanned.files.len(),
        assets: std::mem::take(&mut scanned.assets),
        provider_contracts: validation.contracts,
        external_capabilities: vec![ExternalCapability {
            command: "paneflow-mcp".to_owned(),
            required: false,
        }],
    };
    let mut manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("cannot serialize catalog manifest: {error}"))?;
    manifest_json.push('\n');

    Ok(GeneratedCatalog {
        manifest_json,
        embedded_source: embedded_source(&scanned.files)?,
        source_paths,
    })
}

#[cfg(test)]
pub fn validate_relative_for_test(repo_root: &Path, path: &Path) -> Result<String, String> {
    path_policy::relative_utf8(repo_root, path)
}

fn catalog_hash(files: &[model::SourceFile]) -> String {
    let mut digest = Sha256::new();
    for file in files {
        digest.update(file.manifest.relative_path.as_bytes());
        digest.update([0]);
        digest.update(file.manifest.sha256.as_bytes());
        digest.update([0]);
        digest.update(file.manifest.mode.to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn embedded_source(files: &[model::SourceFile]) -> Result<String, String> {
    let mut output = String::from("pub static EMBEDDED_FILES: &[EmbeddedFile] = &[\n");
    for file in files {
        let source = file.source_path.to_str().ok_or_else(|| {
            format!(
                "{}: non-UTF-8 source cannot be embedded",
                file.source_path.display()
            )
        })?;
        output.push_str("    EmbeddedFile { path: ");
        output.push_str(&format!("{:?}", file.manifest.relative_path));
        output.push_str(", bytes: include_bytes!(");
        output.push_str(&format!("{source:?}"));
        output.push_str("), mode: ");
        output.push_str(&file.manifest.mode.to_string());
        output.push_str(", sha256: ");
        output.push_str(&format!("{:?}", file.manifest.sha256));
        output.push_str(" },\n");
    }
    output.push_str("];\n");
    Ok(output)
}
