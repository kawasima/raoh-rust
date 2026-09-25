use super::{missing, type_mismatch};
use crate::codes;
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::path::Path;
use crate::presence::Presence;
use serde_json::Value;
use std::borrow::Cow;

/// A decoder of a group of members of one object, which [`strict`](Object::strict) can check for
/// members it does not declare.
///
/// `fields` is a tuple of [`field`], [`optional_field`] and [`presence_field`]; the decoder gives
/// their outputs as a tuple and reports the issues of every one of them.
///
/// ```
/// use raoh::json::prelude::*;
///
/// let point = object((field("x", i64()), field("y", i64()))).strict();
/// let issues = point.decode(&json!({"x": 1, "y": "2", "z": 3})).unwrap_err();
/// let codes: Vec<&str> = issues.iter().map(|i| i.code()).collect();
/// assert_eq!(codes, ["type_mismatch", "unknown_field"]);
/// ```
pub fn object<F: FieldSet>(fields: F) -> Object<F> {
    Object(fields)
}

/// The decoder [`object`] returns.
#[derive(Clone, Copy, Debug)]
pub struct Object<F>(F);

impl<F: FieldSet + Decoder<Value>> Decoder<Value> for Object<F> {
    type Output = F::Output;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<F::Output, Issues> {
        self.0.decode_at(input, path)
    }
}

impl<F: FieldSet> Object<F> {
    /// A decoder that also reports every member the fields do not declare as `unknown_field`,
    /// with the member's name as `field`, after the fields' own issues.
    ///
    /// The members are reported in the order the [`Value`] keeps its keys: the order of the input
    /// with `serde_json`'s `preserve_order` feature, as Raoh for Java reports them, and sorted
    /// without it.
    pub fn strict(self) -> Strict<F> {
        Strict(self.0)
    }
}

/// The decoder [`Object::strict`] returns.
#[derive(Clone, Copy, Debug)]
pub struct Strict<F>(F);

impl<F: FieldSet + Decoder<Value>> Decoder<Value> for Strict<F> {
    type Output = F::Output;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<F::Output, Issues> {
        let result = self.0.decode_at(input, path);
        let mut unknown = Issues::new();
        if let Value::Object(members) = input {
            let mut declared = Vec::new();
            self.0.field_names(&mut declared);
            for name in members.keys() {
                if !declared.contains(&name.as_str()) {
                    unknown.push(
                        Issue::at_path(&path.key(name), codes::UNKNOWN_FIELD, "unknown field")
                            .with_meta("field", name.clone()),
                    );
                }
            }
        }
        match result {
            Ok(value) if unknown.is_empty() => Ok(value),
            Ok(_) => Err(unknown),
            Err(mut issues) => {
                issues.merge(unknown);
                Err(issues)
            }
        }
    }
}

/// The members a group of fields declares.
pub trait FieldSet {
    /// Adds the name of every member declared to `names`.
    fn field_names<'a>(&'a self, names: &mut Vec<&'a str>);
}

/// A decoder of the member `name` of an object, which must be there.
///
/// A missing member is handed to `decoder` as [`missing()`](super::missing), so a built-in decoder
/// reports it as `required` at the member's path. An input that is not an object is
/// `type_mismatch` at the member's path, with `expected` `object`.
pub fn field<D>(name: impl Into<Cow<'static, str>>, decoder: D) -> Field<D> {
    Field {
        name: name.into(),
        decoder,
    }
}

/// The decoder [`field`] returns.
#[derive(Clone, Debug)]
pub struct Field<D> {
    name: Cow<'static, str>,
    decoder: D,
}

impl<D: Decoder<Value>> Decoder<Value> for Field<D> {
    type Output = D::Output;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<D::Output, Issues> {
        let at = path.key(&self.name);
        match input {
            Value::Object(members) => self
                .decoder
                .decode_at(members.get(self.name.as_ref()).unwrap_or(missing()), &at),
            other => Err(type_mismatch(&at, "object", other).into()),
        }
    }
}

/// A decoder of the member `name` of an object, which may be left out.
///
/// A missing member, or an input that is not an object, gives `None`. A member present as `null`
/// is handed to `decoder`; use [`presence_field`] to tell `null` apart, or
/// [`nullable`](super::JsonDecoderExt::nullable) to accept it.
pub fn optional_field<D>(name: impl Into<Cow<'static, str>>, decoder: D) -> OptionalField<D> {
    OptionalField {
        name: name.into(),
        decoder,
    }
}

/// The decoder [`optional_field`] returns.
#[derive(Clone, Debug)]
pub struct OptionalField<D> {
    name: Cow<'static, str>,
    decoder: D,
}

impl<D: Decoder<Value>> Decoder<Value> for OptionalField<D> {
    type Output = Option<D::Output>;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<Self::Output, Issues> {
        match input.get(self.name.as_ref()) {
            Some(member) => self
                .decoder
                .decode_at(member, &path.key(&self.name))
                .map(Some),
            None => Ok(None),
        }
    }
}

/// A decoder of the member `name` of an object that tells a missing member, a `null` one and one
/// with a value apart, as a PATCH request needs to.
///
/// A missing member, or an input that is not an object, gives [`Presence::Absent`].
pub fn presence_field<D>(name: impl Into<Cow<'static, str>>, decoder: D) -> PresenceField<D> {
    PresenceField {
        name: name.into(),
        decoder,
    }
}

/// The decoder [`presence_field`] returns.
#[derive(Clone, Debug)]
pub struct PresenceField<D> {
    name: Cow<'static, str>,
    decoder: D,
}

impl<D: Decoder<Value>> Decoder<Value> for PresenceField<D> {
    type Output = Presence<D::Output>;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<Self::Output, Issues> {
        match input.get(self.name.as_ref()) {
            None => Ok(Presence::Absent),
            Some(Value::Null) => Ok(Presence::Null),
            Some(member) => self
                .decoder
                .decode_at(member, &path.key(&self.name))
                .map(Presence::Present),
        }
    }
}

macro_rules! named_field_set {
    ($($t:ident),+) => {
        $(
            impl<D> FieldSet for $t<D> {
                fn field_names<'a>(&'a self, names: &mut Vec<&'a str>) {
                    names.push(&self.name);
                }
            }
        )+
    };
}

named_field_set!(Field, OptionalField, PresenceField);

macro_rules! tuple_field_set {
    ($($T:ident $idx:tt),+) => {
        impl<$($T: FieldSet),+> FieldSet for ($($T,)+) {
            fn field_names<'a>(&'a self, names: &mut Vec<&'a str>) {
                $( self.$idx.field_names(names); )+
            }
        }
    };
}

tuple_field_set!(A 0);
tuple_field_set!(A 0, B 1);
tuple_field_set!(A 0, B 1, C 2);
tuple_field_set!(A 0, B 1, C 2, D 3);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12, O 13);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12, O 13, P 14);
tuple_field_set!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12, O 13, P 14, Q 15);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{JsonDecoderExt, i64, string};
    use serde_json::json;

    #[test]
    fn a_missing_field_is_required_at_its_path() {
        let issues = object((field("name", string()),))
            .decode(&json!({}))
            .unwrap_err();
        let issue = issues.iter().next().unwrap();
        assert_eq!(issue.code(), "required");
        assert_eq!(issue.path().to_string(), "/name");
    }

    #[test]
    fn a_non_object_is_a_type_mismatch_at_each_field() {
        let decoder = object((field("a", i64()), field("b", i64())));
        let issues = decoder.decode(&json!([1])).unwrap_err();
        let paths: Vec<String> = issues.iter().map(|i| i.path().to_string()).collect();
        assert_eq!(paths, ["/a", "/b"]);
        assert_eq!(issues.iter().next().unwrap().meta()["actual"], "array");
    }

    #[test]
    fn nested_paths_join() {
        let item = object((field("name", string()),));
        let decoder = object((field("items", item.list()),));
        let issues = decoder
            .decode(&json!({"items": [{"name": "a"}, {}]}))
            .unwrap_err();
        assert_eq!(
            issues.iter().next().unwrap().path().to_string(),
            "/items/1/name"
        );
    }

    #[test]
    fn keys_holding_slash_and_tilde_are_escaped() {
        let decoder = object((field("a/b", i64()), field("", i64())));
        let issues = decoder.decode(&json!({})).unwrap_err();
        let paths: Vec<String> = issues.iter().map(|i| i.path().to_string()).collect();
        assert_eq!(paths, ["/a~1b", "/"]);
    }

    #[test]
    fn optional_field_gives_none_when_missing_and_requires_non_null() {
        let decoder = object((optional_field("nick", string()),));
        assert_eq!(decoder.decode(&json!({})).unwrap(), (None,));
        let issues = decoder.decode(&json!({"nick": null})).unwrap_err();
        assert_eq!(issues.iter().next().unwrap().code(), "required");
    }

    #[test]
    fn presence_field_tells_the_three_apart() {
        let decoder = object((presence_field("n", i64()),));
        assert_eq!(decoder.decode(&json!({})).unwrap(), (Presence::Absent,));
        assert_eq!(
            decoder.decode(&json!({"n": null})).unwrap(),
            (Presence::Null,)
        );
        assert_eq!(
            decoder.decode(&json!({"n": 1})).unwrap(),
            (Presence::Present(1),)
        );
    }

    #[test]
    fn strict_reports_every_unknown_member() {
        let decoder = object((field("a", i64()),)).strict();
        let issues = decoder
            .decode(&json!({"a": 1, "b": 2, "c": 3}))
            .unwrap_err();
        let paths: Vec<String> = issues.iter().map(|i| i.path().to_string()).collect();
        assert_eq!(paths, ["/b", "/c"]);
        assert_eq!(issues.iter().next().unwrap().meta()["field"], "b");
    }
}
