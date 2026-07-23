use std::fs;
use std::path::{Path, PathBuf};
#[cfg(not(coverage))]
use std::process::Command;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn string_array<'a>(table: &'a toml::Table, key: &str) -> Vec<&'a str> {
    let Some(values) = table[key].as_array() else {
        panic!("contract value {key} must be an array");
    };
    values
        .iter()
        .map(|value| {
            let Some(value) = value.as_str() else {
                panic!("contract entries for {key} must be strings");
            };
            value
        })
        .collect()
}

#[test]
fn cargo_dist_configuration_covers_native_artifacts_and_provenance() -> TestResult {
    let source = fs::read_to_string(workspace().join("dist-workspace.toml"))?;
    let document = source.parse::<toml::Table>()?;
    assert_eq!(
        string_array(
            document["workspace"]
                .as_table()
                .ok_or("dist-workspace.toml must contain [workspace]")?,
            "members"
        ),
        ["cargo:."]
    );
    let Some(dist) = document["dist"].as_table() else {
        panic!("dist-workspace.toml must contain [dist]");
    };

    assert_eq!(dist["cargo-dist-version"].as_str(), Some("0.32.0"));
    assert_eq!(string_array(dist, "allow-dirty"), ["ci"]);
    assert_eq!(dist["checksum"].as_str(), Some("sha256"));
    assert_eq!(string_array(dist, "installers"), ["shell", "powershell"]);
    assert_eq!(
        string_array(dist, "include"),
        ["LICENSE", "README.md", "THIRD_PARTY.md"]
    );
    assert_eq!(
        string_array(dist, "targets"),
        [
            "x86_64-unknown-linux-musl",
            "aarch64-unknown-linux-musl",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ]
    );
    assert!(dist.get("source-tarball").is_none());
    assert_eq!(dist["github-attestations"].as_bool(), Some(true));
    let filters = string_array(dist, "github-attestations-filters");
    for required in ["*.json", "*.ps1", "*.sh", "*.tar.xz", "*.zip", "*.sum"] {
        assert!(
            filters.contains(&required),
            "missing attestation filter {required}"
        );
    }

    Ok(())
}

#[test]
fn workflow_gates_every_native_archive_before_draft_publication() -> TestResult {
    let release = fs::read_to_string(workspace().join(".github/workflows/release.yml"))?;

    for contract in [
        "workflow_dispatch:",
        "group: release-${{ inputs.tag || github.ref_name }}",
        "release_tag: ${{ steps.mode.outputs.release_tag }}",
        "Release tag mismatch",
        "RUSTFLAGS: --remap-path-prefix=${{ github.workspace }}=.",
        "dist print-upload-files-from-manifest --manifest dist-manifest.json",
        "archives=(target/distrib/*-\"$TARGET\".tar.xz)",
        "$archives = @(Get-ChildItem \"target/distrib/*-$env:TARGET.zip\")",
        "for required in arthur-skills LICENSE README.md THIRD_PARTY.md",
        "arthur-skills.exe",
        "! -type f",
        "cd \"$smoke_work\"",
        "PATH=\"$empty_path\"",
        "\"$binary\" plan --json --provider codex",
        "& $binary plan --json --provider codex",
        "data.catalog_sha256",
        "vtool -show-build",
        "otool -L",
        "Dynamic musl binary",
        "trap cleanup_smoke EXIT",
        "test -s target/distrib/sha256.sum",
        "sha256sum --check --strict sha256.sum",
        "*pc-windows-msvc.zip",
        "*installer.ps1",
        "rm -f target/distrib/*-dist-manifest.json",
        "ATTESTATION_ID: ${{ steps.attest.outputs.attestation-id }}",
        "test -s \"$ATTESTATION_BUNDLE\"",
        "for artifact in target/distrib/*",
        "gh attestation verify \"$artifact\"",
        "--bundle \"$ATTESTATION_BUNDLE\"",
        "--signer-workflow \"$GITHUB_REPOSITORY/.github/workflows/release.yml\"",
        "Missing GitHub token",
        "if: needs.plan.outputs.publishing == 'true'",
        "dispatch releases from ${DEFAULT_BRANCH}",
        "git/refs/tags/${RELEASE_TAG}",
        "-f ref=\"refs/tags/${RELEASE_TAG}\"",
        "Tag cleanup refused",
        "gh release create \"$RELEASE_TAG\" --verify-tag --draft",
        "gh release view \"$RELEASE_TAG\" --json isDraft",
        "gh release delete \"$RELEASE_TAG\" --yes",
    ] {
        assert!(
            release.contains(contract),
            "missing release gate: {contract}"
        );
    }

    let Some(build_global) = release.find("--artifacts=global > dist-manifest.json") else {
        panic!("global manifest must be written outside target/distrib");
    };
    let Some(publish_manifest) =
        release.find("cp dist-manifest.json target/distrib/dist-manifest.json")
    else {
        panic!("external manifest must be copied only after cargo-dist succeeds");
    };
    assert!(build_global < publish_manifest);
    assert!(!release.contains("> target/distrib/dist-manifest.json"));

    let ordered_gates = [
        "sha256sum --check --strict sha256.sum",
        "- name: Attest every release artifact",
        "- name: Require the attestation created by this gate",
        "- name: Verify provenance before publication",
        "- name: Create the tag and publish atomically through a draft release",
    ];
    let mut previous = 0;
    for gate in ordered_gates {
        let Some(position) = release.find(gate) else {
            panic!("missing ordered release gate {gate}");
        };
        assert!(position >= previous, "release gate is out of order: {gate}");
        previous = position;
    }
    let tag_creation = release
        .find("-f ref=\"refs/tags/${RELEASE_TAG}\"")
        .ok_or("release tag creation is missing")?;
    let provenance = release
        .find("- name: Verify provenance before publication")
        .ok_or("provenance gate is missing")?;
    assert!(tag_creation > provenance);
    assert!(!release.contains("push:\n    tags:"));

    for target in [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ] {
        assert!(
            release.contains(&format!("- target: {target}")),
            "missing native build lane {target}"
        );
    }

    Ok(())
}

#[cfg(not(coverage))]
#[test]
fn compiled_corrupt_catalog_fails_before_user_state_is_scanned() -> TestResult {
    let target = tempfile::tempdir()?;
    let home = tempfile::tempdir()?;
    let build = Command::new("cargo")
        .current_dir(workspace())
        .args([
            "build",
            "--locked",
            "-p",
            "arthur-skills",
            "--bin",
            "arthur-skills",
        ])
        .env("ARTHUR_SKILLS_TEST_CORRUPT_CATALOG", "1")
        .env("CARGO_TARGET_DIR", target.path())
        .output()?;
    assert!(
        build.status.success(),
        "corrupt fixture did not compile: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let mut binary = target.path().join("debug/arthur-skills");
    if cfg!(windows) {
        binary.set_extension("exe");
    }
    let output = Command::new(binary)
        .args(["--json", "install", "--provider", "codex", "--yes"])
        .env_clear()
        .env("HOME", home.path())
        .env("PATH", "")
        .env("TERM", "dumb")
        .output()?;
    assert_eq!(output.status.code(), Some(5));
    let envelope = serde_json::from_slice::<serde_json::Value>(&output.stdout)?;
    assert_eq!(envelope["diagnostics"][0]["code"], "catalog_invalid");
    assert!(!home.path().join(".agents").exists());
    assert!(!home.path().join(".codex").exists());
    Ok(())
}
