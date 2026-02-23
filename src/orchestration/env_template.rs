use anyhow::{Context, Result};
use minijinja::{Environment, UndefinedBehavior};
use std::collections::HashMap;

/// Render a template string by substituting `{{ key }}` placeholders with values.
///
/// Uses minijinja for Jinja2-compatible template rendering.
/// Strict mode: returns an error if a placeholder key is not found in the values map.
pub fn render(template: &str, values: &HashMap<String, String>) -> Result<String> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    let result = env
        .render_str(template, values)
        .context("failed to render template")?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_substitution() {
        let values = HashMap::from([
            ("port".to_string(), "54321".to_string()),
            ("username".to_string(), "dev".to_string()),
            ("password".to_string(), "dev".to_string()),
            ("database".to_string(), "myapp_dev".to_string()),
        ]);
        let result = render(
            "postgresql://{{ username }}:{{ password }}@localhost:{{ port }}/{{ database }}",
            &values,
        )
        .unwrap();
        assert_eq!(result, "postgresql://dev:dev@localhost:54321/myapp_dev");
    }

    #[test]
    fn missing_key_returns_error() {
        let values = HashMap::new();
        let result = render("http://localhost:{{ port }}", &values);
        assert!(result.is_err());
    }

    #[test]
    fn no_placeholders_passes_through() {
        let values = HashMap::new();
        let result = render("minioadmin", &values).unwrap();
        assert_eq!(result, "minioadmin");
    }

    #[test]
    fn multiple_occurrences_of_same_placeholder() {
        let values = HashMap::from([
            ("host".to_string(), "localhost".to_string()),
            ("port".to_string(), "9000".to_string()),
        ]);
        let result = render("{{ host }}:{{ port }} and {{ host }}:{{ port }}", &values).unwrap();
        assert_eq!(result, "localhost:9000 and localhost:9000");
    }

    #[test]
    fn empty_template() {
        let values = HashMap::new();
        let result = render("", &values).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn literal_braces_not_confused() {
        let values = HashMap::from([("name".to_string(), "test".to_string())]);
        let result = render("hello {{ name }}", &values).unwrap();
        assert_eq!(result, "hello test");
    }
}
