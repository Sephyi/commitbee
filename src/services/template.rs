// SPDX-FileCopyrightText: 2026 Sephyi <me@sephy.io>
//
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;
use std::path::Path;

use crate::error::{Error, Result};

/// Load a template file from disk and substitute `{{variable}}` placeholders.
///
/// Variables use the syntax `{{name}}` (no spaces around the name).
/// If a `{{variable}}` in the template has no matching key in `vars`,
/// it is left as-is in the output (no error).
///
/// # Errors
///
/// Returns `Error::Config` if the file cannot be read.
pub fn render_template(path: &Path, vars: &HashMap<&str, &str>) -> Result<String> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        Error::Config(format!(
            "failed to read template file '{}': {}",
            path.display(),
            e
        ))
    })?;
    Ok(substitute(&content, vars))
}

/// Load a file and return its contents as a string.
///
/// # Errors
///
/// Returns `Error::Config` if the file cannot be read.
pub fn load_file(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        Error::Config(format!(
            "failed to read prompt file '{}': {}",
            path.display(),
            e
        ))
    })
}

/// Substitute `{{key}}` placeholders in `template` with values from `vars`.
///
/// Unknown variables (no matching key) are left untouched.
fn substitute(template: &str, vars: &HashMap<&str, &str>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_replaces_known_variables() {
        let mut vars = HashMap::new();
        vars.insert("diff", "some diff content");
        vars.insert("files", "src/main.rs");

        let template = "Diff:\n{{diff}}\nFiles: {{files}}";
        let result = substitute(template, &vars);
        assert_eq!(result, "Diff:\nsome diff content\nFiles: src/main.rs");
    }

    #[test]
    fn substitute_leaves_unknown_variables_intact() {
        let vars = HashMap::new();
        let template = "Hello {{unknown}} world";
        let result = substitute(template, &vars);
        assert_eq!(result, "Hello {{unknown}} world");
    }

    #[test]
    fn substitute_handles_empty_template() {
        let vars = HashMap::new();
        let result = substitute("", &vars);
        assert_eq!(result, "");
    }

    #[test]
    fn substitute_handles_multiple_occurrences() {
        let mut vars = HashMap::new();
        vars.insert("name", "commitbee");
        let template = "{{name}} is {{name}}";
        let result = substitute(template, &vars);
        assert_eq!(result, "commitbee is commitbee");
    }
}
