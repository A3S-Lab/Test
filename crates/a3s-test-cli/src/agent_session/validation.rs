use a3s_test_core::Target;
use anyhow::Result;

pub(super) fn compact_target(raw_target: &str) -> Result<Target> {
    if raw_target.trim().is_empty() {
        anyhow::bail!("target must not be empty");
    }
    if (raw_target.starts_with("@e") || raw_target.starts_with("@c"))
        && raw_target.get(2..).is_some_and(|suffix| {
            !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
        })
    {
        Ok(Target::Ref {
            value: raw_target.to_string(),
        })
    } else {
        Ok(Target::Css {
            selector: raw_target.to_string(),
        })
    }
}

pub(super) fn validate_session_id(session: &str) -> Result<()> {
    if session.is_empty()
        || session.len() > 48
        || !session
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        anyhow::bail!("session id must be 1-48 ASCII letters, digits, '-' or '_'");
    }
    Ok(())
}
