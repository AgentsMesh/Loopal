use std::path::Path;

use loopal_protocol::{Envelope, MessageSource, SkillInvocation};

pub fn expand_human_skill(envelope: &mut Envelope, cwd: &Path) -> bool {
    if !matches!(envelope.source, MessageSource::Human) || envelope.content.skill_info.is_some() {
        return false;
    }
    let raw = envelope.content.text.trim();
    if !raw.starts_with('/') {
        return false;
    }
    let split = raw.find(char::is_whitespace).unwrap_or(raw.len());
    let name = raw[..split].to_string();
    let args = raw[split..].trim().to_string();
    let Ok(config) = loopal_config::load_config(cwd) else {
        return false;
    };
    let Some(entry) = config.skills.get(&name) else {
        return false;
    };
    envelope.content.text = loopal_config::expand_skill(&entry.skill.body, &args);
    envelope.content.skill_info = Some(SkillInvocation {
        name,
        user_args: args,
    });
    true
}

#[cfg(test)]
mod tests {
    use loopal_protocol::{Envelope, MessageSource, QualifiedAddress};

    use super::expand_human_skill;

    #[test]
    fn expands_project_skill_and_preserves_invocation() {
        let root = tempfile::tempdir().unwrap();
        let skills = root.path().join(".loopal/skills");
        std::fs::create_dir_all(&skills).unwrap();
        std::fs::write(
            skills.join("desktop-check.md"),
            "---\ndescription: Desktop check\n---\nVerify $ARGUMENTS\n",
        )
        .unwrap();
        let mut envelope = Envelope::new(
            MessageSource::Human,
            QualifiedAddress::local("main"),
            "/desktop-check alpha beta",
        );

        assert!(expand_human_skill(&mut envelope, root.path()));
        assert_eq!(envelope.content.text, "Verify alpha beta\n");
        let skill = envelope.content.skill_info.unwrap();
        assert_eq!(skill.name, "/desktop-check");
        assert_eq!(skill.user_args, "alpha beta");
    }

    #[test]
    fn ignores_unknown_and_non_human_commands() {
        let root = tempfile::tempdir().unwrap();
        let mut human = Envelope::new(MessageSource::Human, "main", "/unknown");
        assert!(!expand_human_skill(&mut human, root.path()));
        let mut scheduled = Envelope::new(MessageSource::Scheduled, "main", "/unknown");
        assert!(!expand_human_skill(&mut scheduled, root.path()));
    }
}
