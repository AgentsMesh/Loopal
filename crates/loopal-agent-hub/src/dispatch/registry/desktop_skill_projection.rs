use loopal_config::{LayerSource, ManagedSkill, ResolvedConfig};
use loopal_ipc::protocol::methods::{DesktopSkillDefinition, DesktopSkillScope};

pub(super) fn append(
    output: &mut Vec<DesktopSkillDefinition>,
    documents: Vec<ManagedSkill>,
    source: LayerSource,
    scope: DesktopSkillScope,
    effective: &ResolvedConfig,
) {
    for document in documents {
        if !display_name(&document.skill.name) {
            continue;
        }
        let editable = matches!(scope, DesktopSkillScope::Global)
            && canonical_name(&document.skill.name)
            && document.direct_regular_file;
        output.push(DesktopSkillDefinition {
            name: document.skill.name.clone(),
            description: public_description(&document.skill.description),
            has_arguments: document.skill.has_arg,
            source: source.to_string(),
            scope,
            editable,
            effective: source_for(effective, &document.skill.name) == Some(&source),
            revision: editable.then_some(document.revision),
        });
    }
}

pub(super) fn source_for<'a>(config: &'a ResolvedConfig, name: &str) -> Option<&'a LayerSource> {
    config.skills.get(name).map(|entry| &entry.source)
}

pub(super) fn public_description(value: &str) -> String {
    value
        .chars()
        .map(|value| if value.is_control() { ' ' } else { value })
        .scan(0, |units, value| {
            *units += value.len_utf16();
            (*units <= 512).then_some(value)
        })
        .collect()
}

fn display_name(name: &str) -> bool {
    let Some(slug) = name.strip_prefix('/') else {
        return false;
    };
    !slug.is_empty()
        && name.encode_utf16().count() <= 256
        && !slug
            .chars()
            .any(|value| matches!(value, '/' | '\\') || value.is_control())
}

fn canonical_name(name: &str) -> bool {
    let Some(slug) = name.strip_prefix('/') else {
        return false;
    };
    let mut chars = slug.chars();
    slug.len() <= 64
        && chars
            .next()
            .is_some_and(|value| value.is_ascii_alphanumeric())
        && chars.all(|value| value.is_ascii_alphanumeric() || matches!(value, '_' | '-'))
}
