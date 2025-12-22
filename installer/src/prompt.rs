//! Unified manifest/CLI prompting system
//!
//! This module provides a system where manifest schema fields automatically
//! generate CLI prompts. The manifest is the single source of truth - if a
//! value is provided in the manifest, it's used directly. Otherwise, the
//! user is prompted interactively.

use anyhow::{bail, Result};
use std::io::{self, Write};

/// Type of prompt to display for a field
#[derive(Debug, Clone)]
pub enum PromptKind {
    /// Simple text input
    Text,
    /// Text input with a default value
    TextWithDefault(String),
    /// Password input (hidden)
    Password,
    /// Password with confirmation
    PasswordConfirm,
    /// Yes/no boolean
    Boolean { default: bool },
    /// Selection from a list of options
    Select {
        options: Vec<SelectOption>,
        default: Option<usize>,
    },
    /// Optional selection (can choose "none")
    OptionalSelect {
        options: Vec<SelectOption>,
        default: Option<usize>,
    },
    /// Number input with range
    Number { min: i64, max: i64, default: i64 },
}

/// An option in a select prompt
#[derive(Debug, Clone)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Metadata for a configuration field
#[derive(Debug, Clone)]
pub struct FieldSpec {
    /// Field identifier (matches manifest key)
    pub key: String,
    /// Human-readable prompt text
    pub prompt: String,
    /// Type of prompt to show
    pub kind: PromptKind,
    /// Whether this field should be prompted (can be conditional)
    pub enabled: bool,
}

impl FieldSpec {
    pub fn text(key: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            prompt: prompt.into(),
            kind: PromptKind::Text,
            enabled: true,
        }
    }

    pub fn text_default(
        key: impl Into<String>,
        prompt: impl Into<String>,
        default: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            prompt: prompt.into(),
            kind: PromptKind::TextWithDefault(default.into()),
            enabled: true,
        }
    }

    pub fn password(key: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            prompt: prompt.into(),
            kind: PromptKind::Password,
            enabled: true,
        }
    }

    pub fn password_confirm(key: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            prompt: prompt.into(),
            kind: PromptKind::PasswordConfirm,
            enabled: true,
        }
    }

    pub fn boolean(key: impl Into<String>, prompt: impl Into<String>, default: bool) -> Self {
        Self {
            key: key.into(),
            prompt: prompt.into(),
            kind: PromptKind::Boolean { default },
            enabled: true,
        }
    }

    pub fn select(
        key: impl Into<String>,
        prompt: impl Into<String>,
        options: Vec<SelectOption>,
    ) -> Self {
        Self {
            key: key.into(),
            prompt: prompt.into(),
            kind: PromptKind::Select {
                options,
                default: None,
            },
            enabled: true,
        }
    }

    pub fn select_default(
        key: impl Into<String>,
        prompt: impl Into<String>,
        options: Vec<SelectOption>,
        default: usize,
    ) -> Self {
        Self {
            key: key.into(),
            prompt: prompt.into(),
            kind: PromptKind::Select {
                options,
                default: Some(default),
            },
            enabled: true,
        }
    }

    pub fn optional_select(
        key: impl Into<String>,
        prompt: impl Into<String>,
        options: Vec<SelectOption>,
    ) -> Self {
        Self {
            key: key.into(),
            prompt: prompt.into(),
            kind: PromptKind::OptionalSelect {
                options,
                default: None,
            },
            enabled: true,
        }
    }

    pub fn number(
        key: impl Into<String>,
        prompt: impl Into<String>,
        min: i64,
        max: i64,
        default: i64,
    ) -> Self {
        Self {
            key: key.into(),
            prompt: prompt.into(),
            kind: PromptKind::Number { min, max, default },
            enabled: true,
        }
    }

    /// Disable this field (won't be prompted)
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Conditionally enable/disable
    pub fn when(mut self, condition: bool) -> Self {
        self.enabled = condition;
        self
    }
}

/// Result of prompting for a field
#[derive(Debug, Clone)]
pub enum FieldValue {
    Text(String),
    Boolean(bool),
    Number(i64),
    None,
}

impl FieldValue {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            FieldValue::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            FieldValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<i64> {
        match self {
            FieldValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, FieldValue::None)
    }
}

/// Prompt for a single field value
pub fn prompt_field(spec: &FieldSpec) -> Result<FieldValue> {
    if !spec.enabled {
        return Ok(FieldValue::None);
    }

    match &spec.kind {
        PromptKind::Text => {
            let value = prompt_text(&spec.prompt)?;
            Ok(FieldValue::Text(value))
        }
        PromptKind::TextWithDefault(default) => {
            let value = prompt_text_default(&spec.prompt, default)?;
            Ok(FieldValue::Text(value))
        }
        PromptKind::Password => {
            let value = prompt_password(&spec.prompt)?;
            Ok(FieldValue::Text(value))
        }
        PromptKind::PasswordConfirm => {
            let value = prompt_password_confirm(&spec.prompt)?;
            Ok(FieldValue::Text(value))
        }
        PromptKind::Boolean { default } => {
            let value = prompt_yes_no(&spec.prompt, *default)?;
            Ok(FieldValue::Boolean(value))
        }
        PromptKind::Select { options, default } => {
            let idx = prompt_select(&spec.prompt, options, *default)?;
            Ok(FieldValue::Text(options[idx].value.clone()))
        }
        PromptKind::OptionalSelect { options, default } => {
            match prompt_optional_select(&spec.prompt, options, *default)? {
                Some(idx) => Ok(FieldValue::Text(options[idx].value.clone())),
                None => Ok(FieldValue::None),
            }
        }
        PromptKind::Number { min, max, default } => {
            let value = prompt_number(&spec.prompt, *min, *max, *default)?;
            Ok(FieldValue::Number(value))
        }
    }
}

// ============================================================================
// Low-level prompt functions
// ============================================================================

fn read_line() -> Result<String> {
    print!("");
    io::stdout().flush()?;
    let mut input = String::new();
    let bytes_read = io::stdin().read_line(&mut input)?;

    if bytes_read == 0 {
        bail!("Unexpected end of input. Is stdin connected to a terminal?");
    }

    Ok(input.trim().to_string())
}

fn prompt_text(prompt: &str) -> Result<String> {
    print!("{}: ", prompt);
    io::stdout().flush()?;
    read_line()
}

fn prompt_text_default(prompt: &str, default: &str) -> Result<String> {
    print!("{} [{}]: ", prompt, default);
    io::stdout().flush()?;
    let input = read_line()?;
    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

fn prompt_password(prompt: &str) -> Result<String> {
    rpassword::prompt_password(format!("{}: ", prompt))
        .map_err(|e| anyhow::anyhow!("Failed to read password: {}", e))
}

fn prompt_password_confirm(prompt: &str) -> Result<String> {
    loop {
        let pass1 = rpassword::prompt_password(format!("{}: ", prompt))
            .map_err(|e| anyhow::anyhow!("Failed to read password: {}", e))?;

        if pass1.is_empty() {
            println!("Password cannot be empty");
            continue;
        }

        let pass2 = rpassword::prompt_password(format!("Confirm {}: ", prompt.to_lowercase()))
            .map_err(|e| anyhow::anyhow!("Failed to read password: {}", e))?;

        if pass1 != pass2 {
            println!("Passwords do not match");
            continue;
        }

        return Ok(pass1);
    }
}

pub fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let default_str = if default { "Y/n" } else { "y/N" };
    print!("{} [{}]: ", prompt, default_str);
    io::stdout().flush()?;

    let input = read_line()?.to_lowercase();

    if input.is_empty() {
        Ok(default)
    } else if input == "y" || input == "yes" {
        Ok(true)
    } else if input == "n" || input == "no" {
        Ok(false)
    } else {
        Ok(default)
    }
}

fn prompt_select(prompt: &str, options: &[SelectOption], default: Option<usize>) -> Result<usize> {
    println!("\n{}:", prompt);
    for (i, opt) in options.iter().enumerate() {
        let marker = if default == Some(i) { " (default)" } else { "" };
        if let Some(desc) = &opt.description {
            println!("  [{}] {} - {}{}", i + 1, opt.label, desc, marker);
        } else {
            println!("  [{}] {}{}", i + 1, opt.label, marker);
        }
    }

    loop {
        let prompt_text = match default {
            Some(d) => format!("Select [1-{}, default={}]: ", options.len(), d + 1),
            None => format!("Select [1-{}]: ", options.len()),
        };
        print!("{}", prompt_text);
        io::stdout().flush()?;

        let input = read_line()?;

        if input.is_empty() {
            if let Some(d) = default {
                return Ok(d);
            }
            println!("Please make a selection");
            continue;
        }

        if let Ok(n) = input.parse::<usize>() {
            if n >= 1 && n <= options.len() {
                return Ok(n - 1);
            }
        }
        println!("Invalid selection");
    }
}

fn prompt_optional_select(
    prompt: &str,
    options: &[SelectOption],
    default: Option<usize>,
) -> Result<Option<usize>> {
    println!("\n{}:", prompt);
    for (i, opt) in options.iter().enumerate() {
        let marker = if default == Some(i) { " (default)" } else { "" };
        if let Some(desc) = &opt.description {
            println!("  [{}] {} - {}{}", i + 1, opt.label, desc, marker);
        } else {
            println!("  [{}] {}{}", i + 1, opt.label, marker);
        }
    }
    let none_marker = if default.is_none() { " (default)" } else { "" };
    println!("  [{}] None{}", options.len() + 1, none_marker);

    loop {
        let default_num = default.map(|d| d + 1).unwrap_or(options.len() + 1);
        print!("Select [1-{}, default={}]: ", options.len() + 1, default_num);
        io::stdout().flush()?;

        let input = read_line()?;

        if input.is_empty() {
            return Ok(default);
        }

        if let Ok(n) = input.parse::<usize>() {
            if n >= 1 && n <= options.len() {
                return Ok(Some(n - 1));
            } else if n == options.len() + 1 {
                return Ok(None);
            }
        }
        println!("Invalid selection");
    }
}

fn prompt_number(prompt: &str, min: i64, max: i64, default: i64) -> Result<i64> {
    loop {
        print!("{} [{}-{}, default={}]: ", prompt, min, max, default);
        io::stdout().flush()?;

        let input = read_line()?;

        if input.is_empty() {
            return Ok(default);
        }

        if let Ok(n) = input.parse::<i64>() {
            if n >= min && n <= max {
                return Ok(n);
            }
            println!("Value must be between {} and {}", min, max);
        } else {
            println!("Invalid number");
        }
    }
}

// ============================================================================
// Manifest-aware prompting
// ============================================================================

/// Check if a manifest value differs from its default
pub fn has_manifest_value<T: PartialEq>(manifest_value: &T, default: &T) -> bool {
    manifest_value != default
}

/// Print a message indicating a value came from the manifest
pub fn print_manifest_value(key: &str, value: impl std::fmt::Display) {
    println!("  {} (from manifest): {}", key, value);
}

/// Prompt for a value, using manifest value if provided
pub fn prompt_or_manifest<T: Clone + std::fmt::Display>(
    spec: &FieldSpec,
    manifest_value: Option<&T>,
) -> Result<FieldValue> {
    if let Some(val) = manifest_value {
        print_manifest_value(&spec.key, val);
        Ok(FieldValue::Text(val.to_string()))
    } else {
        prompt_field(spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_option_builder() {
        let opt = SelectOption::new("val", "Label")
            .with_description("A description");
        assert_eq!(opt.value, "val");
        assert_eq!(opt.label, "Label");
        assert_eq!(opt.description, Some("A description".to_string()));
    }

    #[test]
    fn test_field_spec_builders() {
        let text = FieldSpec::text("hostname", "Enter hostname");
        assert_eq!(text.key, "hostname");
        assert!(text.enabled);

        let disabled = FieldSpec::text("test", "Test").disabled();
        assert!(!disabled.enabled);

        let conditional = FieldSpec::text("test", "Test").when(false);
        assert!(!conditional.enabled);
    }
}
