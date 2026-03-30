pub(crate) fn suggestions(raw: &str, token: &str, models: &[String]) -> Option<Vec<String>> {
    if token == "/models" {
        if raw.ends_with(' ') {
            return Some(vec![
                String::from("/models refresh"),
                String::from("/models add "),
                String::from("/models remove "),
            ]);
        }
        return Some(vec![String::from("/models ")]);
    }

    if let Some(rest_raw) = raw.strip_prefix("/models ") {
        let rest = rest_raw.trim_start();
        if rest.is_empty() {
            return Some(vec![
                String::from("/models refresh"),
                String::from("/models add "),
                String::from("/models remove "),
            ]);
        }
        let parts = rest.split_whitespace().collect::<Vec<_>>();
        let sub = parts.first().copied().unwrap_or_default();
        let subcommands = ["refresh", "add", "remove"];
        let has_sub_arg = rest.contains(' ');
        if !has_sub_arg {
            return Some(
                subcommands
                    .into_iter()
                    .filter(|candidate| candidate.starts_with(sub))
                    .map(|candidate| match candidate {
                        "refresh" => String::from("/models refresh"),
                        _ => format!("/models {candidate} "),
                    })
                    .collect(),
            );
        }
        if sub == "remove" {
            let arg_partial = rest[sub.len()..].trim();
            return Some(
                models
                    .iter()
                    .filter(|model| {
                        arg_partial.is_empty()
                            || model
                                .to_lowercase()
                                .starts_with(&arg_partial.to_lowercase())
                    })
                    .map(|model| format!("/models remove {model}"))
                    .collect(),
            );
        }
    }

    if token == "/model" || token == "/mod" {
        if raw.ends_with(' ') {
            return Some(
                models
                    .iter()
                    .map(|model| format!("/model {model}"))
                    .collect(),
            );
        }
        return Some(vec![String::from("/model ")]);
    }

    if let Some(partial) = raw
        .strip_prefix("/model ")
        .or_else(|| raw.strip_prefix("/mod "))
    {
        let partial = partial.trim();
        if partial.is_empty() {
            return Some(
                models
                    .iter()
                    .map(|model| format!("/model {model}"))
                    .collect(),
            );
        }
        return Some(
            models
                .iter()
                .filter(|model| model.starts_with(partial))
                .map(|found| format!("/model {found}"))
                .collect(),
        );
    }

    None
}

#[cfg(test)]
mod tests {
    use super::suggestions;

    #[test]
    fn model_provider_supports_mod_prefix_alias() {
        let models = vec![
            String::from("qwen2.5-coder:7b"),
            String::from("llama3.2:latest"),
        ];
        let items = suggestions("/mod qwen", "/mod qwen", &models).expect("provider response");
        assert_eq!(items, vec![String::from("/model qwen2.5-coder:7b")]);
    }
}
