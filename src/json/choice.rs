use super::object::{Field, field};
use super::string::{StringDecoder, string};
use crate::codes;
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::path::Path;
use serde_json::Value;
use std::borrow::Cow;

/// A decoder of a JSON string naming one of `variants`, matched without regard to case.
///
/// A string naming none is `invalid_format` with the names, in the order given, as `allowed`.
///
/// ```
/// use raoh::json::prelude::*;
///
/// #[derive(Clone, Debug, PartialEq)]
/// enum Color { Red, Green }
///
/// let color = enum_of([("red", Color::Red), ("green", Color::Green)]);
/// assert_eq!(color.decode(&json!("RED")).unwrap(), Color::Red);
/// assert!(color.decode(&json!("blue")).is_err());
/// ```
pub fn enum_of<'a, T: Clone>(variants: impl IntoIterator<Item = (&'a str, T)>) -> EnumOf<T> {
    let variants: Vec<(String, T)> = variants
        .into_iter()
        .map(|(name, value)| (name.to_lowercase(), value))
        .collect();
    let allowed: Vec<String> = variants.iter().map(|(name, _)| name.clone()).collect();
    EnumOf { variants, allowed }
}

/// The decoder [`enum_of`] returns.
#[derive(Clone, Debug)]
pub struct EnumOf<T> {
    variants: Vec<(String, T)>,
    allowed: Vec<String>,
}

impl<T: Clone> Decoder<Value> for EnumOf<T> {
    type Output = T;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<T, Issues> {
        let name = string().decode_at(input, path)?.to_lowercase();
        self.variants
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| {
                Issue::at_path(path, codes::INVALID_FORMAT, "invalid value")
                    .with_meta("allowed", self.allowed.clone())
                    .into()
            })
    }
}

/// A decoder of a JSON string that must be exactly `expected`: `invalid_format` with `expected`
/// otherwise.
pub fn literal(expected: impl Into<String>) -> Literal {
    Literal(expected.into())
}

/// The decoder [`literal`] returns.
#[derive(Clone, Debug)]
pub struct Literal(String);

impl Decoder<Value> for Literal {
    type Output = String;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<String, Issues> {
        let found = string().decode_at(input, path)?;
        if found == self.0 {
            Ok(found)
        } else {
            Err(Issue::at_path(path, codes::INVALID_FORMAT, "invalid value")
                .with_meta("expected", self.0.clone())
                .into())
        }
    }
}

/// A decoder that reads the string member `tag_field` and decodes the whole input with the
/// variant it names.
///
/// `variants` is a tuple of [`variant`]s with the same output. A tag naming none of them is
/// `not_allowed` at the tag's path, with the sorted tags as `allowed`.
///
/// ```
/// use raoh::json::prelude::*;
///
/// #[derive(Debug, PartialEq)]
/// enum Contact { Email(String), Phone(String) }
///
/// let contact = discriminate(
///     "type",
///     (
///         variant("email", object((field("address", string().email()),)).map(|(a,)| Contact::Email(a))),
///         variant("phone", object((field("number", string()),)).map(|(n,)| Contact::Phone(n))),
///     ),
/// );
/// let found = contact.decode(&json!({"type": "phone", "number": "03-0000-0000"})).unwrap();
/// assert_eq!(found, Contact::Phone("03-0000-0000".into()));
/// ```
///
/// # Panics
///
/// When two variants have the same tag.
pub fn discriminate<V: Variants>(
    tag_field: impl Into<Cow<'static, str>>,
    variants: V,
) -> Discriminate<V> {
    let tag_field = tag_field.into();
    let mut allowed: Vec<String> = variants.tags().into_iter().map(str::to_owned).collect();
    allowed.sort();
    if let Some(pair) = allowed.windows(2).find(|pair| pair[0] == pair[1]) {
        panic!("duplicate variant tag '{}'", pair[0]);
    }
    Discriminate {
        tag: field(tag_field.clone(), string()),
        tag_field,
        variants,
        allowed,
    }
}

/// The decoder [`discriminate`] returns.
#[derive(Clone, Debug)]
pub struct Discriminate<V> {
    tag: Field<StringDecoder>,
    tag_field: Cow<'static, str>,
    variants: V,
    allowed: Vec<String>,
}

impl<V: Variants> Decoder<Value> for Discriminate<V> {
    type Output = V::Output;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<V::Output, Issues> {
        let tag = self.tag.decode_at(input, path)?;
        self.variants
            .decode_variant(&tag, input, path)
            .unwrap_or_else(|| {
                let listed = self.allowed.join(", ");
                Err(Issue::at_path(
                    &path.key(&self.tag_field),
                    codes::NOT_ALLOWED,
                    format!("must be one of [{listed}]"),
                )
                .with_meta("allowed", self.allowed.clone())
                .into())
            })
    }
}

/// One case of [`discriminate`]: the tag that names it and the decoder of its input.
pub fn variant<D>(tag: impl Into<Cow<'static, str>>, decoder: D) -> Variant<D> {
    Variant {
        tag: tag.into(),
        decoder,
    }
}

/// The case [`variant`] returns.
#[derive(Clone, Debug)]
pub struct Variant<D> {
    tag: Cow<'static, str>,
    decoder: D,
}

/// A tuple of [`Variant`]s with the same output.
pub trait Variants {
    /// What each variant gives.
    type Output;

    /// The tag of every variant.
    fn tags(&self) -> Vec<&str>;

    /// The result of the variant named `tag`, or `None` when no variant has that tag.
    fn decode_variant(
        &self,
        tag: &str,
        input: &Value,
        path: &Path<'_>,
    ) -> Option<Result<Self::Output, Issues>>;
}

macro_rules! variants {
    ($First:ident $first:tt $(, $T:ident $idx:tt)*) => {
        impl<$First: Decoder<Value>, $($T: Decoder<Value, Output = $First::Output>),*> Variants
            for (Variant<$First>, $(Variant<$T>,)*)
        {
            type Output = $First::Output;

            fn tags(&self) -> Vec<&str> {
                vec![&self.$first.tag $(, &self.$idx.tag)*]
            }

            fn decode_variant(
                &self,
                tag: &str,
                input: &Value,
                path: &Path<'_>,
            ) -> Option<Result<Self::Output, Issues>> {
                if self.$first.tag == tag {
                    return Some(self.$first.decoder.decode_at(input, path));
                }
                $(
                    if self.$idx.tag == tag {
                        return Some(self.$idx.decoder.decode_at(input, path));
                    }
                )*
                None
            }
        }
    };
}

variants!(A 0);
variants!(A 0, B 1);
variants!(A 0, B 1, C 2);
variants!(A 0, B 1, C 2, D 3);
variants!(A 0, B 1, C 2, D 3, E 4);
variants!(A 0, B 1, C 2, D 3, E 4, F 5);
variants!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
variants!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);
variants!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8);
variants!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9);
variants!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10);
variants!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11);
variants!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12);
variants!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12, O 13);
variants!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12, O 13, P 14);
variants!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12, O 13, P 14, Q 15);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{i64, object};
    use serde_json::json;

    fn shape() -> impl Decoder<Value, Output = i64> {
        discriminate(
            "kind",
            (
                variant("square", object((field("side", i64()),)).map(|(s,)| s * s)),
                variant(
                    "rect",
                    object((field("w", i64()), field("h", i64()))).map(|(w, h)| w * h),
                ),
            ),
        )
    }

    #[test]
    fn the_tag_picks_the_variant() {
        assert_eq!(
            shape()
                .decode(&json!({"kind": "rect", "w": 2, "h": 3}))
                .unwrap(),
            6
        );
    }

    #[test]
    fn an_unknown_tag_is_not_allowed_at_the_tag_path() {
        let issues = shape().decode(&json!({"kind": "circle"})).unwrap_err();
        let issue = issues.iter().next().unwrap();
        assert_eq!(issue.code(), "not_allowed");
        assert_eq!(issue.path().to_string(), "/kind");
        assert_eq!(issue.meta()["allowed"], json!(["rect", "square"]));
    }

    #[test]
    fn a_missing_tag_is_required() {
        let issues = shape().decode(&json!({})).unwrap_err();
        assert_eq!(issues.iter().next().unwrap().code(), "required");
    }

    #[test]
    #[should_panic(expected = "duplicate variant tag 'a'")]
    fn duplicate_tags_panic() {
        discriminate("t", (variant("a", i64()), variant("a", i64())));
    }

    #[test]
    fn literal_requires_the_exact_string() {
        let issues = literal("v1").decode(&json!("v2")).unwrap_err();
        assert_eq!(issues.iter().next().unwrap().meta()["expected"], "v1");
    }
}
