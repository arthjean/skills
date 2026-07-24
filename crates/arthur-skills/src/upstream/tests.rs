use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;

use sha2::{Digest, Sha256};
use tempfile::{Builder, TempDir, tempdir};

use super::git::{
    FetchedSkill, FetchedSource, GitFetcher, SourceFetcher, concise_stderr, git_capture,
    map_git_start, optional_metadata, validate_git_hash,
};
use super::model::{
    SkillLock, SkillManifest, SkillState, SourceLock, SourceManifest, UpstreamError, UpstreamLock,
    UpstreamManifest, content_sha256, load_configuration, validate_configuration,
};
use super::{execute_at, inspect_at, sync};
use crate::cli::{UpstreamArgs, UpstreamCommand, UpstreamSyncArgs};
use crate::output::OutputStatus;

struct FixtureFetcher {
    content: &'static [u8],
    revision: String,
    tree_sha1: String,
    available: bool,
}

impl SourceFetcher for FixtureFetcher {
    fn fetch(
        &self,
        source: &SourceManifest,
        destination: &Path,
    ) -> Result<FetchedSource, UpstreamError> {
        let mut skills = BTreeMap::new();
        for skill in &source.skill {
            if !self.available {
                skills.insert(
                    skill.name.clone(),
                    FetchedSkill {
                        directory: None,
                        tree_sha1: None,
                    },
                );
                continue;
            }
            let directory = destination.join(&skill.path);
            fs::create_dir_all(&directory)
                .map_err(|error| UpstreamError::fetch(format!("cannot create fixture: {error}")))?;
            fs::write(directory.join("SKILL.md"), self.content)
                .map_err(|error| UpstreamError::fetch(format!("cannot write fixture: {error}")))?;
            skills.insert(
                skill.name.clone(),
                FetchedSkill {
                    directory: Some(directory),
                    tree_sha1: Some(self.tree_sha1.clone()),
                },
            );
        }
        Ok(FetchedSource {
            revision: self.revision.clone(),
            skills,
        })
    }
}

#[test]
fn content_hash_is_path_sensitive_and_deterministic() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    fs::create_dir(directory.path().join("references"))?;
    fs::write(directory.path().join("SKILL.md"), b"skill")?;
    fs::write(directory.path().join("references/guide.md"), b"guide")?;

    let mut expected = Sha256::new();
    expected.update(b"arthur-skills.snapshot.v2\0");
    expected.update([1]);
    expected.update(test_mode(directory.path())?.to_le_bytes());
    expected.update(0_u64.to_le_bytes());
    expected.update(0_u64.to_le_bytes());
    expected.update([2]);
    expected.update(test_mode(&directory.path().join("SKILL.md"))?.to_le_bytes());
    expected.update(8_u64.to_le_bytes());
    expected.update(b"SKILL.md");
    expected.update(5_u64.to_le_bytes());
    expected.update(b"skill");
    expected.update([1]);
    expected.update(test_mode(&directory.path().join("references"))?.to_le_bytes());
    expected.update(10_u64.to_le_bytes());
    expected.update(b"references");
    expected.update(0_u64.to_le_bytes());
    expected.update([2]);
    expected.update(test_mode(&directory.path().join("references/guide.md"))?.to_le_bytes());
    expected.update(19_u64.to_le_bytes());
    expected.update(b"references/guide.md");
    expected.update(5_u64.to_le_bytes());
    expected.update(b"guide");

    assert_eq!(
        content_sha256(directory.path())?,
        format!("{:x}", expected.finalize())
    );
    assert_eq!(
        content_sha256(directory.path())?,
        content_sha256(directory.path())?
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let before = content_sha256(directory.path())?;
        fs::set_permissions(
            directory.path().join("SKILL.md"),
            fs::Permissions::from_mode(0o755),
        )?;
        assert_ne!(content_sha256(directory.path())?, before);
    }
    Ok(())
}

#[test]
fn content_hash_rejects_symlinks() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    fs::write(directory.path().join("SKILL.md"), b"skill")?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("SKILL.md", directory.path().join("linked.md"))?;
        assert!(content_sha256(directory.path()).is_err());
    }
    Ok(())
}

#[test]
fn content_hash_rejects_invalid_roots_and_entries() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = tempdir()?;
    let file = workspace.path().join("file");
    fs::write(&file, b"not a directory")?;
    assert!(content_sha256(&file).is_err());

    let missing_skill = workspace.path().join("missing-skill");
    fs::create_dir(&missing_skill)?;
    assert!(content_sha256(&missing_skill).is_err());

    let invalid_skill = workspace.path().join("invalid-skill");
    fs::create_dir(&invalid_skill)?;
    fs::create_dir(invalid_skill.join("SKILL.md"))?;
    assert!(content_sha256(&invalid_skill).is_err());

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;

        let linked_skill = workspace.path().join("linked-skill");
        fs::create_dir(&linked_skill)?;
        fs::write(linked_skill.join("target"), b"skill")?;
        std::os::unix::fs::symlink("target", linked_skill.join("SKILL.md"))?;
        assert!(content_sha256(&linked_skill).is_err());

        let non_utf8 = workspace.path().join("non-utf8");
        fs::create_dir(&non_utf8)?;
        fs::write(non_utf8.join("SKILL.md"), b"skill")?;
        fs::write(
            non_utf8.join(std::ffi::OsString::from_vec(vec![b'x', 0xff])),
            b"value",
        )?;
        assert!(content_sha256(&non_utf8).is_err());

        let unsupported = workspace.path().join("unsupported");
        fs::create_dir(&unsupported)?;
        fs::write(unsupported.join("SKILL.md"), b"skill")?;
        let status = ProcessCommand::new("mkfifo")
            .arg(unsupported.join("pipe"))
            .status()?;
        assert!(status.success());
        assert!(content_sha256(&unsupported).is_err());
    }
    Ok(())
}

#[test]
fn synchronization_updates_clean_snapshots_and_then_detects_drift()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = fixture_repository()?;
    let fetcher = fixture_fetcher(true);

    let inspection = inspect_at(repository.path(), &fetcher)?;
    assert_eq!(inspection.reports[0].state, SkillState::UpdateAvailable);
    let applied = sync::apply(&inspection)?;
    assert_eq!(applied, ["alpha"]);
    assert_eq!(
        fs::read(repository.path().join("skills/alpha/SKILL.md"))?,
        b"new"
    );

    fs::write(
        repository.path().join("skills/alpha/SKILL.md"),
        b"local edit",
    )?;
    let inspection = inspect_at(repository.path(), &fetcher)?;
    assert_eq!(inspection.reports[0].state, SkillState::Drifted);
    Ok(())
}

#[test]
fn synchronization_revalidates_local_content_before_writing()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = fixture_repository()?;
    let fetcher = fixture_fetcher(true);
    let inspection = inspect_at(repository.path(), &fetcher)?;
    fs::write(
        repository.path().join("skills/alpha/SKILL.md"),
        b"concurrent local edit",
    )?;
    assert!(sync::apply(&inspection).is_err());
    assert_eq!(
        fs::read(repository.path().join("skills/alpha/SKILL.md"))?,
        b"concurrent local edit"
    );
    Ok(())
}

#[test]
fn commit_moves_only_snapshots_that_still_match() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = tempdir()?;
    let skill = workspace.path().join("skill");
    let skill_backup = workspace.path().join("skill-backup");
    fs::create_dir(&skill)?;
    fs::write(skill.join("SKILL.md"), b"local edit")?;
    assert!(sync::move_directory_if_unchanged(&skill, &skill_backup, &"0".repeat(64)).is_err());
    assert_eq!(fs::read(skill.join("SKILL.md"))?, b"local edit");
    assert!(!skill_backup.exists());

    let lock = workspace.path().join("upstreams.lock.json");
    let lock_backup = workspace.path().join("lock-backup.json");
    fs::write(&lock, b"local edit")?;
    assert!(sync::move_file_if_unchanged(&lock, &lock_backup, &"0".repeat(64)).is_err());
    assert_eq!(fs::read(&lock)?, b"local edit");
    assert!(!lock_backup.exists());

    let matching_skill = workspace.path().join("matching-skill");
    let matching_backup = workspace.path().join("matching-backup");
    fs::create_dir(&matching_skill)?;
    fs::write(matching_skill.join("SKILL.md"), b"matching")?;
    let matching_hash = content_sha256(&matching_skill)?;
    assert!(
        sync::move_directory_if_unchanged(&matching_skill, &matching_backup, &matching_hash)
            .is_ok()
    );
    assert!(!matching_skill.exists());
    assert_eq!(fs::read(matching_backup.join("SKILL.md"))?, b"matching");

    assert!(
        sync::move_directory_if_unchanged(
            &workspace.path().join("absent"),
            &skill_backup,
            &"0".repeat(64),
        )
        .is_err()
    );
    let invalid_skill = workspace.path().join("invalid-skill");
    fs::create_dir(&invalid_skill)?;
    assert!(
        sync::move_directory_if_unchanged(
            &invalid_skill,
            &workspace.path().join("invalid-backup"),
            &"0".repeat(64),
        )
        .is_err()
    );
    let lock_hash = super::model::hash_bytes(b"current");
    fs::write(&lock, b"current")?;
    assert!(sync::move_file_if_unchanged(&lock, &lock_backup, &lock_hash).is_ok());
    assert!(!lock.exists());
    assert_eq!(fs::read(&lock_backup)?, b"current");
    assert!(
        sync::move_file_if_unchanged(
            &workspace.path().join("missing-lock"),
            &workspace.path().join("missing-lock-backup"),
            &lock_hash,
        )
        .is_err()
    );
    let directory_lock = workspace.path().join("directory-lock");
    fs::create_dir(&directory_lock)?;
    assert!(
        sync::move_file_if_unchanged(
            &directory_lock,
            &workspace.path().join("directory-lock-backup"),
            &lock_hash,
        )
        .is_err()
    );

    let restore_source = workspace.path().join("restore-source");
    fs::write(&restore_source, b"occupied")?;
    let failure = sync::restore_moved_backup(
        &workspace.path().join("missing-restore-backup"),
        &restore_source,
        "changed",
    );
    assert!(failure.preserve);

    let install_backup = workspace.path().join("install-backup");
    let install_destination = workspace.path().join("install-destination");
    fs::write(&install_backup, b"backup")?;
    let failure = sync::restore_after_install_failure(
        &install_backup,
        &install_destination,
        std::io::Error::other("install"),
        "snapshot",
    );
    assert!(!failure.preserve);
    assert_eq!(fs::read(&install_destination)?, b"backup");
    let failure = sync::restore_after_install_failure(
        &workspace.path().join("missing-install-backup"),
        &install_destination,
        std::io::Error::other("install"),
        "snapshot",
    );
    assert!(failure.preserve);
    Ok(())
}

#[test]
fn synchronization_rejects_configuration_and_staging_races()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = fixture_repository()?;
    let fetcher = fixture_fetcher(true);
    let inspection = inspect_at(repository.path(), &fetcher)?;
    fs::write(
        repository.path().join("upstreams.toml"),
        b"concurrent manifest edit",
    )?;
    assert!(sync::apply(&inspection).is_err());

    let repository = fixture_repository()?;
    let inspection = inspect_at(repository.path(), &fetcher)?;
    fs::remove_file(repository.path().join("upstreams.lock.json"))?;
    assert!(sync::apply(&inspection).is_err());

    let repository = fixture_repository()?;
    let inspection = inspect_at(repository.path(), &fetcher)?;
    fs::write(
        inspection.updates[0].source_directory.join("SKILL.md"),
        b"changed after fetch",
    )?;
    assert!(sync::apply(&inspection).is_err());
    assert_eq!(
        fs::read(repository.path().join("skills/alpha/SKILL.md"))?,
        b"old"
    );

    let repository = fixture_repository()?;
    let inspection = inspect_at(repository.path(), &fetcher)?;
    fs::remove_dir_all(&inspection.updates[0].source_directory)?;
    assert!(sync::apply(&inspection).is_err());

    let repository = fixture_repository()?;
    let inspection = inspect_at(repository.path(), &fetcher)?;
    fs::remove_file(repository.path().join("skills/alpha/SKILL.md"))?;
    assert!(sync::apply(&inspection).is_err());
    Ok(())
}

#[test]
fn commit_rolls_back_skill_and_lock_boundary_failures() -> Result<(), Box<dyn std::error::Error>> {
    let fetcher = fixture_fetcher(true);

    let repository = fixture_repository()?;
    let inspection = inspect_at(repository.path(), &fetcher)?;
    let staging = prepared_staging(&inspection, false, true)?;
    assert!(sync::commit_staging(staging, &inspection).is_err());
    assert_eq!(
        fs::read(repository.path().join("skills/alpha/SKILL.md"))?,
        b"old"
    );

    let repository = fixture_repository()?;
    let inspection = inspect_at(repository.path(), &fetcher)?;
    let staging = prepared_staging(&inspection, true, true)?;
    fs::write(
        repository.path().join("skills/alpha/SKILL.md"),
        b"concurrent skill edit",
    )?;
    assert!(sync::commit_staging(staging, &inspection).is_err());
    assert_eq!(
        fs::read(repository.path().join("skills/alpha/SKILL.md"))?,
        b"concurrent skill edit"
    );

    let repository = fixture_repository()?;
    let inspection = inspect_at(repository.path(), &fetcher)?;
    let staging = prepared_staging(&inspection, true, true)?;
    fs::write(
        repository.path().join("upstreams.lock.json"),
        b"concurrent lock edit",
    )?;
    assert!(sync::commit_staging(staging, &inspection).is_err());
    assert_eq!(
        fs::read(repository.path().join("skills/alpha/SKILL.md"))?,
        b"old"
    );
    assert_eq!(
        fs::read(repository.path().join("upstreams.lock.json"))?,
        b"concurrent lock edit"
    );

    let repository = fixture_repository()?;
    let inspection = inspect_at(repository.path(), &fetcher)?;
    let staging = prepared_staging(&inspection, true, false)?;
    assert!(sync::commit_staging(staging, &inspection).is_err());
    assert_eq!(
        fs::read(repository.path().join("skills/alpha/SKILL.md"))?,
        b"old"
    );
    assert!(repository.path().join("upstreams.lock.json").is_file());
    Ok(())
}

#[test]
fn rollback_and_copy_helpers_cover_nested_and_invalid_trees()
-> Result<(), Box<dyn std::error::Error>> {
    let error = sync::io_failure(
        Path::new("/fixture"),
        "cannot inspect",
        std::io::Error::other("denied"),
    );
    assert!(error.message.contains("/fixture: cannot inspect: denied"));
    assert!(
        sync::staging_creation_failure(std::io::Error::other("denied"))
            .message
            .contains("cannot create repository staging directory")
    );
    assert!(
        sync::staging_initialization_failure(std::io::Error::other("denied"))
            .message
            .contains("cannot initialize repository staging directory")
    );

    let source = tempdir()?;
    fs::create_dir_all(source.path().join("nested/empty"))?;
    fs::write(source.path().join("SKILL.md"), b"skill")?;
    fs::write(source.path().join("nested/file"), b"value")?;
    let workspace = tempdir()?;
    let destination = workspace.path().join("copy");
    sync::copy_directory(source.path(), &destination)?;
    assert_eq!(
        content_sha256(source.path())?,
        content_sha256(&destination)?
    );
    assert!(sync::copy_directory(source.path(), &destination).is_err());
    assert!(
        sync::copy_directory(
            &workspace.path().join("absent"),
            &workspace.path().join("missing"),
        )
        .is_err()
    );
    let regular_file = workspace.path().join("regular");
    fs::write(&regular_file, b"value")?;
    assert!(sync::copy_directory(&regular_file, &workspace.path().join("file-copy")).is_err());

    #[cfg(unix)]
    {
        let symlink_source = workspace.path().join("source-link");
        std::os::unix::fs::symlink(source.path(), &symlink_source)?;
        assert!(
            sync::copy_directory(&symlink_source, &workspace.path().join("link-copy")).is_err()
        );

        let linked_child = tempdir()?;
        fs::write(linked_child.path().join("SKILL.md"), b"skill")?;
        std::os::unix::fs::symlink("SKILL.md", linked_child.path().join("linked"))?;
        assert!(
            sync::copy_directory(
                linked_child.path(),
                &workspace.path().join("linked-child-copy"),
            )
            .is_err()
        );

        let unsupported = tempdir()?;
        fs::write(unsupported.path().join("SKILL.md"), b"skill")?;
        let status = ProcessCommand::new("mkfifo")
            .arg(unsupported.path().join("pipe"))
            .status()?;
        assert!(status.success());
        assert!(
            sync::copy_directory(
                unsupported.path(),
                &workspace.path().join("unsupported-copy"),
            )
            .is_err()
        );
    }

    let repository = fixture_repository()?;
    let inspection = inspect_at(repository.path(), &fixture_fetcher(true))?;
    let staging = Builder::new()
        .prefix(".rollback-test-")
        .tempdir_in(repository.path())?;
    fs::create_dir_all(staging.path().join("backups/alpha"))?;
    fs::create_dir(staging.path().join("failed"))?;
    fs::write(staging.path().join("backups/alpha/SKILL.md"), b"old")?;
    fs::write(repository.path().join("skills/alpha/SKILL.md"), b"new")?;
    sync::rollback_applied(staging.path(), &inspection, &["alpha".to_owned()])?;
    assert_eq!(
        fs::read(repository.path().join("skills/alpha/SKILL.md"))?,
        b"old"
    );
    assert!(sync::rollback_applied(staging.path(), &inspection, &["missing".to_owned()]).is_err());
    assert!(sync::rollback_applied(staging.path(), &inspection, &["alpha".to_owned()]).is_err());

    let mut missing_source = inspection.configuration.lock.clone();
    missing_source.sources.clear();
    assert!(sync::apply_lock_updates(&mut missing_source, &inspection.updates).is_err());
    let mut missing_skill = inspection.configuration.lock.clone();
    missing_skill.sources[0].skills.clear();
    assert!(sync::apply_lock_updates(&mut missing_skill, &inspection.updates).is_err());

    let preserved = Builder::new()
        .prefix(".preserve-test-")
        .tempdir_in(repository.path())?;
    let preserved_path = preserved.path().to_path_buf();
    let error =
        sync::rollback_or_preserve(preserved, &inspection, &[], "preserve".to_owned(), true);
    assert!(error.message.contains("backups preserved"));
    fs::remove_dir_all(&preserved_path)?;

    let failed_rollback = Builder::new()
        .prefix(".rollback-failure-test-")
        .tempdir_in(repository.path())?;
    let failed_path = failed_rollback.path().to_path_buf();
    let error = sync::rollback_or_preserve(
        failed_rollback,
        &inspection,
        &["missing".to_owned()],
        "failure".to_owned(),
        false,
    );
    assert!(error.message.contains("rollback failed"));
    fs::remove_dir_all(&failed_path)?;
    Ok(())
}

#[test]
fn command_contract_covers_check_dry_run_sync_noop_and_blockers()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = fixture_repository()?;
    let fetcher = fixture_fetcher(true);
    let check = UpstreamArgs {
        command: UpstreamCommand::Check,
    };
    let envelope = execute_at(repository.path(), &check, &fetcher);
    assert_eq!(envelope.status, OutputStatus::Success);
    assert_eq!(envelope.summary["update"], 1);

    let unconfirmed = UpstreamArgs {
        command: UpstreamCommand::Sync(UpstreamSyncArgs::default()),
    };
    assert_eq!(super::execute(&unconfirmed).status, OutputStatus::Failed);
    assert_eq!(
        execute_at(repository.path(), &unconfirmed, &fetcher).status,
        OutputStatus::Failed
    );

    let dry_run = UpstreamArgs {
        command: UpstreamCommand::Sync(UpstreamSyncArgs {
            yes: false,
            dry_run: true,
        }),
    };
    assert_eq!(
        execute_at(repository.path(), &dry_run, &fetcher).status,
        OutputStatus::Success
    );

    let synchronize = UpstreamArgs {
        command: UpstreamCommand::Sync(UpstreamSyncArgs {
            yes: true,
            dry_run: false,
        }),
    };
    let envelope = execute_at(repository.path(), &synchronize, &fetcher);
    assert_eq!(envelope.status, OutputStatus::Success);
    assert_eq!(envelope.data["result"], "synced");
    assert_eq!(
        execute_at(repository.path(), &check, &fetcher).status,
        OutputStatus::Noop
    );
    assert_eq!(
        execute_at(repository.path(), &synchronize, &fetcher).status,
        OutputStatus::Noop
    );

    fs::write(
        repository.path().join("skills/alpha/SKILL.md"),
        b"local edit",
    )?;
    assert_eq!(
        execute_at(repository.path(), &synchronize, &fetcher).status,
        OutputStatus::Blocked
    );
    Ok(())
}

#[test]
fn command_contract_reports_removed_paths_and_fetch_failures()
-> Result<(), Box<dyn std::error::Error>> {
    struct FailingFetcher;
    impl SourceFetcher for FailingFetcher {
        fn fetch(
            &self,
            _source: &SourceManifest,
            _destination: &Path,
        ) -> Result<FetchedSource, UpstreamError> {
            Err(UpstreamError::fetch("fixture fetch failed"))
        }
    }

    let repository = fixture_repository()?;
    let check = UpstreamArgs {
        command: UpstreamCommand::Check,
    };
    let envelope = execute_at(repository.path(), &check, &fixture_fetcher(false));
    assert_eq!(envelope.status, OutputStatus::Blocked);
    assert_eq!(envelope.summary["removed"], 1);

    let envelope = execute_at(repository.path(), &check, &FailingFetcher);
    assert_eq!(envelope.status, OutputStatus::Failed);
    assert_eq!(envelope.diagnostics[0].code, "upstream_fetch_failed");
    let synchronize = UpstreamArgs {
        command: UpstreamCommand::Sync(UpstreamSyncArgs {
            yes: true,
            dry_run: false,
        }),
    };
    assert_eq!(
        execute_at(repository.path(), &synchronize, &FailingFetcher).status,
        OutputStatus::Failed
    );
    Ok(())
}

#[test]
fn command_reports_apply_races_and_fetcher_contract_violations()
-> Result<(), Box<dyn std::error::Error>> {
    struct MutatingFetcher {
        root: std::path::PathBuf,
    }
    impl SourceFetcher for MutatingFetcher {
        fn fetch(
            &self,
            source: &SourceManifest,
            destination: &Path,
        ) -> Result<FetchedSource, UpstreamError> {
            let fetched = fixture_fetcher(true).fetch(source, destination)?;
            fs::write(
                self.root.join("upstreams.toml"),
                b"changed during inspection",
            )
            .map_err(|error| UpstreamError::fetch(error.to_string()))?;
            Ok(fetched)
        }
    }

    struct OmittingFetcher;
    impl SourceFetcher for OmittingFetcher {
        fn fetch(
            &self,
            _source: &SourceManifest,
            _destination: &Path,
        ) -> Result<FetchedSource, UpstreamError> {
            Ok(FetchedSource {
                revision: "b".repeat(40),
                skills: BTreeMap::new(),
            })
        }
    }

    let repository = fixture_repository()?;
    let synchronize = UpstreamArgs {
        command: UpstreamCommand::Sync(UpstreamSyncArgs {
            yes: true,
            dry_run: false,
        }),
    };
    let envelope = execute_at(
        repository.path(),
        &synchronize,
        &MutatingFetcher {
            root: repository.path().to_path_buf(),
        },
    );
    assert_eq!(envelope.status, OutputStatus::Failed);

    let repository = fixture_repository()?;
    let check = UpstreamArgs {
        command: UpstreamCommand::Check,
    };
    assert_eq!(
        execute_at(repository.path(), &check, &OmittingFetcher).status,
        OutputStatus::Failed
    );
    Ok(())
}

#[test]
fn repository_discovery_and_update_preparation_fail_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = fixture_repository()?;
    let nested = repository.path().join("nested/deep");
    fs::create_dir_all(&nested)?;
    assert_eq!(
        super::discover_repository_root_from(&nested)?,
        repository.path()
    );
    let unrelated = tempdir()?;
    assert!(super::discover_repository_root_from(unrelated.path()).is_err());

    let (manifest, lock) = valid_configuration();
    let source = &manifest.source[0];
    let skill = &source.skill[0];
    let locked = &lock.sources[0].skills[0];
    let missing_directory = FetchedSkill {
        directory: None,
        tree_sha1: Some("c".repeat(40)),
    };
    let fetched = FetchedSource {
        revision: "b".repeat(40),
        skills: BTreeMap::new(),
    };
    assert!(super::required_source_lock(&BTreeMap::new(), "missing").is_err());
    assert!(super::required_skill_lock(&BTreeMap::new(), "missing").is_err());
    assert!(super::prepare_update(skill, source, locked, &fetched, &missing_directory).is_err());
    let workspace = tempdir()?;
    let missing_tree = FetchedSkill {
        directory: Some(workspace.path().to_path_buf()),
        tree_sha1: None,
    };
    assert!(super::prepare_update(skill, source, locked, &fetched, &missing_tree).is_err());
    let invalid_snapshot = FetchedSkill {
        directory: Some(workspace.path().to_path_buf()),
        tree_sha1: Some("c".repeat(40)),
    };
    assert!(super::prepare_update(skill, source, locked, &fetched, &invalid_snapshot).is_err());
    Ok(())
}

#[test]
fn git_fetcher_groups_present_and_missing_skills_from_one_checkout()
-> Result<(), Box<dyn std::error::Error>> {
    let upstream = tempdir()?;
    run_git(upstream.path(), &["init", "-q", "-b", "main"])?;
    run_git(upstream.path(), &["config", "user.name", "Arthur"])?;
    run_git(
        upstream.path(),
        &["config", "user.email", "arthur@example.test"],
    )?;
    fs::create_dir_all(upstream.path().join("skills/alpha"))?;
    fs::write(upstream.path().join("skills/alpha/SKILL.md"), b"fixture")?;
    fs::write(upstream.path().join("README.md"), b"readme")?;
    run_git(upstream.path(), &["add", "."])?;
    run_git(upstream.path(), &["commit", "-q", "-m", "fixture"])?;

    let checkout = tempdir()?;
    let source = SourceManifest {
        id: "fixture/repository".to_owned(),
        repository: upstream.path().to_string_lossy().into_owned(),
        track: "main".to_owned(),
        skill: vec![
            SkillManifest {
                name: "alpha".to_owned(),
                path: "skills/alpha".to_owned(),
            },
            SkillManifest {
                name: "missing".to_owned(),
                path: "skills/missing".to_owned(),
            },
        ],
    };
    let fetched = GitFetcher.fetch(&source, &checkout.path().join("checkout"))?;
    assert_eq!(fetched.revision.len(), 40);
    assert!(fetched.skills["alpha"].directory.is_some());
    assert_eq!(
        fetched.skills["alpha"].tree_sha1.as_ref().map(String::len),
        Some(40)
    );
    assert!(fetched.skills["missing"].directory.is_none());

    let invalid_path = SourceManifest {
        skill: vec![SkillManifest {
            name: "readme".to_owned(),
            path: "README.md".to_owned(),
        }],
        ..source.clone()
    };
    assert!(
        GitFetcher
            .fetch(&invalid_path, &checkout.path().join("invalid"))
            .is_err()
    );
    let unavailable = SourceManifest {
        repository: upstream
            .path()
            .join("absent")
            .to_string_lossy()
            .into_owned(),
        ..source
    };
    assert!(
        GitFetcher
            .fetch(&unavailable, &checkout.path().join("absent"))
            .is_err()
    );

    assert!(git_capture(upstream.path(), &["rev-parse", "--verify", "missing"]).is_err());
    let missing_git = map_git_start(std::io::Error::from(std::io::ErrorKind::NotFound));
    assert_eq!(missing_git.code, "upstream_configuration_invalid");
    let failed_git = map_git_start(std::io::Error::other("blocked"));
    assert_eq!(failed_git.code, "upstream_fetch_failed");
    assert_eq!(concise_stderr(&vec![b'x'; 600]).len(), 500);
    assert!(validate_git_hash(&"a".repeat(40), "valid").is_ok());
    assert!(validate_git_hash("invalid", "invalid revision").is_err());
    assert!(
        optional_metadata(
            Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
            "source",
            "path",
        )
        .is_ok()
    );
    assert!(
        optional_metadata(
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            "source",
            "path",
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn git_fetcher_rejects_invalid_skill_snapshots() -> Result<(), Box<dyn std::error::Error>> {
    let upstream = tempdir()?;
    run_git(upstream.path(), &["init", "-q", "-b", "main"])?;
    run_git(upstream.path(), &["config", "user.name", "Arthur"])?;
    run_git(
        upstream.path(),
        &["config", "user.email", "arthur@example.test"],
    )?;
    fs::create_dir_all(upstream.path().join("skills/invalid"))?;
    fs::write(upstream.path().join("skills/invalid/SKILL.md"), b"fixture")?;
    #[cfg(unix)]
    std::os::unix::fs::symlink("SKILL.md", upstream.path().join("skills/invalid/linked"))?;
    run_git(upstream.path(), &["add", "."])?;
    run_git(upstream.path(), &["commit", "-q", "-m", "fixture"])?;

    let source = SourceManifest {
        id: "fixture/repository".to_owned(),
        repository: upstream.path().to_string_lossy().into_owned(),
        track: "main".to_owned(),
        skill: vec![SkillManifest {
            name: "invalid".to_owned(),
            path: "skills/invalid".to_owned(),
        }],
    };
    let checkout = tempdir()?;
    #[cfg(unix)]
    assert!(
        GitFetcher
            .fetch(&source, &checkout.path().join("checkout"))
            .is_err()
    );
    Ok(())
}

#[test]
fn configuration_validation_rejects_unsafe_or_inconsistent_inputs() {
    let (manifest, lock) = valid_configuration();

    let mut invalid = manifest.clone();
    invalid.schema_version = 2;
    assert!(validate_configuration(&invalid, &lock).is_err());
    let mut invalid = manifest.clone();
    invalid.source.clear();
    assert!(validate_configuration(&invalid, &lock).is_err());
    let mut invalid = manifest.clone();
    invalid.source.push(invalid.source[0].clone());
    assert!(validate_configuration(&invalid, &lock).is_err());
    let mut invalid = manifest.clone();
    let mut duplicate_repository = invalid.source[0].clone();
    duplicate_repository.id = "other/repository".to_owned();
    duplicate_repository.skill[0].name = "other".to_owned();
    invalid.source.push(duplicate_repository);
    assert!(validate_configuration(&invalid, &lock).is_err());
    for repository in [
        "http://github.com/owner/repository.git",
        "https://github.com/only-owner.git",
    ] {
        let mut invalid = manifest.clone();
        invalid.source[0].repository = repository.to_owned();
        assert!(validate_configuration(&invalid, &lock).is_err());
    }
    let mut invalid = manifest.clone();
    invalid.source[0].track = "../main".to_owned();
    assert!(validate_configuration(&invalid, &lock).is_err());
    let mut invalid = manifest.clone();
    invalid.source[0].skill.clear();
    assert!(validate_configuration(&invalid, &lock).is_err());
    let mut invalid = manifest.clone();
    let duplicate_skill = invalid.source[0].skill[0].clone();
    invalid.source[0].skill.push(duplicate_skill);
    assert!(validate_configuration(&invalid, &lock).is_err());
    let mut invalid = manifest.clone();
    invalid.source[0].skill[0].name = "Invalid".to_owned();
    assert!(validate_configuration(&invalid, &lock).is_err());
    let mut invalid = manifest.clone();
    invalid.source[0].skill[0].path = "../escape".to_owned();
    assert!(validate_configuration(&invalid, &lock).is_err());

    let mut invalid = lock.clone();
    invalid.schema_version = 2;
    assert!(validate_configuration(&manifest, &invalid).is_err());
    let mut invalid = lock.clone();
    invalid.sources.clear();
    assert!(validate_configuration(&manifest, &invalid).is_err());
    let mut invalid = lock.clone();
    invalid.sources[0].id = "other/repository".to_owned();
    assert!(validate_configuration(&manifest, &invalid).is_err());
    let mut invalid = lock.clone();
    invalid.sources.push(invalid.sources[0].clone());
    assert!(validate_configuration(&manifest, &invalid).is_err());
    let mut invalid = lock.clone();
    invalid.sources[0].revision = "invalid".to_owned();
    assert!(validate_configuration(&manifest, &invalid).is_err());
    let mut invalid = lock.clone();
    invalid.sources[0].skills.clear();
    assert!(validate_configuration(&manifest, &invalid).is_err());
    let mut invalid = lock.clone();
    let duplicate_skill = invalid.sources[0].skills[0].clone();
    invalid.sources[0].skills.push(duplicate_skill);
    assert!(validate_configuration(&manifest, &invalid).is_err());
    let mut invalid = lock.clone();
    invalid.sources[0].skills[0].name = "other".to_owned();
    assert!(validate_configuration(&manifest, &invalid).is_err());
    let mut invalid = lock.clone();
    invalid.sources[0].skills[0].tree_sha1 = "invalid".to_owned();
    assert!(validate_configuration(&manifest, &invalid).is_err());
    let mut invalid = lock;
    invalid.sources[0].skills[0].content_sha256 = "invalid".to_owned();
    assert!(validate_configuration(&manifest, &invalid).is_err());

    let error = UpstreamError::configuration("invalid configuration");
    assert_eq!(error.to_string(), "invalid configuration");
}

#[test]
fn configuration_loading_reports_missing_and_malformed_files()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = tempdir()?;
    assert!(load_configuration(repository.path()).is_err());
    fs::write(repository.path().join("upstreams.toml"), b"\xff")?;
    assert!(load_configuration(repository.path()).is_err());
    fs::write(repository.path().join("upstreams.toml"), "not = [valid")?;
    assert!(load_configuration(repository.path()).is_err());
    fs::write(
        repository.path().join("upstreams.toml"),
        concat!(
            "schema_version = 1\n",
            "[[source]]\n",
            "id = \"owner/repository\"\n",
            "repository = \"https://github.com/owner/repository.git\"\n",
            "track = \"main\"\n",
            "[[source.skill]]\n",
            "name = \"alpha\"\n",
            "path = \"skills/alpha\"\n",
        ),
    )?;
    assert!(load_configuration(repository.path()).is_err());
    fs::write(repository.path().join("upstreams.lock.json"), "not-json")?;
    assert!(load_configuration(repository.path()).is_err());
    Ok(())
}

#[test]
fn repository_configuration_covers_declared_upstream_skills()
-> Result<(), Box<dyn std::error::Error>> {
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = crate_root
        .parent()
        .and_then(Path::parent)
        .ok_or("crate is not nested under repository")?;
    let configuration = load_configuration(root)?;
    assert_eq!(configuration.manifest.source.len(), 11);
    assert_eq!(
        configuration
            .manifest
            .source
            .iter()
            .map(|source| source.skill.len())
            .sum::<usize>(),
        32
    );
    Ok(())
}

fn fixture_fetcher(available: bool) -> FixtureFetcher {
    FixtureFetcher {
        content: b"new",
        revision: "b".repeat(40),
        tree_sha1: "c".repeat(40),
        available,
    }
}

fn prepared_staging(
    inspection: &super::PreparedInspection,
    include_skill: bool,
    include_lock: bool,
) -> Result<TempDir, Box<dyn std::error::Error>> {
    let staging = Builder::new()
        .prefix(".commit-test-")
        .tempdir_in(&inspection.root)?;
    let prepared = staging.path().join("prepared");
    fs::create_dir(&prepared)?;
    fs::create_dir(staging.path().join("backups"))?;
    fs::create_dir(staging.path().join("failed"))?;
    if include_skill {
        sync::copy_directory(
            &inspection.updates[0].source_directory,
            &prepared.join("alpha"),
        )?;
    }
    if include_lock {
        let mut lock = inspection.configuration.lock.clone();
        sync::apply_lock_updates(&mut lock, &inspection.updates)?;
        let mut bytes = serde_json::to_vec_pretty(&lock)?;
        bytes.push(b'\n');
        fs::write(prepared.join("upstreams.lock.json"), bytes)?;
    }
    Ok(staging)
}

#[cfg(unix)]
fn test_mode(path: &Path) -> Result<u32, std::io::Error> {
    use std::os::unix::fs::PermissionsExt;

    Ok(fs::metadata(path)?.permissions().mode() & 0o7777)
}

#[cfg(not(unix))]
fn test_mode(path: &Path) -> Result<u32, std::io::Error> {
    Ok(u32::from(fs::metadata(path)?.permissions().readonly()))
}

fn fixture_repository() -> Result<tempfile::TempDir, Box<dyn std::error::Error>> {
    let repository = tempdir()?;
    fs::create_dir_all(repository.path().join("skills/alpha"))?;
    fs::write(repository.path().join("skills/alpha/SKILL.md"), b"old")?;
    let old_content = content_sha256(&repository.path().join("skills/alpha"))?;
    write_fixture_configuration(repository.path(), &old_content)?;
    Ok(repository)
}

fn write_fixture_configuration(
    root: &Path,
    content_sha256: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(
        root.join("upstreams.toml"),
        concat!(
            "schema_version = 1\n\n",
            "[[source]]\n",
            "id = \"owner/repository\"\n",
            "repository = \"https://github.com/owner/repository.git\"\n",
            "track = \"main\"\n\n",
            "[[source.skill]]\n",
            "name = \"alpha\"\n",
            "path = \"skills/alpha\"\n",
        ),
    )?;
    let lock = serde_json::json!({
        "schema_version": 1,
        "sources": [{
            "id": "owner/repository",
            "revision": "a".repeat(40),
            "skills": [{
                "name": "alpha",
                "tree_sha1": "a".repeat(40),
                "content_sha256": content_sha256,
            }],
        }],
    });
    let mut bytes = serde_json::to_vec_pretty(&lock)?;
    bytes.push(b'\n');
    fs::write(root.join("upstreams.lock.json"), bytes)?;
    Ok(())
}

fn valid_configuration() -> (UpstreamManifest, UpstreamLock) {
    (
        UpstreamManifest {
            schema_version: 1,
            source: vec![SourceManifest {
                id: "owner/repository".to_owned(),
                repository: "https://github.com/owner/repository.git".to_owned(),
                track: "main".to_owned(),
                skill: vec![SkillManifest {
                    name: "alpha".to_owned(),
                    path: "skills/alpha".to_owned(),
                }],
            }],
        },
        UpstreamLock {
            schema_version: 1,
            sources: vec![SourceLock {
                id: "owner/repository".to_owned(),
                revision: "a".repeat(40),
                skills: vec![SkillLock {
                    name: "alpha".to_owned(),
                    tree_sha1: "b".repeat(40),
                    content_sha256: "c".repeat(64),
                }],
            }],
        },
    )
}

fn run_git(directory: &Path, arguments: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = ProcessCommand::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}
