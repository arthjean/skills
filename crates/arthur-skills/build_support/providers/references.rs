use std::collections::BTreeSet;

use super::super::model::SourceFile;

pub fn validate(files: &[SourceFile]) -> Result<(), String> {
    const SUPPORT: [&str; 4] = [
        "agent-boundaries.md",
        "scope-guard.md",
        "synthesis-template.md",
        "three-tier-constraints.md",
    ];
    let paths = files
        .iter()
        .map(|file| file.manifest.relative_path.as_str())
        .collect::<BTreeSet<_>>();
    for name in SUPPORT {
        let required = format!("shared/claude/skills/_shared/{name}");
        if !paths.contains(required.as_str()) {
            return Err(format!(
                "{required}: required Claude support document is absent"
            ));
        }
    }

    const MARKER: &str = "~/.claude/skills/_shared/";
    for file in files {
        let Ok(text) = std::str::from_utf8(&file.bytes) else {
            continue;
        };
        let mut remaining = text;
        while let Some(index) = remaining.find(MARKER) {
            let referenced = &remaining[index + MARKER.len()..];
            if referenced.starts_with('{') || referenced.starts_with('<') {
                remaining = &referenced[1..];
                continue;
            }
            let length = referenced
                .bytes()
                .take_while(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(*byte, b'.' | b'-' | b'_')
                })
                .count();
            if length == 0 {
                remaining = referenced.get(1..).unwrap_or_default();
                continue;
            }
            let name = &referenced[..length];
            let target = format!("shared/claude/skills/_shared/{name}");
            if !paths.contains(target.as_str()) {
                return Err(format!(
                    "{}: internal reference {MARKER}{name} is not packaged",
                    file.manifest.relative_path
                ));
            }
            remaining = &referenced[length..];
        }
    }
    Ok(())
}
