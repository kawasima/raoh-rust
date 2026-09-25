use super::steps::Steps;
use super::{is_missing, unexpected};
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::message::display;
use crate::path::Path;
use crate::{codes, message_keys};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::hash::Hash;

/// What every decoder over a JSON value can also do.
pub trait JsonDecoderExt: Decoder<Value> + Sized {
    /// A decoder that gives `None` for `null` and `Some` of this decoder's output otherwise.
    ///
    /// A missing member is not `null`: it is still handed to this decoder, which reports it as
    /// `required`. To accept a member that may be left out, use
    /// [`optional_field`](super::optional_field).
    fn nullable(self) -> Nullable<Self> {
        Nullable(self)
    }

    /// A decoder of a JSON array whose every element this decoder reads.
    ///
    /// Missing or `null` is `required`; any other type is `type_mismatch`. The issues of every
    /// element are reported, each under its index.
    fn list(self) -> ListDecoder<Self> {
        ListDecoder {
            element: self,
            steps: Steps::default(),
        }
    }
}

impl<D: Decoder<Value>> JsonDecoderExt for D {}

/// The decoder [`JsonDecoderExt::nullable`] returns.
#[derive(Clone, Copy, Debug)]
pub struct Nullable<D>(D);

impl<D: Decoder<Value>> Decoder<Value> for Nullable<D> {
    type Output = Option<D::Output>;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<Self::Output, Issues> {
        if input.is_null() && !is_missing(input) {
            Ok(None)
        } else {
            self.0.decode_at(input, path).map(Some)
        }
    }
}

/// The decoder [`JsonDecoderExt::list`] returns.
///
/// Constraints on the whole list run once every element has decoded, in the order they are
/// written, and the first to fail is the one reported.
pub struct ListDecoder<D: Decoder<Value>> {
    element: D,
    steps: Steps<Vec<D::Output>>,
}

impl<D: Decoder<Value> + Clone> Clone for ListDecoder<D> {
    fn clone(&self) -> Self {
        Self {
            element: self.element.clone(),
            steps: self.steps.clone(),
        }
    }
}

impl<D: Decoder<Value> + std::fmt::Debug> std::fmt::Debug for ListDecoder<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListDecoder")
            .field("element", &self.element)
            .field("steps", &self.steps)
            .finish()
    }
}

impl<D: Decoder<Value>> Decoder<Value> for ListDecoder<D> {
    type Output = Vec<D::Output>;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<Self::Output, Issues> {
        let Value::Array(items) = input else {
            return Err(self.steps.base_issue(unexpected(path, "array", input)));
        };
        let mut values = Vec::with_capacity(items.len());
        let mut issues = Issues::new();
        for (i, item) in items.iter().enumerate() {
            match self.element.decode_at(item, &path.index(i)) {
                Ok(value) => values.push(value),
                Err(found) => issues.merge(found),
            }
        }
        if !issues.is_empty() {
            return Err(issues);
        }
        self.steps.run(values, path)
    }
}

impl<D: Decoder<Value>> ListDecoder<D>
where
    D::Output: 'static,
{
    /// Gives the constraint written just before this, or the type check when there is none, a
    /// custom message that no resolver rewrites.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.steps.set_message(message.into());
        self
    }

    /// Requires at least one element: `too_small` with `min` 1 and `actual` 0.
    pub fn non_empty(mut self) -> Self {
        self.steps.require(
            |items| !items.is_empty(),
            |_| {
                Issue::new(codes::TOO_SMALL, "must not be empty")
                    .with_message_key(message_keys::TOO_SMALL_NONEMPTY)
                    .with_meta("min", 1)
                    .with_meta("actual", 0)
            },
        );
        self
    }

    /// Requires at least `n` elements: `too_small` with `min` and `actual`.
    pub fn min_size(mut self, n: usize) -> Self {
        self.steps.require(
            move |items| items.len() >= n,
            move |items| {
                Issue::new(codes::TOO_SMALL, format!("must have at least {n} elements"))
                    .with_meta("min", n)
                    .with_meta("actual", items.len())
            },
        );
        self
    }

    /// Allows at most `n` elements: `too_big` with `max` and `actual`.
    pub fn max_size(mut self, n: usize) -> Self {
        self.steps.require(
            move |items| items.len() <= n,
            move |items| {
                Issue::new(codes::TOO_BIG, format!("must have at most {n} elements"))
                    .with_meta("max", n)
                    .with_meta("actual", items.len())
            },
        );
        self
    }

    /// Requires exactly `n` elements: `invalid_size` with `expected` and `actual`.
    pub fn size(mut self, n: usize) -> Self {
        self.steps.require(
            move |items| items.len() == n,
            move |items| {
                Issue::new(
                    codes::INVALID_SIZE,
                    format!("must have exactly {n} elements"),
                )
                .with_meta("expected", n)
                .with_meta("actual", items.len())
            },
        );
        self
    }

    /// Requires no element to appear twice: `duplicate_element` with each repeated element once,
    /// in the order its repetition was found, as `duplicates`.
    pub fn unique(mut self) -> Self
    where
        D::Output: Eq + Hash + Serialize,
    {
        self.steps.require(
            |items| {
                let mut seen = HashSet::with_capacity(items.len());
                items.iter().all(|item| seen.insert(item))
            },
            |items| {
                let mut seen = HashSet::with_capacity(items.len());
                let mut repeated = HashSet::new();
                let mut duplicates: Vec<Value> = Vec::new();
                for item in items {
                    if !seen.insert(item) && repeated.insert(item) {
                        duplicates.push(serde_json::to_value(item).unwrap_or(Value::Null));
                    }
                }
                let listed = display(&Value::Array(duplicates.clone()));
                Issue::new(
                    codes::DUPLICATE_ELEMENT,
                    format!("must not contain duplicates: {listed}"),
                )
                .with_meta("duplicates", duplicates)
            },
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{i64, missing, string};
    use serde_json::json;

    #[test]
    fn nullable_accepts_null_but_not_missing() {
        let decoder = string().nullable();
        assert_eq!(decoder.decode(&Value::Null).unwrap(), None);
        let issues = decoder.decode(missing()).unwrap_err();
        assert_eq!(issues.iter().next().unwrap().code(), "required");
    }

    #[test]
    fn every_element_issue_is_reported_under_its_index() {
        let issues = i64().list().decode(&json!([1, "a", 3, true])).unwrap_err();
        let paths: Vec<String> = issues.iter().map(|i| i.path().to_string()).collect();
        assert_eq!(paths, ["/1", "/3"]);
    }

    #[test]
    fn unique_reports_each_duplicate_once() {
        let issues = i64()
            .list()
            .unique()
            .decode(&json!([1, 2, 1, 2, 1]))
            .unwrap_err();
        let issue = issues.iter().next().unwrap();
        assert_eq!(issue.meta()["duplicates"], json!([1, 2]));
        assert_eq!(issue.message(), "must not contain duplicates: [1, 2]");
    }

    #[test]
    fn non_empty_uses_its_own_message_key() {
        let issues = i64().list().non_empty().decode(&json!([])).unwrap_err();
        assert_eq!(
            issues.iter().next().unwrap().message_key(),
            "too_small.nonempty"
        );
    }
}
