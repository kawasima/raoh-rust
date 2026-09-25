use super::unexpected;
use crate::decoder::Decoder;
use crate::issue::Issues;
use crate::path::Path;
use serde_json::Value;
use std::collections::HashMap;

/// A decoder of a JSON object used as a map, whose every member `value` reads.
///
/// Missing or `null` is `required`; any other type is `type_mismatch`. The issues of every member
/// are reported, each under its key.
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
    type Output = HashMap<String, D::Output>;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<Self::Output, Issues> {
        let Value::Object(members) = input else {
            return Err(unexpected(path, "object", input).into());
        };
        let mut values = HashMap::with_capacity(members.len());
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
