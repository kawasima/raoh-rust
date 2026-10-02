//! Decoders over a [`serde_json::Value`].
//!
//! Bring everything into scope with `use raoh::json::prelude::*`.
//!
//! A member missing from an object and a member present as `null` are told apart: a missing
//! member is handed to its decoder as [`missing()`], a `null` that [`is_missing`] recognises. Both
//! fail a built-in decoder with `required`, but [`nullable`](JsonDecoderExt::nullable) accepts
//! only `null`, and [`presence_field`] reports which of the three it was.
//!
//! # Numbers
//!
//! A decoder reads a number from the text its [`Value`] holds, as the Raoh Specification's input
//! model has it: `int` takes `1` and refuses `1.0`, `decimal` gives `1.50` with scale 2, and
//! `double` gives -0 for `-0.0`. `serde_json` keeps that text only with its `arbitrary_precision`
//! feature, which this crate's feature of the same name turns on. Without it a number is the
//! `i64`, `u64` or `f64` `serde_json` read, and is read as the text that value is written as:
//! `1.50` is read as `1.5`, `-0` as `-0.0`, which `int` refuses, and an integer past `u64` as a
//! float. The results are then `serde_json`'s reading of the input, not the input's.
//!
//! Even with the feature, `serde_json`'s parser, which [`from_str`] and `serde_json::from_str`
//! use, reads an integer as an `i64` or `u64` before keeping its text, so the text `-0` becomes
//! `0` and `double` gives +0 for it. A `Value` can hold the text `-0`, and a decoder given such a
//! value gives -0; it is the parser that loses the sign.

mod bool;
mod choice;
mod decimal;
mod dict;
mod ext;
pub(crate) mod ip;
mod number;
mod object;
mod steps;
mod string;
mod temporal;
mod text;

pub use self::bool::{BoolDecoder, bool};
pub use choice::{
    Discriminate, DiscriminateBy, EnumOf, Literal, Variant, Variants, discriminate,
    discriminate_by, enum_of, literal, variant,
};
pub use decimal::{DecimalDecoder, decimal};
pub use dict::{Dict, dict};
pub use ext::{JsonDecoderExt, ListDecoder, Nullable, ToSet};
pub use number::{
    F32Decoder, F64Decoder, FloatDecoder, IntDecoder, Integer, SignedInteger, f32, f64, i32, i64,
    u32, u64,
};
pub use object::{
    BoxFieldSet, Field, FieldSet, Flat, MapFields, Object, OptionalField, PresenceField, Strict,
    StrictMembers, field, flat, object, optional_field, presence_field, strict,
};
pub use string::{NormalizationForm, Parse, StringDecoder, UriDecoder, UuidDecoder, string};
pub use temporal::TemporalDecoder;

use crate::decoder::{Decoder, Nullish};
use crate::issue::{Issue, Issues};
use crate::path::Path;
use crate::{codes, message_keys};
use serde_json::Value;

/// Everything needed to write decoders over JSON.
pub mod prelude {
    pub use super::{
        JsonDecoderExt, bool, decimal, dict, discriminate, discriminate_by, enum_of, f32, f64,
        field, flat, from_str, i32, i64, literal, object, optional_field, presence_field, strict,
        string, u32, u64, variant,
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

/// A JSON null, or a missing member, which is handed to a decoder as [`missing()`].
impl Nullish for Value {
    fn is_null_or_missing(&self) -> bool {
        self.is_null()
    }
}

/// Parses `text` as JSON and decodes it with `decoder`. Text that is not JSON is reported as one
/// `invalid_format` issue at the root, under the message key `invalid_format.json`, with the
/// `line` and `column` where it stopped being JSON.
///
/// The text is read by `serde_json`, which reads the number `-0` as `0` (see
/// [Numbers](self#numbers)).
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

/// The kind of `value`, as `type_mismatch` names it in `actual`.
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

/// The text a number was written with, or with `serde_json`'s `arbitrary_precision` off, the text
/// of the value it was read as.
pub(crate) fn lexeme(n: &serde_json::Number) -> String {
    n.to_string()
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
