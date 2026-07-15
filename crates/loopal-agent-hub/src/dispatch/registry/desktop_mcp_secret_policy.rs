use std::collections::{HashMap, HashSet};

use loopal_config::{LayerSource, LocalSettingsFieldPatch};

use super::desktop_mcp_secret_patches;

pub(super) fn inherited_patches(
    name: &str,
    target: &str,
    enabled: bool,
    source: &LayerSource,
    values: &HashMap<String, String>,
    explicit: &HashSet<String>,
) -> Result<Vec<LocalSettingsFieldPatch>, String> {
    if !enabled || *source == LayerSource::Local {
        return Ok(Vec::new());
    }
    let missing: Vec<_> = values
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .filter(|(key, _)| {
            let identity = if target == "headers" {
                key.to_ascii_lowercase()
            } else {
                (*key).clone()
            };
            !explicit.contains(&identity)
        })
        .collect();
    if *source != LayerSource::Project {
        if !missing.is_empty() {
            return Err(format!(
                "{} MCP secrets cannot be copied into project settings; replace or remove every configured secret",
                source_label(source),
            ));
        }
        return Ok(Vec::new());
    }
    values
        .keys()
        .try_for_each(|key| desktop_mcp_secret_patches::validate_existing_name(target, key))?;
    Ok(missing
        .into_iter()
        .map(|(key, value)| {
            LocalSettingsFieldPatch::Set(
                format!("mcp_servers.{name}.{target}.{key}"),
                value.clone().into(),
            )
        })
        .collect())
}

fn source_label(source: &LayerSource) -> &'static str {
    match source {
        LayerSource::Global => "global",
        LayerSource::Plugin(_) => "plugin",
        LayerSource::Project => "project",
        LayerSource::Local => "local",
        LayerSource::Env => "environment",
        LayerSource::Cli => "CLI",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> HashMap<String, String> {
        HashMap::from([("TOKEN".into(), "secret-value".into())])
    }

    #[test]
    fn local_stays_in_place_and_project_may_copy() {
        let none = HashSet::new();
        assert!(
            inherited_patches("server", "env", true, &LayerSource::Local, &values(), &none)
                .unwrap()
                .is_empty()
        );
        let project = inherited_patches(
            "server",
            "env",
            true,
            &LayerSource::Project,
            &values(),
            &none,
        )
        .unwrap();
        assert_eq!(project.len(), 1);
    }

    #[test]
    fn global_and_plugin_require_every_secret_to_be_explicit() {
        for source in [LayerSource::Global, LayerSource::Plugin("tools".into())] {
            let error =
                inherited_patches("server", "env", true, &source, &values(), &HashSet::new())
                    .unwrap_err();
            assert!(error.contains("replace or remove every configured secret"));
            assert!(!error.contains("secret-value"));
            let explicit = HashSet::from(["TOKEN".into()]);
            assert!(
                inherited_patches("server", "env", true, &source, &values(), &explicit,)
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn disabling_never_copies_or_requires_inherited_secrets() {
        for source in [
            LayerSource::Project,
            LayerSource::Global,
            LayerSource::Plugin("tools".into()),
        ] {
            assert!(
                inherited_patches("server", "env", false, &source, &values(), &HashSet::new(),)
                    .unwrap()
                    .is_empty()
            );
        }
    }
}
