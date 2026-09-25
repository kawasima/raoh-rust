use super::steps::Steps;
use super::unexpected;
use crate::codes;
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::path::Path;
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

fn char_count(s: &str) -> usize {
    s.chars().count()
}

impl StringDecoder {
    fn transform(mut self, f: fn(&str) -> String) -> Self {
        self.steps.push(move |s| Ok(f(&s)));
        self
    }

    fn format(
        mut self,
        ok: impl Fn(&str) -> bool + Send + Sync + 'static,
        message: &'static str,
    ) -> Self {
        self.steps.require(
            move |s| ok(s),
            move |_| Issue::new(codes::INVALID_FORMAT, message),
        );
        self
    }

    /// Gives the constraint written just before this, or the type check when there is none, a
    /// custom message that no resolver rewrites.
    ///
    /// ```
    /// use raoh::json::prelude::*;
    ///
    /// let code = string().min_length(3).message("too short a code");
    /// let issues = code.decode(&json!("ab")).unwrap_err();
    /// assert_eq!(issues.iter().next().unwrap().message(), "too short a code");
    /// ```
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.steps.set_message(message.into());
        self
    }

    /// Removes leading and trailing whitespace.
    pub fn trim(self) -> Self {
        self.transform(|s| s.trim().to_owned())
    }

    /// Converts to lower case.
    pub fn lowercase(self) -> Self {
        self.transform(str::to_lowercase)
    }

    /// Converts to upper case.
    pub fn uppercase(self) -> Self {
        self.transform(str::to_uppercase)
    }

    /// Requires a character other than whitespace: `blank`.
    pub fn non_blank(mut self) -> Self {
        self.steps.require(
            |s| !s.trim().is_empty(),
            |_| Issue::new(codes::BLANK, "must not be blank"),
        );
        self
    }

    /// Requires at least `n` characters: `too_short` with `min` and `actual`.
    pub fn min_length(mut self, n: usize) -> Self {
        self.steps.require(
            move |s| char_count(s) >= n,
            move |s| {
                Issue::new(codes::TOO_SHORT, format!("must be at least {n} characters"))
                    .with_meta("min", n)
                    .with_meta("actual", char_count(s))
            },
        );
        self
    }

    /// Allows at most `n` characters: `too_long` with `max` and `actual`.
    pub fn max_length(mut self, n: usize) -> Self {
        self.steps.require(
            move |s| char_count(s) <= n,
            move |s| {
                Issue::new(codes::TOO_LONG, format!("must be at most {n} characters"))
                    .with_meta("max", n)
                    .with_meta("actual", char_count(s))
            },
        );
        self
    }

    /// Requires exactly `n` characters: `invalid_length` with `expected` and `actual`.
    pub fn length(mut self, n: usize) -> Self {
        self.steps.require(
            move |s| char_count(s) == n,
            move |s| {
                Issue::new(
                    codes::INVALID_LENGTH,
                    format!("must be exactly {n} characters"),
                )
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
                Issue::new(
                    codes::INVALID_FORMAT,
                    format!("must start with \"{prefix}\""),
                )
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
                Issue::new(codes::INVALID_FORMAT, format!("must end with \"{suffix}\""))
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
                Issue::new(
                    codes::INVALID_FORMAT,
                    format!("must include \"{substring}\""),
                )
                .with_meta("substring", substring.clone())
            },
        );
        self
    }

    /// Requires one of `allowed`: `not_allowed` with the sorted `allowed` and `actual`.
    pub fn one_of<S: Into<String>>(mut self, allowed: impl IntoIterator<Item = S>) -> Self {
        let mut allowed: Vec<String> = allowed.into_iter().map(Into::into).collect();
        allowed.sort();
        allowed.dedup();
        let message = format!("must be one of [{}]", allowed.join(", "));
        let check = allowed.clone();
        self.steps.require(
            move |s| check.binary_search(s).is_ok(),
            move |s| {
                Issue::new(codes::NOT_ALLOWED, message.clone())
                    .with_meta("allowed", allowed.clone())
                    .with_meta("actual", s.clone())
            },
        );
        self
    }

    /// Requires the form of an email address, as Raoh for Java checks it: `invalid_format`.
    pub fn email(self) -> Self {
        self.format(is_email, "not a valid email")
    }

    /// Requires an IPv4 address in dotted decimal: `invalid_format`.
    pub fn ipv4(self) -> Self {
        self.format(
            |s| s.len() <= MAX_IP_LENGTH && is_ipv4(s),
            "not a valid IPv4 address",
        )
    }

    /// Requires an IPv6 address, optionally in brackets: `invalid_format`.
    pub fn ipv6(self) -> Self {
        self.format(
            |s| s.len() <= MAX_IP_LENGTH && is_ipv6(s),
            "not a valid IPv6 address",
        )
    }

    /// Requires an IPv4 or IPv6 address: `invalid_format`.
    pub fn ip(self) -> Self {
        self.format(
            |s| s.len() <= MAX_IP_LENGTH && (is_ipv4(s) || is_ipv6(s)),
            "not a valid IP address",
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
            "not a valid ULID",
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
            "not a valid CUID",
        )
    }

    /// Requires the whole string to match `pattern`: `invalid_format` with `pattern`.
    ///
    /// The pattern is anchored at both ends, as Java's `Matcher.matches` is.
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
            move |_| {
                Issue::new(codes::INVALID_FORMAT, "invalid format")
                    .with_meta("pattern", pattern.clone())
            },
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

    /// A decoder that reads the string as a UUID: `invalid_format` when it is not one.
    #[cfg(feature = "uuid")]
    pub fn uuid(self) -> UuidDecoder {
        UuidDecoder {
            string: self,
            message: None,
        }
    }

    /// A decoder that reads the string as an `http` or `https` URL with a host: `invalid_format`
    /// when it is not one.
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

fn invalid_format(path: &Path<'_>, message: &str, custom: &Option<String>) -> Issues {
    let issue = Issue::at_path(path, codes::INVALID_FORMAT, message);
    match custom {
        Some(custom) => issue.with_custom_message(custom.clone()).into(),
        None => issue.into(),
    }
}

impl<T: FromStr> Decoder<Value> for Parse<T> {
    type Output = T;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<T, Issues> {
        let s = self.string.decode_at(input, path)?;
        s.parse()
            .map_err(|_| invalid_format(path, "invalid format", &self.message))
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
            .map_err(|_| invalid_format(path, "not a valid UUID", &self.message))
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
        let fail = || invalid_format(path, "not a valid URL", &self.message);
        if s.len() > MAX_URL_LENGTH {
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

/// `^[a-zA-Z0-9._%+\-]{1,64}@[a-zA-Z0-9.\-]{1,255}\.[a-zA-Z]{2,}$`, at most 254 characters.
fn is_email(s: &str) -> bool {
    if s.chars().count() > MAX_EMAIL_LENGTH {
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

fn is_ipv6(s: &str) -> bool {
    let bare = s
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(s);
    bare.contains(':') && bare.parse::<std::net::Ipv6Addr>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn code_of<D: Decoder<Value>>(decoder: &D, input: Value) -> String {
        decoder
            .decode(&input)
            .err()
            .unwrap()
            .iter()
            .next()
            .unwrap()
            .code()
            .to_owned()
    }

    #[test]
    fn missing_and_null_are_required_and_other_types_mismatch() {
        let s = string();
        assert_eq!(code_of(&s, Value::Null), "required");
        assert_eq!(code_of(&s, json!(1)), "type_mismatch");
        let issues = s.decode(&json!(1)).unwrap_err();
        assert_eq!(issues.iter().next().unwrap().meta()["actual"], "number");
    }

    #[test]
    fn the_first_failing_constraint_is_reported() {
        let s = string().min_length(3).email();
        let issues = s.decode(&json!("a")).unwrap_err();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues.iter().next().unwrap().code(), "too_short");
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        assert!(string().max_length(2).decode(&json!("日本")).is_ok());
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
    fn ipv6_accepts_brackets() {
        assert!(is_ipv6("::1"));
        assert!(is_ipv6("[2001:db8::1]"));
        assert!(!is_ipv6("1.2.3.4"));
    }

    #[test]
    fn one_of_reports_the_sorted_allowed_values() {
        let s = string().one_of(["b", "a"]);
        let issues = s.decode(&json!("c")).unwrap_err();
        let issue = issues.iter().next().unwrap();
        assert_eq!(issue.meta()["allowed"], json!(["a", "b"]));
        assert_eq!(issue.message(), "must be one of [a, b]");
    }

    #[test]
    fn a_message_before_any_constraint_applies_to_the_type_check() {
        let s = string().message("give a name");
        let issues = s.decode(&Value::Null).unwrap_err();
        assert!(issues.iter().next().unwrap().is_custom_message());
    }
}
