//! Writing an issue's message in a person's language.

use crate::issue::Issue;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Writes the message of an issue.
///
/// A closure `Fn(&Issue) -> String` is a resolver too.
pub trait MessageResolver {
    /// The message for `issue`.
    fn resolve(&self, issue: &Issue) -> String;
}

impl<F: Fn(&Issue) -> String> MessageResolver for F {
    fn resolve(&self, issue: &Issue) -> String {
        self(issue)
    }
}

/// A catalogue of message templates keyed by message key or code.
///
/// An issue is looked up by its message key first and by its code second. A template's
/// `{name}` placeholders are filled from the issue's metadata; a template naming an entry the
/// metadata lacks is passed over. When no template fits, the issue keeps the message it has.
///
/// ```
/// use raoh::{Issue, MessageResolver, Messages};
///
/// let issue = Issue::new("too_short", "must be at least 3 characters").with_meta("min", 3);
/// assert_eq!(Messages::japanese().resolve(&issue), "3文字以上で入力してください");
///
/// let mine = Messages::english().with_overrides([("too_short", "{min}+ characters, please")]);
/// assert_eq!(mine.resolve(&issue), "3+ characters, please");
/// ```
#[derive(Clone, Debug, Default)]
pub struct Messages {
    templates: HashMap<String, String>,
}

static ENGLISH: LazyLock<Messages> =
    LazyLock::new(|| Messages::from_properties(include_str!("messages/en.properties")));
static JAPANESE: LazyLock<Messages> =
    LazyLock::new(|| Messages::from_properties(include_str!("messages/ja.properties")));

impl Messages {
    /// The English catalogue, the same one Raoh for Java ships.
    pub fn english() -> &'static Messages {
        &ENGLISH
    }

    /// The Japanese catalogue, the same one Raoh for Java ships.
    pub fn japanese() -> &'static Messages {
        &JAPANESE
    }

    /// An empty catalogue, which leaves every message as it is.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Reads a catalogue in the `.properties` form Raoh for Java uses: one `raoh.<key>=<template>`
    /// per line. Lines that are blank, start with `#` or `!`, or lack `=` are skipped, and the
    /// `raoh.` prefix is optional.
    pub fn from_properties(text: &str) -> Self {
        let templates = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
            .filter_map(|line| line.split_once('='))
            .map(|(key, template)| {
                let key = key.trim();
                let key = key.strip_prefix("raoh.").unwrap_or(key);
                (key.to_owned(), template.trim().to_owned())
            })
            .collect();
        Self { templates }
    }

    /// This catalogue with `overrides` added, replacing templates under the same key.
    pub fn with_overrides<K, T>(&self, overrides: impl IntoIterator<Item = (K, T)>) -> Self
    where
        K: Into<String>,
        T: Into<String>,
    {
        let mut templates = self.templates.clone();
        templates.extend(overrides.into_iter().map(|(k, t)| (k.into(), t.into())));
        Self { templates }
    }

    /// The template under `key`, if there is one.
    pub fn template(&self, key: &str) -> Option<&str> {
        self.templates.get(key).map(String::as_str)
    }
}

impl MessageResolver for Messages {
    fn resolve(&self, issue: &Issue) -> String {
        [issue.message_key(), issue.code()]
            .into_iter()
            .filter_map(|key| self.template(key))
            .find_map(|template| fill(template, issue.meta()))
            .unwrap_or_else(|| issue.message().to_owned())
    }
}

/// `template` with each `{name}` replaced by the metadata entry `name`, or `None` when an entry is
/// missing.
fn fill(template: &str, meta: &Map<String, Value>) -> Option<String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}').map(|close| (close, &after[..close])) {
            Some((close, name)) if is_placeholder_name(name) => {
                out.push_str(&display(meta.get(name)?));
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Some(out)
}

fn is_placeholder_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// A metadata value as text: strings without quotes, lists as `[a, b]`, as Raoh for Java writes
/// them.
pub(crate) fn display(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(display).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(entries) => {
            let entries: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{k}={}", display(v)))
                .collect();
            format!("{{{}}}", entries.join(", "))
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{codes, message_keys};

    #[test]
    fn the_message_key_is_looked_up_before_the_code() {
        let issue = Issue::new(codes::OUT_OF_RANGE, "must be positive")
            .with_message_key(message_keys::OUT_OF_RANGE_POSITIVE)
            .with_meta("min", 1);
        assert_eq!(
            Messages::japanese().resolve(&issue),
            "正の値で入力してください"
        );
    }

    #[test]
    fn a_template_missing_a_meta_entry_falls_back_to_the_code() {
        let messages = Messages::empty().with_overrides([
            ("out_of_range.minimum", "at least {min}"),
            ("out_of_range", "out of range"),
        ]);
        let issue = Issue::new(codes::OUT_OF_RANGE, "x")
            .with_message_key(message_keys::OUT_OF_RANGE_MINIMUM);
        assert_eq!(messages.resolve(&issue), "out of range");
    }

    #[test]
    fn with_no_template_the_issue_keeps_its_message() {
        let issue = Issue::new("mine", "my message");
        assert_eq!(Messages::english().resolve(&issue), "my message");
    }

    #[test]
    fn a_custom_message_is_not_rewritten() {
        let issue = Issue::new(codes::REQUIRED, "is required").with_custom_message("give a name");
        assert_eq!(issue.resolve(Messages::japanese()).message(), "give a name");
    }

    #[test]
    fn lists_are_written_as_java_writes_them() {
        let issue = Issue::new(codes::NOT_ALLOWED, "x").with_meta("allowed", vec!["a", "b"]);
        assert_eq!(Messages::english().resolve(&issue), "must be one of [a, b]");
    }

    #[test]
    fn a_closure_is_a_resolver() {
        let upper = |issue: &Issue| issue.code().to_uppercase();
        assert_eq!(upper.resolve(&Issue::new("blank", "")), "BLANK");
    }
}
