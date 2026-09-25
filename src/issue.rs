//! What a failed decode reports.

use crate::message::MessageResolver;
use crate::path::{Path, Pointer};
use indexmap::IndexMap;
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::fmt;

/// One problem found in the input: where it is, what kind it is, and what else its code says.
///
/// The `code` and `meta` are what a program reads; `message` is what a person reads. An issue is
/// created with an English message. A [`MessageResolver`] rewrites it in another language unless
/// the message was given by the caller as a custom one, which stays as written.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    inner: Box<Inner>,
}

/// Held behind a box so an `Issue`, and a `Result` whose error is one, stays one pointer wide.
#[derive(Clone, Debug, PartialEq)]
struct Inner {
    path: Pointer,
    code: Cow<'static, str>,
    message_key: Cow<'static, str>,
    message: String,
    meta: Map<String, Value>,
    custom_message: bool,
}

impl Issue {
    /// An issue at the root, with `message` as its English message.
    ///
    /// Returned from a function given to [`Decoder::and_then`](crate::Decoder::and_then), it is
    /// moved to the path the decoder is at.
    pub fn new(code: impl Into<Cow<'static, str>>, message: impl Into<String>) -> Self {
        let code = code.into();
        Self {
            inner: Box::new(Inner {
                path: Pointer::root(),
                message_key: code.clone(),
                code,
                message: message.into(),
                meta: Map::new(),
                custom_message: false,
            }),
        }
    }

    /// An issue at `path`.
    pub(crate) fn at_path(
        path: &Path<'_>,
        code: impl Into<Cow<'static, str>>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(code, message).at(path.to_pointer())
    }

    /// This issue at `path` instead.
    pub fn at(mut self, path: Pointer) -> Self {
        self.inner.path = path;
        self
    }

    /// This issue with `key` naming the constraint that produced it.
    pub fn with_message_key(mut self, key: impl Into<Cow<'static, str>>) -> Self {
        self.inner.message_key = key.into();
        self
    }

    /// This issue with one more entry of metadata.
    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.inner.meta.insert(key.into(), value.into());
        self
    }

    /// This issue with `message` as a custom message, which no resolver rewrites.
    pub fn with_custom_message(mut self, message: impl Into<String>) -> Self {
        self.inner.message = message.into();
        self.inner.custom_message = true;
        self
    }

    /// Where in the input the problem is.
    pub fn path(&self) -> &Pointer {
        &self.inner.path
    }

    /// What kind of problem it is, one of [`codes`](crate::codes) or a caller's own.
    pub fn code(&self) -> &str {
        &self.inner.code
    }

    /// The key a message catalogue looks up first: the code, or a refinement of it from
    /// [`message_keys`](crate::message_keys).
    pub fn message_key(&self) -> &str {
        &self.inner.message_key
    }

    /// The message a person reads.
    pub fn message(&self) -> &str {
        &self.inner.message
    }

    /// What else the code says about the problem, such as the bound a value fell outside of.
    pub fn meta(&self) -> &Map<String, Value> {
        &self.inner.meta
    }

    /// Whether the message was given by the caller and is kept by every resolver.
    pub fn is_custom_message(&self) -> bool {
        self.inner.custom_message
    }

    /// This issue read as relative to `prefix`.
    pub fn rebase(mut self, prefix: &Path<'_>) -> Self {
        if !prefix.is_root() {
            self.inner.path = self.inner.path.prefixed(prefix);
        }
        self
    }

    /// This issue with its message written by `resolver`, unless the message is a custom one.
    pub fn resolve(&self, resolver: &(impl MessageResolver + ?Sized)) -> Self {
        let mut issue = self.clone();
        if !issue.inner.custom_message {
            issue.inner.message = resolver.resolve(self);
        }
        issue
    }

    fn to_json_with(&self, resolver: &(impl MessageResolver + ?Sized)) -> Value {
        let message = if self.inner.custom_message {
            self.inner.message.clone()
        } else {
            resolver.resolve(self)
        };
        let mut object = Map::new();
        object.insert("path".into(), self.inner.path.to_string().into());
        object.insert("code".into(), self.inner.code.as_ref().into());
        object.insert("message".into(), message.into());
        object.insert("meta".into(), Value::Object(self.inner.meta.clone()));
        Value::Object(object)
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.inner.path.is_root() {
            write!(f, "(root): {}", self.inner.message)
        } else {
            write!(f, "{}: {}", self.inner.path, self.inner.message)
        }
    }
}

impl std::error::Error for Issue {}

impl Serialize for Issue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry("path", &self.inner.path.to_string())?;
        map.serialize_entry("code", self.inner.code.as_ref())?;
        map.serialize_entry("message", &self.inner.message)?;
        map.serialize_entry("meta", &self.inner.meta)?;
        map.end()
    }
}

/// Every issue a decode found, in the order it found them.
///
/// A decode that fails returns at least one issue.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Issues(Vec<Issue>);

impl Issues {
    /// No issues.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many there are.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Each issue in order.
    pub fn iter(&self) -> std::slice::Iter<'_, Issue> {
        self.0.iter()
    }

    /// The issues as a slice.
    pub fn as_slice(&self) -> &[Issue] {
        &self.0
    }

    /// Adds `issue` at the end.
    pub fn push(&mut self, issue: Issue) {
        self.0.push(issue);
    }

    /// Adds every issue of `other` at the end.
    pub fn merge(&mut self, other: Issues) {
        self.0.extend(other.0);
    }

    /// These issues read as relative to `prefix`.
    pub fn rebase(self, prefix: &Path<'_>) -> Self {
        if prefix.is_root() {
            return self;
        }
        Self(self.0.into_iter().map(|i| i.rebase(prefix)).collect())
    }

    /// These issues with their messages written by `resolver`, custom messages kept.
    pub fn resolve(&self, resolver: &(impl MessageResolver + ?Sized)) -> Self {
        Self(self.0.iter().map(|i| i.resolve(resolver)).collect())
    }

    /// The messages grouped by the JSON Pointer of their path, in the order each path was first
    /// seen. Messages are the ones the issues were created with, which are English.
    pub fn flatten(&self) -> IndexMap<String, Vec<String>> {
        let mut grouped: IndexMap<String, Vec<String>> = IndexMap::new();
        for issue in &self.0 {
            grouped
                .entry(issue.inner.path.to_string())
                .or_default()
                .push(issue.inner.message.clone());
        }
        grouped
    }

    /// As [`flatten`](Self::flatten), with the messages written by `resolver`.
    pub fn flatten_with(
        &self,
        resolver: &(impl MessageResolver + ?Sized),
    ) -> IndexMap<String, Vec<String>> {
        self.resolve(resolver).flatten()
    }

    /// The issues as a JSON array of `{"path", "code", "message", "meta"}` objects, the form Raoh
    /// for Java and PHP give them in. Messages are the ones the issues were created with.
    pub fn to_json(&self) -> Value {
        Value::Array(
            self.0
                .iter()
                .map(|i| serde_json::to_value(i).unwrap_or(Value::Null))
                .collect(),
        )
    }

    /// As [`to_json`](Self::to_json), with the messages written by `resolver`.
    pub fn to_json_with(&self, resolver: &(impl MessageResolver + ?Sized)) -> Value {
        Value::Array(self.0.iter().map(|i| i.to_json_with(resolver)).collect())
    }
}

impl From<Issue> for Issues {
    fn from(issue: Issue) -> Self {
        Self(vec![issue])
    }
}

impl From<Vec<Issue>> for Issues {
    fn from(issues: Vec<Issue>) -> Self {
        Self(issues)
    }
}

impl FromIterator<Issue> for Issues {
    fn from_iter<T: IntoIterator<Item = Issue>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl Extend<Issue> for Issues {
    fn extend<T: IntoIterator<Item = Issue>>(&mut self, iter: T) {
        self.0.extend(iter);
    }
}

impl IntoIterator for Issues {
    type Item = Issue;
    type IntoIter = std::vec::IntoIter<Issue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Issues {
    type Item = &'a Issue;
    type IntoIter = std::slice::Iter<'a, Issue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl fmt::Display for Issues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, issue) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            issue.fmt(f)?;
        }
        Ok(())
    }
}

impl std::error::Error for Issues {}

impl Serialize for Issues {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for issue in &self.0 {
            seq.serialize_element(issue)?;
        }
        seq.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codes;
    use serde_json::json;

    #[test]
    fn rebase_puts_the_prefix_before_the_issue_path() {
        let issue = Issue::new(codes::REQUIRED, "is required").at(["id"].into_iter().collect());
        let user = Path::ROOT.key("user");
        assert_eq!(issue.rebase(&user).path().to_string(), "/user/id");
    }

    #[test]
    fn flatten_keeps_the_order_paths_were_first_seen() {
        let issues: Issues = vec![
            Issue::new(codes::REQUIRED, "a1").at(["b"].into_iter().collect()),
            Issue::new(codes::REQUIRED, "b1").at(["a"].into_iter().collect()),
            Issue::new(codes::REQUIRED, "a2").at(["b"].into_iter().collect()),
        ]
        .into();
        let flat = issues.flatten();
        assert_eq!(flat.keys().collect::<Vec<_>>(), ["/b", "/a"]);
        assert_eq!(flat["/b"], ["a1", "a2"]);
    }

    #[test]
    fn serialize_is_the_same_as_to_json() {
        let issues: Issues = Issue::new(codes::TOO_SHORT, "must be at least 3 characters")
            .with_meta("min", 3)
            .at(["name"].into_iter().collect())
            .into();
        let expected = json!([{
            "path": "/name",
            "code": "too_short",
            "message": "must be at least 3 characters",
            "meta": {"min": 3}
        }]);
        assert_eq!(issues.to_json(), expected);
        assert_eq!(serde_json::to_value(&issues).unwrap(), expected);
    }
}
