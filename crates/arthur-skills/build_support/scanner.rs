use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::model::{AssetKind, AssetManifest, Provider, ScannedCatalog, SourceFile, SourceType};
use super::path_policy::{
    file_name_utf8, normalized_mode, relative_utf8, sha256, validate_name, validate_portable_bytes,
};

pub fn scan(repo_root: &Path) -> Result<ScannedCatalog, String> {
    let mut assets = Vec::new();
    let mut files = Vec::new();
    scan_skills(repo_root, &mut assets, &mut files)?;
    scan_provider(
        repo_root,
        &repo_root.join("agents/claude"),
        "md",
        Provider::Claude,
        None,
        &mut assets,
        &mut files,
    )?;
    scan_provider(
        repo_root,
        &repo_root.join("agents/codex"),
        "toml",
        Provider::Codex,
        Some("evals"),
        &mut assets,
        &mut files,
    )?;
    scan_provider(
        repo_root,
        &repo_root.join("shared/claude/skills/_shared"),
        "md",
        Provider::Claude,
        None,
        &mut assets,
        &mut files,
    )?;

    assets.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files.sort_by(|left, right| {
        left.manifest
            .relative_path
            .cmp(&right.manifest.relative_path)
    });
    reject_duplicate_paths(&files)?;
    Ok(ScannedCatalog { assets, files })
}

fn scan_skills(
    repo_root: &Path,
    assets: &mut Vec<AssetManifest>,
    files: &mut Vec<SourceFile>,
) -> Result<(), String> {
    let skills_root = repo_root.join("skills");
    for path in sorted_entries(&skills_root)? {
        let metadata = reject_symlink(&path)?;
        if !metadata.is_dir() {
            return Err(format!(
                "{}: top-level skill entries must be directories",
                path.display()
            ));
        }
        let name = file_name_utf8(&path)?;
        validate_name(&path, &name)?;
        let skill_files = scan_directory(repo_root, &path)?;
        if !skill_files
            .iter()
            .any(|file| file.source_path == path.join("SKILL.md"))
        {
            return Err(format!("{}: required SKILL.md is absent", path.display()));
        }
        let size = skill_files.iter().map(|file| file.manifest.size).sum();
        assets.push(AssetManifest {
            name,
            relative_path: relative_utf8(repo_root, &path)?,
            kind: AssetKind::Skill,
            source_type: SourceType::Directory,
            provider: None,
            size,
            files: skill_files
                .iter()
                .map(|file| file.manifest.clone())
                .collect(),
        });
        files.extend(skill_files);
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the source policy is explicit at each call site"
)]
fn scan_provider(
    repo_root: &Path,
    source_root: &Path,
    extension: &str,
    provider: Provider,
    excluded_directory: Option<&str>,
    assets: &mut Vec<AssetManifest>,
    files: &mut Vec<SourceFile>,
) -> Result<(), String> {
    let support = source_root.ends_with(Path::new("shared/claude/skills/_shared"));
    for path in sorted_entries(source_root)? {
        let metadata = reject_symlink(&path)?;
        let name = file_name_utf8(&path)?;
        if metadata.is_dir() && name.starts_with('.') {
            continue;
        }
        if metadata.is_dir() && excluded_directory == Some(name.as_str()) {
            continue;
        }
        if !metadata.is_file() {
            return Err(format!(
                "{}: provider assets must be regular files",
                path.display()
            ));
        }
        if path.extension().and_then(|value| value.to_str()) != Some(extension) {
            return Err(format!(
                "{}: asset format does not match its provider directory",
                path.display()
            ));
        }
        let source_file = scan_file(repo_root, path.clone(), metadata)?;
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("{}: asset stem is not valid UTF-8", path.display()))?;
        validate_name(&path, stem)?;
        assets.push(AssetManifest {
            name: stem.to_owned(),
            relative_path: source_file.manifest.relative_path.clone(),
            kind: if support {
                AssetKind::Support
            } else {
                AssetKind::Agent
            },
            source_type: SourceType::File,
            provider: Some(provider),
            size: source_file.manifest.size,
            files: vec![source_file.manifest.clone()],
        });
        files.push(source_file);
    }
    Ok(())
}

fn scan_directory(repo_root: &Path, root: &Path) -> Result<Vec<SourceFile>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut child_directories = Vec::new();
        for path in sorted_entries(&directory)? {
            let metadata = reject_symlink(&path)?;
            if metadata.is_dir() {
                child_directories.push(path);
            } else if metadata.is_file() {
                files.push(scan_file(repo_root, path, metadata)?);
            } else {
                return Err(format!(
                    "{}: only directories and regular files are allowed",
                    path.display()
                ));
            }
        }
        child_directories.reverse();
        pending.extend(child_directories);
    }
    files.sort_by(|left, right| {
        left.manifest
            .relative_path
            .cmp(&right.manifest.relative_path)
    });
    Ok(files)
}

fn scan_file(
    repo_root: &Path,
    path: PathBuf,
    metadata: fs::Metadata,
) -> Result<SourceFile, String> {
    let bytes =
        fs::read(&path).map_err(|error| format!("{}: cannot read: {error}", path.display()))?;
    validate_portable_bytes(&path, &bytes)?;
    let manifest = super::model::FileManifest {
        relative_path: relative_utf8(repo_root, &path)?,
        size: u64::try_from(bytes.len())
            .map_err(|error| format!("{}: size overflow: {error}", path.display()))?,
        mode: normalized_mode(&path, &metadata)?,
        sha256: sha256(&bytes),
    };
    Ok(SourceFile {
        manifest,
        source_path: path,
        bytes,
    })
}

fn sorted_entries(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("{}: cannot read directory: {error}", directory.display()))?;
    let mut paths = entries
        .map(|entry| {
            entry
                .map(|value| value.path())
                .map_err(|error| format!("{}: cannot read entry: {error}", directory.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for path in &paths {
        file_name_utf8(path)?;
    }
    paths.sort();
    Ok(paths)
}

fn reject_symlink(path: &Path) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{}: cannot inspect: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        Err(format!("{}: source symlinks are forbidden", path.display()))
    } else {
        Ok(metadata)
    }
}

fn reject_duplicate_paths(files: &[SourceFile]) -> Result<(), String> {
    let mut paths = BTreeSet::new();
    for file in files {
        if !paths.insert(&file.manifest.relative_path) {
            return Err(format!(
                "{}: duplicate runtime path",
                file.manifest.relative_path
            ));
        }
    }
    Ok(())
}
