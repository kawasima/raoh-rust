use super::steps::Steps;
use super::{node_type, required};
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::path::Path;
use crate::{codes, message_keys};
use serde_json::Value;
use std::fmt::Display;
use std::ops::RangeInclusive;

mod sealed {
    pub trait Sealed {}
    impl Sealed for i32 {}
    impl Sealed for i64 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
}

/// An integer type a JSON number can be decoded into.
pub trait Integer:
    sealed::Sealed + Copy + Ord + Display + Into<Value> + Send + Sync + 'static
{
    /// What an issue names the type in `expected`, as Raoh for Java does.
    const EXPECTED: &'static str;
    /// Whether a number that is not an integer of this type is reported with `actual`, as Raoh
    /// for Java's `long_()` does and its `int_()` does not.
    const NUMBER_HAS_ACTUAL: bool;
    /// The smallest positive value.
    const ONE: Self;

    /// The value of `n`, if it is an integer this type holds.
    fn from_number(n: &serde_json::Number) -> Option<Self>;

    /// Whether `self` is a multiple of `divisor`, which is not zero.
    fn is_multiple_of(self, divisor: Self) -> bool;

    /// Whether `self` is zero.
    fn is_zero(self) -> bool;
}

/// An integer type that holds negative values.
pub trait SignedInteger: Integer {
    /// Zero.
    const ZERO: Self;
    /// The largest negative value.
    const MINUS_ONE: Self;
}

macro_rules! integer {
    ($t:ty, $expected:literal, $actual:literal, $via:ident) => {
        impl Integer for $t {
            const EXPECTED: &'static str = $expected;
            const NUMBER_HAS_ACTUAL: bool = $actual;
            const ONE: Self = 1;

            fn from_number(n: &serde_json::Number) -> Option<Self> {
                n.$via().and_then(|v| <$t>::try_from(v).ok())
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

integer!(i32, "integer", false, as_i64);
integer!(i64, "long", true, as_i64);
integer!(u32, "integer", false, as_u64);
integer!(u64, "long", true, as_u64);

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
/// Missing or `null` is `required`. A value of another type, a number with a fraction or an
/// exponent, and an integer `T` cannot hold are `type_mismatch`. The type found is named in
/// `actual` for a value of another type, and for a number too when `T` is `i64` or `u64`, as
/// Raoh for Java does for `int_()` and `long_()`. Constraints run in the order they are written, and the first to fail is the
/// one reported.
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

impl<T: Integer> Decoder<Value> for IntDecoder<T> {
    type Output = T;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<T, Issues> {
        let found = match input {
            Value::Number(n) => T::from_number(n).ok_or_else(|| {
                let issue = Issue::at_path(
                    path,
                    codes::TYPE_MISMATCH,
                    format!("expected {}", T::EXPECTED),
                )
                .with_meta("expected", T::EXPECTED);
                if T::NUMBER_HAS_ACTUAL {
                    issue.with_meta("actual", "number")
                } else {
                    issue
                }
            }),
            Value::Null => Err(required(path)),
            other => Err(Issue::at_path(
                path,
                codes::TYPE_MISMATCH,
                format!("expected {}", T::EXPECTED),
            )
            .with_meta("expected", T::EXPECTED)
            .with_meta("actual", node_type(other))),
        };
        let value = found.map_err(|issue| self.steps.base_issue(issue))?;
        self.steps.run(value, path)
    }
}

fn out_of_range(key: &'static str, message: String) -> Issue {
    Issue::new(codes::OUT_OF_RANGE, message).with_message_key(key)
}

impl<T: Integer> IntDecoder<T> {
    /// Gives the constraint written just before this, or the type check when there is none, a
    /// custom message that no resolver rewrites.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.steps.set_message(message.into());
        self
    }

    /// Requires at least `min`: `out_of_range` with `min` and `actual`.
    pub fn min(mut self, min: T) -> Self {
        self.steps.require(
            move |v| *v >= min,
            move |v| {
                out_of_range(
                    message_keys::OUT_OF_RANGE_MINIMUM,
                    format!("must be at least {min}"),
                )
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
                out_of_range(
                    message_keys::OUT_OF_RANGE_MAXIMUM,
                    format!("must be at most {max}"),
                )
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
                out_of_range(
                    message_keys::OUT_OF_RANGE_RANGE,
                    format!("must be between {min} and {max}"),
                )
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
                out_of_range(
                    message_keys::OUT_OF_RANGE_POSITIVE,
                    "must be positive".into(),
                )
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
                Issue::new(
                    codes::NOT_MULTIPLE_OF,
                    format!("must be a multiple of {divisor}"),
                )
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
        let listed: Vec<String> = allowed.iter().map(ToString::to_string).collect();
        let message = format!("must be one of [{}]", listed.join(", "));
        let check = allowed.clone();
        self.steps.require(
            move |v| check.binary_search(v).is_ok(),
            move |v| {
                Issue::new(codes::NOT_ALLOWED, message.clone())
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
                out_of_range(
                    message_keys::OUT_OF_RANGE_NEGATIVE,
                    "must be negative".into(),
                )
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
                out_of_range(
                    message_keys::OUT_OF_RANGE_NON_NEGATIVE,
                    "must be non-negative".into(),
                )
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
                out_of_range(
                    message_keys::OUT_OF_RANGE_NON_POSITIVE,
                    "must be non-positive".into(),
                )
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
/// nearest `f64`.
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
            Value::Number(n) => n.as_f64().filter(|v| v.is_finite()).ok_or_else(|| {
                Issue::at_path(path, codes::TYPE_MISMATCH, "expected double")
                    .with_meta("expected", "double")
            }),
            Value::Null => Err(required(path)),
            other => Err(
                Issue::at_path(path, codes::TYPE_MISMATCH, "expected double")
                    .with_meta("expected", "double")
                    .with_meta("actual", node_type(other)),
            ),
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
        message: String,
        bounds: Vec<(&'static str, f64)>,
    ) -> Self {
        self.steps.require(
            move |v| ok(*v),
            move |v| {
                bounds
                    .iter()
                    .fold(
                        out_of_range(key, message.clone()),
                        |issue, (name, bound)| issue.with_meta(*name, *bound),
                    )
                    .with_meta("actual", *v)
            },
        );
        self
    }

    /// Gives the constraint written just before this, or the type check when there is none, a
    /// custom message that no resolver rewrites.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.steps.set_message(message.into());
        self
    }

    /// Requires at least `min`: `out_of_range` with `min` and `actual`.
    pub fn min(self, min: f64) -> Self {
        self.bound(
            move |v| v >= min,
            message_keys::OUT_OF_RANGE_MINIMUM,
            format!("must be at least {min:?}"),
            vec![("min", min)],
        )
    }

    /// Allows at most `max`: `out_of_range` with `max` and `actual`.
    pub fn max(self, max: f64) -> Self {
        self.bound(
            move |v| v <= max,
            message_keys::OUT_OF_RANGE_MAXIMUM,
            format!("must be at most {max:?}"),
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
            format!("must be between {min:?} and {max:?}"),
            vec![("min", min), ("max", max)],
        )
    }

    /// Requires a value above zero: `out_of_range` with `min` 0.0 and `actual`.
    pub fn positive(self) -> Self {
        self.bound(
            |v| v > 0.0,
            message_keys::OUT_OF_RANGE_POSITIVE,
            "must be positive".into(),
            vec![("min", 0.0)],
        )
    }

    /// Requires a value below zero: `out_of_range` with `max` 0.0 and `actual`.
    pub fn negative(self) -> Self {
        self.bound(
            |v| v < 0.0,
            message_keys::OUT_OF_RANGE_NEGATIVE,
            "must be negative".into(),
            vec![("max", 0.0)],
        )
    }

    /// Requires zero or above: `out_of_range` with `min` 0.0 and `actual`.
    pub fn non_negative(self) -> Self {
        self.bound(
            |v| v >= 0.0,
            message_keys::OUT_OF_RANGE_NON_NEGATIVE,
            "must be non-negative".into(),
            vec![("min", 0.0)],
        )
    }

    /// Requires zero or below: `out_of_range` with `max` 0.0 and `actual`.
    pub fn non_positive(self) -> Self {
        self.bound(
            |v| v <= 0.0,
            message_keys::OUT_OF_RANGE_NON_POSITIVE,
            "must be non-positive".into(),
            vec![("max", 0.0)],
        )
    }

    /// Requires one of `allowed`: `not_allowed` with the sorted `allowed` and `actual`.
    pub fn one_of(mut self, allowed: impl IntoIterator<Item = f64>) -> Self {
        let mut allowed: Vec<f64> = allowed.into_iter().collect();
        allowed.sort_by(f64::total_cmp);
        allowed.dedup();
        let listed: Vec<String> = allowed.iter().map(|a| format!("{a:?}")).collect();
        let message = format!("must be one of [{}]", listed.join(", "));
        let check = allowed.clone();
        self.steps.require(
            move |v| check.contains(v),
            move |v| {
                Issue::new(codes::NOT_ALLOWED, message.clone())
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
    fn a_fraction_names_actual_only_for_64_bit_types() {
        let issue = first(i32().decode(&json!(1.5)));
        assert_eq!(issue.meta()["expected"], "integer");
        assert!(!issue.meta().contains_key("actual"));
        let issue = first(i64().decode(&json!(1.5)));
        assert_eq!(issue.meta()["expected"], "long");
        assert_eq!(issue.meta()["actual"], "number");
    }

    #[test]
    fn an_integer_too_large_for_the_type_is_a_type_mismatch() {
        assert_eq!(
            first(i32().decode(&json!(3_000_000_000_i64))).code(),
            "type_mismatch"
        );
        assert_eq!(
            first(i64().decode(&json!(u64::MAX))).code(),
            "type_mismatch"
        );
        assert_eq!(first(u32().decode(&json!(-1))).code(), "type_mismatch");
        assert_eq!(u64().decode(&json!(u64::MAX)).unwrap(), u64::MAX);
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
        let issue = first(i32().negative().decode(&json!(0)));
        assert_eq!(issue.meta()["max"], -1);
        let issue = first(i32().positive().decode(&json!(0)));
        assert_eq!(issue.meta()["min"], 1);
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
    fn f64_accepts_integers_and_rejects_strings() {
        assert_eq!(f64().decode(&json!(2)).unwrap(), 2.0);
        let issue = first(f64().decode(&json!("2")));
        assert_eq!(issue.meta()["actual"], "string");
        let issue = first(f64().positive().decode(&json!(0)));
        assert_eq!(issue.meta()["min"], 0.0);
    }
}
