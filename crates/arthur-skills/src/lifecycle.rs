use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::catalog::{AssetKind, Catalog, Provider as CatalogProvider};
use crate::engine::{EngineError, plan_desired_state_with_removal_policy};
use crate::plan::{DesiredAsset, DesiredPayload, Plan, PlanAction, RemovalPolicy};
use crate::provider::{ProviderId, ProviderRegistry, ResolvedProvider, ResolvedRoots};
use crate::receipt::{OwnedAsset, OwnedAssetKind, Receipt, ReceiptError, RetainedUnmanagedAsset};

const DIRECTORY_MODE: u32 = 0o755;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleIntent {
    Install { providers: Vec<ProviderId> },
    UninstallProvider(ProviderId),
    UninstallAll,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleNoticeCode {
    CodexUsesImplicitSkills,
    CodexMayDiscoverCanonicalSkills,
    CodexIntegrationRemovedSkillsRemainVisible,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LifecycleNotice {
    pub code: LifecycleNoticeCode,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleTransition {
    pub selected_providers: Vec<ProviderId>,
    pub plan: Plan,
    pub receipt: Receipt,
    pub notices: Vec<LifecycleNotice>,
}

#[derive(Debug)]
pub enum LifecycleError {
    EmptyProviderSelection,
    MissingProviderRoot(ProviderId),
    InvalidCatalogPath(String),
    MissingEmbeddedFile(String),
    UnsafeContainer { path: PathBuf, detail: String },
    Engine(EngineError),
    Receipt(ReceiptError),
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProviderSelection => {
                formatter.write_str("install requires at least one provider")
            }
            Self::MissingProviderRoot(provider) => {
                write!(formatter, "resolved roots do not include {provider}")
            }
            Self::InvalidCatalogPath(path) => {
                write!(
                    formatter,
                    "catalog path is not valid for installation: {path}"
                )
            }
            Self::MissingEmbeddedFile(path) => {
                write!(formatter, "catalog bytes are missing for {path}")
            }
            Self::UnsafeContainer { path, detail } => {
                write!(
                    formatter,
                    "unsafe shared container {}: {detail}",
                    path.display()
                )
            }
            Self::Engine(error) => error.fmt(formatter),
            Self::Receipt(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LifecycleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            Self::Receipt(error) => Some(error),
            _ => None,
        }
    }
}

impl From<EngineError> for LifecycleError {
    fn from(error: EngineError) -> Self {
        Self::Engine(error)
    }
}

impl From<ReceiptError> for LifecycleError {
    fn from(error: ReceiptError) -> Self {
        Self::Receipt(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManagedDesired {
    asset: DesiredAsset,
    references: Vec<ProviderId>,
}

pub fn prepare_lifecycle_transition(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    current: Option<&Receipt>,
    intent: &LifecycleIntent,
) -> Result<LifecycleTransition, LifecycleError> {
    if let Some(receipt) = current {
        receipt.validate()?;
        receipt.validate_roots(roots)?;
    }

    let current_providers = managed_providers(current);
    let selected_providers = selected_after(intent, &current_providers)?;
    let required_roots = current_providers
        .iter()
        .chain(selected_providers.iter())
        .copied()
        .collect::<BTreeSet<_>>();
    for provider in required_roots {
        if roots.provider(provider).is_none() {
            return Err(LifecycleError::MissingProviderRoot(provider));
        }
    }

    let managed = build_desired(catalog, roots, current, &selected_providers)?;
    let desired = managed
        .values()
        .map(|entry| entry.asset.clone())
        .collect::<Vec<_>>();
    let removal_policy = if matches!(
        intent,
        LifecycleIntent::UninstallProvider(_) | LifecycleIntent::UninstallAll
    ) {
        RemovalPolicy::RetainUnmanaged
    } else {
        RemovalPolicy::BlockOnDrift
    };
    let plan = plan_desired_state_with_removal_policy(roots, current, &desired, removal_policy)?;
    let receipt = build_receipt(
        catalog,
        roots,
        current,
        &selected_providers,
        &managed,
        &plan,
    )?;
    let notices = lifecycle_notices(intent, &current_providers, &selected_providers);

    Ok(LifecycleTransition {
        selected_providers,
        plan,
        receipt,
        notices,
    })
}

fn managed_providers(receipt: Option<&Receipt>) -> Vec<ProviderId> {
    receipt.map_or_else(Vec::new, |receipt| {
        receipt
            .providers
            .iter()
            .filter(|provider| provider.managed_integration)
            .map(|provider| provider.provider)
            .collect()
    })
}

fn selected_after(
    intent: &LifecycleIntent,
    current: &[ProviderId],
) -> Result<Vec<ProviderId>, LifecycleError> {
    let selected = match intent {
        LifecycleIntent::Install { providers } => {
            if providers.is_empty() {
                return Err(LifecycleError::EmptyProviderSelection);
            }
            providers.iter().copied().collect::<BTreeSet<_>>()
        }
        LifecycleIntent::UninstallProvider(removed) => current
            .iter()
            .copied()
            .filter(|provider| provider != removed)
            .collect(),
        LifecycleIntent::UninstallAll => BTreeSet::new(),
    };
    Ok(selected.into_iter().collect())
}

fn build_desired(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    current: Option<&Receipt>,
    selected: &[ProviderId],
) -> Result<BTreeMap<PathBuf, ManagedDesired>, LifecycleError> {
    let mut desired = BTreeMap::new();
    if selected.is_empty() {
        return Ok(desired);
    }

    maybe_insert_container(
        &mut desired,
        current,
        "container:canonical-skills",
        &roots.canonical_skills,
        selected,
    )?;
    insert_canonical_skills(catalog, roots, selected, &mut desired)?;

    if selected.contains(&ProviderId::Claude) {
        let provider = required_provider(roots, ProviderId::Claude)?;
        insert_provider_containers(&mut desired, current, provider, ProviderId::Claude)?;
        insert_claude_activations(catalog, roots, provider, &mut desired)?;
        insert_provider_files(catalog, provider, ProviderId::Claude, &mut desired)?;
        insert_claude_support(catalog, current, provider, &mut desired)?;
    }
    if selected.contains(&ProviderId::Codex) {
        let provider = required_provider(roots, ProviderId::Codex)?;
        insert_provider_containers(&mut desired, current, provider, ProviderId::Codex)?;
        insert_provider_files(catalog, provider, ProviderId::Codex, &mut desired)?;
    }
    Ok(desired)
}

fn insert_canonical_skills(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    references: &[ProviderId],
    desired: &mut BTreeMap<PathBuf, ManagedDesired>,
) -> Result<(), LifecycleError> {
    for asset in catalog
        .manifest()
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Skill)
    {
        let asset_path = Path::new(&asset.relative_path);
        let skill_relative = strip_catalog_prefix(asset_path, Path::new("skills"))?;
        insert_directory(
            desired,
            format!("directory:{}", asset.relative_path),
            roots.canonical_skills.join(skill_relative),
            references,
        )?;

        for record in &asset.files {
            let record_path = Path::new(&record.relative_path);
            let mut parent = record_path.parent();
            while let Some(directory) = parent.filter(|path| path.starts_with(asset_path)) {
                let relative = strip_catalog_prefix(directory, Path::new("skills"))?;
                insert_directory(
                    desired,
                    format!("directory:{}", directory.display()),
                    roots.canonical_skills.join(relative),
                    references,
                )?;
                if directory == asset_path {
                    break;
                }
                parent = directory.parent();
            }
            let relative = strip_catalog_prefix(record_path, Path::new("skills"))?;
            insert_catalog_file(
                catalog,
                desired,
                &record.relative_path,
                roots.canonical_skills.join(relative),
                record.mode,
                references,
            )?;
        }
    }
    Ok(())
}

fn insert_provider_containers(
    desired: &mut BTreeMap<PathBuf, ManagedDesired>,
    current: Option<&Receipt>,
    provider: &ResolvedProvider,
    provider_id: ProviderId,
) -> Result<(), LifecycleError> {
    let references = [provider_id];
    maybe_insert_container(
        desired,
        current,
        &format!("container:{}-root", provider_id.as_str()),
        &provider.root.lexical,
        &references,
    )?;
    if let Some(skills) = &provider.skills {
        maybe_insert_container(
            desired,
            current,
            &format!("container:{}-skills", provider_id.as_str()),
            skills,
            &references,
        )?;
    }
    maybe_insert_container(
        desired,
        current,
        &format!("container:{}-agents", provider_id.as_str()),
        &provider.agents,
        &references,
    )
}

fn insert_claude_activations(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    provider: &ResolvedProvider,
    desired: &mut BTreeMap<PathBuf, ManagedDesired>,
) -> Result<(), LifecycleError> {
    let skills_root = provider
        .skills
        .as_ref()
        .ok_or(LifecycleError::MissingProviderRoot(ProviderId::Claude))?;
    for asset in catalog
        .manifest()
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Skill)
    {
        let name = Path::new(&asset.relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| LifecycleError::InvalidCatalogPath(asset.relative_path.clone()))?;
        insert_managed(
            desired,
            DesiredAsset {
                source_id: format!("activation:claude:{name}"),
                destination: skills_root.join(name),
                payload: DesiredPayload::Symlink {
                    target: PathBuf::from(format!("../../.agents/skills/{name}")),
                    canonical_target: roots.canonical_skills.join(name),
                },
            },
            &[ProviderId::Claude],
        )?;
    }
    Ok(())
}

fn insert_provider_files(
    catalog: &Catalog,
    provider: &ResolvedProvider,
    provider_id: ProviderId,
    desired: &mut BTreeMap<PathBuf, ManagedDesired>,
) -> Result<(), LifecycleError> {
    let (catalog_provider, prefix) = match provider_id {
        ProviderId::Claude => (CatalogProvider::Claude, Path::new("agents/claude")),
        ProviderId::Codex => (CatalogProvider::Codex, Path::new("agents/codex")),
    };
    for asset in
        catalog.manifest().assets.iter().filter(|asset| {
            asset.kind == AssetKind::Agent && asset.provider == Some(catalog_provider)
        })
    {
        for record in &asset.files {
            let relative = strip_catalog_prefix(Path::new(&record.relative_path), prefix)?;
            insert_catalog_file(
                catalog,
                desired,
                &record.relative_path,
                provider.agents.join(relative),
                record.mode,
                &[provider_id],
            )?;
        }
    }
    Ok(())
}

fn insert_claude_support(
    catalog: &Catalog,
    current: Option<&Receipt>,
    provider: &ResolvedProvider,
    desired: &mut BTreeMap<PathBuf, ManagedDesired>,
) -> Result<(), LifecycleError> {
    let skills_root = provider
        .skills
        .as_ref()
        .ok_or(LifecycleError::MissingProviderRoot(ProviderId::Claude))?;
    let shared_root = skills_root.join("_shared");
    maybe_insert_container(
        desired,
        current,
        "container:claude-shared",
        &shared_root,
        &[ProviderId::Claude],
    )?;
    let prefix = Path::new("shared/claude/skills");
    for asset in catalog
        .manifest()
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Support)
    {
        for record in &asset.files {
            let relative = strip_catalog_prefix(Path::new(&record.relative_path), prefix)?;
            insert_catalog_file(
                catalog,
                desired,
                &record.relative_path,
                skills_root.join(relative),
                record.mode,
                &[ProviderId::Claude],
            )?;
        }
    }
    Ok(())
}

fn maybe_insert_container(
    desired: &mut BTreeMap<PathBuf, ManagedDesired>,
    current: Option<&Receipt>,
    source_id: &str,
    destination: &Path,
    references: &[ProviderId],
) -> Result<(), LifecycleError> {
    if current
        .and_then(|receipt| receipt.owned_asset(destination))
        .is_some()
    {
        return insert_directory(
            desired,
            source_id.to_owned(),
            destination.to_path_buf(),
            references,
        );
    }
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            Ok(())
        }
        Ok(_) => Err(LifecycleError::UnsafeContainer {
            path: destination.to_path_buf(),
            detail: "expected a real directory".to_owned(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => insert_directory(
            desired,
            source_id.to_owned(),
            destination.to_path_buf(),
            references,
        ),
        Err(error) => Err(LifecycleError::UnsafeContainer {
            path: destination.to_path_buf(),
            detail: error.to_string(),
        }),
    }
}

fn insert_directory(
    desired: &mut BTreeMap<PathBuf, ManagedDesired>,
    source_id: String,
    destination: PathBuf,
    references: &[ProviderId],
) -> Result<(), LifecycleError> {
    if let Some(existing) = desired.get_mut(&destination) {
        if existing.asset.payload
            != (DesiredPayload::Directory {
                mode: DIRECTORY_MODE,
            })
        {
            return Err(LifecycleError::InvalidCatalogPath(format!(
                "directory collides with another asset at {}",
                destination.display()
            )));
        }
        existing.references.extend_from_slice(references);
        existing.references.sort_unstable();
        existing.references.dedup();
        return Ok(());
    }
    insert_managed(
        desired,
        DesiredAsset {
            source_id,
            destination,
            payload: DesiredPayload::Directory {
                mode: DIRECTORY_MODE,
            },
        },
        references,
    )
}

fn insert_catalog_file(
    catalog: &Catalog,
    desired: &mut BTreeMap<PathBuf, ManagedDesired>,
    source_id: &str,
    destination: PathBuf,
    mode: u32,
    references: &[ProviderId],
) -> Result<(), LifecycleError> {
    let embedded = catalog
        .embedded_file(source_id)
        .ok_or_else(|| LifecycleError::MissingEmbeddedFile(source_id.to_owned()))?;
    insert_managed(
        desired,
        DesiredAsset {
            source_id: source_id.to_owned(),
            destination,
            payload: DesiredPayload::File {
                bytes: embedded.bytes.to_vec(),
                mode,
            },
        },
        references,
    )
}

fn insert_managed(
    desired: &mut BTreeMap<PathBuf, ManagedDesired>,
    asset: DesiredAsset,
    references: &[ProviderId],
) -> Result<(), LifecycleError> {
    let destination = asset.destination.clone();
    let mut references = references.to_vec();
    references.sort_unstable();
    references.dedup();
    if desired
        .insert(
            asset.destination.clone(),
            ManagedDesired { asset, references },
        )
        .is_some()
    {
        return Err(LifecycleError::InvalidCatalogPath(format!(
            "duplicate destination {}",
            destination.display()
        )));
    }
    Ok(())
}

fn strip_catalog_prefix<'a>(path: &'a Path, prefix: &Path) -> Result<&'a Path, LifecycleError> {
    let relative = path
        .strip_prefix(prefix)
        .map_err(|_| LifecycleError::InvalidCatalogPath(path.display().to_string()))?;
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(LifecycleError::InvalidCatalogPath(
            path.display().to_string(),
        ));
    }
    Ok(relative)
}

fn required_provider(
    roots: &ResolvedRoots,
    provider: ProviderId,
) -> Result<&ResolvedProvider, LifecycleError> {
    roots
        .provider(provider)
        .ok_or(LifecycleError::MissingProviderRoot(provider))
}

fn build_receipt(
    catalog: &Catalog,
    roots: &ResolvedRoots,
    current: Option<&Receipt>,
    selected: &[ProviderId],
    managed: &BTreeMap<PathBuf, ManagedDesired>,
    plan: &Plan,
) -> Result<Receipt, LifecycleError> {
    let mut receipt = Receipt::new(
        env!("CARGO_PKG_VERSION"),
        &catalog.manifest().catalog_sha256,
        roots,
    );
    for provider in &mut receipt.providers {
        provider.managed_integration = selected.contains(&provider.provider);
        provider.implicit_skill_visibility = ProviderRegistry::get(provider.provider)
            .capabilities
            .implicit_skill_visibility;
        if provider.root.is_none() {
            provider.root = current.and_then(|receipt| {
                receipt
                    .providers
                    .iter()
                    .find(|prior| prior.provider == provider.provider)
                    .and_then(|prior| prior.root.clone())
            });
        }
    }
    receipt.assets = managed.values().map(owned_asset).collect();

    let mut retained = current
        .into_iter()
        .flat_map(|receipt| receipt.retained_unmanaged.iter().cloned())
        .filter(|entry| !managed.contains_key(&entry.destination))
        .map(|entry| (entry.destination.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    for entry in plan
        .entries
        .iter()
        .filter(|entry| entry.action == PlanAction::RetainedUnmanaged)
    {
        retained.insert(
            entry.destination.clone(),
            RetainedUnmanagedAsset {
                source_id: entry.source.clone(),
                destination: entry.destination.clone(),
                reason: entry.reason.clone(),
            },
        );
    }
    receipt.retained_unmanaged = retained.into_values().collect();
    receipt.validate()?;
    Ok(receipt)
}

fn owned_asset(entry: &ManagedDesired) -> OwnedAsset {
    let expected = entry.asset.payload.expected();
    let kind = match expected.kind {
        crate::plan::NodeKind::Directory => OwnedAssetKind::Directory,
        crate::plan::NodeKind::File => OwnedAssetKind::File,
        crate::plan::NodeKind::Symlink => OwnedAssetKind::Symlink,
    };
    OwnedAsset {
        source_id: entry.asset.source_id.clone(),
        destination: entry.asset.destination.clone(),
        kind,
        hash: expected.sha256,
        mode: expected.mode,
        link_target: expected.link_target,
        references: entry.references.clone(),
    }
}

fn lifecycle_notices(
    intent: &LifecycleIntent,
    current: &[ProviderId],
    selected: &[ProviderId],
) -> Vec<LifecycleNotice> {
    let mut notices = Vec::new();
    if selected.contains(&ProviderId::Codex) {
        notices.push(LifecycleNotice {
            code: LifecycleNoticeCode::CodexUsesImplicitSkills,
            message: "Codex reads the canonical skills directly; only its agents are managed as an integration."
                .to_owned(),
        });
    } else if !selected.is_empty() {
        notices.push(LifecycleNotice {
            code: LifecycleNoticeCode::CodexMayDiscoverCanonicalSkills,
            message: "A Codex installation can discover the canonical skills while another provider keeps them installed."
                .to_owned(),
        });
    }
    if matches!(
        intent,
        LifecycleIntent::UninstallProvider(ProviderId::Codex)
    ) && current.contains(&ProviderId::Codex)
        && !selected.is_empty()
    {
        notices.push(LifecycleNotice {
            code: LifecycleNoticeCode::CodexIntegrationRemovedSkillsRemainVisible,
            message: "Codex agents are removed, but canonical skills remain discoverable while another provider references them."
                .to_owned(),
        });
    }
    notices.sort_by_key(|notice| notice.code);
    notices
}

#[cfg(test)]
#[path = "lifecycle/tests.rs"]
mod tests;
