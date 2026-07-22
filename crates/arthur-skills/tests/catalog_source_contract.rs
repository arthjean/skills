#![cfg(all(unix, not(coverage)))]
#![forbid(unsafe_code)]

#[path = "../build_support/mod.rs"]
mod build_support;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;

use tempfile::TempDir;

#[test]
fn repository_generation_is_byte_deterministic() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    let first = build_support::generate(&root).map_err(std::io::Error::other)?;
    let second = build_support::generate(&root).map_err(std::io::Error::other)?;
    assert_eq!(
        first.manifest_json.as_bytes(),
        second.manifest_json.as_bytes()
    );
    assert_eq!(
        first.embedded_source.as_bytes(),
        second.embedded_source.as_bytes()
    );
    assert_eq!(first.source_paths, second.source_paths);
    assert!(!first.manifest_json.contains("agents/codex/evals"));
    assert!(!first.embedded_source.contains("agents/codex/evals"));
    Ok(())
}

#[test]
fn malicious_source_shapes_fail_before_embedding() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let linked = fixture.root().join("skills/alpha/linked.md");
    std::os::unix::fs::symlink(fixture.root().join("skills/alpha/SKILL.md"), &linked)?;
    let result = build_support::generate(fixture.root());
    assert!(result.is_err_and(|error| error.contains("source symlinks are forbidden")));

    let fixture = Fixture::new()?;
    fs::write(
        fixture.root().join("skills/alpha/local.md"),
        "machine path: /home/alice/private",
    )?;
    let result = build_support::generate(fixture.root());
    assert!(result.is_err_and(|error| error.contains("machine-bound path marker")));

    let fixture = Fixture::new()?;
    fs::write(
        fixture.root().join("agents/codex/wrong.md"),
        "wrong provider",
    )?;
    let result = build_support::generate(fixture.root());
    assert!(result.is_err_and(|error| error.contains("format does not match")));

    let root = fixture.root();
    let traversing = root.join("skills/../outside");
    let result = build_support::validate_relative_for_test(root, &traversing);
    assert!(result.is_err_and(|error| error.contains("traversal")));

    let fixture = Fixture::new()?;
    let bad_mode = fixture.root().join("skills/alpha/private.md");
    fs::write(&bad_mode, "private")?;
    fs::set_permissions(&bad_mode, fs::Permissions::from_mode(0o600))?;
    let result = build_support::generate(fixture.root());
    assert!(result.is_err_and(|error| error.contains("unsupported source mode")));

    let fixture = Fixture::new()?;
    let non_utf8 = PathBuf::from(std::ffi::OsString::from_vec(vec![0xff, b'.', b'm', b'd']));
    fs::write(
        fixture.root().join("skills/alpha").join(non_utf8),
        "invalid name",
    )?;
    let result = build_support::generate(fixture.root());
    assert!(result.is_err_and(|error| error.contains("non-UTF-8 path rejected")));

    let fixture = Fixture::new()?;
    let binary = fixture.root().join("skills/alpha/binary.dat");
    let mut bytes = vec![0xff, 0x00];
    bytes.extend_from_slice(b"/home/alice/private");
    fs::write(binary, bytes)?;
    let result = build_support::generate(fixture.root());
    assert!(result.is_err_and(|error| error.contains("machine-bound path marker")));

    let fixture = Fixture::new()?;
    fs::remove_file(
        fixture
            .root()
            .join("shared/claude/skills/_shared/scope-guard.md"),
    )?;
    let result = build_support::generate(fixture.root());
    assert!(result.is_err_and(|error| error.contains("differ from inventory")));
    Ok(())
}

#[test]
fn provider_contract_rejects_unknown_models() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let agent = fixture.root().join("agents/codex/codex-agent.toml");
    let content = fs::read_to_string(&agent)?.replace("gpt-5.6-sol", "unknown-model");
    fs::write(agent, content)?;
    let result = build_support::generate(fixture.root());
    assert!(result.is_err_and(|error| error.contains("unsupported model")));
    Ok(())
}

struct Fixture {
    directory: TempDir,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        for path in [
            "skills/alpha",
            "agents/claude",
            "agents/codex/evals",
            "shared/claude/skills/_shared",
            "crates/arthur-skills/schemas",
        ] {
            fs::create_dir_all(root.join(path))?;
        }
        fs::write(
            root.join("skills/alpha/SKILL.md"),
            "---\nname: alpha\ndescription: fixture\n---\n\n# Alpha\n",
        )?;
        fs::write(
            root.join("agents/claude/claude-agent.md"),
            "---\nname: claude-agent\ndescription: Fixture agent\ntools: Read, Grep\npermissionMode: dontAsk\nmaxTurns: 2\nmodel: \"claude-fable-5[1m]\"\neffort: low\ncolor: green\n---\n\nFixture instructions.\n",
        )?;
        fs::write(
            root.join("agents/codex/codex-agent.toml"),
            concat!(
                "name = \"codex-agent\"\n",
                "description = \"Fixture agent\"\n",
                "model = \"gpt-5.6-sol\"\n",
                "model_reasoning_effort = \"low\"\n",
                "default_permissions = \"codex-agent\"\n",
                "web_search = \"disabled\"\n",
                "developer_instructions = \"Fixture instructions.\"\n\n",
                "[mcp_servers.paneflow]\n",
                "enabled = false\n",
                "command = \"paneflow-mcp\"\n\n",
                "[permissions.codex-agent.filesystem]\n",
                "\":minimal\" = \"read\"\n",
            ),
        )?;
        for name in [
            "agent-boundaries.md",
            "scope-guard.md",
            "synthesis-template.md",
            "three-tier-constraints.md",
        ] {
            fs::write(
                root.join("shared/claude/skills/_shared").join(name),
                "# Support\n",
            )?;
        }
        let schema_source = repository_root()?.join("crates/arthur-skills/schemas");
        for name in [
            "claude-agent-2.1.217.schema.json",
            "codex-agent-0.144.6.schema.json",
        ] {
            fs::copy(
                schema_source.join(name),
                root.join("crates/arthur-skills/schemas").join(name),
            )?;
        }
        fs::write(
            root.join("crates/arthur-skills/schemas/catalog-v1.inventory.json"),
            concat!(
                "{\n",
                "  \"schema_version\": 1,\n",
                "  \"skills\": [\"alpha\"],\n",
                "  \"claude_agents\": [\"claude-agent\"],\n",
                "  \"codex_agents\": [\"codex-agent\"],\n",
                "  \"claude_support\": [\"agent-boundaries\", \"scope-guard\", ",
                "\"synthesis-template\", \"three-tier-constraints\"]\n",
                "}\n",
            ),
        )?;
        Ok(Self { directory })
    }

    fn root(&self) -> &Path {
        self.directory.path()
    }
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_root
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "crate root is not nested under the repository".into())
}
