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

    std::fs::write(
        output_dir.join("catalog-manifest.json"),
        first.manifest_json,
    )
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
    Ok(())
}
