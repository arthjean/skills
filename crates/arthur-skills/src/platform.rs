use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

#[cfg(windows)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
#[cfg(windows)]
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

pub(crate) fn path_key(value: &OsStr) -> Vec<u8> {
    value.as_encoded_bytes().to_vec()
}

#[cfg(unix)]
pub(crate) fn metadata_mode(metadata: &fs::Metadata) -> u32 {
    metadata.mode() & 0o7777
}

#[cfg(windows)]
pub(crate) fn metadata_mode(metadata: &fs::Metadata) -> u32 {
    if metadata.is_dir() {
        0o755
    } else if metadata.permissions().readonly() {
        0o444
    } else {
        0o644
    }
}

#[cfg(unix)]
pub(crate) fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(windows)]
pub(crate) fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_readonly(effective_file_mode(mode) & 0o222 == 0);
    fs::set_permissions(path, permissions)
}

#[cfg(unix)]
pub(crate) fn metadata_device(metadata: &fs::Metadata) -> u64 {
    metadata.dev()
}

#[cfg(windows)]
pub(crate) fn metadata_device(_metadata: &fs::Metadata) -> u64 {
    // Stable Rust does not expose the Windows volume serial number. Root validation
    // therefore uses canonical identity, while operation preconditions carry node data.
    0
}

#[cfg(unix)]
pub(crate) fn metadata_inode(metadata: &fs::Metadata) -> u64 {
    metadata.ino()
}

#[cfg(windows)]
pub(crate) fn metadata_inode(metadata: &fs::Metadata) -> u64 {
    // Creation time is the strongest stable per-node value exposed by MetadataExt.
    // Callers combine it with kind, size, mode, mtime, and hashes where destructive.
    metadata.creation_time()
}

#[cfg(unix)]
pub(crate) fn metadata_mtime_seconds(metadata: &fs::Metadata) -> i64 {
    metadata.mtime()
}

#[cfg(windows)]
pub(crate) fn metadata_mtime_seconds(metadata: &fs::Metadata) -> i64 {
    i64::try_from(metadata.last_write_time() / 10_000_000).unwrap_or(i64::MAX)
}

#[cfg(unix)]
pub(crate) fn metadata_mtime_nanoseconds(metadata: &fs::Metadata) -> i64 {
    metadata.mtime_nsec()
}

#[cfg(windows)]
pub(crate) fn metadata_mtime_nanoseconds(metadata: &fs::Metadata) -> i64 {
    i64::try_from(metadata.last_write_time() % 10_000_000).unwrap_or_default() * 100
}

pub(crate) fn same_node(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    metadata_device(left) == metadata_device(right) && metadata_inode(left) == metadata_inode(right)
}

#[cfg(unix)]
pub(crate) fn open_directory(path: &Path) -> io::Result<File> {
    File::open(path)
}

#[cfg(windows)]
pub(crate) fn open_directory(path: &Path) -> io::Result<File> {
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(unix)]
pub(crate) fn sync_directory_handle(directory: &File) -> io::Result<()> {
    directory.sync_all()
}

#[cfg(windows)]
pub(crate) fn sync_directory_handle(_directory: &File) -> io::Result<()> {
    Ok(())
}

pub(crate) fn is_normalized_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| !matches!(component, Component::CurDir | Component::ParentDir))
}

pub(crate) fn normalize_absolute(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    let mut normal_components = 0_usize;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                if !normalized.as_os_str().is_empty() {
                    return None;
                }
                normalized.push(prefix.as_os_str());
            }
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(value) => {
                normalized.push(value);
                normal_components += 1;
            }
            Component::ParentDir => {
                if normal_components == 0 || !normalized.pop() {
                    return None;
                }
                normal_components -= 1;
            }
        }
    }
    Some(normalized)
}

pub(crate) fn effective_file_mode(mode: u32) -> u32 {
    if cfg!(windows) {
        let _ = mode;
        0o644
    } else {
        mode
    }
}

pub(crate) fn effective_directory_mode(mode: u32) -> u32 {
    if cfg!(windows) { 0o755 } else { mode }
}
