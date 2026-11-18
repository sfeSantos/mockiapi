use std::collections::HashMap;
use chrono::Utc;

const OPEN: &str = "{{";
const CLOSE: &str = "}}";

/// Replaces `{{name}}` placeholders in a response template.
///
/// - `{{timestamp}}` becomes the current UTC time.
/// - Any other name is looked up in `variables`.
/// - A placeholder with no value is left as it is.
///
/// The template is walked once and the result is written into a single buffer.
pub fn replace_variables(template: &str, variables: &HashMap<String, String>) -> String {
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;

    while let Some(start) = remaining.find(OPEN) {
        let after_open = &remaining[start + OPEN.len()..];
        let name = after_open
            .find(CLOSE)
            .map(|end| &after_open[..end])
            .filter(|name| is_variable_name(name));

        output.push_str(&remaining[..start]);

        match name {
            Some(name) => {
                push_value(&mut output, name, variables);
                remaining = &after_open[name.len() + CLOSE.len()..];
            }
            // Not a placeholder: keep one brace and look again from the next character,
            // so the inner pair of `{{{name}}}` is still found.
            None => {
                output.push('{');
                remaining = &remaining[start + 1..];
            }
        }
    }

    output.push_str(remaining);

    output
}

fn is_variable_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|character| character.is_alphanumeric() || character == '_')
}

fn push_value(output: &mut String, name: &str, variables: &HashMap<String, String>) {
    match (name, variables.get(name)) {
        ("timestamp", _) => output.push_str(&Utc::now().to_rfc3339()),
        (_, Some(value)) => output.push_str(value),
        (_, None) => {
            output.push_str(OPEN);
            output.push_str(name);
            output.push_str(CLOSE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variables(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn replaces_known_variables() {
        let replaced = replace_variables(
            r#"{"id":"{{id}}","greeting":"Hello, {{name}}!"}"#,
            &variables(&[("id", "123"), ("name", "John")]),
        );

        assert_eq!(replaced, r#"{"id":"123","greeting":"Hello, John!"}"#);
    }

    #[test]
    fn leaves_unknown_variables_untouched() {
        let replaced = replace_variables("{{known}} and {{unknown}}", &variables(&[("known", "yes")]));

        assert_eq!(replaced, "yes and {{unknown}}");
    }

    #[test]
    fn replaces_the_timestamp() {
        let replaced = replace_variables("{{timestamp}}", &HashMap::new());

        assert!(chrono::DateTime::parse_from_rfc3339(&replaced).is_ok());
    }

    #[test]
    fn ignores_braces_that_are_not_placeholders() {
        let known = variables(&[("name", "John")]);

        assert_eq!(replace_variables("{{ name }} {{}} {{a-b}}", &known), "{{ name }} {{}} {{a-b}}");
        assert_eq!(replace_variables("{{name", &known), "{{name");
        assert_eq!(replace_variables("{{{name}}}", &known), "{John}");
        assert_eq!(replace_variables("{{x {{name}}", &known), "{{x John");
    }

    #[test]
    fn keeps_text_that_is_not_ascii() {
        let replaced = replace_variables("Olá, {{nome}} ✓", &variables(&[("nome", "João")]));

        assert_eq!(replaced, "Olá, João ✓");
    }

    #[test]
    fn template_without_placeholders_is_returned_as_is() {
        assert_eq!(replace_variables(r#"{"plain":true}"#, &HashMap::new()), r#"{"plain":true}"#);
    }
}
