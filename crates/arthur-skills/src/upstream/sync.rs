use std::fs;
use std::io;
use std::path::Path;

use tempfile::{Builder, TempDir};

use super::model::{UpstreamError, content_sha256, hash_bytes};
use super::{PreparedInspection, PreparedUpdate};

pub(super) fn apply(inspection: &PreparedInspection) -> Result<Vec<String>, UpstreamError> {
    revalidate_configuration(inspection)?;
    for update in &inspection.updates {
        let destination = inspection.root.join("skills").join(&update.name);
        let observed = content_sha256(&destination).map_err(UpstreamError::synchronization)?;
        if observed != update.pinned_content_sha256 {
            return Err(UpstreamError::synchronization(format!(
                "{} changed after upstream planning",
                update.name
            )));
        }
    }

    let staging = Builder::new()
        .prefix(".arthur-upstream-")
        .tempdir_in(&inspection.root)
        .map_err(staging_creation_failure)?;
    let prepared_root = staging.path().join("prepared");
    let backup_root = staging.path().join("backups");
    let failed_root = staging.path().join("failed");
    fs::create_dir(&prepared_root)
        .and_then(|()| fs::create_dir(&backup_root))
        .and_then(|()| fs::create_dir(&failed_root))
        .map_err(staging_initialization_failure)?;

    for update in &inspection.updates {
        let prepared = prepared_root.join(&update.name);
        copy_directory(&update.source_directory, &prepared)?;
        let observed = content_sha256(&prepared).map_err(UpstreamError::synchronization)?;
        if observed != update.latest_content_sha256 {
            return Err(UpstreamError::synchronization(format!(
                "{} changed while staging",
                update.name
            )));
        }
    }

    let mut next_lock = inspection.configuration.lock.clone();
    apply_lock_updates(&mut next_lock, &inspection.updates)?;
    let mut lock_bytes = serde_json::to_vec_pretty(&next_lock).map_err(|error| {
        UpstreamError::synchronization(format!("cannot serialize updated upstream lock: {error}"))
    })?;
    lock_bytes.push(b'\n');
    let prepared_lock = prepared_root.join("upstreams.lock.json");
    fs::write(&prepared_lock, lock_bytes)
        .map_err(|error| io_failure(&prepared_lock, "cannot stage updated lock", error))?;
    if let Ok(metadata) = fs::metadata(&inspection.configuration.lock_path) {
        fs::set_permissions(&prepared_lock, metadata.permissions()).map_err(|error| {
            io_failure(&prepared_lock, "cannot preserve lock permissions", error)
        })?;
    }

    commit_staging(staging, inspection)
}

pub(super) fn revalidate_configuration(
    inspection: &PreparedInspection,
) -> Result<(), UpstreamError> {
    for (path, expected, label) in [
        (
            &inspection.configuration.manifest_path,
            &inspection.configuration.manifest_sha256,
            "upstream manifest",
        ),
        (
            &inspection.configuration.lock_path,
            &inspection.configuration.lock_sha256,
            "upstream lock",
        ),
    ] {
        let bytes = fs::read(path).map_err(|error| {
            UpstreamError::synchronization(format!(
                "{}: cannot revalidate {label}: {error}",
                path.display()
            ))
        })?;
        if hash_bytes(&bytes) != *expected {
            return Err(UpstreamError::synchronization(format!(
                "{} changed after upstream planning",
                path.display()
            )));
        }
    }
    Ok(())
}

pub(super) fn apply_lock_updates(
    lock: &mut super::model::UpstreamLock,
    updates: &[PreparedUpdate],
) -> Result<(), UpstreamError> {
    for update in updates {
        let source = lock
            .sources
            .iter_mut()
            .find(|source| source.id == update.source_id)
            .ok_or_else(|| {
                UpstreamError::synchronization(format!(
                    "{}: source disappeared from lock",
                    update.source_id
                ))
            })?;
        source.revision.clone_from(&update.source_revision);
        let skill = source
            .skills
            .iter_mut()
            .find(|skill| skill.name == update.name)
            .ok_or_else(|| {
                UpstreamError::synchronization(format!(
                    "{}: skill disappeared from lock",
                    update.name
                ))
            })?;
        skill.tree_sha1.clone_from(&update.latest_tree_sha1);
        skill
            .content_sha256
            .clone_from(&update.latest_content_sha256);
    }
    Ok(())
}

pub(super) fn commit_staging(
    staging: TempDir,
    inspection: &PreparedInspection,
) -> Result<Vec<String>, UpstreamError> {
    let prepared_root = staging.path().join("prepared");
    let backup_root = staging.path().join("backups");
    let mut applied = Vec::new();

    for update in &inspection.updates {
        let destination = inspection.root.join("skills").join(&update.name);
        let backup = backup_root.join(&update.name);
        let prepared = prepared_root.join(&update.name);
        if let Err(error) =
            move_directory_if_unchanged(&destination, &backup, &update.pinned_content_sha256)
        {
            return Err(rollback_or_preserve(
                staging,
                inspection,
                &applied,
                error.message,
                error.preserve,
            ));
        }
        if let Err(error) = fs::rename(&prepared, &destination) {
            let failure =
                restore_after_install_failure(&backup, &destination, error, "staged snapshot");
            return Err(rollback_or_preserve(
                staging,
                inspection,
                &applied,
                failure.message,
                failure.preserve,
            ));
        }
        applied.push(update.name.clone());
    }

    let lock_path = &inspection.configuration.lock_path;
    let backup_lock = backup_root.join("upstreams.lock.json");
    let prepared_lock = prepared_root.join("upstreams.lock.json");
    if let Err(error) = move_file_if_unchanged(
        lock_path,
        &backup_lock,
        &inspection.configuration.lock_sha256,
    ) {
        return Err(rollback_or_preserve(
            staging,
            inspection,
            &applied,
            error.message,
            error.preserve,
        ));
    }
    if let Err(error) = fs::rename(&prepared_lock, lock_path) {
        let failure = restore_after_install_failure(&backup_lock, lock_path, error, "updated lock");
        return Err(rollback_or_preserve(
            staging,
            inspection,
            &applied,
            failure.message,
            failure.preserve,
        ));
    }
    Ok(applied)
}

pub(super) struct MoveError {
    pub(super) message: String,
    pub(super) preserve: bool,
}

pub(super) fn move_directory_if_unchanged(
    source: &Path,
    backup: &Path,
    expected: &str,
) -> Result<(), MoveError> {
    fs::rename(source, backup).map_err(|error| MoveError {
        message: format!("{}: cannot create backup: {error}", source.display()),
        preserve: false,
    })?;
    let observed = content_sha256(backup);
    if matches!(observed.as_deref(), Ok(actual) if actual == expected) {
        return Ok(());
    }
    let reason = match observed {
        Ok(_) => "changed while the update was being prepared".to_owned(),
        Err(error) => format!("cannot revalidate backup: {error}"),
    };
    Err(restore_moved_backup(backup, source, &reason))
}

pub(super) fn restore_moved_backup(backup: &Path, source: &Path, reason: &str) -> MoveError {
    match fs::rename(backup, source) {
        Ok(()) => MoveError {
            message: format!("{} {reason}", source.display()),
            preserve: false,
        },
        Err(restore_error) => MoveError {
            message: format!(
                "{} {reason}; cannot restore backup: {restore_error}",
                source.display()
            ),
            preserve: true,
        },
    }
}

pub(super) fn move_file_if_unchanged(
    source: &Path,
    backup: &Path,
    expected: &str,
) -> Result<(), MoveError> {
    fs::rename(source, backup).map_err(|error| MoveError {
        message: format!("{}: cannot create backup: {error}", source.display()),
        preserve: false,
    })?;
    let observed = fs::read(backup).map(|bytes| hash_bytes(&bytes));
    if matches!(observed.as_deref(), Ok(actual) if actual == expected) {
        return Ok(());
    }
    let reason = match observed {
        Ok(_) => "changed while the update was being prepared".to_owned(),
        Err(error) => format!("cannot revalidate backup: {error}"),
    };
    Err(restore_moved_backup(backup, source, &reason))
}

pub(super) fn restore_after_install_failure(
    backup: &Path,
    destination: &Path,
    install_error: io::Error,
    label: &str,
) -> MoveError {
    match fs::rename(backup, destination) {
        Ok(()) => MoveError {
            message: format!(
                "{}: cannot install {label}: {install_error}",
                destination.display()
            ),
            preserve: false,
        },
        Err(restore_error) => MoveError {
            message: format!(
                "{}: cannot install {label} ({install_error}) or restore backup ({restore_error})",
                destination.display()
            ),
            preserve: true,
        },
    }
}

pub(super) fn rollback_or_preserve(
    staging: TempDir,
    inspection: &PreparedInspection,
    applied: &[String],
    original: String,
    preserve: bool,
) -> UpstreamError {
    match rollback_applied(staging.path(), inspection, applied) {
        Ok(()) if !preserve => UpstreamError::synchronization(original),
        Ok(()) => {
            let preserved = staging.keep();
            UpstreamError::synchronization(format!(
                "{original}; backups preserved at {}",
                preserved.display()
            ))
        }
        Err(rollback) => {
            let preserved = staging.keep();
            UpstreamError::synchronization(format!(
                "{original}; rollback failed: {rollback}; backups preserved at {}",
                preserved.display()
            ))
        }
    }
}

pub(super) fn rollback_applied(
    staging: &Path,
    inspection: &PreparedInspection,
    applied: &[String],
) -> io::Result<()> {
    for name in applied.iter().rev() {
        let update = inspection
            .updates
            .iter()
            .find(|update| update.name == *name)
            .ok_or_else(|| io::Error::other(format!("{name}: update metadata is unavailable")))?;
        let destination = inspection.root.join("skills").join(name);
        let failed = staging.join("failed").join(name);
        let backup = staging.join("backups").join(name);
        let observed = content_sha256(&destination).map_err(io::Error::other)?;
        if observed != update.latest_content_sha256 {
            return Err(io::Error::other(format!(
                "{} changed after installation; original backup was preserved",
                destination.display()
            )));
        }
        fs::rename(&destination, failed)?;
        fs::rename(backup, destination)?;
    }
    Ok(())
}

pub(super) fn copy_directory(source: &Path, destination: &Path) -> Result<(), UpstreamError> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| io_failure(source, "cannot inspect source directory", error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(UpstreamError::synchronization(format!(
            "{}: staged source must be a real directory",
            source.display()
        )));
    }
    fs::create_dir(destination)
        .map_err(|error| io_failure(destination, "cannot create staged directory", error))?;
    fs::set_permissions(destination, metadata.permissions())
        .map_err(|error| io_failure(destination, "cannot preserve directory permissions", error))?;

    let entries = fs::read_dir(source)
        .map_err(|error| io_failure(source, "cannot read source directory", error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_failure(source, "cannot read source entry", error))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)
            .map_err(|error| io_failure(&source_path, "cannot inspect source entry", error))?;
        if metadata.file_type().is_symlink() {
            return Err(UpstreamError::synchronization(format!(
                "{}: source symlinks are forbidden",
                source_path.display()
            )));
        }
        if metadata.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path)
                .map_err(|error| io_failure(&source_path, "cannot copy source file", error))?;
            fs::set_permissions(&destination_path, metadata.permissions()).map_err(|error| {
                io_failure(&destination_path, "cannot preserve file permissions", error)
            })?;
        } else {
            return Err(UpstreamError::synchronization(format!(
                "{}: unsupported source type",
                source_path.display()
            )));
        }
    }
    Ok(())
}

pub(super) fn io_failure(path: &Path, action: &str, error: io::Error) -> UpstreamError {
    UpstreamError::synchronization(format!("{}: {action}: {error}", path.display()))
}

pub(super) fn staging_creation_failure(error: io::Error) -> UpstreamError {
    UpstreamError::synchronization(format!(
        "cannot create repository staging directory: {error}"
    ))
}

pub(super) fn staging_initialization_failure(error: io::Error) -> UpstreamError {
    UpstreamError::synchronization(format!(
        "cannot initialize repository staging directory: {error}"
    ))
}
