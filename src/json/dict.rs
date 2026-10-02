use super::ext::{max_size_issue, min_size_issue, non_empty_issue, size_issue};
use super::steps::Steps;
use super::unexpected;
use crate::decoder::Decoder;
use crate::issue::Issues;
use crate::path::Path;
use indexmap::IndexMap;
use serde_json::Value;

/// A decoder of a JSON object used as a map, whose every member `value` reads.
///
/// Missing or `null` is `required`; any other type is `type_mismatch`. The issues of every member
/// are reported, each under its key. Constraints on the size of the map run once every member has
/// decoded, in the order they are written, and the first to fail is the one reported.
///
/// The map keeps the members in the order the [`Value`] keeps its keys: the order of the input
/// with `serde_json`'s `preserve_order` feature, and sorted without it.
///
/// ```
/// use raoh::json::prelude::*;
///
/// let stock = dict(u32()).non_empty().decode(&json!({"apple": 3, "pear": 0})).unwrap();
/// assert_eq!(stock["apple"], 3);
/// ```
pub fn dict<D: Decoder<Value>>(value: D) -> Dict<D> {
    Dict {
        value,
        steps: Steps::default(),
    }
}

/// The decoder [`dict`] returns.
pub struct Dict<D: Decoder<Value>> {
    value: D,
    steps: Steps<IndexMap<String, D::Output>>,
}

impl<D: Decoder<Value> + Clone> Clone for Dict<D> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            steps: self.steps.clone(),
        }
    }
}

impl<D: Decoder<Value> + std::fmt::Debug> std::fmt::Debug for Dict<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dict")
            .field("value", &self.value)
            .field("steps", &self.steps)
            .finish()
    }
}

impl<D: Decoder<Value>> Decoder<Value> for Dict<D> {
    type Output = IndexMap<String, D::Output>;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<Self::Output, Issues> {
        let Value::Object(members) = input else {
            return Err(self.steps.base_issue(unexpected(path, "object", input)));
        };
        let mut values = IndexMap::with_capacity(members.len());
        let mut issues = Issues::new();
        for (key, member) in members {
            match self.value.decode_at(member, &path.key(key)) {
                Ok(value) => {
                    values.insert(key.clone(), value);
                }
                Err(found) => issues.merge(found),
            }
        }
        if !issues.is_empty() {
            return Err(issues);
        }
        self.steps.run(values, path)
    }
}

impl<D: Decoder<Value>> Dict<D>
where
    D::Output: 'static,
{
    /// Gives the most recent constraint written before this, or the type check when there is
    /// none, a custom message that every language shows as written.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.steps.set_message(message.into());
        self
    }

    /// Requires at least one member: `too_small` with `min` 1 and `actual` 0.
    pub fn non_empty(mut self) -> Self {
        self.steps
            .require(|members| !members.is_empty(), |_| non_empty_issue());
        self
    }

    /// Requires at least `n` members: `too_small` with `min` and `actual`.
    pub fn min_size(mut self, n: usize) -> Self {
        self.steps.require(
            move |members| members.len() >= n,
            move |members| min_size_issue(n, members.len()),
        );
        self
    }

    /// Allows at most `n` members: `too_big` with `max` and `actual`.
    pub fn max_size(mut self, n: usize) -> Self {
        self.steps.require(
            move |members| members.len() <= n,
            move |members| max_size_issue(n, members.len()),
        );
        self
    }

    /// Requires exactly `n` members: `invalid_size` with `expected` and `actual`.
    pub fn size(mut self, n: usize) -> Self {
        self.steps.require(
            move |members| members.len() == n,
            move |members| size_issue(n, members.len()),
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::i64;
    use serde_json::json;

    #[test]
    fn members_keep_the_order_of_the_value() {
        let input: Value = serde_json::from_str(r#"{"b": 1, "a": 2, "c": 3}"#).unwrap();
        let decoded = dict(i64()).decode(&input).unwrap();
        let order: Vec<&String> = decoded.keys().collect();
        let expected: Vec<&String> = input.as_object().unwrap().keys().collect();
        assert_eq!(order, expected);
        assert!(dict(i64()).decode(&json!([])).is_err());
    }

    #[test]
    fn size_constraints_count_members() {
        let issues = dict(i64()).size(2).decode(&json!({"a": 1})).unwrap_err();
        let issue = issues.iter().next().unwrap();
        assert_eq!(issue.code(), "invalid_size");
        assert_eq!(issue.message(), "must have exactly 2 elements");
    }
}
