use super::steps::Steps;
use super::unexpected;
use crate::codes;
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::path::Path;
use serde_json::Value;

/// A decoder of a JSON boolean.
///
/// Missing or `null` is `required`; any other type is `type_mismatch`.
#[derive(Clone, Debug, Default)]
pub struct BoolDecoder {
    steps: Steps<bool>,
}

/// A decoder of a JSON boolean.
pub fn bool() -> BoolDecoder {
    BoolDecoder::default()
}

impl Decoder<Value> for BoolDecoder {
    type Output = bool;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<bool, Issues> {
        match input {
            Value::Bool(b) => self.steps.run(*b, path),
            other => Err(self.steps.base_issue(unexpected(path, "boolean", other))),
        }
    }
}

impl BoolDecoder {
    fn exactly(mut self, expected: bool) -> Self {
        self.steps.require(
            move |b| *b == expected,
            move |b| {
                Issue::new(codes::INVALID_VALUE)
                    .with_meta("expected", expected)
                    .with_meta("actual", *b)
            },
        );
        self
    }

    /// Gives the most recent constraint written before this, or the type check when there is
    /// none, a custom message that every language shows as written.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.steps.set_message(message.into());
        self
    }

    /// Requires `true`, as agreeing to terms does: `invalid_value` with `expected` and `actual`.
    pub fn is_true(self) -> Self {
        self.exactly(true)
    }

    /// Requires `false`: `invalid_value` with `expected` and `actual`.
    pub fn is_false(self) -> Self {
        self.exactly(false)
    }
}
