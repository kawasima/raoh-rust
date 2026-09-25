//! Decoders over a [`serde_json::Value`].
//!
//! Bring everything into scope with `use raoh::json::prelude::*`.
//!
//! A member missing from an object and a member present as `null` are told apart: a missing
//! member is handed to its decoder as [`missing()`], a `null` that [`is_missing`] recognises. Both
//! fail a built-in decoder with `required`, but [`nullable`](JsonDecoderExt::nullable) accepts
//! only `null`, and [`presence_field`] reports which of the three it was.

mod bool;
mod choice;
#[cfg(feature = "decimal")]
mod decimal;
mod dict;
mod ext;
mod number;
mod object;
mod steps;
mod string;

pub use self::bool::{BoolDecoder, bool};
pub use choice::{
    Discriminate, EnumOf, Literal, Variant, Variants, discriminate, enum_of, literal, variant,
};
#[cfg(feature = "decimal")]
pub use decimal::{DecimalDecoder, decimal};
pub use dict::{Dict, dict};
pub use ext::{JsonDecoderExt, ListDecoder, Nullable};
pub use number::{F64Decoder, IntDecoder, Integer, SignedInteger, f64, i32, i64, u32, u64};
pub use object::{
    Field, FieldSet, Object, OptionalField, PresenceField, Strict, field, object, optional_field,
    presence_field,
};
#[cfg(feature = "url")]
pub use string::UrlDecoder;
#[cfg(feature = "uuid")]
pub use string::UuidDecoder;
pub use string::{Parse, StringDecoder, string};

use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::path::Path;
use crate::{codes, message_keys};
use serde_json::Value;

/// Everything needed to write decoders over JSON.
pub mod prelude {
    #[cfg(feature = "decimal")]
    pub use super::decimal;
    pub use super::{
        JsonDecoderExt, bool, dict, discriminate, enum_of, f64, field, from_str, i32, i64, literal,
        object, optional_field, presence_field, string, u32, u64, variant,
    };
    pub use crate::{BoxDecoder, Decoder, Issue, Issues, Presence, lazy, one_of};
    pub use serde_json::{Value, json};
}

static MISSING: Value = Value::Null;

/// The value a missing member is decoded from: a `null` that [`is_missing`] tells apart from a
/// `null` in the input.
pub fn missing() -> &'static Value {
    &MISSING
}

/// Whether `value` is [`missing()`] rather than a `null` read from the input.
pub fn is_missing(value: &Value) -> bool {
    std::ptr::eq(value, &MISSING)
}

/// Parses `text` as JSON and decodes it with `decoder`. Text that is not JSON is reported as one
/// `invalid_format` issue at the root, with the `line` and `column` where it stopped being JSON.
///
/// ```
/// use raoh::json::prelude::*;
///
/// let issues = from_str(&i64(), "{").unwrap_err();
/// assert_eq!(issues.iter().next().unwrap().code(), "invalid_format");
/// ```
pub fn from_str<D: Decoder<Value>>(decoder: &D, text: &str) -> Result<D::Output, Issues> {
    let value: Value = serde_json::from_str(text).map_err(|e| {
        Issue::new(codes::INVALID_FORMAT)
            .with_message_key(message_keys::INVALID_FORMAT_JSON)
            .with_meta("line", e.line())
            .with_meta("column", e.column())
    })?;
    decoder.decode(&value)
}

/// The JSON type of `value`, as Raoh names it in `actual`.
pub(crate) fn node_type(value: &Value) -> &'static str {
    match value {
        v if is_missing(v) => "missing",
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub(crate) fn required(path: &Path<'_>) -> Issue {
    Issue::at_path(path, codes::REQUIRED)
}

pub(crate) fn type_mismatch(path: &Path<'_>, expected: &'static str, found: &Value) -> Issue {
    Issue::at_path(path, codes::TYPE_MISMATCH)
        .with_meta("expected", expected)
        .with_meta("actual", node_type(found))
}

/// `required` for a missing or null value, `type_mismatch` for anything else.
pub(crate) fn unexpected(path: &Path<'_>, expected: &'static str, found: &Value) -> Issue {
    if found.is_null() {
        required(path)
    } else {
        type_mismatch(path, expected, found)
    }
}
