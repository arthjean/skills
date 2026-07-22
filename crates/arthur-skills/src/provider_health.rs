use crate::catalog::{Manifest, Provider};

#[derive(Debug, Eq, PartialEq)]
pub enum ProviderHealth {
    Healthy,
    Unhealthy(ProviderIssue),
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProviderIssue {
    ContractAbsent,
    InvalidVersion { observed: String },
    VersionBelowMinimum { observed: String, minimum: String },
    UnknownModel { published: String },
}

pub fn assess(
    manifest: &Manifest,
    provider: Provider,
    observed_version: &str,
    published_model: &str,
) -> ProviderHealth {
    let Some(contract) = manifest
        .provider_contracts
        .iter()
        .find(|contract| contract.provider == provider)
    else {
        return ProviderHealth::Unhealthy(ProviderIssue::ContractAbsent);
    };
    let Some(observed) = Version::parse(observed_version) else {
        return ProviderHealth::Unhealthy(ProviderIssue::InvalidVersion {
            observed: observed_version.to_owned(),
        });
    };
    let Some(minimum) = Version::parse(&contract.validated_version) else {
        return ProviderHealth::Unhealthy(ProviderIssue::InvalidVersion {
            observed: contract.validated_version.clone(),
        });
    };
    if observed < minimum {
        return ProviderHealth::Unhealthy(ProviderIssue::VersionBelowMinimum {
            observed: observed_version.to_owned(),
            minimum: contract.validated_version.clone(),
        });
    }
    if !contract.models.iter().any(|model| model == published_model) {
        return ProviderHealth::Unhealthy(ProviderIssue::UnknownModel {
            published: published_model.to_owned(),
        });
    }
    ProviderHealth::Healthy
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Version([u64; 3]);

impl Version {
    fn parse(value: &str) -> Option<Self> {
        let mut parts = value.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self([major, minor, patch]))
    }
}

#[cfg(test)]
mod tests {
    use crate::catalog::{Catalog, Provider};

    use super::{ProviderHealth, ProviderIssue, assess};

    #[test]
    fn provider_health_preserves_unknown_models_and_enforces_minimums() {
        let catalog = match Catalog::load() {
            Ok(catalog) => catalog,
            Err(error) => panic!("catalog failed validation: {error}"),
        };
        assert_eq!(
            assess(
                catalog.manifest(),
                Provider::Codex,
                "0.144.6",
                "gpt-5.6-sol"
            ),
            ProviderHealth::Healthy
        );
        assert_eq!(
            assess(
                catalog.manifest(),
                Provider::Codex,
                "0.143.9",
                "gpt-5.6-sol"
            ),
            ProviderHealth::Unhealthy(ProviderIssue::VersionBelowMinimum {
                observed: "0.143.9".to_owned(),
                minimum: "0.144.6".to_owned(),
            })
        );
        assert_eq!(
            assess(
                catalog.manifest(),
                Provider::Codex,
                "0.145.0",
                "unknown-model"
            ),
            ProviderHealth::Unhealthy(ProviderIssue::UnknownModel {
                published: "unknown-model".to_owned(),
            })
        );
        assert!(matches!(
            assess(
                catalog.manifest(),
                Provider::Claude,
                "not-a-version",
                "claude-fable-5[1m]"
            ),
            ProviderHealth::Unhealthy(ProviderIssue::InvalidVersion { .. })
        ));

        let mut missing_contract = catalog.manifest().clone();
        missing_contract.provider_contracts.clear();
        assert_eq!(
            assess(&missing_contract, Provider::Codex, "0.144.6", "gpt-5.6-sol"),
            ProviderHealth::Unhealthy(ProviderIssue::ContractAbsent)
        );

        let mut invalid_contract = catalog.manifest().clone();
        invalid_contract.provider_contracts[0].validated_version = "invalid".to_owned();
        assert!(matches!(
            assess(
                &invalid_contract,
                invalid_contract.provider_contracts[0].provider,
                "2.1.217",
                "claude-fable-5[1m]"
            ),
            ProviderHealth::Unhealthy(ProviderIssue::InvalidVersion { .. })
        ));
    }
}
