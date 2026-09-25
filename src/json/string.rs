use super::steps::Steps;
use super::unexpected;
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::java;
use crate::path::Path;
use crate::{codes, message_keys};
use serde_json::Value;
use std::marker::PhantomData;
use std::str::FromStr;

const MAX_EMAIL_LENGTH: usize = 254;
const MAX_IP_LENGTH: usize = 45;
#[cfg(feature = "url")]
const MAX_URL_LENGTH: usize = 2048;

/// A decoder of a JSON string.
///
/// Missing or `null` is `required`; any other type is `type_mismatch`. Constraints and
/// transformations run in the order they are written, and the first constraint to fail is the one
/// reported. An empty string is accepted unless [`non_blank`](Self::non_blank) says otherwise.
///
/// What counts as whitespace, a character and the order of strings are as in Raoh for Java; see
/// each method.
///
/// ```
/// use raoh::json::prelude::*;
///
/// let name = string().trim().non_blank().max_length(5);
/// assert_eq!(name.decode(&json!("  Ken ")).unwrap(), "Ken");
/// assert_eq!(name.decode(&json!("   ")).unwrap_err().iter().next().unwrap().code(), "blank");
/// ```
#[derive(Clone, Debug, Default)]
pub struct StringDecoder {
    steps: Steps<String>,
}

/// A decoder of a JSON string.
pub fn string() -> StringDecoder {
    StringDecoder::default()
}

impl Decoder<Value> for StringDecoder {
    type Output = String;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<String, Issues> {
        match input {
            Value::String(s) => self.steps.run(s.clone(), path),
            other => Err(self.steps.base_issue(unexpected(path, "string", other))),
        }
    }
}

/// The number of characters, as Raoh for Java counts them: code points.
fn char_count(s: &str) -> usize {
    s.chars().count()
}

fn invalid_format(key: &'static str) -> Issue {
    Issue::new(codes::INVALID_FORMAT).with_message_key(key)
}

impl StringDecoder {
    fn transform(mut self, f: fn(&str) -> String) -> Self {
        self.steps.transform(move |s| f(&s));
        self
    }

    fn format(
        mut self,
        ok: impl Fn(&str) -> bool + Send + Sync + 'static,
        key: &'static str,
    ) -> Self {
        self.steps
            .require(move |s| ok(s), move |_| invalid_format(key));
        self
    }

    /// Gives the most recent constraint written before this, or the type check when there is
    /// none, a custom message that every language shows as written. Transformations such as
    /// [`trim`](Self::trim) are passed over, as they cannot fail.
    ///
    /// ```
    /// use raoh::json::prelude::*;
    ///
    /// let code = string().min_length(3).message("too short a code");
    /// let issues = code.decode(&json!("ab")).unwrap_err();
    /// assert_eq!(issues.iter().next().unwrap().message(), "too short a code");
    ///
    /// let name = string().trim().message("give a name");
    /// let issues = name.decode(&Value::Null).unwrap_err();
    /// assert_eq!(issues.iter().next().unwrap().message(), "give a name");
    /// ```
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.steps.set_message(message.into());
        self
    }

    /// Removes the characters up to U+0020 from both ends, as Java's `String.trim()` does: spaces,
    /// tabs, line breaks and the other control characters. Other Unicode spaces, such as U+3000
    /// and U+00A0, are kept.
    pub fn trim(self) -> Self {
        self.transform(|s| java::trim(s).to_owned())
    }

    /// Converts to lower case with Unicode's case mapping, as Java's `toLowerCase` does under any
    /// locale but Turkish, Azerbaijani and Lithuanian.
    pub fn lowercase(self) -> Self {
        self.transform(str::to_lowercase)
    }

    /// Converts to upper case with Unicode's case mapping, as Java's `toUpperCase` does under any
    /// locale but Turkish, Azerbaijani and Lithuanian.
    pub fn uppercase(self) -> Self {
        self.transform(str::to_uppercase)
    }

    /// Requires a character that is not whitespace, as Java's `String.isBlank()` decides it:
    /// `blank`. The non-breaking spaces U+00A0, U+2007 and U+202F are not whitespace there.
    pub fn non_blank(mut self) -> Self {
        self.steps
            .require(|s| !java::is_blank(s), |_| Issue::new(codes::BLANK));
        self
    }

    /// Requires at least `n` characters, counted as code points: `too_short` with `min` and
    /// `actual`.
    pub fn min_length(mut self, n: usize) -> Self {
        self.steps.require(
            move |s| char_count(s) >= n,
            move |s| {
                Issue::new(codes::TOO_SHORT)
                    .with_meta("min", n)
                    .with_meta("actual", char_count(s))
            },
        );
        self
    }

    /// Allows at most `n` characters, counted as code points: `too_long` with `max` and `actual`.
    pub fn max_length(mut self, n: usize) -> Self {
        self.steps.require(
            move |s| char_count(s) <= n,
            move |s| {
                Issue::new(codes::TOO_LONG)
                    .with_meta("max", n)
                    .with_meta("actual", char_count(s))
            },
        );
        self
    }

    /// Requires exactly `n` characters, counted as code points: `invalid_length` with `expected`
    /// and `actual`.
    pub fn length(mut self, n: usize) -> Self {
        self.steps.require(
            move |s| char_count(s) == n,
            move |s| {
                Issue::new(codes::INVALID_LENGTH)
                    .with_meta("expected", n)
                    .with_meta("actual", char_count(s))
            },
        );
        self
    }

    /// Requires the string to start with `prefix`: `invalid_format` with `prefix`.
    pub fn starts_with(mut self, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        let expected = prefix.clone();
        self.steps.require(
            move |s| s.starts_with(expected.as_str()),
            move |_| {
                invalid_format(message_keys::INVALID_FORMAT_STARTS_WITH)
                    .with_meta("prefix", prefix.clone())
            },
        );
        self
    }

    /// Requires the string to end with `suffix`: `invalid_format` with `suffix`.
    pub fn ends_with(mut self, suffix: impl Into<String>) -> Self {
        let suffix = suffix.into();
        let expected = suffix.clone();
        self.steps.require(
            move |s| s.ends_with(expected.as_str()),
            move |_| {
                invalid_format(message_keys::INVALID_FORMAT_ENDS_WITH)
                    .with_meta("suffix", suffix.clone())
            },
        );
        self
    }

    /// Requires the string to contain `substring`: `invalid_format` with `substring`.
    pub fn contains(mut self, substring: impl Into<String>) -> Self {
        let substring = substring.into();
        let expected = substring.clone();
        self.steps.require(
            move |s| s.contains(expected.as_str()),
            move |_| {
                invalid_format(message_keys::INVALID_FORMAT_INCLUDES)
                    .with_meta("substring", substring.clone())
            },
        );
        self
    }

    /// Requires one of `allowed`: `not_allowed` with `allowed` sorted as Java sorts strings, by
    /// UTF-16 code units, and `actual`.
    pub fn one_of<S: Into<String>>(mut self, allowed: impl IntoIterator<Item = S>) -> Self {
        let mut allowed: Vec<String> = allowed.into_iter().map(Into::into).collect();
        java::sort_strings(&mut allowed);
        let check = allowed.clone();
        self.steps.require(
            move |s| check.contains(s),
            move |s| {
                Issue::new(codes::NOT_ALLOWED)
                    .with_meta("allowed", allowed.clone())
                    .with_meta("actual", s.clone())
            },
        );
        self
    }

    /// Requires the form of an email address, as Raoh for Java checks it: `invalid_format`.
    pub fn email(self) -> Self {
        self.format(is_email, message_keys::INVALID_FORMAT_EMAIL)
    }

    /// Requires an IPv4 address in dotted decimal: `invalid_format`.
    pub fn ipv4(self) -> Self {
        self.format(
            |s| java::utf16_len(s) <= MAX_IP_LENGTH && is_ipv4(s),
            message_keys::INVALID_FORMAT_IPV4,
        )
    }

    /// Requires an IPv6 address as Java's `InetAddress` reads one: `invalid_format`. Brackets
    /// and a numeric scope are allowed; an IPv4-mapped address and a scope naming an interface
    /// are not.
    pub fn ipv6(self) -> Self {
        self.format(
            |s| java::utf16_len(s) <= MAX_IP_LENGTH && java::is_ipv6_literal(s),
            message_keys::INVALID_FORMAT_IPV6,
        )
    }

    /// Requires an address [`ipv4`](Self::ipv4) or [`ipv6`](Self::ipv6) accepts:
    /// `invalid_format`.
    pub fn ip(self) -> Self {
        self.format(
            |s| java::utf16_len(s) <= MAX_IP_LENGTH && (is_ipv4(s) || java::is_ipv6_literal(s)),
            message_keys::INVALID_FORMAT_IP,
        )
    }

    /// Requires a ULID, 26 characters of Crockford's base 32 in upper case: `invalid_format`.
    pub fn ulid(self) -> Self {
        self.format(
            |s| {
                s.len() == 26
                    && s.bytes().all(|b| {
                        b.is_ascii_digit()
                            || (b.is_ascii_uppercase() && !matches!(b, b'I' | b'L' | b'O' | b'U'))
                    })
            },
            message_keys::INVALID_FORMAT_ULID,
        )
    }

    /// Requires a CUID, `c` followed by 24 lower-case letters or digits: `invalid_format`.
    pub fn cuid(self) -> Self {
        self.format(
            |s| {
                s.len() == 25
                    && s.starts_with('c')
                    && s[1..]
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            },
            message_keys::INVALID_FORMAT_CUID,
        )
    }

    /// Requires the whole string to match `pattern`: `invalid_format` with `pattern`.
    ///
    /// The pattern is written in the syntax of the [`regex`](https://docs.rs/regex) crate, not
    /// Java's, and is anchored at both ends as Java's `Matcher.matches` is. The two differ: here
    /// `\d`, `\w` and `\s` match any Unicode digit, word character and space, where Java's match
    /// ASCII only; write `[0-9]` for an ASCII digit. Lookaround and backreferences are not
    /// available.
    ///
    /// # Panics
    ///
    /// When `pattern` is not a valid regular expression.
    #[cfg(feature = "regex")]
    pub fn pattern(mut self, pattern: &str) -> Self {
        let anchored = regex::Regex::new(&format!("^(?:{pattern})$"))
            .unwrap_or_else(|e| panic!("invalid pattern {pattern:?}: {e}"));
        let pattern = pattern.to_owned();
        self.steps.require(
            move |s| anchored.is_match(s),
            move |_| Issue::new(codes::INVALID_FORMAT).with_meta("pattern", pattern.clone()),
        );
        self
    }

    /// A decoder that parses the string into a `T` with [`FromStr`]: `invalid_format` when it does
    /// not parse.
    ///
    /// ```
    /// use raoh::json::prelude::*;
    /// use std::net::IpAddr;
    ///
    /// let addr = string().parse::<IpAddr>();
    /// assert!(addr.decode(&json!("::1")).is_ok());
    /// ```
    pub fn parse<T: FromStr>(self) -> Parse<T> {
        Parse {
            string: self,
            message: None,
            target: PhantomData,
        }
    }

    /// A decoder that reads the string as a UUID with the `uuid` crate: `invalid_format` when it
    /// is not one.
    ///
    /// The `uuid` crate accepts the hyphenated form, the 32 digits without hyphens, and those in
    /// braces or after `urn:uuid:`. Java's `UUID.fromString` accepts only hyphenated groups, but
    /// also groups shorter than the standard ones, such as `1-1-1-1-1`.
    #[cfg(feature = "uuid")]
    pub fn uuid(self) -> UuidDecoder {
        UuidDecoder {
            string: self,
            message: None,
        }
    }

    /// A decoder that reads the string as an `http` or `https` URL with a host, parsed by the
    /// `url` crate: `invalid_format` when it is not one.
    ///
    /// The `url` crate follows the WHATWG URL Standard, as browsers do, where Java's `URI` follows
    /// RFC 2396. It accepts a host holding `_` and non-ASCII characters, which Java's `URI` does
    /// not, and gives the URL normalised: `https://example.com` becomes `https://example.com/`.
    #[cfg(feature = "url")]
    pub fn url(self) -> UrlDecoder {
        UrlDecoder {
            string: self,
            message: None,
        }
    }
}

/// The decoder [`StringDecoder::parse`] returns.
pub struct Parse<T> {
    string: StringDecoder,
    message: Option<String>,
    target: PhantomData<fn() -> T>,
}

impl<T> std::fmt::Debug for Parse<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parse")
            .field("string", &self.string)
            .field("target", &std::any::type_name::<T>())
            .finish()
    }
}

impl<T> Clone for Parse<T> {
    fn clone(&self) -> Self {
        Self {
            string: self.string.clone(),
            message: self.message.clone(),
            target: PhantomData,
        }
    }
}

impl<T> Parse<T> {
    /// Gives the issue a string that does not parse is reported with a custom message.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

fn conversion_failed(path: &Path<'_>, key: &'static str, custom: &Option<String>) -> Issues {
    let issue = Issue::at_path(path, codes::INVALID_FORMAT).with_message_key(key);
    match custom {
        Some(custom) => issue.with_message(custom.clone()).into(),
        None => issue.into(),
    }
}

impl<T: FromStr> Decoder<Value> for Parse<T> {
    type Output = T;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<T, Issues> {
        let s = self.string.decode_at(input, path)?;
        s.parse()
            .map_err(|_| conversion_failed(path, codes::INVALID_FORMAT, &self.message))
    }
}

/// The decoder [`StringDecoder::uuid`] returns.
#[cfg(feature = "uuid")]
#[derive(Clone, Debug)]
pub struct UuidDecoder {
    string: StringDecoder,
    message: Option<String>,
}

#[cfg(feature = "uuid")]
impl UuidDecoder {
    /// Gives the issue a string that is not a UUID is reported with a custom message.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

#[cfg(feature = "uuid")]
impl Decoder<Value> for UuidDecoder {
    type Output = uuid::Uuid;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<uuid::Uuid, Issues> {
        let s = self.string.decode_at(input, path)?;
        uuid::Uuid::parse_str(&s)
            .map_err(|_| conversion_failed(path, message_keys::INVALID_FORMAT_UUID, &self.message))
    }
}

/// The decoder [`StringDecoder::url`] returns.
#[cfg(feature = "url")]
#[derive(Clone, Debug)]
pub struct UrlDecoder {
    string: StringDecoder,
    message: Option<String>,
}

#[cfg(feature = "url")]
impl UrlDecoder {
    /// Gives the issue a string that is not a URL is reported with a custom message.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

#[cfg(feature = "url")]
impl Decoder<Value> for UrlDecoder {
    type Output = url::Url;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<url::Url, Issues> {
        let s = self.string.decode_at(input, path)?;
        let fail = || conversion_failed(path, message_keys::INVALID_FORMAT_URL, &self.message);
        if java::utf16_len(&s) > MAX_URL_LENGTH {
            return Err(fail());
        }
        let url = url::Url::parse(&s).map_err(|_| fail())?;
        let web = matches!(url.scheme(), "http" | "https");
        let has_host = url.host_str().is_some_and(|h| !h.is_empty());
        if web && has_host {
            Ok(url)
        } else {
            Err(fail())
        }
    }
}

/// `^[a-zA-Z0-9._%+\-]{1,64}@[a-zA-Z0-9.\-]{1,255}\.[a-zA-Z]{2,}$`, at most 254 UTF-16 units, as
/// Raoh for Java checks it.
fn is_email(s: &str) -> bool {
    if java::utf16_len(s) > MAX_EMAIL_LENGTH {
        return false;
    }
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    let local_ok = (1..=64).contains(&local.len())
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'%' | b'+' | b'-'));
    let Some((host, tld)) = domain.rsplit_once('.') else {
        return false;
    };
    let host_ok = (1..=255).contains(&host.len())
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'));
    let tld_ok = tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic());
    local_ok && host_ok && tld_ok
}

/// Four decimal octets from 0 to 255, without leading zeros.
fn is_ipv4(s: &str) -> bool {
    let octets: Vec<&str> = s.split('.').collect();
    octets.len() == 4
        && octets.iter().all(|o| {
            !o.is_empty()
                && o.len() <= 3
                && o.bytes().all(|b| b.is_ascii_digit())
                && (o.len() == 1 || !o.starts_with('0'))
                && o.parse::<u16>().is_ok_and(|n| n <= 255)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn first<T: std::fmt::Debug>(result: Result<T, Issues>) -> Issue {
        result.unwrap_err().into_iter().next().unwrap()
    }

    #[test]
    fn missing_and_null_are_required_and_other_types_mismatch() {
        assert_eq!(first(string().decode(&Value::Null)).code(), "required");
        let issue = first(string().decode(&json!(1)));
        assert_eq!(issue.code(), "type_mismatch");
        assert_eq!(issue.meta()["actual"], "number");
    }

    #[test]
    fn the_first_failing_constraint_is_reported() {
        let issues = string()
            .min_length(3)
            .email()
            .decode(&json!("a"))
            .unwrap_err();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues.iter().next().unwrap().code(), "too_short");
    }

    #[test]
    fn length_counts_code_points() {
        assert!(string().max_length(2).decode(&json!("日本")).is_ok());
        assert!(string().length(1).decode(&json!("😀")).is_ok());
    }

    #[test]
    fn trim_and_non_blank_follow_java() {
        assert_eq!(
            string().trim().decode(&json!("\u{3000}a\u{3000}")).unwrap(),
            "\u{3000}a\u{3000}"
        );
        assert_eq!(string().trim().decode(&json!("\u{0}a\n")).unwrap(), "a");
        assert!(string().non_blank().decode(&json!("\u{a0}")).is_ok());
        assert_eq!(
            first(string().non_blank().decode(&json!("\u{3000}"))).code(),
            "blank"
        );
    }

    #[test]
    fn email_follows_the_java_pattern() {
        for ok in ["a@b.co", "first.last+tag@sub.example.com"] {
            assert!(is_email(ok), "{ok}");
        }
        for bad in ["a@b", "@b.co", "a@@b.co", "a@b.c", "a b@c.co", "a@b.c0"] {
            assert!(!is_email(bad), "{bad}");
        }
    }

    #[test]
    fn ipv4_rejects_leading_zeros_and_large_octets() {
        assert!(is_ipv4("192.168.0.1"));
        assert!(!is_ipv4("192.168.00.1"));
        assert!(!is_ipv4("256.0.0.1"));
        assert!(!is_ipv4("1.2.3"));
    }

    #[test]
    fn one_of_sorts_as_java_does() {
        let issue = first(
            string()
                .one_of(["\u{ff21}", "\u{1f600}"])
                .decode(&json!("z")),
        );
        assert_eq!(issue.meta()["allowed"], json!(["\u{1f600}", "\u{ff21}"]));
        assert_eq!(issue.message(), "must be one of [\u{1f600}, \u{ff21}]");
    }

    #[test]
    fn format_issues_name_the_check_in_their_key() {
        let issue = first(string().email().decode(&json!("x")));
        assert_eq!(issue.code(), "invalid_format");
        assert_eq!(issue.message_key(), "invalid_format.email");
        assert_eq!(issue.message(), "not a valid email");
    }

    #[test]
    fn a_message_goes_to_the_latest_constraint_past_transformations() {
        let decoder = string().min_length(3).trim().message("three or more");
        assert_eq!(
            first(decoder.decode(&json!("ab"))).message(),
            "three or more"
        );
        let decoder = string().trim().message("give a name");
        assert_eq!(first(decoder.decode(&Value::Null)).message(), "give a name");
    }
}
