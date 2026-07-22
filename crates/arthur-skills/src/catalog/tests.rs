use super::{AssetKind, Catalog, EMBEDDED_FILES, Provider, verify};

#[test]
fn generated_catalog_is_complete_and_verified() {
    let catalog = Catalog::load();
    assert!(catalog.is_ok());
    let catalog = match catalog {
        Ok(catalog) => catalog,
        Err(error) => panic!("catalog failed validation: {error}"),
    };
    assert_eq!(catalog.skill_count(), 50);
    assert_eq!(
        catalog
            .manifest()
            .assets
            .iter()
            .filter(|asset| asset.kind == AssetKind::Agent)
            .count(),
        6
    );
    assert_eq!(
        catalog
            .manifest()
            .assets
            .iter()
            .filter(|asset| asset.kind == AssetKind::Support)
            .count(),
        4
    );
    assert!(
        EMBEDDED_FILES
            .iter()
            .all(|file| !file.path.starts_with("agents/codex/evals/"))
    );
    assert!(
        catalog
            .manifest()
            .provider_contracts
            .iter()
            .any(|contract| {
                contract.provider == Provider::Claude && contract.validated_version == "2.1.217"
            })
    );
    assert!(
        catalog
            .manifest()
            .provider_contracts
            .iter()
            .any(|contract| {
                contract.provider == Provider::Codex && contract.validated_version == "0.144.6"
            })
    );
    assert_eq!(catalog.manifest().external_capabilities.len(), 1);
    assert_eq!(
        catalog.manifest().external_capabilities[0].command,
        "paneflow-mcp"
    );
    assert!(!catalog.manifest().external_capabilities[0].required);
}

#[test]
fn embedded_text_is_portable_and_paneflow_uses_path_lookup() {
    let mut portable_skill_roots = 0;
    for file in EMBEDDED_FILES {
        if let Ok(text) = std::str::from_utf8(file.bytes) {
            assert!(
                !text.contains("/home/"),
                "{} contains a Linux home path",
                file.path
            );
            assert!(
                !text.contains("/Users/"),
                "{} contains a macOS home path",
                file.path
            );
            for line in text
                .lines()
                .filter(|line| line.trim_start().contains("_SKILL_DIR="))
            {
                portable_skill_roots += 1;
                assert!(
                    line.contains("=\"${"),
                    "{} has an unquoted skill root",
                    file.path
                );
                assert!(line.contains(":-$HOME/.agents/skills/"));
            }
            if file.path.starts_with("agents/codex/") {
                assert!(text.contains("command = \"paneflow-mcp\""));
            }
        }
    }
    assert_eq!(portable_skill_roots, 10);
}

#[test]
fn verification_rejects_count_metadata_and_fingerprint_drift() {
    let catalog = match Catalog::load() {
        Ok(catalog) => catalog,
        Err(error) => panic!("catalog failed validation: {error}"),
    };
    let manifest = catalog.manifest().clone();
    assert!(verify(&manifest, &EMBEDDED_FILES[1..]).is_err());

    let mut changed_files = EMBEDDED_FILES.to_vec();
    changed_files[0].path = "missing/from/manifest";
    assert!(verify(&manifest, &changed_files).is_err());

    let mut changed_files = EMBEDDED_FILES.to_vec();
    changed_files[0].mode ^= 1;
    assert!(verify(&manifest, &changed_files).is_err());

    let mut changed_manifest = manifest;
    changed_manifest.catalog_sha256 = "0".repeat(64);
    assert!(verify(&changed_manifest, EMBEDDED_FILES).is_err());

    changed_manifest.schema_version = 2;
    assert!(verify(&changed_manifest, EMBEDDED_FILES).is_err());
}
