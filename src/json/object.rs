use super::{missing, unexpected};
use crate::codes;
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::path::Path;
use crate::presence::Presence;
use serde_json::{Map, Value};
use std::borrow::Cow;

mod sealed {
    pub trait Sealed {}
}

/// A decoder of the members of one object.
///
/// The input must be an object: missing or `null` is `required`, and any other type is
/// `type_mismatch` with `expected` `object`, reported once at the object's own path. Once it is
/// one, `fields`, a tuple of [`field`], [`optional_field`] and [`presence_field`], read its
/// members; the decoder gives their outputs as a tuple and reports the issues of every one of
/// them. [`strict`](Object::strict) also reports the members they do not declare.
///
/// A field is not a decoder on its own, so it is only ever read from an object.
///
/// ```
/// use raoh::json::prelude::*;
///
/// let point = object((field("x", i64()), field("y", i64()))).strict();
/// let issues = point.decode(&json!({"x": 1, "y": "2", "z": 3})).unwrap_err();
/// let codes: Vec<&str> = issues.iter().map(|i| i.code()).collect();
/// assert_eq!(codes, ["type_mismatch", "unknown_field"]);
///
/// let nickname = object((optional_field("nickname", string()),));
/// assert!(nickname.decode(&json!("not an object")).is_err());
/// ```
pub fn object<F: FieldSet>(fields: F) -> Object<F> {
    Object(fields)
}

/// The decoder [`object`] returns.
#[derive(Clone, Copy, Debug)]
pub struct Object<F>(F);

impl<F: FieldSet> Decoder<Value> for Object<F> {
    type Output = F::Output;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<F::Output, Issues> {
        match input {
            Value::Object(members) => self.0.decode_fields(members, path),
            other => Err(unexpected(path, "object", other).into()),
        }
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

impl<F: FieldSet> Decoder<Value> for Strict<F> {
    type Output = F::Output;

    fn decode_at(&self, input: &Value, path: &Path<'_>) -> Result<F::Output, Issues> {
        let Value::Object(members) = input else {
            return Err(unexpected(path, "object", input).into());
        };
        let result = self.0.decode_fields(members, path);
        let mut declared = Vec::new();
        self.0.field_names(&mut declared);
        let unknown: Issues = members
            .keys()
            .filter(|name| !declared.contains(&name.as_str()))
            .map(|name| {
                Issue::at_path(&path.key(name), codes::UNKNOWN_FIELD)
                    .with_meta("field", name.clone())
            })
            .collect();
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

/// A group of fields [`object`] reads: a [`Field`], [`OptionalField`] or [`PresenceField`], or a
/// tuple of them. It cannot be implemented outside this crate.
pub trait FieldSet: sealed::Sealed {
    /// What the fields give.
    type Output;

    /// Reads the fields from the members of an object found at `path`.
    #[doc(hidden)]
    fn decode_fields(
        &self,
        members: &Map<String, Value>,
        path: &Path<'_>,
    ) -> Result<Self::Output, Issues>;

    /// Adds the name of every member declared to `names`.
    #[doc(hidden)]
    fn field_names<'a>(&'a self, names: &mut Vec<&'a str>);
}

/// The member `name` of an object, which must be there.
///
/// A missing member is handed to `decoder` as [`missing()`](super::missing), so a built-in decoder
/// reports it as `required` at the member's path.
pub fn field<D>(name: impl Into<Cow<'static, str>>, decoder: D) -> Field<D> {
    Field {
        name: name.into(),
        decoder,
    }
}

/// The field [`field`] returns.
#[derive(Clone, Debug)]
pub struct Field<D> {
    name: Cow<'static, str>,
    decoder: D,
}

impl<D> sealed::Sealed for Field<D> {}

impl<D: Decoder<Value>> FieldSet for Field<D> {
    type Output = D::Output;

    fn decode_fields(
        &self,
        members: &Map<String, Value>,
        path: &Path<'_>,
    ) -> Result<D::Output, Issues> {
        let member = members.get(self.name.as_ref()).unwrap_or(missing());
        self.decoder.decode_at(member, &path.key(&self.name))
    }

    fn field_names<'a>(&'a self, names: &mut Vec<&'a str>) {
        names.push(&self.name);
    }
}

/// The member `name` of an object, which may be left out: `None` when it is missing.
///
/// A member present as `null` is handed to `decoder`; use [`presence_field`] to tell `null` apart,
/// or [`nullable`](super::JsonDecoderExt::nullable) to accept it.
pub fn optional_field<D>(name: impl Into<Cow<'static, str>>, decoder: D) -> OptionalField<D> {
    OptionalField {
        name: name.into(),
        decoder,
    }
}

/// The field [`optional_field`] returns.
#[derive(Clone, Debug)]
pub struct OptionalField<D> {
    name: Cow<'static, str>,
    decoder: D,
}

impl<D> sealed::Sealed for OptionalField<D> {}

impl<D: Decoder<Value>> FieldSet for OptionalField<D> {
    type Output = Option<D::Output>;

    fn decode_fields(
        &self,
        members: &Map<String, Value>,
        path: &Path<'_>,
    ) -> Result<Self::Output, Issues> {
        members
            .get(self.name.as_ref())
            .map(|member| self.decoder.decode_at(member, &path.key(&self.name)))
            .transpose()
    }

    fn field_names<'a>(&'a self, names: &mut Vec<&'a str>) {
        names.push(&self.name);
    }
}

/// The member `name` of an object, telling a missing member, a `null` one and one with a value
/// apart, as a PATCH request needs to.
pub fn presence_field<D>(name: impl Into<Cow<'static, str>>, decoder: D) -> PresenceField<D> {
    PresenceField {
        name: name.into(),
        decoder,
    }
}

/// The field [`presence_field`] returns.
#[derive(Clone, Debug)]
pub struct PresenceField<D> {
    name: Cow<'static, str>,
    decoder: D,
}

impl<D> sealed::Sealed for PresenceField<D> {}

impl<D: Decoder<Value>> FieldSet for PresenceField<D> {
    type Output = Presence<D::Output>;

    fn decode_fields(
        &self,
        members: &Map<String, Value>,
        path: &Path<'_>,
    ) -> Result<Self::Output, Issues> {
        match members.get(self.name.as_ref()) {
            None => Ok(Presence::Absent),
            Some(Value::Null) => Ok(Presence::Null),
            Some(member) => self
                .decoder
                .decode_at(member, &path.key(&self.name))
                .map(Presence::Present),
        }
    }

    fn field_names<'a>(&'a self, names: &mut Vec<&'a str>) {
        names.push(&self.name);
    }
}

/// Reads every field of the tuple and keeps every issue, in the order the fields are written.
macro_rules! tuple_field_set {
    ($($T:ident $v:ident $idx:tt),+) => {
        impl<$($T: FieldSet),+> sealed::Sealed for ($($T,)+) {}

        impl<$($T: FieldSet),+> FieldSet for ($($T,)+) {
            type Output = ($($T::Output,)+);

            fn decode_fields(
                &self,
                members: &Map<String, Value>,
                path: &Path<'_>,
            ) -> Result<Self::Output, Issues> {
                let mut issues = Issues::new();
                $(
                    let $v = match self.$idx.decode_fields(members, path) {
                        Ok(value) => Some(value),
                        Err(found) => {
                            issues.merge(found);
                            None
                        }
                    };
                )+
                match ($($v,)+) {
                    ($(Some($v),)+) => Ok(($($v,)+)),
                    _ => Err(issues),
                }
            }

            fn field_names<'a>(&'a self, names: &mut Vec<&'a str>) {
                $( self.$idx.field_names(names); )+
            }
        }
    };
}

tuple_field_set!(A a 0);
tuple_field_set!(A a 0, B b 1);
tuple_field_set!(A a 0, B b 1, C c 2);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10, M m 11);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10, M m 11, N n 12);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10, M m 11, N n 12, O o 13);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10, M m 11, N n 12, O o 13, P p 14);
tuple_field_set!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10, M m 11, N n 12, O o 13, P p 14, Q q 15);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{JsonDecoderExt, i64, string};
    use serde_json::json;

    fn paths(issues: &Issues) -> Vec<String> {
        issues.iter().map(|i| i.path().to_string()).collect()
    }

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
    fn a_non_object_is_one_issue_at_the_object_path() {
        let decoder = object((field("a", i64()), field("b", i64())));
        let issues = decoder.decode(&json!([1])).unwrap_err();
        assert_eq!(paths(&issues), [""]);
        let issue = issues.iter().next().unwrap();
        assert_eq!(issue.code(), "type_mismatch");
        assert_eq!(issue.meta()["expected"], "object");
        assert_eq!(issue.meta()["actual"], "array");
        let issues = decoder.decode(&Value::Null).unwrap_err();
        assert_eq!(issues.iter().next().unwrap().code(), "required");
    }

    #[test]
    fn optional_and_presence_fields_do_not_accept_a_non_object() {
        for input in [json!("x"), json!(1), json!([]), Value::Null] {
            assert!(
                object((optional_field("n", string()),))
                    .decode(&input)
                    .is_err(),
                "{input}"
            );
            assert!(
                object((presence_field("n", string()),))
                    .decode(&input)
                    .is_err(),
                "{input}"
            );
            assert!(
                object((optional_field("n", string()),))
                    .strict()
                    .decode(&input)
                    .is_err(),
                "{input}"
            );
        }
    }

    #[test]
    fn nested_paths_join() {
        let item = object((field("name", string()),));
        let decoder = object((field("items", item.list()),));
        let issues = decoder
            .decode(&json!({"items": [{"name": "a"}, {}]}))
            .unwrap_err();
        assert_eq!(paths(&issues), ["/items/1/name"]);
    }

    #[test]
    fn keys_holding_slash_and_tilde_are_escaped() {
        let decoder = object((field("a/b", i64()), field("", i64())));
        let issues = decoder.decode(&json!({})).unwrap_err();
        assert_eq!(paths(&issues), ["/a~1b", "/"]);
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
    fn strict_reports_every_unknown_member_after_the_fields_issues() {
        let decoder = object((field("a", i64()),)).strict();
        let issues = decoder
            .decode(&json!({"a": "x", "b": 2, "c": 3}))
            .unwrap_err();
        assert_eq!(paths(&issues), ["/a", "/b", "/c"]);
        assert_eq!(issues.iter().nth(1).unwrap().meta()["field"], "b");
    }
}
