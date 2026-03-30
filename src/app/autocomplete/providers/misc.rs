use crate::app::COMMAND_SUGGESTIONS;

pub(crate) fn top_level(token: &str) -> Vec<String> {
    COMMAND_SUGGESTIONS
        .iter()
        .find(|candidate| candidate.starts_with(token))
        .map(|found| vec![format!("{found} ")])
        .unwrap_or_default()
}
