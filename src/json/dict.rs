use super::unexpected;
use crate::decoder::Decoder;
use crate::issue::Issues;
use crate::path::Path;
use indexmap::IndexMap;
use serde_json::Value;

/// A decoder of a JSON object used as a map, whose every member `value` reads.
///
/// Missing or `null` is `required`; any other type is `type_mismatch`. The issues of every member
/// are reported, each under its key.
///
/// The map keeps the members in the order the [`Value`] keeps its keys: the order of the input
/// with `serde_json`'s `preserve_order` feature, as Raoh for Java keeps it, and sorted without
/// it.
///
/// ```
/// use raoh::json::prelude::*;
///
/// let stock = dict(u32()).decode(&json!({"apple": 3, "pear": 0})).unwrap();
/// assert_eq!(stock["apple"], 3);
/// ```
pub fn dict<D>(value: D) -> Dict<D> {
    Dict(value)
}

/// The decoder [`dict`] returns.
#[derive(Clone, Copy, Debug)]
pub struct Dict<D>(D);

impl<D: Decoder<Value>> Decoder<Value> for Dict<D> {
    type Output = IndexMap<String, D::Output>;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<Self::Output, Issues> {
        let Value::Object(members) = input else {
            return Err(unexpected(path, "object", input).into());
        };
        let mut values = IndexMap::with_capacity(members.len());
        let mut issues = Issues::new();
        for (key, member) in members {
            match self.0.decode_at(member, &path.key(key)) {
                Ok(value) => {
                    values.insert(key.clone(), value);
                }
                Err(found) => issues.merge(found),
            }
        }
        if issues.is_empty() {
            Ok(values)
        } else {
            Err(issues)
        }
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
}
