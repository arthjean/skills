use std::collections::{BTreeMap, BTreeSet};

use super::super::model::{AssetKind, ContractFixture, ScannedCatalog, SourceFile};
use super::{utf8, validate_allowed, validate_stem_name};

pub fn validate_skills(catalog: &ScannedCatalog) -> Result<(), String> {
    let mut names = BTreeMap::<String, String>::new();
    for asset in catalog
        .assets
        .iter()
        .filter(|asset| asset.kind == AssetKind::Skill)
    {
        let skill_path = format!("{}/SKILL.md", asset.relative_path);
        let file = catalog
            .files
            .iter()
            .find(|file| file.manifest.relative_path == skill_path)
            .ok_or_else(|| format!("{skill_path}: required skill metadata is absent"))?;
        let (fields, _) = parse_frontmatter(utf8(file)?, &skill_path)?;
        let public_name = fields
            .get("name")
            .ok_or_else(|| format!("{skill_path}: frontmatter name is absent"))?;
        if public_name != &asset.name {
            return Err(format!(
                "{skill_path}: public name {public_name:?} differs from folder {:?}",
                asset.name
            ));
        }
        if let Some(previous) = names.insert(public_name.clone(), skill_path.clone()) {
            return Err(format!(
                "{skill_path}: duplicate public skill name {public_name:?}, first declared by {previous}"
            ));
        }
    }
    Ok(())
}

pub fn validate_agent(file: &SourceFile, fixture: &ContractFixture) -> Result<(), String> {
    let (fields, body) = parse_frontmatter(utf8(file)?, &file.manifest.relative_path)?;
    validate_fields(&file.manifest.relative_path, &fields, fixture)?;
    if body.trim().is_empty() {
        return Err(format!(
            "{}: agent instructions are empty",
            file.manifest.relative_path
        ));
    }
    validate_stem_name(file, field(&fields, "name", file)?)?;
    validate_allowed(
        file,
        "model",
        field(&fields, "model", file)?,
        &fixture.models,
    )?;
    validate_allowed(
        file,
        "effort",
        field(&fields, "effort", file)?,
        &fixture.efforts,
    )?;
    validate_allowed(
        file,
        "permissionMode",
        field(&fields, "permissionMode", file)?,
        &fixture.permission_modes,
    )?;
    validate_allowed(
        file,
        "color",
        field(&fields, "color", file)?,
        &fixture.colors,
    )?;
    let turns = field(&fields, "maxTurns", file)?
        .parse::<u16>()
        .map_err(|error| {
            format!(
                "{}: maxTurns is invalid: {error}",
                file.manifest.relative_path
            )
        })?;
    if turns == 0 {
        return Err(format!(
            "{}: maxTurns must be positive",
            file.manifest.relative_path
        ));
    }
    for tool in field(&fields, "tools", file)?.split(',').map(str::trim) {
        let base = if let Some((name, arguments)) = tool.split_once('(') {
            if name != "Bash" || !arguments.ends_with(')') || arguments == ")" {
                return Err(format!(
                    "{}: malformed Claude tool expression {tool:?}",
                    file.manifest.relative_path
                ));
            }
            name
        } else {
            tool
        };
        if !fixture.tools.iter().any(|allowed| allowed == base) {
            return Err(format!(
                "{}: unsupported Claude tool {tool:?}",
                file.manifest.relative_path
            ));
        }
    }
    Ok(())
}

fn parse_frontmatter<'a>(
    text: &'a str,
    path: &str,
) -> Result<(BTreeMap<String, String>, &'a str), String> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Err(format!("{path}: frontmatter must start with ---"));
    }
    let mut fields = BTreeMap::new();
    let mut offset = 4;
    for line in lines {
        offset += line.len() + 1;
        if line == "---" {
            let body = text.get(offset..).unwrap_or_default();
            return Ok((fields, body));
        }
        if line.starts_with(char::is_whitespace) || line.trim().is_empty() {
            continue;
        }
        let (key, raw_value) = line
            .split_once(':')
            .ok_or_else(|| format!("{path}: invalid frontmatter line {line:?}"))?;
        let value = unquote(raw_value.trim());
        if fields.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err(format!("{path}: duplicate frontmatter field {key:?}"));
        }
    }
    Err(format!("{path}: frontmatter closing --- is absent"))
}

fn validate_fields(
    path: &str,
    fields: &BTreeMap<String, String>,
    fixture: &ContractFixture,
) -> Result<(), String> {
    let allowed = fixture
        .required_fields
        .iter()
        .chain(&fixture.optional_fields)
        .collect::<BTreeSet<_>>();
    for required in &fixture.required_fields {
        if !fields.contains_key(required) {
            return Err(format!("{path}: required field {required:?} is absent"));
        }
    }
    if let Some(unknown) = fields.keys().find(|field| !allowed.contains(field)) {
        return Err(format!("{path}: unsupported field {unknown:?}"));
    }
    Ok(())
}

fn field<'a>(
    fields: &'a BTreeMap<String, String>,
    key: &str,
    file: &SourceFile,
) -> Result<&'a str, String> {
    fields
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("{}: field {key:?} is absent", file.manifest.relative_path))
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}
