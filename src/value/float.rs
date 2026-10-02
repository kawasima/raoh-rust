//! Floats as the value model holds them: one NaN, two zeros, and a total order.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

mod sealed {
    pub trait Sealed {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

/// `f32` or `f64`. It cannot be implemented outside this crate.
pub trait Float:
    sealed::Sealed
    + Copy
    + PartialEq
    + fmt::Display
    + fmt::LowerExp
    + FromStr
    + Into<crate::MetaValue>
    + Send
    + Sync
    + 'static
{
    /// What an issue names the type in `expected`.
    const EXPECTED: &'static str;
    /// +0.
    const ZERO: Self;

    /// Whether this is NaN.
    #[doc(hidden)]
    fn nan(self) -> bool;

    /// Whether this is +∞ or -∞.
    #[doc(hidden)]
    fn infinite(self) -> bool;

    /// Whether the sign bit is set.
    #[doc(hidden)]
    fn negative_sign(self) -> bool;

    /// Compares as [`f64::total_cmp`] does.
    #[doc(hidden)]
    fn total(self, other: Self) -> Ordering;

    /// The bits of the value, with every NaN given the same bits.
    #[doc(hidden)]
    fn canonical_bits(self) -> u64;

    /// The JSON number rounded to this type once, or `None` when it cannot be read.
    #[doc(hidden)]
    fn from_number(n: &serde_json::Number) -> Option<Self>;
}

impl Float for f32 {
    const EXPECTED: &'static str = "float";
    const ZERO: Self = 0.0;

    fn nan(self) -> bool {
        self.is_nan()
    }

    fn infinite(self) -> bool {
        self.is_infinite()
    }

    fn negative_sign(self) -> bool {
        self.is_sign_negative()
    }

    fn total(self, other: Self) -> Ordering {
        self.total_cmp(&other)
    }

    fn canonical_bits(self) -> u64 {
        if self.is_nan() {
            u64::from(f32::NAN.to_bits())
        } else {
            u64::from(self.to_bits())
        }
    }

    /// From the number's text, so that it is rounded once, to binary32, and not first to binary64.
    fn from_number(n: &serde_json::Number) -> Option<Self> {
        crate::json::lexeme(n).parse().ok()
    }
}

impl Float for f64 {
    const EXPECTED: &'static str = "double";
    const ZERO: Self = 0.0;

    fn nan(self) -> bool {
        self.is_nan()
    }

    fn infinite(self) -> bool {
        self.is_infinite()
    }

    fn negative_sign(self) -> bool {
        self.is_sign_negative()
    }

    fn total(self, other: Self) -> Ordering {
        self.total_cmp(&other)
    }

    fn canonical_bits(self) -> u64 {
        if self.is_nan() {
            f64::NAN.to_bits()
        } else {
            self.to_bits()
        }
    }

    /// What `serde_json` reads: with `arbitrary_precision`, the text parsed, rounded once, and
    /// nothing for a number beyond the range; without it, the `f64` it read, or an integer
    /// rounded to the nearest `f64`.
    fn from_number(n: &serde_json::Number) -> Option<Self> {
        n.as_f64()
    }
}

/// The float order of the value model: -∞, the negative values, -0, +0, the positive values, +∞
/// and last NaN, every NaN equal to every other.
///
/// `min`, `max`, `range`, `positive`, `negative`, `non_negative`, `non_positive` and the sorting of
/// `one_of`'s values compare with it, so -0 is below +0 and every value is comparable.
pub fn float_order<F: Float>(a: F, b: F) -> Ordering {
    match (a.nan(), b.nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => a.total(b),
    }
}

/// Whether two floats are the same value: +0 and -0 differ, and every NaN is the same.
pub fn float_same<F: Float>(a: F, b: F) -> bool {
    a.canonical_bits() == b.canonical_bits()
}

/// The canonical decimal of a finite non-zero float, as the significant digits, with no
/// trailing zero, and the exponent of the first digit: 0.25 is `("25", -1)`.
///
/// It is the closest of the shortest decimals that round to the float, except that where one
/// digit would do, two are allowed and the closer taken: the least positive `f64` is 4.9E-324,
/// not 5E-324.
fn canonical_decimal<F: Float>(v: F) -> (String, i32) {
    // `{:e}` gives the least length; of two decimals of that length equally close to the float it
    // takes either, so the decimal is written again at that length with `{:.*e}`, which rounds
    // the float's exact value to nearest, ties to even, and so gives the closest.
    let (shortest, _) = split_exponential(&format!("{v:e}"));
    let length = shortest.len().max(2);
    let closest = format!("{v:.*e}", length - 1);
    if closest.parse::<F>().is_ok_and(|back| float_same(back, v)) {
        return split_exponential(&closest);
    }
    split_exponential(&format!("{v:e}"))
}

/// The digits and exponent of `d.ddde±n` as Rust's `{:e}` writes a positive float, with trailing
/// zeros removed.
fn split_exponential(text: &str) -> (String, i32) {
    let text = text.trim_start_matches('-');
    let (mantissa, exponent) = text.split_once('e').expect("`{:e}` writes an exponent");
    let exponent: i32 = exponent.parse().expect("`{:e}` writes an integer exponent");
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    (digits.to_owned(), exponent)
}

/// A float as a message writes it: its canonical decimal, plainly with at least one digit after
/// the point when the exponent of its first digit is from -3 to 6 (`0.5`, `100.0`, `0.001`), and
/// otherwise as a mantissa with at least one digit after the point, `E` and that exponent
/// (`1.0E7`, `1.0E-4`). Zeros are `0.0` and `-0.0`, and the others `NaN`, `Infinity` and
/// `-Infinity`.
pub(crate) fn float_message<F: Float>(v: F) -> String {
    if v.nan() {
        return "NaN".into();
    }
    let sign = if v.negative_sign() { "-" } else { "" };
    if v.infinite() {
        return format!("{sign}Infinity");
    }
    // IEEE 754 equality, under which -0 is +0.
    if v == F::ZERO {
        return format!("{sign}0.0");
    }
    let (digits, exponent) = canonical_decimal(v);
    let body = if (-3..=6).contains(&exponent) {
        let point = exponent + 1;
        if point <= 0 {
            format!("0.{}{digits}", "0".repeat(point.unsigned_abs() as usize))
        } else {
            let point = point as usize;
            if point >= digits.len() {
                format!("{digits}{}.0", "0".repeat(point - digits.len()))
            } else {
                format!("{}.{}", &digits[..point], &digits[point..])
            }
        }
    } else {
        let (first, rest) = digits.split_at(1);
        let rest = if rest.is_empty() { "0" } else { rest };
        format!("{first}.{rest}E{exponent}")
    };
    format!("{sign}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_are_written_as_the_specification_writes_them() {
        let cases: [(f64, &str); 16] = [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (100.0, "100.0"),
            (0.5, "0.5"),
            (0.001, "0.001"),
            (0.0001, "1.0E-4"),
            (1234567.0, "1234567.0"),
            (1e7, "1.0E7"),
            (1.25e7, "1.25E7"),
            (-1.5e-5, "-1.5E-5"),
            (f64::MAX, "1.7976931348623157E308"),
            (5e-324, "4.9E-324"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
        ];
        for (v, written) in cases {
            assert_eq!(float_message(v), written, "{v:e}");
        }
    }

    #[test]
    fn a_float32_is_written_at_its_own_width() {
        assert_eq!(float_message(0.1f32), "0.1");
        assert_eq!(float_message(1.4e-45f32), "1.4E-45");
        assert_eq!(float_message(16777216f32), "1.6777216E7");
        assert_eq!(float_message(1.0000001f32), "1.0000001");
    }

    #[test]
    fn of_two_equally_close_decimals_the_even_one_is_taken() {
        assert_eq!(
            float_message("1765629.25".parse::<f32>().unwrap()),
            "1765629.2"
        );
        assert_eq!(
            float_message("-1749220892028010.25".parse::<f64>().unwrap()),
            "-1.7492208920280102E15"
        );
    }

    #[test]
    fn the_order_puts_negative_zero_below_zero_and_nan_last() {
        let mut values = [
            f64::NAN,
            1.0,
            0.0,
            -0.0,
            f64::NEG_INFINITY,
            f64::INFINITY,
            -1.0,
        ];
        values.sort_by(|a, b| float_order(*a, *b));
        let written: Vec<String> = values.iter().map(|v| float_message(*v)).collect();
        assert_eq!(
            written,
            ["-Infinity", "-1.0", "-0.0", "0.0", "1.0", "Infinity", "NaN"]
        );
        assert!(float_same(f64::NAN, -f64::NAN));
        assert!(!float_same(0.0, -0.0));
    }

    proptest::proptest! {
        #[test]
        fn a_canonical_decimal_is_shortest_and_reads_back(bits in proptest::prelude::any::<u32>()) {
            let v = f32::from_bits(bits);
            proptest::prop_assume!(v.is_finite() && v != 0.0);
            let (digits, _) = canonical_decimal(v);
            let (shortest, _) = split_exponential(&format!("{v:e}"));
            proptest::prop_assert_eq!(float_message(v).replace('E', "e").parse::<f32>().unwrap(), v);
            proptest::prop_assert!(digits.len() <= shortest.len().max(2));
        }
    }
}
