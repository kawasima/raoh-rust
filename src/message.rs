//! Writing an issue's message in a person's language.

use crate::issue::Issue;
use crate::java;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

/// Writes the sentence for an issue that has no custom message.
///
/// [`Issue::message_with`] calls it only when the issue carries no custom message, so an
/// implementation does not look for one. A closure `Fn(&Issue) -> String` is a resolver too.
pub trait MessageResolver {
    /// The sentence for `issue`.
    fn resolve(&self, issue: &Issue) -> String;
}

impl<F: Fn(&Issue) -> String> MessageResolver for F {
    fn resolve(&self, issue: &Issue) -> String {
        self(issue)
    }
}

/// A catalogue of message templates keyed by message key or code.
///
/// An issue is looked up by its message key first and by its code second, as Raoh for Java's
/// `ResourceBundleMessageResolver` does. A template's `{name}` placeholders are filled from the
/// issue's metadata; a template naming an entry the metadata lacks is passed over. When no
/// template fits, the sentence is `validation failed: <code>`.
///
/// ```
/// use raoh::{Issue, MessageResolver, Messages};
///
/// let issue = Issue::new("too_short").with_meta("min", 3);
/// assert_eq!(Messages::japanese().resolve(&issue), "3文字以上で入力してください");
///
/// let mine = Messages::english().with_overrides([("too_short", "{min}+ characters, please")]);
/// assert_eq!(mine.resolve(&issue), "3+ characters, please");
/// ```
#[derive(Clone, Debug, Default)]
pub struct Messages {
    templates: HashMap<String, String>,
}

static ENGLISH: LazyLock<Messages> = LazyLock::new(|| {
    Messages::from_properties(include_str!("messages/en.properties"))
        .expect("the English catalogue is well formed")
});

/// The Japanese templates over the English ones, as a `ResourceBundle` falls back to its parent.
static JAPANESE: LazyLock<Messages> = LazyLock::new(|| {
    let japanese = Messages::from_properties(include_str!("messages/ja.properties"))
        .expect("the Japanese catalogue is well formed");
    ENGLISH.with_overrides(japanese.templates)
});

impl Messages {
    /// The English catalogue. It is the one every issue's [`message`](Issue::message) comes
    /// from.
    pub fn english() -> &'static Messages {
        &ENGLISH
    }

    /// The Japanese catalogue, falling back to the English one for a key it lacks.
    pub fn japanese() -> &'static Messages {
        &JAPANESE
    }

    /// An empty catalogue, in which every issue reads `validation failed: <code>`.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Reads a catalogue written as Java's `Properties.load` reads a `.properties` file, such as
    /// the `messages*.properties` of Raoh for Java: one `raoh.<key>=<template>` per entry,
    /// `\uXXXX` escapes and continued lines included. The `raoh.` prefix is optional.
    pub fn from_properties(text: &str) -> Result<Self, PropertiesError> {
        let pairs = java::load_properties(text).map_err(|e| PropertiesError {
            line: e.line,
            reason: e.reason,
        })?;
        let templates = pairs
            .into_iter()
            .map(|(key, template)| match key.strip_prefix("raoh.") {
                Some(bare) => (bare.to_owned(), template),
                None => (key, template),
            })
            .collect();
        Ok(Self { templates })
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

    /// Every key and its template, in no particular order.
    pub fn templates(&self) -> impl Iterator<Item = (&str, &str)> {
        self.templates.iter().map(|(k, t)| (k.as_str(), t.as_str()))
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
            .unwrap_or_else(|| format!("validation failed: {}", issue.code()))
    }
}

/// Where [`Messages::from_properties`] stopped reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertiesError {
    line: usize,
    reason: &'static str,
}

impl PropertiesError {
    /// The line the malformed entry starts on, counting from 1.
    pub fn line(&self) -> usize {
        self.line
    }
}

impl fmt::Display for PropertiesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.reason)
    }
}

impl std::error::Error for PropertiesError {}

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

/// A metadata value as text, as Java's `String.valueOf` writes the value Raoh for Java holds:
/// strings without quotes, lists as `[a, b]`, maps as `{k=v}`, and a fractional number as
/// `Double.toString` does.
pub(crate) fn display(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_f64() => n
            .as_f64()
            .map_or_else(|| n.to_string(), java::double_to_string),
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
        let issue = Issue::new(codes::OUT_OF_RANGE)
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
        let issue =
            Issue::new(codes::OUT_OF_RANGE).with_message_key(message_keys::OUT_OF_RANGE_MINIMUM);
        assert_eq!(messages.resolve(&issue), "out of range");
    }

    #[test]
    fn a_refined_key_falls_back_to_its_code_in_a_java_catalogue() {
        let java = Messages::from_properties("raoh.invalid_format=bad form").unwrap();
        let issue =
            Issue::new(codes::INVALID_FORMAT).with_message_key(message_keys::INVALID_FORMAT_EMAIL);
        assert_eq!(java.resolve(&issue), "bad form");
    }

    #[test]
    fn lists_and_fractions_are_written_as_java_writes_them() {
        let issue = Issue::new(codes::NOT_ALLOWED).with_meta("allowed", vec!["a", "b"]);
        assert_eq!(Messages::english().resolve(&issue), "must be one of [a, b]");
        let issue = Issue::new(codes::OUT_OF_RANGE)
            .with_message_key(message_keys::OUT_OF_RANGE_MINIMUM)
            .with_meta("min", 1e7);
        assert_eq!(
            Messages::english().resolve(&issue),
            "must be at least 1.0E7"
        );
    }

    #[test]
    fn escaped_catalogues_read_as_java_reads_them() {
        let messages =
            Messages::from_properties("raoh.required=\\u5fc5\\u9808\nraoh.blank : empty").unwrap();
        assert_eq!(messages.template("required"), Some("必須"));
        assert_eq!(messages.template("blank"), Some("empty"));
        assert_eq!(Messages::from_properties("x=\\u12").unwrap_err().line(), 1);
    }

    #[test]
    fn a_closure_is_a_resolver() {
        let upper = |issue: &Issue| issue.code().to_uppercase();
        assert_eq!(upper.resolve(&Issue::new("blank")), "BLANK");
    }

    /// Every code and message key has a sentence in each catalogue, and the Japanese one does not
    /// fall back to English for any of them.
    #[test]
    fn the_catalogues_cover_every_code_and_message_key() {
        let english = Messages::from_properties(include_str!("messages/en.properties")).unwrap();
        let japanese = Messages::from_properties(include_str!("messages/ja.properties")).unwrap();
        for key in codes::ALL.iter().chain(message_keys::ALL) {
            assert!(
                english.template(key).is_some(),
                "no English template for {key}"
            );
            assert!(
                japanese.template(key).is_some(),
                "no Japanese template for {key}"
            );
        }
        let mut english_keys: Vec<&String> = english.templates.keys().collect();
        let mut japanese_keys: Vec<&String> = japanese.templates.keys().collect();
        english_keys.sort();
        japanese_keys.sort();
        assert_eq!(english_keys, japanese_keys);
    }
}
