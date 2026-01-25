//! Dotenv support for loading environment variables from .env files
//!
//! Supports standard .env file format:
//! - KEY=value
//! - KEY="quoted value"
//! - KEY='single quoted'
//! - # comments
//! - export KEY=value (optional export prefix)
//! - Multiline values with quotes
//! - Variable interpolation: ${VAR} or $VAR

use std::collections::HashMap;
use std::path::Path;

use crate::{Error, Result};

/// Configuration for dotenv loading
#[derive(Debug, Clone, Default)]
pub struct DotenvConfig {
    /// Whether dotenv is enabled (default: true)
    pub enabled: bool,
    /// Path to .env file relative to project root (default: ".env")
    pub path: String,
    /// Whether to override existing environment variables (default: false)
    pub override_env: bool,
    /// Additional .env files to load (e.g., ".env.local", ".env.development")
    pub extra_files: Vec<String>,
}

impl DotenvConfig {
    /// Create default configuration
    pub fn new() -> Self {
        Self {
            enabled: true,
            path: ".env".to_string(),
            override_env: false,
            extra_files: vec![],
        }
    }

    /// Parse configuration from TOML table
    pub fn from_toml(table: &toml::Table) -> Self {
        let mut config = Self::new();

        if let Some(enabled) = table.get("enabled").and_then(|v| v.as_bool()) {
            config.enabled = enabled;
        }

        if let Some(path) = table.get("path").and_then(|v| v.as_str()) {
            config.path = path.to_string();
        }

        if let Some(override_env) = table.get("override").and_then(|v| v.as_bool()) {
            config.override_env = override_env;
        }

        if let Some(extra) = table.get("extra_files").and_then(|v| v.as_array()) {
            config.extra_files = extra
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }

        config
    }
}

/// Load environment variables from a .env file
pub fn load_dotenv(project_dir: &Path, config: &DotenvConfig) -> Result<HashMap<String, String>> {
    let mut env_vars = HashMap::new();

    if !config.enabled {
        return Ok(env_vars);
    }

    // Load main .env file
    let main_env_path = project_dir.join(&config.path);
    if main_env_path.exists() {
        let vars = parse_dotenv_file(&main_env_path)?;
        for (key, value) in vars {
            env_vars.insert(key, value);
        }
    }

    // Load extra files in order (later files override earlier ones)
    for extra_file in &config.extra_files {
        let extra_path = project_dir.join(extra_file);
        if extra_path.exists() {
            let vars = parse_dotenv_file(&extra_path)?;
            for (key, value) in vars {
                env_vars.insert(key, value);
            }
        }
    }

    // Perform variable interpolation
    let env_vars = interpolate_variables(env_vars);

    Ok(env_vars)
}

/// Parse a .env file and return key-value pairs
pub fn parse_dotenv_file(path: &Path) -> Result<HashMap<String, String>> {
    let content = std::fs::read_to_string(path).map_err(Error::Io)?;
    parse_dotenv(&content)
}

/// Parse dotenv content string
pub fn parse_dotenv(content: &str) -> Result<HashMap<String, String>> {
    let mut vars = HashMap::new();
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Handle export prefix
        let line = line.strip_prefix("export ").unwrap_or(line);

        // Find the = sign
        let Some(eq_pos) = line.find('=') else {
            continue;
        };

        let key = line[..eq_pos].trim().to_string();
        let mut value = line[eq_pos + 1..].trim().to_string();

        // Handle quoted values
        if value.starts_with('"') {
            value = parse_double_quoted(&value, &mut lines);
        } else if value.starts_with('\'') {
            value = parse_single_quoted(&value, &mut lines);
        } else {
            // Unquoted: remove inline comments
            if let Some(comment_pos) = value.find(" #") {
                value = value[..comment_pos].trim().to_string();
            }
        }

        if !key.is_empty() {
            vars.insert(key, value);
        }
    }

    Ok(vars)
}

/// Parse a double-quoted value (supports escape sequences and multiline)
fn parse_double_quoted(first_line: &str, lines: &mut std::iter::Peekable<std::str::Lines>) -> String {
    let mut value = first_line[1..].to_string(); // Remove opening quote

    // Check if closed on same line
    if let Some(end_pos) = find_unescaped_quote(&value, '"') {
        return unescape_double_quoted(&value[..end_pos]);
    }

    // Multiline value
    while let Some(line) = lines.next() {
        value.push('\n');
        value.push_str(line);

        if let Some(end_pos) = find_unescaped_quote(line, '"') {
            // Found closing quote
            let total_len = value.len();
            let trimmed = &value[..total_len - line.len() + end_pos];
            return unescape_double_quoted(trimmed);
        }
    }

    // No closing quote found, return as-is
    unescape_double_quoted(&value)
}

/// Parse a single-quoted value (literal, no escape sequences)
fn parse_single_quoted(first_line: &str, lines: &mut std::iter::Peekable<std::str::Lines>) -> String {
    let mut value = first_line[1..].to_string(); // Remove opening quote

    // Check if closed on same line
    if let Some(end_pos) = value.find('\'') {
        return value[..end_pos].to_string();
    }

    // Multiline value
    while let Some(line) = lines.next() {
        value.push('\n');
        value.push_str(line);

        if let Some(end_pos) = line.find('\'') {
            let total_len = value.len();
            return value[..total_len - line.len() + end_pos].to_string();
        }
    }

    value
}

/// Find an unescaped quote character
fn find_unescaped_quote(s: &str, quote: char) -> Option<usize> {
    let mut chars = s.chars().enumerate();
    let mut escaped = false;

    while let Some((i, c)) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == quote {
            return Some(i);
        }
    }

    None
}

/// Unescape double-quoted string
fn unescape_double_quoted(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('$') => result.push('$'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Interpolate ${VAR} and $VAR references
fn interpolate_variables(mut vars: HashMap<String, String>) -> HashMap<String, String> {
    // We need to handle the case where variables reference each other
    // Do multiple passes until no more interpolation is possible
    let max_passes = 10;

    for _ in 0..max_passes {
        let mut changed = false;

        let keys: Vec<String> = vars.keys().cloned().collect();
        for key in keys {
            let value = vars.get(&key).cloned().unwrap_or_default();
            let new_value = interpolate_value(&value, &vars);

            if new_value != value {
                vars.insert(key, new_value);
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    vars
}

/// Interpolate a single value
fn interpolate_value(value: &str, vars: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            // Check for ${VAR} or $VAR
            if chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                let var_name: String = chars.by_ref().take_while(|&c| c != '}').collect();

                // Look up in our vars first, then system env
                if let Some(var_value) = vars.get(&var_name) {
                    result.push_str(var_value);
                } else if let Ok(env_value) = std::env::var(&var_name) {
                    result.push_str(&env_value);
                }
                // If not found, just omit it (empty string)
            } else {
                // $VAR format - collect alphanumeric and underscore using peek
                let mut var_name = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_alphanumeric() || next_c == '_' {
                        var_name.push(next_c);
                        chars.next();
                    } else {
                        break;
                    }
                }

                if !var_name.is_empty() {
                    if let Some(var_value) = vars.get(&var_name) {
                        result.push_str(var_value);
                    } else if let Ok(env_value) = std::env::var(&var_name) {
                        result.push_str(&env_value);
                    }
                } else {
                    result.push('$');
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let content = r#"
KEY=value
ANOTHER=test
"#;
        let vars = parse_dotenv(content).unwrap();
        assert_eq!(vars.get("KEY"), Some(&"value".to_string()));
        assert_eq!(vars.get("ANOTHER"), Some(&"test".to_string()));
    }

    #[test]
    fn test_parse_with_comments() {
        let content = r#"
# This is a comment
KEY=value # inline comment
ANOTHER=test
"#;
        let vars = parse_dotenv(content).unwrap();
        assert_eq!(vars.get("KEY"), Some(&"value".to_string()));
        assert_eq!(vars.get("ANOTHER"), Some(&"test".to_string()));
    }

    #[test]
    fn test_parse_quoted() {
        let content = r#"
DOUBLE="hello world"
SINGLE='hello world'
WITH_HASH="value # not a comment"
"#;
        let vars = parse_dotenv(content).unwrap();
        assert_eq!(vars.get("DOUBLE"), Some(&"hello world".to_string()));
        assert_eq!(vars.get("SINGLE"), Some(&"hello world".to_string()));
        assert_eq!(vars.get("WITH_HASH"), Some(&"value # not a comment".to_string()));
    }

    #[test]
    fn test_parse_export_prefix() {
        let content = r#"
export KEY=value
export QUOTED="hello"
"#;
        let vars = parse_dotenv(content).unwrap();
        assert_eq!(vars.get("KEY"), Some(&"value".to_string()));
        assert_eq!(vars.get("QUOTED"), Some(&"hello".to_string()));
    }

    #[test]
    fn test_parse_escape_sequences() {
        let content = r#"
NEWLINE="hello\nworld"
TAB="hello\tworld"
ESCAPED="hello\"world"
"#;
        let vars = parse_dotenv(content).unwrap();
        assert_eq!(vars.get("NEWLINE"), Some(&"hello\nworld".to_string()));
        assert_eq!(vars.get("TAB"), Some(&"hello\tworld".to_string()));
        assert_eq!(vars.get("ESCAPED"), Some(&"hello\"world".to_string()));
    }

    #[test]
    fn test_interpolation() {
        let content = r#"
BASE=/usr/local
PATH_VAR=${BASE}/bin
SIMPLE=$BASE/lib
"#;
        let vars = parse_dotenv(content).unwrap();
        let vars = interpolate_variables(vars);
        assert_eq!(vars.get("PATH_VAR"), Some(&"/usr/local/bin".to_string()));
        assert_eq!(vars.get("SIMPLE"), Some(&"/usr/local/lib".to_string()));
    }

    #[test]
    fn test_multiline_double_quoted() {
        let content = r#"MULTI="line1
line2
line3""#;
        let vars = parse_dotenv(content).unwrap();
        assert_eq!(vars.get("MULTI"), Some(&"line1\nline2\nline3".to_string()));
    }

    #[test]
    fn test_empty_value() {
        let content = r#"
EMPTY=
QUOTED_EMPTY=""
"#;
        let vars = parse_dotenv(content).unwrap();
        assert_eq!(vars.get("EMPTY"), Some(&"".to_string()));
        assert_eq!(vars.get("QUOTED_EMPTY"), Some(&"".to_string()));
    }

    #[test]
    fn test_dotenv_config_from_toml() {
        let toml_str = r#"
enabled = true
path = ".env.local"
override = true
extra_files = [".env.development", ".env.local"]
"#;
        let table: toml::Table = toml::from_str(toml_str).unwrap();
        let config = DotenvConfig::from_toml(&table);

        assert!(config.enabled);
        assert_eq!(config.path, ".env.local");
        assert!(config.override_env);
        assert_eq!(config.extra_files, vec![".env.development", ".env.local"]);
    }
}
