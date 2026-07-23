use std::ffi::OsStr;
use std::path::{Component, Path};

use sha2::{Digest, Sha256};

pub fn validate_name(path: &Path, name: &str) -> Result<(), String> {
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{}: names must contain only lowercase ASCII letters, digits, and hyphens",
            path.display()
        ))
    }
}

pub fn relative_utf8(repo_root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(repo_root).map_err(|_| {
        format!(
            "{}: source path escapes repository root {}",
            path.display(),
            repo_root.display()
        )
    })?;
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "{}: absolute paths and traversal components are forbidden",
            lossless_path(relative.as_os_str())
        ));
    }
    relative
        .components()
        .map(|component| {
            let Component::Normal(value) = component else {
                return Err(format!(
                    "{}: absolute paths and traversal components are forbidden",
                    lossless_path(relative.as_os_str())
                ));
            };
            value.to_str().ok_or_else(|| {
                format!(
                    "non-UTF-8 path rejected: {}",
                    lossless_path(relative.as_os_str())
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|components| components.join("/"))
}

pub fn file_name_utf8(path: &Path) -> Result<String, String> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("{}: source has no file name", path.display()))?;
    name.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("non-UTF-8 path rejected: {}", lossless_path(name)))
}

pub fn normalized_mode(path: &Path, metadata: &std::fs::Metadata) -> Result<u32, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = metadata.permissions().mode() & 0o7777;
        if matches!(mode, 0o644 | 0o755) {
            Ok(mode)
        } else {
            Err(format!(
                "{}: unsupported source mode {mode:#o}; expected 0644 or 0755",
                path.display()
            ))
        }
    }
    #[cfg(not(unix))]
    {
        if metadata.is_file() {
            Ok(0o644)
        } else {
            Err(format!("{}: catalog source is not a file", path.display()))
        }
    }
}

pub fn validate_portable_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    const FORBIDDEN: [&[u8]; 7] = [
        b"/home/",
        b"/Users/",
        b"/private/Users/",
        b"/root/",
        b"/mnt/",
        b"/Volumes/",
        b":\\Users\\",
    ];
    for marker in FORBIDDEN {
        if let Some(index) = bytes
            .windows(marker.len())
            .enumerate()
            .find_map(|(index, window)| {
                (window == marker && has_absolute_context(bytes, index, marker)).then_some(index)
            })
        {
            let line = bytes[..index].iter().filter(|byte| **byte == b'\n').count() + 1;
            return Err(format!(
                "{}:{line}: machine-bound path marker {:?} is forbidden",
                path.display(),
                String::from_utf8_lossy(marker)
            ));
        }
    }
    Ok(())
}

fn has_absolute_context(bytes: &[u8], index: usize, marker: &[u8]) -> bool {
    if marker.starts_with(b":\\") {
        return index > 0 && bytes[index - 1].is_ascii_alphabetic();
    }
    if index == 0 {
        return true;
    }
    let previous = bytes[index - 1];
    previous != b'~' && !previous.is_ascii_alphanumeric() && previous != b':'
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(unix)]
fn lossless_path(value: &OsStr) -> String {
    use std::os::unix::ffi::OsStrExt;

    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(not(unix))]
fn lossless_path(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}
