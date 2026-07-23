use std::error::Error as _;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use super::{
    DIRECTORY_MODE, LifecycleError, LifecycleIntent, ManagedDesired, build_receipt,
    insert_catalog_file, insert_directory, insert_managed, maybe_insert_container,
    prepare_lifecycle_transition, required_provider, selected_after, strip_catalog_prefix,
};
use crate::catalog::Catalog;
use crate::engine::EngineError;
use crate::plan::{DesiredAsset, DesiredPayload, PLAN_SCHEMA_VERSION, Plan};
use crate::provider::{ProviderId, resolve_roots_from};
use crate::receipt::{OwnedAsset, OwnedAssetKind, Receipt, ReceiptError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn lifecycle_errors_and_selection_have_stable_contracts() {
    let cases = [
        (
            LifecycleError::EmptyProviderSelection,
            "install requires at least one provider",
        ),
        (
            LifecycleError::MissingProviderRoot(ProviderId::Codex),
            "resolved roots do not include codex",
        ),
        (
            LifecycleError::InvalidCatalogPath("bad".to_owned()),
            "catalog path is not valid for installation: bad",
        ),
        (
            LifecycleError::MissingEmbeddedFile("missing".to_owned()),
            "catalog bytes are missing for missing",
        ),
        (
            LifecycleError::UnsafeContainer {
                path: PathBuf::from("/unsafe"),
                detail: "wrong type".to_owned(),
            },
            "unsafe shared container /unsafe: wrong type",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none());
    }

    let engine = LifecycleError::from(EngineError::RecoveryRequired);
    assert_eq!(
        engine.to_string(),
        "installation state requires recovery before another mutation can be planned"
    );
    assert!(engine.source().is_some());
    let receipt = LifecycleError::from(ReceiptError::MissingField("cli_version"));
    assert_eq!(
        receipt.to_string(),
        "receipt field cli_version cannot be empty"
    );
    assert!(receipt.source().is_some());

    assert!(matches!(
        selected_after(&LifecycleIntent::Install { providers: vec![] }, &[]),
        Err(LifecycleError::EmptyProviderSelection)
    ));
    assert_eq!(
        selected_after(
            &LifecycleIntent::UninstallProvider(ProviderId::Claude),
            &ProviderId::ALL,
        )
        .unwrap_or_else(|error| panic!("selection failed: {error}")),
        vec![ProviderId::Codex]
    );
    assert!(
        selected_after(&LifecycleIntent::UninstallAll, &ProviderId::ALL)
            .unwrap_or_else(|error| panic!("selection failed: {error}"))
            .is_empty()
    );
}

#[test]
fn container_and_catalog_helpers_fail_closed() -> TestResult {
    let home = tempdir()?;
    let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Codex])?;
    let catalog = Catalog::load()?;
    let mut desired = std::collections::BTreeMap::new();

    let existing_directory = home.path().join("existing-directory");
    std::fs::create_dir(&existing_directory)?;
    maybe_insert_container(
        &mut desired,
        None,
        "existing",
        &existing_directory,
        &[ProviderId::Codex],
    )?;
    assert!(!desired.contains_key(&existing_directory));

    let missing_directory = home.path().join("missing-directory");
    maybe_insert_container(
        &mut desired,
        None,
        "missing",
        &missing_directory,
        &[ProviderId::Codex],
    )?;
    assert!(desired.contains_key(&missing_directory));

    let existing_file = home.path().join("existing-file");
    std::fs::write(&existing_file, b"file")?;
    assert!(matches!(
        maybe_insert_container(
            &mut desired,
            None,
            "file",
            &existing_file,
            &[ProviderId::Codex],
        ),
        Err(LifecycleError::UnsafeContainer { .. })
    ));
    let overlong = home.path().join("x".repeat(300));
    assert!(matches!(
        maybe_insert_container(
            &mut desired,
            None,
            "overlong",
            &overlong,
            &[ProviderId::Codex],
        ),
        Err(LifecycleError::UnsafeContainer { .. })
    ));

    let owned_directory = home.path().join("owned-directory");
    let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
    receipt.assets.push(OwnedAsset {
        source_id: "owned".to_owned(),
        destination: owned_directory.clone(),
        kind: OwnedAssetKind::Directory,
        hash: None,
        mode: Some(DIRECTORY_MODE),
        link_target: None,
        references: vec![ProviderId::Codex],
    });
    maybe_insert_container(
        &mut desired,
        Some(&receipt),
        "owned",
        &owned_directory,
        &[ProviderId::Codex],
    )?;
    assert!(desired.contains_key(&owned_directory));

    let shared = home.path().join("shared");
    insert_directory(
        &mut desired,
        "shared-claude".to_owned(),
        shared.clone(),
        &[ProviderId::Claude],
    )?;
    insert_directory(
        &mut desired,
        "shared-codex".to_owned(),
        shared.clone(),
        &[ProviderId::Codex],
    )?;
    assert_eq!(
        desired
            .get(&shared)
            .ok_or("shared directory missing")?
            .references,
        ProviderId::ALL
    );

    let collision = home.path().join("collision");
    insert_managed(
        &mut desired,
        DesiredAsset {
            source_id: "file".to_owned(),
            destination: collision.clone(),
            payload: DesiredPayload::File {
                bytes: b"file".to_vec(),
                mode: 0o644,
            },
        },
        &[],
    )?;
    assert!(matches!(
        insert_directory(&mut desired, "directory".to_owned(), collision, &[]),
        Err(LifecycleError::InvalidCatalogPath(_))
    ));

    let duplicate = home.path().join("duplicate");
    let file = DesiredAsset {
        source_id: "first".to_owned(),
        destination: duplicate.clone(),
        payload: DesiredPayload::File {
            bytes: b"first".to_vec(),
            mode: 0o644,
        },
    };
    insert_managed(&mut desired, file.clone(), &[])?;
    assert!(matches!(
        insert_managed(&mut desired, file, &[]),
        Err(LifecycleError::InvalidCatalogPath(_))
    ));
    assert!(matches!(
        insert_catalog_file(
            &catalog,
            &mut desired,
            "missing",
            home.path().join("missing-file"),
            0o644,
            &[],
        ),
        Err(LifecycleError::MissingEmbeddedFile(_))
    ));
    assert_eq!(
        strip_catalog_prefix(Path::new("skills/example"), Path::new("skills"))?,
        Path::new("example")
    );
    assert!(strip_catalog_prefix(Path::new("agents/example"), Path::new("skills")).is_err());
    assert!(strip_catalog_prefix(Path::new("skills"), Path::new("skills")).is_err());
    assert!(strip_catalog_prefix(Path::new("skills/../escape"), Path::new("skills")).is_err());
    assert!(matches!(
        required_provider(&roots, ProviderId::Claude),
        Err(LifecycleError::MissingProviderRoot(ProviderId::Claude))
    ));
    Ok(())
}

#[test]
fn transition_requires_every_managed_root_and_preserves_historical_roots() -> TestResult {
    let home = tempdir()?;
    let both = resolve_roots_from(Some(home.path().as_os_str()), None, &ProviderId::ALL)?;
    let claude = resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Claude])?;
    let current = Receipt::new("0.1.0", "a".repeat(64), &both);
    let catalog = Catalog::load()?;
    assert!(matches!(
        prepare_lifecycle_transition(
            &catalog,
            &claude,
            Some(&current),
            &LifecycleIntent::UninstallProvider(ProviderId::Claude),
        ),
        Err(LifecycleError::MissingProviderRoot(ProviderId::Codex))
    ));
    assert!(matches!(
        prepare_lifecycle_transition(
            &catalog,
            &claude,
            None,
            &LifecycleIntent::Install { providers: vec![] },
        ),
        Err(LifecycleError::EmptyProviderSelection)
    ));

    let empty_plan = Plan {
        schema_version: PLAN_SCHEMA_VERSION,
        applicable: true,
        entries: Vec::new(),
        operations: Vec::new(),
        diagnostics: Vec::new(),
    };
    let next = build_receipt(
        &catalog,
        &claude,
        Some(&current),
        &[],
        &std::collections::BTreeMap::<PathBuf, ManagedDesired>::new(),
        &empty_plan,
    )?;
    assert!(
        next.providers
            .iter()
            .find(|provider| provider.provider == ProviderId::Codex)
            .and_then(|provider| provider.root.as_ref())
            .is_some()
    );

    let canonical_container = claude.canonical_skills.clone();
    std::fs::create_dir_all(canonical_container.parent().ok_or("no canonical parent")?)?;
    std::fs::write(&canonical_container, b"wrong type")?;
    assert!(matches!(
        prepare_lifecycle_transition(
            &catalog,
            &claude,
            None,
            &LifecycleIntent::Install {
                providers: vec![ProviderId::Claude],
            },
        ),
        Err(LifecycleError::UnsafeContainer { .. })
    ));
    Ok(())
}

#[test]
fn provider_helpers_reject_missing_claude_skill_roots() {
    let home = tempdir().unwrap_or_else(|error| panic!("temporary HOME failed: {error}"));
    let claude_root = home.path().join(".claude");
    let provider = crate::provider::ResolvedProvider {
        id: ProviderId::Claude,
        root: crate::provider::RootIdentity {
            lexical: claude_root.clone(),
            real: claude_root.clone(),
            device: 1,
        },
        skills: None,
        agents: claude_root.join("agents"),
    };
    let catalog = Catalog::load().unwrap_or_else(|error| panic!("catalog fixture failed: {error}"));
    let mut desired = std::collections::BTreeMap::new();
    assert!(
        super::insert_claude_activations(
            &catalog,
            &resolve_roots_from(Some(home.path().as_os_str()), None, &[ProviderId::Claude],)
                .unwrap_or_else(|error| panic!("root fixture failed: {error}")),
            &provider,
            &mut desired,
        )
        .is_err()
    );
    assert!(super::insert_claude_support(&catalog, None, &provider, &mut desired).is_err());
}
