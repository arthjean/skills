#![forbid(unsafe_code)]

mod build_support;

use std::env;
use std::path::PathBuf;

fn main() {
    if let Err(error) = build_catalog() {
        panic!("catalog generation failed: {error}");
    }
}

fn build_catalog() -> Result<(), String> {
    let crate_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| "CARGO_MANIFEST_DIR is absent".to_owned())?,
    );
    let repo_root = crate_dir
        .parent()
        .and_then(|path| path.parent())
        .ok_or_else(|| {
            format!(
                "{}: crate must be two levels below the repository",
                crate_dir.display()
            )
        })?;
    let output_dir =
        PathBuf::from(env::var_os("OUT_DIR").ok_or_else(|| "OUT_DIR is absent".to_owned())?);

    let first = build_support::generate(repo_root)?;
    let second = build_support::generate(repo_root)?;
    if first.manifest_json != second.manifest_json
        || first.embedded_source != second.embedded_source
        || first.source_paths != second.source_paths
    {
        return Err("two unchanged catalog generations produced different bytes".to_owned());
    }

    for path in &first.source_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let manifest_json = if env::var_os("ARTHUR_SKILLS_TEST_CORRUPT_CATALOG").is_some() {
        corrupt_manifest_for_fixture(&first.manifest_json)?
    } else {
        first.manifest_json
    };
    std::fs::write(output_dir.join("catalog-manifest.json"), manifest_json)
        .map_err(|error| format!("cannot write generated manifest: {error}"))?;
    std::fs::write(
        output_dir.join("embedded-catalog.rs"),
        first.embedded_source,
    )
    .map_err(|error| format!("cannot write embedded catalog source: {error}"))?;

    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join("skills").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join("agents").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join("shared").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        crate_dir.join("schemas").display()
    );
    println!("cargo:rerun-if-env-changed=ARTHUR_SKILLS_TEST_CORRUPT_CATALOG");
    Ok(())
}

fn corrupt_manifest_for_fixture(manifest_json: &str) -> Result<String, String> {
    let mut manifest = serde_json::from_str::<serde_json::Value>(manifest_json)
        .map_err(|error| format!("cannot parse catalog fixture manifest: {error}"))?;
    manifest["catalog_sha256"] = serde_json::Value::String("0".repeat(64));
    let mut encoded = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("cannot serialize catalog fixture manifest: {error}"))?;
    encoded.push('\n');
    Ok(encoded)
}
