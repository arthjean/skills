use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const ENVIRONMENT_EXIT_CODE: u8 = 4;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Claude,
    Codex,
}

impl ProviderId {
    pub const ALL: [Self; 2] = [Self::Claude, Self::Codex];

    pub const fn label(self) -> &'static str {
        ProviderRegistry::get(self).labels.name
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderLabels {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillStrategy {
    RelativeSymlinks { directory: &'static str },
    CanonicalDiscovery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentStrategy {
    Copy { directory: &'static str },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderCapabilities {
    pub managed_skill_activation: bool,
    pub managed_agents: bool,
    pub implicit_skill_visibility: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderDefinition {
    pub id: ProviderId,
    pub labels: ProviderLabels,
    pub skills: SkillStrategy,
    pub agents: AgentStrategy,
    pub capabilities: ProviderCapabilities,
    root: RootStrategy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootStrategy {
    HomeChild(&'static str),
    CodexHome,
}

const PROVIDERS: [ProviderDefinition; 2] = [
    ProviderDefinition {
        id: ProviderId::Claude,
        labels: ProviderLabels {
            name: "Claude Code",
            description: "Activates canonical skills with relative links and installs Claude agents.",
        },
        skills: SkillStrategy::RelativeSymlinks {
            directory: "skills",
        },
        agents: AgentStrategy::Copy {
            directory: "agents",
        },
        capabilities: ProviderCapabilities {
            managed_skill_activation: true,
            managed_agents: true,
            implicit_skill_visibility: false,
        },
        root: RootStrategy::HomeChild(".claude"),
    },
    ProviderDefinition {
        id: ProviderId::Codex,
        labels: ProviderLabels {
            name: "Codex",
            description: "Discovers canonical skills implicitly and installs Codex agents.",
        },
        skills: SkillStrategy::CanonicalDiscovery,
        agents: AgentStrategy::Copy {
            directory: "agents",
        },
        capabilities: ProviderCapabilities {
            managed_skill_activation: false,
            managed_agents: true,
            implicit_skill_visibility: true,
        },
        root: RootStrategy::CodexHome,
    },
];

pub struct ProviderRegistry;

impl ProviderRegistry {
    pub const fn all() -> &'static [ProviderDefinition] {
        &PROVIDERS
    }

    pub const fn get(id: ProviderId) -> &'static ProviderDefinition {
        match id {
            ProviderId::Claude => &PROVIDERS[0],
            ProviderId::Codex => &PROVIDERS[1],
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RootIdentity {
    pub lexical: PathBuf,
    pub real: PathBuf,
    pub device: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProvider {
    pub id: ProviderId,
    pub root: RootIdentity,
    pub skills: Option<PathBuf>,
    pub agents: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedRoots {
    pub home: RootIdentity,
    pub canonical: RootIdentity,
    pub canonical_skills: PathBuf,
    pub state_directory: PathBuf,
    pub receipt_path: PathBuf,
    pub providers: Vec<ResolvedProvider>,
}

impl ResolvedRoots {
    pub fn provider(&self, id: ProviderId) -> Option<&ResolvedProvider> {
        self.providers.iter().find(|provider| provider.id == id)
    }

    pub fn allowed_top_level_roots(&self) -> impl Iterator<Item = &RootIdentity> {
        std::iter::once(&self.canonical).chain(self.providers.iter().map(|provider| &provider.root))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PathDiagnostic {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_utf8: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_bytes_hex: Option<String>,
}

impl PathDiagnostic {
    fn new(path: &OsStr) -> Self {
        if let Some(path) = path.to_str() {
            Self {
                path_utf8: Some(path.to_owned()),
                path_bytes_hex: None,
            }
        } else {
            Self {
                path_utf8: None,
                path_bytes_hex: Some(os_str_hex(path)),
            }
        }
    }

    fn display(&self) -> &str {
        self.path_utf8
            .as_deref()
            .or(self.path_bytes_hex.as_deref())
            .unwrap_or("<missing path>")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolveError {
    UnsupportedPlatform,
    MissingHome,
    EmptyPath {
        variable: &'static str,
    },
    NonUtf8Path {
        variable: &'static str,
        path: PathDiagnostic,
    },
    NotAbsolute {
        variable: &'static str,
        path: PathDiagnostic,
    },
    EscapesFilesystemRoot {
        variable: &'static str,
        path: PathDiagnostic,
    },
    NotDirectory {
        variable: &'static str,
        path: PathDiagnostic,
    },
    Inaccessible {
        variable: &'static str,
        path: PathDiagnostic,
        detail: String,
    },
}

impl ResolveError {
    pub const fn exit_code(&self) -> u8 {
        ENVIRONMENT_EXIT_CODE
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter.write_str("only Linux and macOS are supported"),
            Self::MissingHome => formatter.write_str(
                "cannot resolve a safe user home: set HOME to an accessible absolute directory",
            ),
            Self::EmptyPath { variable } => write!(formatter, "{variable} cannot be empty"),
            Self::NonUtf8Path { variable, path } => write!(
                formatter,
                "{variable} is not valid UTF-8 (path bytes: {})",
                path.display()
            ),
            Self::NotAbsolute { variable, path } => {
                write!(formatter, "{variable} must be absolute: {}", path.display())
            }
            Self::EscapesFilesystemRoot { variable, path } => write!(
                formatter,
                "{variable} escapes the filesystem root: {}",
                path.display()
            ),
            Self::NotDirectory { variable, path } => write!(
                formatter,
                "{variable} must resolve to a directory: {}",
                path.display()
            ),
            Self::Inaccessible {
                variable,
                path,
                detail,
            } => write!(
                formatter,
                "{variable} is not accessible at {}: {detail}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ResolveError {}

pub fn resolve_roots(selected: &[ProviderId]) -> Result<ResolvedRoots, ResolveError> {
    let home = env::var_os("HOME");
    let codex_home = env::var_os("CODEX_HOME");
    resolve_roots_from(home.as_deref(), codex_home.as_deref(), selected)
}

pub fn resolve_roots_from(
    home: Option<&OsStr>,
    codex_home: Option<&OsStr>,
    selected: &[ProviderId],
) -> Result<ResolvedRoots, ResolveError> {
    if !cfg!(unix) {
        return Err(ResolveError::UnsupportedPlatform);
    }

    let home = home.ok_or(ResolveError::MissingHome)?;
    let home_path = validated_path("HOME", home)?;
    let home_identity = resolve_identity("HOME", &home_path, true)?;
    fs::read_dir(&home_identity.lexical).map_err(|error| ResolveError::Inaccessible {
        variable: "HOME",
        path: PathDiagnostic::new(home_identity.lexical.as_os_str()),
        detail: error.to_string(),
    })?;

    let canonical_path = home_identity.lexical.join(".agents");
    let canonical = resolve_identity("canonical root", &canonical_path, false)?;
    let canonical_skills = canonical.lexical.join("skills");
    let state_directory = canonical.lexical.join(".arthur-workflow");
    let receipt_path = state_directory.join("receipt.json");
    let mut providers = Vec::new();

    for definition in ProviderRegistry::all() {
        if !selected.contains(&definition.id) {
            continue;
        }
        let root_path = match definition.root {
            RootStrategy::HomeChild(child) => home_identity.lexical.join(child),
            RootStrategy::CodexHome => match codex_home {
                Some(path) => validated_path("CODEX_HOME", path)?,
                None => home_identity.lexical.join(".codex"),
            },
        };
        let root = resolve_identity(definition.id.as_str(), &root_path, false)?;
        let skills = match definition.skills {
            SkillStrategy::RelativeSymlinks { directory } => Some(root.lexical.join(directory)),
            SkillStrategy::CanonicalDiscovery => None,
        };
        let AgentStrategy::Copy { directory } = definition.agents;
        let agents = root.lexical.join(directory);
        providers.push(ResolvedProvider {
            id: definition.id,
            root,
            skills,
            agents,
        });
    }

    Ok(ResolvedRoots {
        home: home_identity,
        canonical,
        canonical_skills,
        state_directory,
        receipt_path,
        providers,
    })
}

fn validated_path(variable: &'static str, value: &OsStr) -> Result<PathBuf, ResolveError> {
    let diagnostic = PathDiagnostic::new(value);
    if value.to_str().is_none() {
        return Err(ResolveError::NonUtf8Path {
            variable,
            path: diagnostic,
        });
    }
    if value.is_empty() {
        return Err(ResolveError::EmptyPath { variable });
    }
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(ResolveError::NotAbsolute {
            variable,
            path: diagnostic,
        });
    }
    normalize_absolute(variable, path)
}

fn normalize_absolute(variable: &'static str, path: &Path) -> Result<PathBuf, ResolveError> {
    let mut normalized = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::CurDir => {}
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(ResolveError::EscapesFilesystemRoot {
                        variable,
                        path: PathDiagnostic::new(path.as_os_str()),
                    });
                }
            }
            Component::Prefix(_) => return Err(ResolveError::UnsupportedPlatform),
        }
    }
    Ok(normalized)
}

fn resolve_identity(
    variable: &'static str,
    lexical: &Path,
    must_exist: bool,
) -> Result<RootIdentity, ResolveError> {
    let mut existing = lexical;
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !must_exist => {
                existing = existing
                    .parent()
                    .ok_or_else(|| ResolveError::Inaccessible {
                        variable,
                        path: PathDiagnostic::new(lexical.as_os_str()),
                        detail: error.to_string(),
                    })?;
            }
            Err(error) => {
                return Err(ResolveError::Inaccessible {
                    variable,
                    path: PathDiagnostic::new(lexical.as_os_str()),
                    detail: error.to_string(),
                });
            }
        }
    }

    let metadata = fs::metadata(existing).map_err(|error| ResolveError::Inaccessible {
        variable,
        path: PathDiagnostic::new(existing.as_os_str()),
        detail: error.to_string(),
    })?;
    if !metadata.is_dir() {
        return Err(ResolveError::NotDirectory {
            variable,
            path: PathDiagnostic::new(existing.as_os_str()),
        });
    }
    let real_existing = fs::canonicalize(existing).map_err(|error| ResolveError::Inaccessible {
        variable,
        path: PathDiagnostic::new(existing.as_os_str()),
        detail: error.to_string(),
    })?;
    let suffix = lexical
        .strip_prefix(existing)
        .map_err(|error| ResolveError::Inaccessible {
            variable,
            path: PathDiagnostic::new(lexical.as_os_str()),
            detail: error.to_string(),
        })?;
    let real = real_existing.join(suffix);
    if real.to_str().is_none() {
        return Err(ResolveError::NonUtf8Path {
            variable,
            path: PathDiagnostic::new(real.as_os_str()),
        });
    }

    Ok(RootIdentity {
        lexical: lexical.to_path_buf(),
        real,
        device: metadata_device(&metadata),
    })
}

#[cfg(unix)]
fn metadata_device(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    metadata.dev()
}

#[cfg(not(unix))]
const fn metadata_device(_metadata: &fs::Metadata) -> u64 {
    0
}

#[cfg(unix)]
fn os_str_hex(value: &OsStr) -> String {
    use std::fmt::Write;
    use std::os::unix::ffi::OsStrExt;

    value
        .as_bytes()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

#[cfg(not(unix))]
fn os_str_hex(value: &OsStr) -> String {
    value
        .to_string_lossy()
        .as_bytes()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            use std::fmt::Write;
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fmt;
    use std::fs;
    use tempfile::tempdir;

    use super::{
        ENVIRONMENT_EXIT_CODE, PathDiagnostic, ProviderId, ProviderRegistry, ResolveError,
        SkillStrategy, resolve_roots, resolve_roots_from,
    };

    fn must_succeed<T, E: fmt::Display>(result: Result<T, E>, context: &str) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("{context}: {error}"),
        }
    }

    fn must_exist<T>(value: Option<T>, context: &str) -> T {
        match value {
            Some(value) => value,
            None => panic!("{context}"),
        }
    }

    fn must_fail<T, E>(result: Result<T, E>, context: &str) -> E {
        match result {
            Err(error) => error,
            Ok(_) => panic!("{context}"),
        }
    }

    #[test]
    fn registry_and_standard_roots_encode_provider_behavior() {
        let home = must_succeed(tempdir(), "temporary HOME failed");
        let roots = must_succeed(
            resolve_roots_from(Some(home.path().as_os_str()), None, &ProviderId::ALL),
            "root resolution failed",
        );

        assert_eq!(roots.canonical_skills, home.path().join(".agents/skills"));
        assert_eq!(
            roots.receipt_path,
            home.path().join(".agents/.arthur-workflow/receipt.json")
        );
        let claude = must_exist(roots.provider(ProviderId::Claude), "missing Claude");
        assert_eq!(
            claude.skills.as_deref(),
            Some(home.path().join(".claude/skills").as_path())
        );
        assert_eq!(claude.agents, home.path().join(".claude/agents"));
        let codex = must_exist(roots.provider(ProviderId::Codex), "missing Codex");
        assert_eq!(codex.skills, None);
        assert_eq!(codex.agents, home.path().join(".codex/agents"));
        assert_eq!(ProviderRegistry::all().len(), 2);
        assert_eq!(ProviderId::Claude.label(), "Claude Code");
        assert_eq!(ProviderId::Claude.as_str(), "claude");
        assert_eq!(ProviderId::Codex.as_str(), "codex");
        assert_eq!(ProviderId::Claude.to_string(), "claude");
        assert_eq!(ProviderId::Codex.to_string(), "codex");
        assert_eq!(
            ProviderRegistry::get(ProviderId::Codex).skills,
            SkillStrategy::CanonicalDiscovery
        );
        assert!(
            ProviderRegistry::get(ProviderId::Codex)
                .capabilities
                .implicit_skill_visibility
        );
        assert!(
            !roots.canonical.lexical.exists(),
            "resolution must stay read-only"
        );
    }

    #[test]
    fn selected_providers_define_allowed_roots_and_custom_codex_home() {
        let home = must_succeed(tempdir(), "temporary HOME failed");
        let custom_codex = home.path().join("custom-codex");
        must_succeed(fs::create_dir(&custom_codex), "fixture failed");
        let custom_spelling = home.path().join("unused/../custom-codex");
        let roots = must_succeed(
            resolve_roots_from(
                Some(home.path().as_os_str()),
                Some(custom_spelling.as_os_str()),
                &[ProviderId::Codex],
            ),
            "root resolution failed",
        );

        assert!(roots.provider(ProviderId::Claude).is_none());
        assert_eq!(
            roots
                .provider(ProviderId::Codex)
                .map(|provider| provider.root.lexical.as_path()),
            Some(custom_codex.as_path())
        );
        assert_eq!(
            roots
                .allowed_top_level_roots()
                .map(|root| root.lexical.as_path())
                .collect::<Vec<_>>(),
            vec![roots.canonical.lexical.as_path(), custom_codex.as_path()]
        );
    }

    #[test]
    fn invalid_home_values_report_the_precise_failure() {
        assert!(matches!(
            resolve_roots_from(None, None, &[]),
            Err(ResolveError::MissingHome)
        ));
        assert!(matches!(
            resolve_roots_from(Some(OsStr::new("")), None, &[]),
            Err(ResolveError::EmptyPath { variable: "HOME" })
        ));
        assert!(matches!(
            resolve_roots_from(Some(OsStr::new("relative")), None, &[]),
            Err(ResolveError::NotAbsolute {
                variable: "HOME",
                ..
            })
        ));
        assert!(matches!(
            resolve_roots_from(Some(OsStr::new("/../../home")), None, &[]),
            Err(ResolveError::EscapesFilesystemRoot {
                variable: "HOME",
                ..
            })
        ));

        let parent = must_succeed(tempdir(), "temporary parent failed");
        let missing = parent.path().join("missing-home");
        assert!(matches!(
            resolve_roots_from(Some(missing.as_os_str()), None, &[]),
            Err(ResolveError::Inaccessible {
                variable: "HOME",
                ..
            })
        ));

        let file = parent.path().join("home-file");
        must_succeed(fs::write(&file, b"not a directory"), "fixture failed");
        assert!(matches!(
            resolve_roots_from(Some(file.as_os_str()), None, &[]),
            Err(ResolveError::NotDirectory {
                variable: "HOME",
                ..
            })
        ));

        let home = must_succeed(tempdir(), "temporary HOME failed");
        must_succeed(
            fs::write(home.path().join(".agents"), b"not a directory"),
            "fixture failed",
        );
        assert!(matches!(
            resolve_roots_from(Some(home.path().as_os_str()), None, &[]),
            Err(ResolveError::NotDirectory {
                variable: "canonical root",
                ..
            })
        ));
    }

    #[test]
    fn custom_codex_home_must_be_absolute_nonempty_and_directory_like() {
        let home = must_succeed(tempdir(), "temporary HOME failed");
        assert!(matches!(
            resolve_roots_from(
                Some(home.path().as_os_str()),
                Some(OsStr::new("")),
                &[ProviderId::Codex]
            ),
            Err(ResolveError::EmptyPath {
                variable: "CODEX_HOME"
            })
        ));
        assert!(matches!(
            resolve_roots_from(
                Some(home.path().as_os_str()),
                Some(OsStr::new("relative")),
                &[ProviderId::Codex]
            ),
            Err(ResolveError::NotAbsolute {
                variable: "CODEX_HOME",
                ..
            })
        ));
        let file = home.path().join("codex-file");
        must_succeed(fs::write(&file, b"not a directory"), "fixture failed");
        assert!(matches!(
            resolve_roots_from(
                Some(home.path().as_os_str()),
                Some(file.as_os_str()),
                &[ProviderId::Codex]
            ),
            Err(ResolveError::NotDirectory { .. })
        ));
        assert!(
            resolve_roots_from(
                Some(home.path().as_os_str()),
                Some(OsStr::new("relative")),
                &[ProviderId::Claude]
            )
            .is_ok()
        );
    }

    #[test]
    fn resolve_error_messages_and_exit_codes_are_stable() {
        let utf8_path = |value: &str| PathDiagnostic {
            path_utf8: Some(value.to_owned()),
            path_bytes_hex: None,
        };
        let errors = [
            (
                ResolveError::UnsupportedPlatform,
                "only Linux and macOS are supported",
            ),
            (
                ResolveError::MissingHome,
                "cannot resolve a safe user home: set HOME to an accessible absolute directory",
            ),
            (
                ResolveError::EmptyPath {
                    variable: "CODEX_HOME",
                },
                "CODEX_HOME cannot be empty",
            ),
            (
                ResolveError::NonUtf8Path {
                    variable: "HOME",
                    path: PathDiagnostic {
                        path_utf8: None,
                        path_bytes_hex: Some("ff".to_owned()),
                    },
                },
                "HOME is not valid UTF-8 (path bytes: ff)",
            ),
            (
                ResolveError::NotAbsolute {
                    variable: "CODEX_HOME",
                    path: utf8_path("relative"),
                },
                "CODEX_HOME must be absolute: relative",
            ),
            (
                ResolveError::EscapesFilesystemRoot {
                    variable: "HOME",
                    path: utf8_path("/../../home"),
                },
                "HOME escapes the filesystem root: /../../home",
            ),
            (
                ResolveError::NotDirectory {
                    variable: "HOME",
                    path: utf8_path("/tmp/home-file"),
                },
                "HOME must resolve to a directory: /tmp/home-file",
            ),
            (
                ResolveError::Inaccessible {
                    variable: "HOME",
                    path: utf8_path("/tmp/home"),
                    detail: "permission denied".to_owned(),
                },
                "HOME is not accessible at /tmp/home: permission denied",
            ),
        ];

        for (error, expected) in errors {
            assert_eq!(error.to_string(), expected);
            assert_eq!(error.exit_code(), ENVIRONMENT_EXIT_CODE);
        }
        assert_eq!(
            PathDiagnostic {
                path_utf8: None,
                path_bytes_hex: None,
            }
            .display(),
            "<missing path>"
        );
    }

    #[test]
    fn environment_wrapper_reads_current_process_paths() {
        assert!(resolve_roots(&[]).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_home_preserves_lexical_and_real_identities() {
        use std::os::unix::fs::symlink;

        let real_home = must_succeed(tempdir(), "temporary HOME failed");
        let parent = must_succeed(tempdir(), "temporary parent failed");
        let lexical_home = parent.path().join("home-link");
        must_succeed(
            symlink(real_home.path(), &lexical_home),
            "symlink fixture failed",
        );

        let roots = must_succeed(
            resolve_roots_from(Some(lexical_home.as_os_str()), None, &[ProviderId::Claude]),
            "root resolution failed",
        );
        let canonical_home = must_succeed(
            fs::canonicalize(real_home.path()),
            "canonicalization failed",
        );

        assert_eq!(roots.home.lexical, lexical_home);
        assert_eq!(roots.home.real, canonical_home);
        assert_eq!(roots.canonical.real, canonical_home.join(".agents"));
        let expected_claude_real = canonical_home.join(".claude");
        assert_eq!(
            roots
                .provider(ProviderId::Claude)
                .map(|provider| &provider.root.real),
            Some(&expected_claude_real)
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_home_symlink_is_inaccessible() {
        use std::os::unix::fs::symlink;

        let parent = must_succeed(tempdir(), "temporary parent failed");
        let dangling = parent.path().join("home-link");
        must_succeed(
            symlink(parent.path().join("missing"), &dangling),
            "symlink fixture failed",
        );

        assert!(matches!(
            resolve_roots_from(Some(dangling.as_os_str()), None, &[]),
            Err(ResolveError::Inaccessible {
                variable: "HOME",
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_environment_path_is_rejected_losslessly() {
        use std::os::unix::ffi::OsStrExt;

        let result = resolve_roots_from(
            Some(OsStr::from_bytes(&[b'/', b't', b'm', b'p', 0xff])),
            None,
            &[],
        );
        assert_eq!(
            must_fail(result, "non-UTF-8 HOME was accepted"),
            ResolveError::NonUtf8Path {
                variable: "HOME",
                path: PathDiagnostic {
                    path_utf8: None,
                    path_bytes_hex: Some("2f746d70ff".to_owned()),
                },
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_real_path_is_rejected_even_when_lexical_path_is_utf8() {
        use std::os::unix::ffi::OsStringExt;
        use std::os::unix::fs::symlink;

        let parent = must_succeed(tempdir(), "temporary parent failed");
        let non_utf8 = parent.path().join(std::ffi::OsString::from_vec(vec![
            b'h', b'o', b'm', b'e', 0xff,
        ]));
        must_succeed(fs::create_dir(&non_utf8), "fixture failed");
        let lexical = parent.path().join("home-link");
        must_succeed(symlink(&non_utf8, &lexical), "symlink fixture failed");

        assert!(matches!(
            resolve_roots_from(Some(lexical.as_os_str()), None, &[]),
            Err(ResolveError::NonUtf8Path { .. })
        ));
    }
}
