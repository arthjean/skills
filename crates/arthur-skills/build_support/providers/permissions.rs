use toml::Value;

use super::super::model::{ContractFixture, SourceFile};

pub fn validate(
    file: &SourceFile,
    table: &toml::Table,
    fixture: &ContractFixture,
) -> Result<(), String> {
    let default_name = string(file, table, "default_permissions")?;
    let permissions = table
        .get("permissions")
        .and_then(Value::as_table)
        .ok_or_else(|| {
            format!(
                "{}: permissions must be a table",
                file.manifest.relative_path
            )
        })?;
    if !permissions.contains_key(default_name) {
        return Err(format!(
            "{}: default permission profile {default_name:?} is absent",
            file.manifest.relative_path
        ));
    }
    for (name, value) in permissions {
        let profile = value.as_table().ok_or_else(|| {
            format!(
                "{}: permissions.{name} must be a table",
                file.manifest.relative_path
            )
        })?;
        reject_unknown(
            file,
            profile,
            &fixture.permission_sections,
            &format!("permissions.{name}"),
        )?;
        validate_filesystem(file, name, profile)?;
        if let Some(network) = profile.get("network") {
            validate_network(file, name, network)?;
        }
    }
    Ok(())
}

fn validate_filesystem(
    file: &SourceFile,
    profile_name: &str,
    profile: &toml::Table,
) -> Result<(), String> {
    let Some(filesystem) = profile.get("filesystem") else {
        return Ok(());
    };
    let filesystem = filesystem.as_table().ok_or_else(|| {
        format!(
            "{}: permissions.{profile_name}.filesystem must be a table",
            file.manifest.relative_path
        )
    })?;
    for (path, access) in filesystem {
        if !matches!(access.as_str(), Some("read" | "write")) || path.is_empty() {
            return Err(format!(
                "{}: invalid filesystem permission in profile {profile_name}",
                file.manifest.relative_path
            ));
        }
    }
    Ok(())
}

fn validate_network(file: &SourceFile, profile: &str, value: &Value) -> Result<(), String> {
    let network = value.as_table().ok_or_else(|| {
        format!(
            "{}: permissions.{profile}.network must be a table",
            file.manifest.relative_path
        )
    })?;
    let allowed = [
        "enabled".to_owned(),
        "mode".to_owned(),
        "domains".to_owned(),
    ];
    reject_unknown(
        file,
        network,
        &allowed,
        &format!("permissions.{profile}.network"),
    )?;
    if network.get("enabled").and_then(Value::as_bool) != Some(true)
        || network.get("mode").and_then(Value::as_str) != Some("limited")
    {
        return Err(format!(
            "{}: network permission must be enabled and limited",
            file.manifest.relative_path
        ));
    }
    if let Some(domains) = network.get("domains") {
        let domains = domains.as_table().ok_or_else(|| {
            format!(
                "{}: permissions.{profile}.network.domains must be a table",
                file.manifest.relative_path
            )
        })?;
        if domains
            .values()
            .any(|value| value.as_str() != Some("allow"))
        {
            return Err(format!(
                "{}: network domains must use allow",
                file.manifest.relative_path
            ));
        }
    }
    Ok(())
}

fn reject_unknown(
    file: &SourceFile,
    table: &toml::Table,
    allowed: &[String],
    location: &str,
) -> Result<(), String> {
    if let Some(unknown) = table.keys().find(|key| !allowed.contains(key)) {
        Err(format!(
            "{}: unsupported {location} field {unknown:?}",
            file.manifest.relative_path
        ))
    } else {
        Ok(())
    }
}

fn string<'a>(file: &SourceFile, table: &'a toml::Table, key: &str) -> Result<&'a str, String> {
    table
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{}: {key} must be a string", file.manifest.relative_path))
}
