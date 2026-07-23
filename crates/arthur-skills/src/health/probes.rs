use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

#[cfg(unix)]
use rustix::fs::{Mode, OFlags};

use super::{HealthIssue, IssueSeverity, issue};
use crate::catalog::{Catalog, Provider as CatalogProvider};
#[cfg(unix)]
use crate::platform::metadata_mode;
use crate::platform::{
    metadata_device, metadata_inode, metadata_mtime_nanoseconds, metadata_mtime_seconds,
    open_directory, same_node,
};
use crate::provider::{ProviderId, RootIdentity};
use crate::provider_health::{ProviderHealth, ProviderIssue, assess};
use crate::receipt::{OwnedAssetKind, Receipt};

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const OUTPUT_LIMIT: u64 = 4 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderProbe {
    pub provider: ProviderId,
    pub executable: Option<PathBuf>,
    pub observed_version: Option<String>,
    pub validated_minimum: Option<String>,
    pub compatible: Option<bool>,
    pub published_models: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityProbe {
    pub command: String,
    pub required: bool,
    pub available: bool,
}

pub(super) fn inspect_providers(
    catalog: &Catalog,
    receipt: &Receipt,
    execute: bool,
    issues: &mut Vec<HealthIssue>,
) -> Vec<ProviderProbe> {
    receipt
        .providers
        .iter()
        .filter(|provider| provider.managed_integration)
        .map(|provider| probe_provider(catalog, receipt, provider.provider, execute, issues))
        .collect()
}

fn probe_provider(
    catalog: &Catalog,
    receipt: &Receipt,
    provider: ProviderId,
    execute: bool,
    issues: &mut Vec<HealthIssue>,
) -> ProviderProbe {
    let catalog_provider = match provider {
        ProviderId::Claude => CatalogProvider::Claude,
        ProviderId::Codex => CatalogProvider::Codex,
    };
    let contract = catalog
        .manifest()
        .provider_contracts
        .iter()
        .find(|contract| contract.provider == catalog_provider);
    let minimum = contract.map(|contract| contract.validated_version.clone());
    let published_models = installed_models(receipt, provider, issues);
    for model in &published_models {
        if !contract.is_some_and(|contract| contract.models.contains(model)) {
            issues.push(provider_issue(
                provider,
                ProviderIssue::UnknownModel {
                    published: model.clone(),
                },
            ));
        }
    }

    let executable = if execute {
        match resolve_executable(provider.as_str()) {
            Ok(Some(executable)) => Some(executable),
            Ok(None) => {
                issues.push(issue(
                    "provider_cli_missing",
                    IssueSeverity::Warning,
                    format!(
                        "{provider} is not available on PATH; runtime compatibility is unverified"
                    ),
                    None,
                ));
                None
            }
            Err(message) => {
                issues.push(issue(
                    "provider_path_unsafe",
                    IssueSeverity::Error,
                    message,
                    None,
                ));
                None
            }
        }
    } else {
        None
    };
    let observed = executable
        .as_deref()
        .and_then(|executable| match bounded_version(executable) {
            Ok(Some(version)) => Some(version),
            Ok(None) => {
                issues.push(provider_issue(
                    provider,
                    ProviderIssue::InvalidVersion {
                        observed: "unrecognized --version output".to_owned(),
                    },
                ));
                None
            }
            Err(message) => {
                issues.push(issue(
                    "provider_cli_failed",
                    IssueSeverity::Error,
                    message,
                    Some(executable.to_path_buf()),
                ));
                None
            }
        });
    let compatible = observed.as_deref().map(|version| {
        let Some(model) = published_models
            .first()
            .or_else(|| contract.and_then(|contract| contract.models.first()))
        else {
            issues.push(issue(
                "provider_contract_missing",
                IssueSeverity::Error,
                format!("no published model exists for {provider}"),
                None,
            ));
            return false;
        };
        match assess(catalog.manifest(), catalog_provider, version, model) {
            ProviderHealth::Healthy => true,
            ProviderHealth::Unhealthy(problem) => {
                issues.push(provider_issue(provider, problem));
                false
            }
        }
    });
    ProviderProbe {
        provider,
        executable,
        observed_version: observed,
        validated_minimum: minimum,
        compatible,
        published_models,
    }
}

fn installed_models(
    receipt: &Receipt,
    provider: ProviderId,
    issues: &mut Vec<HealthIssue>,
) -> Vec<String> {
    let Some(root) = receipt
        .providers
        .iter()
        .find(|entry| entry.provider == provider && entry.managed_integration)
        .and_then(|entry| entry.root.as_ref())
    else {
        issues.push(issue(
            "provider_root_missing",
            IssueSeverity::Error,
            format!("managed {provider} integration has no recorded root"),
            None,
        ));
        return Vec::new();
    };
    let prefix = match provider {
        ProviderId::Claude => "agents/claude/",
        ProviderId::Codex => "agents/codex/",
    };
    let mut models = Vec::new();
    for asset in receipt
        .assets
        .iter()
        .filter(|asset| asset.kind == OwnedAssetKind::File && asset.source_id.starts_with(prefix))
    {
        match read_model(&asset.destination, provider, root) {
            Ok(Some(model)) => models.push(model),
            Ok(None) => issues.push(issue(
                "provider_model_missing",
                IssueSeverity::Error,
                format!("installed {provider} agent has no model declaration"),
                Some(asset.destination.clone()),
            )),
            Err(error) => issues.push(issue(
                "provider_model_unreadable",
                IssueSeverity::Error,
                error.to_string(),
                Some(asset.destination.clone()),
            )),
        }
    }
    models.sort();
    models.dedup();
    models
}

fn read_model(
    path: &Path,
    provider: ProviderId,
    root: &RootIdentity,
) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    open_regular_beneath(path, root)?
        .take(64 * 1024)
        .read_to_end(&mut bytes)?;
    let text = String::from_utf8(bytes).map_err(io::Error::other)?;
    let model = text.lines().find_map(|line| match provider {
        ProviderId::Claude => line.trim().strip_prefix("model:").map(clean_model_value),
        ProviderId::Codex => line.trim().strip_prefix("model =").map(clean_model_value),
    });
    if model
        .as_ref()
        .is_some_and(|model| model.len() > 256 || model.chars().any(char::is_control))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "provider model contains control characters or exceeds 256 bytes",
        ));
    }
    Ok(model)
}

fn open_regular_beneath(path: &Path, root: &RootIdentity) -> io::Result<File> {
    let relative = path.strip_prefix(&root.lexical).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "agent path is outside its root",
        )
    })?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "agent path has no parent"))?;
    #[cfg(unix)]
    let leaf = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "agent path has no filename"))?;
    let expected_parent = root
        .real
        .join(relative)
        .parent()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "agent path has no real parent")
        })?
        .to_path_buf();

    let before = fs::symlink_metadata(parent)?;
    if !before.file_type().is_dir() || before.file_type().is_symlink() {
        return Err(io::Error::other("agent parent is not a regular directory"));
    }
    let directory = open_directory(parent)?;
    let opened = directory.metadata()?;
    let after = fs::symlink_metadata(parent)?;
    if !opened.is_dir()
        || !after.file_type().is_dir()
        || after.file_type().is_symlink()
        || !same_node(&before, &opened)
        || !same_node(&opened, &after)
        || fs::canonicalize(parent)? != expected_parent
    {
        return Err(io::Error::other(
            "agent parent identity changed during inspection",
        ));
    }

    #[cfg(unix)]
    let file = File::from(
        rustix::fs::openat(
            &directory,
            leaf,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(io::Error::from)?,
    );
    #[cfg(windows)]
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let path_metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || !path_metadata.file_type().is_file()
        || !same_node(&metadata, &path_metadata)
    {
        return Err(io::Error::other("agent is not a stable regular file"));
    }
    Ok(file)
}

fn clean_model_value(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character| matches!(character, '"' | '\''))
        .to_owned()
}

fn provider_issue(provider: ProviderId, problem: ProviderIssue) -> HealthIssue {
    let message = match problem {
        ProviderIssue::ContractAbsent => format!("no provider contract exists for {provider}"),
        ProviderIssue::InvalidVersion { observed } => {
            format!("{provider} reported an invalid version: {observed}")
        }
        ProviderIssue::VersionBelowMinimum { observed, minimum } => {
            format!("{provider} {observed} is below validated minimum {minimum}")
        }
        ProviderIssue::UnknownModel { published } => {
            format!("{provider} agent publishes unknown model {published}")
        }
    };
    issue("provider_incompatible", IssueSeverity::Error, message, None)
}

fn resolve_executable(command: &str) -> Result<Option<PathBuf>, String> {
    let path = std::env::var_os("PATH");
    resolve_executable_from(command, path.as_deref())
}

fn resolve_executable_from(command: &str, path: Option<&OsStr>) -> Result<Option<PathBuf>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    for directory in std::env::split_paths(&path) {
        if !directory.is_absolute() {
            return Err("PATH contains a relative or empty component".to_owned());
        }
        for candidate in executable_candidates(&directory, command) {
            let metadata = match fs::metadata(&candidate) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!("cannot inspect {}: {error}", candidate.display()));
                }
            };
            if !metadata.is_file() || !is_executable(&metadata) {
                continue;
            }
            let canonical = fs::canonicalize(&candidate)
                .map_err(|error| format!("cannot resolve {}: {error}", candidate.display()))?;
            trusted_executable_identity(&canonical)?;
            return Ok(Some(canonical));
        }
    }
    Ok(None)
}

#[cfg(unix)]
fn executable_candidates(directory: &Path, command: &str) -> Vec<PathBuf> {
    vec![directory.join(command)]
}

#[cfg(windows)]
fn executable_candidates(directory: &Path, command: &str) -> Vec<PathBuf> {
    let extensions = std::env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .filter(|extension| !extension.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            [".COM", ".EXE", ".BAT", ".CMD"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        });
    let mut candidates = vec![directory.join(command)];
    candidates.extend(
        extensions
            .into_iter()
            .map(|extension| directory.join(format!("{command}{extension}"))),
    );
    candidates
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    metadata_mode(metadata) & 0o111 != 0
}

#[cfg(windows)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExecutableIdentity {
    device: u64,
    inode: u64,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
}

#[cfg(unix)]
fn trusted_executable_identity(path: &Path) -> Result<ExecutableIdentity, String> {
    use std::os::unix::fs::MetadataExt;

    let effective_uid = rustix::process::geteuid().as_raw();
    let mut current = Some(path);
    while let Some(candidate) = current {
        let metadata = fs::metadata(candidate)
            .map_err(|error| format!("cannot inspect {}: {error}", candidate.display()))?;
        let mode = metadata_mode(&metadata);
        let trusted_owner = metadata.uid() == 0 || metadata.uid() == effective_uid;
        let sticky_root_directory = metadata.is_dir() && metadata.uid() == 0 && mode & 0o1000 != 0;
        if !trusted_owner || (mode & 0o022 != 0 && !sticky_root_directory) {
            return Err(format!(
                "provider executable ancestry is not trusted: {}",
                candidate.display()
            ));
        }
        current = candidate.parent();
    }
    let metadata = fs::metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if !metadata.is_file() || !is_executable(&metadata) {
        return Err(format!(
            "provider executable is not runnable: {}",
            path.display()
        ));
    }
    Ok(ExecutableIdentity {
        device: metadata_device(&metadata),
        inode: metadata_inode(&metadata),
        size: metadata.len(),
        modified_seconds: metadata_mtime_seconds(&metadata),
        modified_nanoseconds: metadata_mtime_nanoseconds(&metadata),
    })
}

#[cfg(windows)]
fn trusted_executable_identity(path: &Path) -> Result<ExecutableIdentity, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "provider executable is not a regular file: {}",
            path.display()
        ));
    }
    Ok(ExecutableIdentity {
        device: metadata_device(&metadata),
        inode: metadata_inode(&metadata),
        size: metadata.len(),
        modified_seconds: metadata_mtime_seconds(&metadata),
        modified_nanoseconds: metadata_mtime_nanoseconds(&metadata),
    })
}

fn bounded_version(executable: &Path) -> Result<Option<String>, String> {
    let expected = trusted_executable_identity(executable)?;
    let mut child = Command::new(executable)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot execute {}: {error}", executable.display()))?;
    let observed = match trusted_executable_identity(executable) {
        Ok(observed) => observed,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    if observed != expected {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "provider executable changed while starting: {}",
            executable.display()
        ));
    }
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("provider stdout was not captured".to_owned());
    };
    let Some(stderr) = child.stderr.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("provider stderr was not captured".to_owned());
    };
    let stdout = bounded_reader(stdout);
    let stderr = bounded_reader(stderr);
    wait_bounded(&mut child)?;
    let stdout = receive_output(stdout)?;
    let stderr = receive_output(stderr)?;
    let output = if stdout.is_empty() { stderr } else { stdout };
    Ok(extract_version(&String::from_utf8_lossy(&output)))
}

fn bounded_reader(reader: impl Read + Send + 'static) -> Receiver<io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = reader
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .and_then(|_| {
                if u64::try_from(bytes.len()).is_ok_and(|length| length > OUTPUT_LIMIT) {
                    Err(io::Error::other("provider version output exceeds 4 KiB"))
                } else {
                    Ok(bytes)
                }
            });
        let _ = sender.send(result);
    });
    receiver
}

fn wait_bounded(child: &mut Child) -> Result<(), String> {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("provider --version exited with {status}")),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("provider --version exceeded the two-second timeout".to_owned());
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("cannot wait for provider --version: {error}"));
            }
        }
    }
}

fn receive_output(receiver: Receiver<io::Result<Vec<u8>>>) -> Result<Vec<u8>, String> {
    receiver
        .recv_timeout(Duration::from_millis(250))
        .map_err(|_| "provider output pipe did not close".to_owned())?
        .map_err(|error| error.to_string())
}

pub(super) fn inspect_capabilities(
    catalog: &Catalog,
    issues: &mut Vec<HealthIssue>,
) -> Vec<CapabilityProbe> {
    catalog
        .manifest()
        .external_capabilities
        .iter()
        .map(|capability| {
            let available = capability_available(
                &capability.command,
                capability.required,
                resolve_executable(&capability.command),
                issues,
            );
            CapabilityProbe {
                command: capability.command.clone(),
                required: capability.required,
                available,
            }
        })
        .collect()
}

fn capability_available(
    command: &str,
    required: bool,
    resolution: Result<Option<PathBuf>, String>,
    issues: &mut Vec<HealthIssue>,
) -> bool {
    let requirement = if required { "required" } else { "optional" };
    match resolution {
        Ok(Some(_)) => true,
        Ok(None) => {
            issues.push(issue(
                "capability_missing",
                if required {
                    IssueSeverity::Error
                } else {
                    IssueSeverity::Warning
                },
                format!("{requirement} command {command} is not available on PATH"),
                None,
            ));
            false
        }
        Err(error) => {
            issues.push(issue(
                "capability_untrusted",
                IssueSeverity::Error,
                format!("cannot safely resolve {requirement} command {command}: {error}"),
                None,
            ));
            false
        }
    }
}

fn extract_version(text: &str) -> Option<String> {
    text.split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .find(|candidate| {
            let mut parts = candidate.split('.');
            parts.next().is_some_and(|part| !part.is_empty())
                && parts.next().is_some_and(|part| !part.is_empty())
                && parts.next().is_some_and(|part| !part.is_empty())
                && parts.next().is_none()
        })
        .map(str::to_owned)
}

#[cfg(all(test, unix))]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::mpsc;

    use tempfile::tempdir;

    use super::{
        OUTPUT_LIMIT, bounded_reader, bounded_version, capability_available, extract_version,
        inspect_providers, provider_issue, receive_output, resolve_executable_from, wait_bounded,
    };
    use crate::catalog::Catalog;
    use crate::provider::{ProviderId, resolve_roots_from};
    use crate::provider_health::ProviderIssue;
    use crate::receipt::{OwnedAsset, OwnedAssetKind, Receipt};

    fn script(directory: &Path, name: &str, body: &str) -> std::io::Result<PathBuf> {
        let path = directory.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        Ok(path)
    }

    #[test]
    fn extracts_provider_versions_without_accepting_partial_versions() {
        assert_eq!(
            extract_version("codex-cli 0.144.6"),
            Some("0.144.6".to_owned())
        );
        assert_eq!(
            extract_version("Claude Code 2.1.217 (stable)"),
            Some("2.1.217".to_owned())
        );
        assert_eq!(extract_version("version 2.1"), None);
    }

    #[test]
    fn capability_resolution_distinguishes_missing_and_untrusted_commands() {
        let mut issues = Vec::new();
        assert!(capability_available(
            "available",
            true,
            Ok(Some(PathBuf::from("/bin/available"))),
            &mut issues
        ));
        assert!(issues.is_empty());

        assert!(!capability_available(
            "optional",
            false,
            Ok(None),
            &mut issues
        ));
        assert_eq!(issues[0].code, "capability_missing");
        assert!(issues[0].message.contains("optional command"));
        assert!(!capability_available(
            "required",
            true,
            Ok(None),
            &mut issues
        ));
        assert_eq!(issues[1].severity, crate::health::IssueSeverity::Error);
        assert!(issues[1].message.contains("required command"));
        assert!(!capability_available(
            "unsafe",
            false,
            Err("unsafe PATH ancestry".to_owned()),
            &mut issues
        ));
        assert_eq!(issues[2].code, "capability_untrusted");
        assert_eq!(issues[2].severity, crate::health::IssueSeverity::Error);
    }

    #[test]
    fn executable_resolution_rejects_unsafe_path_entries_and_modes()
    -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(resolve_executable_from("codex", None)?, None);
        assert!(resolve_executable_from("codex", Some(OsStr::new("relative"))).is_err());

        let directory = tempdir()?;
        let bin = directory.path().join("bin");
        fs::create_dir(&bin)?;
        let executable = script(&bin, "codex", "printf 'codex 0.144.6\\n'")?;
        assert_eq!(
            resolve_executable_from("codex", Some(bin.as_os_str()))?,
            Some(fs::canonicalize(&executable)?)
        );

        fs::set_permissions(&executable, fs::Permissions::from_mode(0o644))?;
        assert_eq!(
            resolve_executable_from("codex", Some(bin.as_os_str()))?,
            None
        );
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o777))?;
        assert!(resolve_executable_from("codex", Some(bin.as_os_str())).is_err());
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o777))?;
        assert!(resolve_executable_from("codex", Some(bin.as_os_str())).is_err());
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))?;
        Ok(())
    }

    #[test]
    fn bounded_provider_versions_cover_stdout_stderr_failure_size_and_timeout()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let stdout = script(directory.path(), "stdout", "printf 'tool 1.2.3\\n'")?;
        let stderr = script(directory.path(), "stderr", "printf 'tool 2.3.4\\n' >&2")?;
        let invalid = script(directory.path(), "invalid", "printf 'tool 2.3\\n'")?;
        let failed = script(directory.path(), "failed", "exit 7")?;
        let oversized = script(
            directory.path(),
            "oversized",
            "i=0; while [ \"$i\" -lt 5000 ]; do printf x; i=$((i + 1)); done",
        )?;
        let timeout = script(directory.path(), "timeout", "/bin/sleep 3")?;

        assert_eq!(bounded_version(&stdout)?, Some("1.2.3".to_owned()));
        assert_eq!(bounded_version(&stderr)?, Some("2.3.4".to_owned()));
        assert_eq!(bounded_version(&invalid)?, None);
        assert!(bounded_version(&failed).is_err());
        assert!(bounded_version(&oversized).is_err());
        assert!(bounded_version(&timeout).is_err());
        assert!(bounded_version(&directory.path().join("missing")).is_err());

        let reader = bounded_reader(std::io::Cursor::new(vec![b'x'; OUTPUT_LIMIT as usize + 1]));
        assert!(receive_output(reader).is_err());
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        assert!(receive_output(receiver).is_err());

        let mut success = Command::new("/bin/sh").args(["-c", "exit 0"]).spawn()?;
        assert!(wait_bounded(&mut success).is_ok());
        let mut failure = Command::new("/bin/sh").args(["-c", "exit 9"]).spawn()?;
        assert!(wait_bounded(&mut failure).is_err());
        Ok(())
    }

    #[test]
    fn installed_models_and_provider_contract_issues_are_reported()
    -> Result<(), Box<dyn std::error::Error>> {
        let home = tempdir()?;
        let roots = resolve_roots_from(Some(home.path().as_os_str()), None, &ProviderId::ALL)?;
        let claude_root = roots
            .provider(ProviderId::Claude)
            .ok_or("Claude root is missing")?
            .root
            .lexical
            .clone();
        let codex_root = roots
            .provider(ProviderId::Codex)
            .ok_or("Codex root is missing")?
            .root
            .lexical
            .clone();
        fs::create_dir_all(claude_root.join("agents"))?;
        fs::create_dir_all(codex_root.join("agents"))?;
        let claude = claude_root.join("agents/example.md");
        let codex = codex_root.join("agents/example.toml");
        let missing_model = codex_root.join("agents/missing-model.toml");
        let escaped_model = codex_root.join("agents/escaped.toml");
        let control_model = codex_root.join("agents/control.toml");
        let outside = home.path().join("outside.toml");
        fs::write(&claude, "---\nmodel: \"claude-fable-5[1m]\"\n---\n")?;
        fs::write(&codex, "model = 'unknown-model'\n")?;
        fs::write(&missing_model, "name = \"missing\"\n")?;
        fs::write(&outside, "model = 'gpt-5.6-sol'\n")?;
        symlink(&outside, &escaped_model)?;
        fs::write(&control_model, b"model = 'bad\x1b[31m'\n")?;
        let mut receipt = Receipt::new("0.1.0", "a".repeat(64), &roots);
        for (source_id, destination) in [
            ("agents/claude/example.md", claude),
            ("agents/codex/example.toml", codex),
            ("agents/codex/missing.toml", missing_model),
            ("agents/codex/escaped.toml", escaped_model),
            ("agents/codex/control.toml", control_model),
            (
                "agents/codex/unreadable.toml",
                codex_root.join("agents/absent.toml"),
            ),
        ] {
            receipt.assets.push(OwnedAsset {
                source_id: source_id.to_owned(),
                destination,
                kind: OwnedAssetKind::File,
                hash: None,
                mode: Some(0o644),
                link_target: None,
                references: Vec::new(),
            });
        }
        let catalog = Catalog::load()?;
        let mut issues = Vec::new();
        let probes = inspect_providers(&catalog, &receipt, false, &mut issues);
        assert_eq!(probes.len(), 2);
        assert!(probes.iter().all(|probe| probe.executable.is_none()));
        for code in [
            "provider_incompatible",
            "provider_model_missing",
            "provider_model_unreadable",
        ] {
            assert!(issues.iter().any(|issue| issue.code == code));
        }

        for problem in [
            ProviderIssue::ContractAbsent,
            ProviderIssue::InvalidVersion {
                observed: "invalid".to_owned(),
            },
            ProviderIssue::VersionBelowMinimum {
                observed: "1.0.0".to_owned(),
                minimum: "2.0.0".to_owned(),
            },
            ProviderIssue::UnknownModel {
                published: "unknown".to_owned(),
            },
        ] {
            assert_eq!(
                provider_issue(ProviderId::Codex, problem).code,
                "provider_incompatible"
            );
        }
        Ok(())
    }
}
