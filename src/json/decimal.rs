use super::steps::Steps;
use super::{node_type, required};
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::path::Path;
use crate::{codes, message_keys};
use rust_decimal::Decimal;
use serde_json::Value;
use std::ops::RangeInclusive;
use std::str::FromStr;

/// A decoder of a JSON number into a [`Decimal`].
///
/// The number is read from its text as `serde_json` keeps it. Without `serde_json`'s
/// `arbitrary_precision` feature that text is the one of the nearest `f64`, so `1.10` is read as
/// `1.1`; enable the feature in the application for exact decimals.
///
/// Missing or `null` is `required`; any other type, and a number `Decimal` cannot hold, is
/// `type_mismatch` with `expected` `number`.
///
/// Bounds are carried in `meta` as JSON numbers, which a message writes as Java writes a
/// `double`. That matches how Raoh for Java writes a `BigDecimal` for values from 0.001 up to
/// 10⁷; outside that range a fractional bound is written in exponent form, such as `1.50000005E7` for
/// what Java writes `15000000.5`.
#[derive(Clone, Debug, Default)]
pub struct DecimalDecoder {
    steps: Steps<Decimal>,
}

/// A decoder of a JSON number into a [`Decimal`].
pub fn decimal() -> DecimalDecoder {
    DecimalDecoder::default()
}

fn read(n: &serde_json::Number) -> Option<Decimal> {
    let text = n.to_string();
    Decimal::from_str(&text)
        .or_else(|_| Decimal::from_scientific(&text))
        .ok()
}

/// A decimal as a JSON number, or as text where `serde_json` cannot hold it as one.
fn to_json(d: Decimal) -> Value {
    serde_json::from_str::<Value>(&d.to_string()).unwrap_or_else(|_| Value::String(d.to_string()))
}

impl Decoder<Value> for DecimalDecoder {
    type Output = Decimal;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<Decimal, Issues> {
        let found = match input {
            Value::Number(n) => read(n).ok_or_else(|| {
                Issue::at_path(path, codes::TYPE_MISMATCH).with_meta("expected", "number")
            }),
            Value::Null => Err(required(path)),
            other => Err(Issue::at_path(path, codes::TYPE_MISMATCH)
                .with_meta("expected", "number")
                .with_meta("actual", node_type(other))),
        };
        let value = found.map_err(|issue| self.steps.base_issue(issue))?;
        self.steps.run(value, path)
    }
}

impl DecimalDecoder {
    fn bound(
        mut self,
        ok: impl Fn(Decimal) -> bool + Send + Sync + 'static,
        key: &'static str,
        bounds: Vec<(&'static str, Decimal)>,
    ) -> Self {
        self.steps.require(
            move |v| ok(*v),
            move |v| {
                bounds
                    .iter()
                    .fold(
                        Issue::new(codes::OUT_OF_RANGE).with_message_key(key),
                        |issue, (name, bound)| issue.with_meta(*name, to_json(*bound)),
                    )
                    .with_meta("actual", to_json(*v))
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

    /// Requires at least `min`: `out_of_range` with `min` and `actual`.
    pub fn min(self, min: Decimal) -> Self {
        self.bound(
            move |v| v >= min,
            message_keys::OUT_OF_RANGE_MINIMUM,
            vec![("min", min)],
        )
    }

    /// Allows at most `max`: `out_of_range` with `max` and `actual`.
    pub fn max(self, max: Decimal) -> Self {
        self.bound(
            move |v| v <= max,
            message_keys::OUT_OF_RANGE_MAXIMUM,
            vec![("max", max)],
        )
    }

    /// Requires a value within `range`, both ends included: `out_of_range` with `min`, `max` and
    /// `actual`.
    pub fn range(self, range: RangeInclusive<Decimal>) -> Self {
        let (min, max) = range.into_inner();
        self.bound(
            move |v| min <= v && v <= max,
            message_keys::OUT_OF_RANGE_RANGE,
            vec![("min", min), ("max", max)],
        )
    }

    /// Requires a value above zero: `out_of_range` with `min` 0 and `actual`.
    pub fn positive(self) -> Self {
        self.bound(
            |v| v > Decimal::ZERO,
            message_keys::OUT_OF_RANGE_POSITIVE,
            vec![("min", Decimal::ZERO)],
        )
    }

    /// Requires a value below zero: `out_of_range` with `max` 0 and `actual`.
    pub fn negative(self) -> Self {
        self.bound(
            |v| v < Decimal::ZERO,
            message_keys::OUT_OF_RANGE_NEGATIVE,
            vec![("max", Decimal::ZERO)],
        )
    }

    /// Requires zero or above: `out_of_range` with `min` 0 and `actual`.
    pub fn non_negative(self) -> Self {
        self.bound(
            |v| v >= Decimal::ZERO,
            message_keys::OUT_OF_RANGE_NON_NEGATIVE,
            vec![("min", Decimal::ZERO)],
        )
    }

    /// Requires zero or below: `out_of_range` with `max` 0 and `actual`.
    pub fn non_positive(self) -> Self {
        self.bound(
            |v| v <= Decimal::ZERO,
            message_keys::OUT_OF_RANGE_NON_POSITIVE,
            vec![("max", Decimal::ZERO)],
        )
    }

    /// Requires a multiple of `divisor`: `not_multiple_of` with `divisor` and `actual`.
    ///
    /// # Panics
    ///
    /// When `divisor` is zero.
    pub fn multiple_of(mut self, divisor: Decimal) -> Self {
        assert!(!divisor.is_zero(), "divisor must not be zero");
        self.steps.require(
            move |v| (*v % divisor).is_zero(),
            move |v| {
                Issue::new(codes::NOT_MULTIPLE_OF)
                    .with_meta("divisor", to_json(divisor))
                    .with_meta("actual", to_json(*v))
            },
        );
        self
    }

    /// Allows at most `digits` digits after the decimal point: `invalid_scale` with `maxScale` and
    /// `actualScale`.
    pub fn scale(mut self, digits: u32) -> Self {
        self.steps.require(
            move |v| v.scale() <= digits,
            move |v| {
                Issue::new(codes::INVALID_SCALE)
                    .with_meta("maxScale", digits)
                    .with_meta("actualScale", v.scale())
            },
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scale_counts_fraction_digits() {
        let issues = decimal().scale(1).decode(&json!(1.25)).unwrap_err();
        let issue = issues.iter().next().unwrap();
        assert_eq!(issue.code(), "invalid_scale");
        assert_eq!(issue.meta()["actualScale"], 2);
    }

    #[test]
    fn positive_reports_zero_as_the_bound() {
        let issues = decimal().positive().decode(&json!(0)).unwrap_err();
        assert_eq!(issues.iter().next().unwrap().meta()["min"], 0);
    }

    #[test]
    fn exponents_are_read() {
        assert_eq!(decimal().decode(&json!(1e2)).unwrap(), Decimal::from(100));
    }
}
