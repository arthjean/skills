use std::collections::BTreeSet;

use toml::Value;

use super::super::model::{ContractFixture, SourceFile};
use super::{utf8, validate_allowed, validate_stem_name};

pub fn validate_agent(file: &SourceFile, fixture: &ContractFixture) -> Result<(), String> {
    let document = toml::from_str::<Value>(utf8(file)?).map_err(|error| {
        format!(
            "{}: invalid Codex TOML: {error}",
            file.manifest.relative_path
        )
    })?;
    let table = document.as_table().ok_or_else(|| {
        format!(
            "{}: Codex document must be a table",
            file.manifest.relative_path
        )
    })?;
    validate_fields(file, table, fixture)?;

    validate_stem_name(file, string(file, table, "name")?)?;
    validate_toml_allowed(file, table, "model", &fixture.models)?;
    validate_toml_allowed(file, table, "model_reasoning_effort", &fixture.efforts)?;
    validate_toml_allowed(
        file,
        table,
        "web_search",
        &["disabled".to_owned(), "live".to_owned()],
    )?;
    if string(file, table, "description")?.is_empty()
        || string(file, table, "developer_instructions")?.is_empty()
    {
        return Err(format!(
            "{}: Codex descriptions and instructions are required",
            file.manifest.relative_path
        ));
    }
    validate_mcp(file, table, fixture)?;
    super::permissions::validate(file, table, fixture)
}

fn validate_fields(
    file: &SourceFile,
    table: &toml::Table,
    fixture: &ContractFixture,
) -> Result<(), String> {
    for required in &fixture.required_fields {
        if !table.contains_key(required) {
            return Err(format!(
                "{}: required field {required:?} is absent",
                file.manifest.relative_path
            ));
        }
    }
    let allowed = fixture
        .required_fields
        .iter()
        .chain(&fixture.optional_fields)
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = table.keys().find(|field| !allowed.contains(field)) {
        return Err(format!(
            "{}: unsupported top-level field {unknown:?}",
            file.manifest.relative_path
        ));
    }
    Ok(())
}

fn validate_mcp(
    file: &SourceFile,
    table: &toml::Table,
    fixture: &ContractFixture,
) -> Result<(), String> {
    let servers = table
        .get("mcp_servers")
        .and_then(Value::as_table)
        .ok_or_else(|| {
            format!(
                "{}: mcp_servers must be a table",
                file.manifest.relative_path
            )
        })?;
    for (name, value) in servers {
        let server = value.as_table().ok_or_else(|| {
            format!(
                "{}: mcp_servers.{name} must be a table",
                file.manifest.relative_path
            )
        })?;
        reject_unknown(
            file,
            server,
            &fixture.mcp_fields,
            &format!("mcp_servers.{name}"),
        )?;
        let enabled = server
            .get("enabled")
            .and_then(Value::as_bool)
            .ok_or_else(|| {
                format!(
                    "{}: mcp_servers.{name}.enabled must be boolean",
                    file.manifest.relative_path
                )
            })?;
        let url = server.get("url").and_then(Value::as_str);
        let command = server.get("command").and_then(Value::as_str);
        let bearer_token = server.get("bearer_token_env_var");
        if url.is_some() == command.is_some() {
            return Err(format!(
                "{}: mcp_servers.{name} must define exactly one of url or command",
                file.manifest.relative_path
            ));
        }
        if url.is_some_and(|url| !url.starts_with("https://")) {
            return Err(format!(
                "{}: mcp_servers.{name}.url must use HTTPS",
                file.manifest.relative_path
            ));
        }
        if let Some(value) = bearer_token {
            let variable = value.as_str().ok_or_else(|| {
                format!(
                    "{}: mcp_servers.{name}.bearer_token_env_var must be a string",
                    file.manifest.relative_path
                )
            })?;
            let portable_name = !variable.is_empty()
                && variable
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_');
            if command.is_some() || !portable_name {
                return Err(format!(
                    "{}: mcp_servers.{name}.bearer_token_env_var must name an environment variable for a URL server",
                    file.manifest.relative_path
                ));
            }
        }
        if let Some(command) = command {
            if command.contains('/') || command.contains('\\') {
                return Err(format!(
                    "{}: mcp_servers.{name}.command must resolve through PATH",
                    file.manifest.relative_path
                ));
            }
            if name == "paneflow" && (enabled || command != "paneflow-mcp") {
                return Err(format!(
                    "{}: disabled Paneflow must use the paneflow-mcp PATH command",
                    file.manifest.relative_path
                ));
            }
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

fn validate_toml_allowed(
    file: &SourceFile,
    table: &toml::Table,
    key: &str,
    allowed: &[String],
) -> Result<(), String> {
    validate_allowed(file, key, string(file, table, key)?, allowed)
}
