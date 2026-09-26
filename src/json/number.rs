use super::steps::Steps;
use super::{node_type, required};
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::path::Path;
use crate::{codes, message_keys};
use serde_json::Value;
use std::ops::RangeInclusive;

mod sealed {
    pub trait Sealed {}
    impl Sealed for i32 {}
    impl Sealed for i64 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
}

/// An integer type a JSON number can be decoded into. It cannot be implemented outside this
/// crate.
pub trait Integer: sealed::Sealed + Copy + Ord + Into<Value> + Send + Sync + 'static {
    /// What an issue names the type in `expected`, as Raoh for Java does.
    const EXPECTED: &'static str;
    /// The smallest positive value.
    const ONE: Self;

    /// `value` as this type, if it holds it.
    fn from_integer(value: i128) -> Option<Self>;

    /// Whether `self` is a multiple of `divisor`, which is not zero.
    fn is_multiple_of(self, divisor: Self) -> bool;

    /// Whether `self` is zero.
    fn is_zero(self) -> bool;
}

/// An integer type that holds negative values. It cannot be implemented outside this crate.
pub trait SignedInteger: Integer {
    /// Zero.
    const ZERO: Self;
    /// The largest negative value.
    const MINUS_ONE: Self;
}

/// The integer a JSON number is, or `None` when it is not one: when it has a fraction or an
/// exponent, or is too large for `serde_json` to keep as an integer.
///
/// `serde_json` reads the text `-0` as the float `-0.0`; it is read here as the integer 0, as
/// Jackson reads it. The text `-0.0` gives the same float and is read as 0 too, where Raoh for
/// Java rejects it.
fn integral(n: &serde_json::Number) -> Option<i128> {
    n.as_i64()
        .map(i128::from)
        .or_else(|| n.as_u64().map(i128::from))
        .or_else(|| {
            n.as_f64()
                .filter(|f| n.is_f64() && *f == 0.0 && f.is_sign_negative())
                .map(|_| 0)
        })
}

macro_rules! integer {
    ($t:ty, $expected:literal) => {
        impl Integer for $t {
            const EXPECTED: &'static str = $expected;
            const ONE: Self = 1;

            fn from_integer(value: i128) -> Option<Self> {
                <$t>::try_from(value).ok()
            }

            fn is_multiple_of(self, divisor: Self) -> bool {
                self.checked_rem(divisor).is_none_or(|r| r == 0)
            }

            fn is_zero(self) -> bool {
                self == 0
            }
        }
    };
}

integer!(i32, "integer");
integer!(i64, "long");
integer!(u32, "integer");
integer!(u64, "long");

impl SignedInteger for i32 {
    const ZERO: Self = 0;
    const MINUS_ONE: Self = -1;
}

impl SignedInteger for i64 {
    const ZERO: Self = 0;
    const MINUS_ONE: Self = -1;
}

/// A decoder of a JSON integer into `T`.
///
/// Missing or `null` is `required`. A value of another type, and a number with a fraction or an
/// exponent, is `type_mismatch` with the type found as `actual` (`number` for such a number). An
/// integer `T` cannot hold is `type_mismatch` under the message key
/// `type_mismatch.numeric_range`, with `expected` alone, as Raoh for Java reports it.
/// Constraints run in the order they are written, and the first to fail is the one reported.
#[derive(Clone, Debug)]
pub struct IntDecoder<T> {
    steps: Steps<T>,
}

impl<T> Default for IntDecoder<T> {
    fn default() -> Self {
        Self {
            steps: Steps::default(),
        }
    }
}

/// A decoder of a JSON integer into an `i32`.
pub fn i32() -> IntDecoder<i32> {
    IntDecoder::default()
}

/// A decoder of a JSON integer into an `i64`.
pub fn i64() -> IntDecoder<i64> {
    IntDecoder::default()
}

/// A decoder of a JSON integer into a `u32`.
pub fn u32() -> IntDecoder<u32> {
    IntDecoder::default()
}

/// A decoder of a JSON integer into a `u64`.
pub fn u64() -> IntDecoder<u64> {
    IntDecoder::default()
}

fn type_mismatch(path: &Path<'_>, expected: &'static str) -> Issue {
    Issue::at_path(path, codes::TYPE_MISMATCH).with_meta("expected", expected)
}

impl<T: Integer> Decoder<Value> for IntDecoder<T> {
    type Output = T;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<T, Issues> {
        let found = match input {
            Value::Number(n) => match integral(n) {
                Some(value) => T::from_integer(value).ok_or_else(|| {
                    type_mismatch(path, T::EXPECTED)
                        .with_message_key(message_keys::TYPE_MISMATCH_NUMERIC_RANGE)
                }),
                None => Err(type_mismatch(path, T::EXPECTED).with_meta("actual", "number")),
            },
            Value::Null => Err(required(path)),
            other => Err(type_mismatch(path, T::EXPECTED).with_meta("actual", node_type(other))),
        };
        let value = found.map_err(|issue| self.steps.base_issue(issue))?;
        self.steps.run(value, path)
    }
}

fn out_of_range(key: &'static str) -> Issue {
    Issue::new(codes::OUT_OF_RANGE).with_message_key(key)
}

impl<T: Integer> IntDecoder<T> {
    /// Gives the most recent constraint written before this, or the type check when there is
    /// none, a custom message that every language shows as written.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.steps.set_message(message.into());
        self
    }

    /// Requires at least `min`: `out_of_range` with `min` and `actual`.
    pub fn min(mut self, min: T) -> Self {
        self.steps.require(
            move |v| *v >= min,
            move |v| {
                out_of_range(message_keys::OUT_OF_RANGE_MINIMUM)
                    .with_meta("min", min)
                    .with_meta("actual", *v)
            },
        );
        self
    }

    /// Allows at most `max`: `out_of_range` with `max` and `actual`.
    pub fn max(mut self, max: T) -> Self {
        self.steps.require(
            move |v| *v <= max,
            move |v| {
                out_of_range(message_keys::OUT_OF_RANGE_MAXIMUM)
                    .with_meta("max", max)
                    .with_meta("actual", *v)
            },
        );
        self
    }

    /// Requires a value within `range`, both ends included: `out_of_range` with `min`, `max` and
    /// `actual`.
    pub fn range(mut self, range: RangeInclusive<T>) -> Self {
        let (min, max) = range.into_inner();
        self.steps.require(
            move |v| min <= *v && *v <= max,
            move |v| {
                out_of_range(message_keys::OUT_OF_RANGE_RANGE)
                    .with_meta("min", min)
                    .with_meta("max", max)
                    .with_meta("actual", *v)
            },
        );
        self
    }

    /// Requires a value above zero: `out_of_range` with `min` 1 and `actual`.
    pub fn positive(mut self) -> Self {
        self.steps.require(
            |v| *v >= T::ONE,
            |v| {
                out_of_range(message_keys::OUT_OF_RANGE_POSITIVE)
                    .with_meta("min", T::ONE)
                    .with_meta("actual", *v)
            },
        );
        self
    }

    /// Requires a multiple of `divisor`: `not_multiple_of` with `divisor` and `actual`.
    ///
    /// # Panics
    ///
    /// When `divisor` is zero.
    pub fn multiple_of(mut self, divisor: T) -> Self {
        assert!(!divisor.is_zero(), "divisor must not be zero");
        self.steps.require(
            move |v| v.is_multiple_of(divisor),
            move |v| {
                Issue::new(codes::NOT_MULTIPLE_OF)
                    .with_meta("divisor", divisor)
                    .with_meta("actual", *v)
            },
        );
        self
    }

    /// Requires one of `allowed`: `not_allowed` with the sorted `allowed` and `actual`.
    pub fn one_of(mut self, allowed: impl IntoIterator<Item = T>) -> Self {
        let mut allowed: Vec<T> = allowed.into_iter().collect();
        allowed.sort();
        allowed.dedup();
        let check = allowed.clone();
        self.steps.require(
            move |v| check.binary_search(v).is_ok(),
            move |v| {
                Issue::new(codes::NOT_ALLOWED)
                    .with_meta(
                        "allowed",
                        allowed.iter().map(|a| (*a).into()).collect::<Vec<Value>>(),
                    )
                    .with_meta("actual", *v)
            },
        );
        self
    }
}

impl<T: SignedInteger> IntDecoder<T> {
    /// Requires a value below zero: `out_of_range` with `max` -1 and `actual`.
    pub fn negative(mut self) -> Self {
        self.steps.require(
            |v| *v <= T::MINUS_ONE,
            |v| {
                out_of_range(message_keys::OUT_OF_RANGE_NEGATIVE)
                    .with_meta("max", T::MINUS_ONE)
                    .with_meta("actual", *v)
            },
        );
        self
    }

    /// Requires zero or above: `out_of_range` with `min` 0 and `actual`.
    pub fn non_negative(mut self) -> Self {
        self.steps.require(
            |v| *v >= T::ZERO,
            |v| {
                out_of_range(message_keys::OUT_OF_RANGE_NON_NEGATIVE)
                    .with_meta("min", T::ZERO)
                    .with_meta("actual", *v)
            },
        );
        self
    }

    /// Requires zero or below: `out_of_range` with `max` 0 and `actual`.
    pub fn non_positive(mut self) -> Self {
        self.steps.require(
            |v| *v <= T::ZERO,
            |v| {
                out_of_range(message_keys::OUT_OF_RANGE_NON_POSITIVE)
                    .with_meta("max", T::ZERO)
                    .with_meta("actual", *v)
            },
        );
        self
    }
}

/// A decoder of a JSON number into an `f64`.
///
/// Missing or `null` is `required`; any other type is `type_mismatch`. An integer is read as the
/// nearest `f64`. Bounds appear in messages as Java's `Double.toString` writes them, such as
/// `1.0E7`.
#[derive(Clone, Debug, Default)]
pub struct F64Decoder {
    steps: Steps<f64>,
}

/// A decoder of a JSON number into an `f64`.
pub fn f64() -> F64Decoder {
    F64Decoder::default()
}

impl Decoder<Value> for F64Decoder {
    type Output = f64;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<f64, Issues> {
        let found = match input {
            Value::Number(n) => n
                .as_f64()
                .filter(|v| v.is_finite())
                .ok_or_else(|| type_mismatch(path, "double")),
            Value::Null => Err(required(path)),
            other => Err(type_mismatch(path, "double").with_meta("actual", node_type(other))),
        };
        let value = found.map_err(|issue| self.steps.base_issue(issue))?;
        self.steps.run(value, path)
    }
}

impl F64Decoder {
    fn bound(
        mut self,
        ok: impl Fn(f64) -> bool + Send + Sync + 'static,
        key: &'static str,
        bounds: Vec<(&'static str, f64)>,
    ) -> Self {
        self.steps.require(
            move |v| ok(*v),
            move |v| {
                bounds
                    .iter()
                    .fold(out_of_range(key), |issue, (name, bound)| {
                        issue.with_meta(*name, *bound)
                    })
                    .with_meta("actual", *v)
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
    pub fn min(self, min: f64) -> Self {
        self.bound(
            move |v| v >= min,
            message_keys::OUT_OF_RANGE_MINIMUM,
            vec![("min", min)],
        )
    }

    /// Allows at most `max`: `out_of_range` with `max` and `actual`.
    pub fn max(self, max: f64) -> Self {
        self.bound(
            move |v| v <= max,
            message_keys::OUT_OF_RANGE_MAXIMUM,
            vec![("max", max)],
        )
    }

    /// Requires a value within `range`, both ends included: `out_of_range` with `min`, `max` and
    /// `actual`.
    pub fn range(self, range: RangeInclusive<f64>) -> Self {
        let (min, max) = range.into_inner();
        self.bound(
            move |v| min <= v && v <= max,
            message_keys::OUT_OF_RANGE_RANGE,
            vec![("min", min), ("max", max)],
        )
    }

    /// Requires a value above zero: `out_of_range` with `min` 0.0 and `actual`.
    pub fn positive(self) -> Self {
        self.bound(
            |v| v > 0.0,
            message_keys::OUT_OF_RANGE_POSITIVE,
            vec![("min", 0.0)],
        )
    }

    /// Requires a value below zero: `out_of_range` with `max` 0.0 and `actual`.
    pub fn negative(self) -> Self {
        self.bound(
            |v| v < 0.0,
            message_keys::OUT_OF_RANGE_NEGATIVE,
            vec![("max", 0.0)],
        )
    }

    /// Requires zero or above: `out_of_range` with `min` 0.0 and `actual`.
    pub fn non_negative(self) -> Self {
        self.bound(
            |v| v >= 0.0,
            message_keys::OUT_OF_RANGE_NON_NEGATIVE,
            vec![("min", 0.0)],
        )
    }

    /// Requires zero or below: `out_of_range` with `max` 0.0 and `actual`.
    pub fn non_positive(self) -> Self {
        self.bound(
            |v| v <= 0.0,
            message_keys::OUT_OF_RANGE_NON_POSITIVE,
            vec![("max", 0.0)],
        )
    }

    /// Requires one of `allowed`: `not_allowed` with the sorted `allowed` and `actual`.
    pub fn one_of(mut self, allowed: impl IntoIterator<Item = f64>) -> Self {
        let mut allowed: Vec<f64> = allowed.into_iter().collect();
        allowed.sort_by(f64::total_cmp);
        allowed.dedup();
        let check = allowed.clone();
        self.steps.require(
            move |v| check.contains(v),
            move |v| {
                Issue::new(codes::NOT_ALLOWED)
                    .with_meta("allowed", allowed.clone())
                    .with_meta("actual", *v)
            },
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn first(result: Result<impl std::fmt::Debug, Issues>) -> Issue {
        result.unwrap_err().into_iter().next().unwrap()
    }

    #[test]
    fn a_number_that_is_not_an_integer_names_what_it_is() {
        for issue in [
            first(i32().decode(&json!(1.5))),
            first(u64().decode(&json!(1e2))),
        ] {
            assert_eq!(issue.message_key(), "type_mismatch");
            assert_eq!(issue.meta()["actual"], "number");
        }
    }

    #[test]
    fn an_integer_the_type_cannot_hold_is_outside_its_range() {
        for (issue, expected) in [
            (first(i32().decode(&json!(3_000_000_000_i64))), "integer"),
            (first(i64().decode(&json!(u64::MAX))), "long"),
            (first(u32().decode(&json!(-1))), "integer"),
            (first(u64().decode(&json!(i64::MIN))), "long"),
        ] {
            assert_eq!(issue.code(), "type_mismatch");
            assert_eq!(issue.message_key(), "type_mismatch.numeric_range");
            assert_eq!(issue.meta().len(), 1);
            assert_eq!(issue.meta()["expected"], expected);
            assert_eq!(
                issue.message(),
                format!("value is outside the {expected} range")
            );
        }
        assert_eq!(u64().decode(&json!(u64::MAX)).unwrap(), u64::MAX);
        assert_eq!(i64().decode(&json!(i64::MIN)).unwrap(), i64::MIN);
    }

    #[test]
    fn negative_zero_text_is_the_integer_zero() {
        let minus_zero: Value = serde_json::from_str("-0").unwrap();
        assert_eq!(i64().decode(&minus_zero).unwrap(), 0);
        assert_eq!(u32().decode(&minus_zero).unwrap(), 0);
    }

    #[test]
    fn range_reports_both_bounds() {
        let issue = first(u32().range(0..=150).decode(&json!(200)));
        assert_eq!(issue.message_key(), "out_of_range.range");
        assert_eq!(issue.meta()["min"], 0);
        assert_eq!(issue.meta()["max"], 150);
        assert_eq!(issue.meta()["actual"], 200);
        assert_eq!(issue.message(), "must be between 0 and 150");
    }

    #[test]
    fn signed_positive_and_negative_bounds_follow_java() {
        assert_eq!(first(i32().negative().decode(&json!(0))).meta()["max"], -1);
        assert_eq!(first(i32().positive().decode(&json!(0))).meta()["min"], 1);
    }

    #[test]
    fn multiple_of_does_not_overflow() {
        assert!(i64().multiple_of(-1).decode(&json!(i64::MIN)).is_ok());
        assert_eq!(
            first(i64().multiple_of(3).decode(&json!(4))).code(),
            "not_multiple_of"
        );
    }

    #[test]
    fn f64_bounds_are_written_as_java_writes_doubles() {
        assert_eq!(f64().decode(&json!(2)).unwrap(), 2.0);
        assert_eq!(first(f64().decode(&json!("2"))).meta()["actual"], "string");
        assert_eq!(first(f64().positive().decode(&json!(0))).meta()["min"], 0.0);
        assert_eq!(
            first(f64().min(1e7).decode(&json!(1))).message(),
            "must be at least 1.0E7"
        );
    }
}
