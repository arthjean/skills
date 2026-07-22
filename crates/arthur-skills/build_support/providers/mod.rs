mod claude;
mod codex;
mod permissions;
mod references;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::model::{
    AssetKind, CatalogInventory, ContractFixture, Provider, ProviderContractSummary,
    ScannedCatalog, SourceFile,
};

pub struct ValidationOutcome {
    pub contracts: Vec<ProviderContractSummary>,
}

pub fn validate(repo_root: &Path, catalog: &ScannedCatalog) -> Result<ValidationOutcome, String> {
    let schema_root = repo_root.join("crates/arthur-skills/schemas");
    let claude = load_fixture(
        &schema_root.join("claude-agent-2.1.217.schema.json"),
        "claude",
    )?;
    let codex = load_fixture(
        &schema_root.join("codex-agent-0.144.6.schema.json"),
        "codex",
    )?;
    let inventory = load_inventory(&schema_root.join("catalog-v1.inventory.json"))?;
    validate_inventory(catalog, &inventory)?;
    claude::validate_skills(catalog)?;

    for file in catalog
        .files
        .iter()
        .filter(|file| file.manifest.relative_path.starts_with("agents/claude/"))
    {
        claude::validate_agent(file, &claude)?;
    }
    for file in catalog
        .files
        .iter()
        .filter(|file| file.manifest.relative_path.starts_with("agents/codex/"))
    {
        codex::validate_agent(file, &codex)?;
    }

    Ok(ValidationOutcome {
        contracts: vec![
            contract_summary(&claude, Provider::Claude),
            contract_summary(&codex, Provider::Codex),
        ],
    })
}

fn load_inventory(path: &Path) -> Result<CatalogInventory, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("{}: cannot read inventory: {error}", path.display()))?;
    let inventory: CatalogInventory = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{}: invalid catalog inventory: {error}", path.display()))?;
    if inventory.schema_version != 1 {
        return Err(format!(
            "{}: unsupported catalog inventory version",
            path.display()
        ));
    }
    Ok(inventory)
}

fn validate_inventory(catalog: &ScannedCatalog, expected: &CatalogInventory) -> Result<(), String> {
    let skills = catalog
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Skill)
        .map(|asset| asset.name.clone())
        .collect();
    let claude_agents = catalog
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Agent && asset.provider == Some(Provider::Claude))
        .map(|asset| asset.name.clone())
        .collect();
    let codex_agents = catalog
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Agent && asset.provider == Some(Provider::Codex))
        .map(|asset| asset.name.clone())
        .collect();
    let support = catalog
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Support)
        .map(|asset| asset.name.clone())
        .collect();
    compare_inventory("skills", &expected.skills, skills)?;
    compare_inventory("Claude agents", &expected.claude_agents, claude_agents)?;
    compare_inventory("Codex agents", &expected.codex_agents, codex_agents)?;
    compare_inventory("Claude support", &expected.claude_support, support)
}

fn compare_inventory(
    label: &str,
    expected: &[String],
    actual: BTreeSet<String>,
) -> Result<(), String> {
    let expected_set = expected.iter().cloned().collect::<BTreeSet<_>>();
    if expected_set.len() != expected.len() {
        return Err(format!(
            "catalog inventory contains a duplicate {label} name"
        ));
    }
    if expected_set == actual {
        return Ok(());
    }
    let missing = expected_set
        .difference(&actual)
        .cloned()
        .collect::<Vec<_>>();
    let unexpected = actual
        .difference(&expected_set)
        .cloned()
        .collect::<Vec<_>>();
    Err(format!(
        "catalog {label} differ from inventory; missing={missing:?}, unexpected={unexpected:?}"
    ))
}

pub fn validate_internal_references(files: &[SourceFile]) -> Result<(), String> {
    references::validate(files)
}

fn load_fixture(path: &Path, expected_provider: &str) -> Result<ContractFixture, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("{}: cannot read schema: {error}", path.display()))?;
    let fixture: ContractFixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{}: invalid provider schema: {error}", path.display()))?;
    if fixture.schema_version != 1
        || fixture.provider != expected_provider
        || fixture.provider_version.is_empty()
        || fixture.format.is_empty()
    {
        return Err(format!(
            "{}: unsupported or mismatched provider schema",
            path.display()
        ));
    }
    Ok(fixture)
}

fn contract_summary(fixture: &ContractFixture, provider: Provider) -> ProviderContractSummary {
    ProviderContractSummary {
        provider,
        validated_version: fixture.provider_version.clone(),
        models: fixture.models.clone(),
    }
}

fn utf8(file: &SourceFile) -> Result<&str, String> {
    std::str::from_utf8(&file.bytes).map_err(|error| {
        format!(
            "{}: expected UTF-8 text: {error}",
            file.manifest.relative_path
        )
    })
}

fn validate_allowed(
    file: &SourceFile,
    field_name: &str,
    value: &str,
    allowed: &[String],
) -> Result<(), String> {
    if allowed.iter().any(|candidate| candidate == value) {
        Ok(())
    } else {
        Err(format!(
            "{}: unsupported {field_name} {value:?}",
            file.manifest.relative_path
        ))
    }
}

fn validate_stem_name(file: &SourceFile, name: &str) -> Result<(), String> {
    let stem = Path::new(&file.manifest.relative_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("{}: invalid agent file stem", file.manifest.relative_path))?;
    if stem == name {
        Ok(())
    } else {
        Err(format!(
            "{}: agent name {name:?} differs from file stem {stem:?}",
            file.manifest.relative_path
        ))
    }
}
