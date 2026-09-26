# raoh

Rust port of [Raoh](https://github.com/kawasima/raoh), a decoder library for turning untyped
boundary input into typed domain values.

It is built around a parse-don't-validate approach:

- decode at the boundary
- keep invalid states out of the domain model
- return failures as values instead of panicking
- attach structured errors to precise paths

Serde already turns JSON text into a `serde_json::Value`. raoh turns that `Value` into domain
values, and when the input is wrong it reports every problem it found, each with the JSON Pointer
of where it was, instead of stopping at the first one.

```text
JSON text --serde_json--> serde_json::Value --raoh--> domain values
                                               \--> Issues (path, code, message, meta)
```

A domain type does not derive `Deserialize` and its fields stay private. The only way to get a
value of it from outside is through its decoder, so a value that exists has been checked.

## Installation

```toml
[dependencies]
raoh = "0.1"
```

Optional features:

| Feature   | Adds                                                        |
|-----------|-------------------------------------------------------------|
| `regex`   | `string().pattern(...)`                                     |
| `decimal` | `decimal()`, decoding into `rust_decimal::Decimal`          |
| `uuid`    | `string().uuid()`, decoding into `uuid::Uuid`               |
| `url`     | `string().url()`, decoding into `url::Url`                  |

The minimum supported Rust version is 1.87, with every feature.

## Quick start

```rust
use raoh::json::prelude::*;

#[derive(Debug)]
pub struct Email(String);

#[derive(Debug)]
pub struct Age(u32);

#[derive(Debug)]
pub struct User {
    email: Email,
    age: Age,
}

fn email() -> impl Decoder<Value, Output = Email> {
    string().trim().lowercase().email().map(Email)
}

fn age() -> impl Decoder<Value, Output = Age> {
    u32().range(0..=150).map(Age)
}

fn user() -> impl Decoder<Value, Output = User> {
    object((
        field("email", email()),
        field("age", age()),
    ))
    .map(|(email, age)| User { email, age })
}

let issues = from_str(&user(), r#"{"email": "not an email", "age": 200}"#).unwrap_err();
assert_eq!(
    issues.to_json(),
    json!([
        {"path": "/email", "code": "invalid_format", "message": "not a valid email", "meta": {}},
        {"path": "/age", "code": "out_of_range", "message": "must be between 0 and 150",
         "meta": {"min": 0, "max": 150, "actual": 200}}
    ])
);
```

`Issues` implements `std::error::Error` and `serde::Serialize`, so it can be returned from a
handler or written out as the response body as it is.

## The model

### `Decoder`

```rust,ignore
pub trait Decoder<I: ?Sized> {
    type Output;
    fn decode_at(&self, input: &I, path: &Path<'_>) -> Result<Self::Output, Issues>;
    fn decode(&self, input: &I) -> Result<Self::Output, Issues>;
    // map, and_then, pipe, refine, with_default, recover, boxed
}
```

A decoder is a value that describes how to read an input. It holds no state and can be reused.
Decoders compose like iterator adapters, and the composed type is hidden behind
`impl Decoder<Value, Output = T>`. Where a type has to be named, such as a recursive decoder or a
decoder kept in a struct field or a `static`, `.boxed()` turns it into a
`BoxDecoder<Value, T>`, which is `Send + Sync`.

The walk down the input uses a `Path` borrowed from the stack, so a successful decode allocates
nothing for paths. A path is copied out into a `Pointer` only when an issue is recorded.

### `Issue` and `Issues`

Each issue has:

- `path`: a JSON Pointer (RFC 6901), such as `/items/0/name`
- `code`: what kind of problem it is, such as `required` or `out_of_range`
- `message_key`: the code, or a refinement of it such as `out_of_range.minimum`
- `meta`: what else the code says, such as `min`, `max` and `actual`

An issue carries no sentence of its own. `issue.message()` writes one from the English catalogue,
and `issue.message_with(Messages::japanese())` from another. The only sentence an issue carries is
one its creator gave with `with_message(...)`, which every language then shows as written:

```rust
use raoh::{Issue, Messages};

let built_in = Issue::new("too_short").with_meta("min", 3);
assert_eq!(built_in.message_with(Messages::japanese()), "3文字以上で入力してください");

let custom = Issue::new("checksum").with_message("the check digit does not match");
assert_eq!(custom.message_with(Messages::japanese()), "the check digit does not match");
```

The codes, message keys and meta keys are the same as in Raoh for Java from 0.7.3 on, and the
codes and meta keys the same as in raoh-php, so the same client-side
handling works for all of them, and a catalogue written for Raoh for Java resolves these issues
too. `tests/compat` runs the same inputs through Raoh for Java and checks this crate gives the same
issues; the cases where it does not on purpose are listed there and under
[Differences from Raoh for Java](#differences-from-raoh-for-java).

`Issues` keeps them in the order they were found. `flatten()` groups the English messages by
path, and `to_json()` gives the `[{"path", "code", "message", "meta"}]` form, which is also what
`serde::Serialize` writes. `flatten_with` and `to_json_with` take another catalogue.

## Combining decoders

### Independent parts: tuples

A tuple of decoders over the same input is a decoder. It runs every part and reports the issues
of all of them. `object` wraps a tuple of fields, which also allows `.strict()`.

```rust
use raoh::json::prelude::*;

let point = object((field("x", i64()), field("y", i64()))).strict();

let issues = point.decode(&json!({"x": "1", "y": 2, "z": 3})).unwrap_err();
let paths: Vec<String> = issues.iter().map(|i| i.path().to_string()).collect();
assert_eq!(paths, ["/x", "/z"]);
```

### Dependent rules: `and_then`

A rule that relates several parts runs once the parts have decoded. The function returns
`Result<T, E>` where `E: Into<Issues>`, so a domain constructor returning `Result<Self, Issue>`
can be passed directly. An issue it returns is read as relative to where the decoder is.

```rust
use raoh::json::prelude::*;
use raoh::{Issue, Pointer};

#[derive(Debug)]
pub struct Period { start: i64, end: i64 }

impl Period {
    pub fn new(start: i64, end: i64) -> Result<Self, Issue> {
        if start > end {
            let at: Pointer = ["end"].into_iter().collect();
            return Err(Issue::new("invalid_value").with_message("must not be before start").at(at));
        }
        Ok(Self { start, end })
    }
}

let period = object((field("start", i64()), field("end", i64())))
    .and_then(|(start, end)| Period::new(start, end));
let trip = object((field("period", period),));

let issues = trip.decode(&json!({"period": {"start": 5, "end": 1}})).unwrap_err();
assert_eq!(issues.iter().next().unwrap().path().to_string(), "/period/end");
```

Tuples accumulate issues; `and_then` stops at the first failure, because the rule cannot be
checked before its parts exist.

### Decoder into decoder: `pipe`

`pipe` hands one decoder's output to another decoder as its input, at the same path. It is how a
decoder written for a domain type is composed with the boundary checks in front of it.

### Constraints without a new type: `refine`

`refine(predicate, code, message)` keeps the output type and adds a check, reported with the
message as a custom one.

## Built-in decoders

All of these live in `raoh::json` and come with `use raoh::json::prelude::*`. Missing or `null`
input is `required` for every one of them, and a value of another JSON type is `type_mismatch`.
The constraints of one decoder run in the order written, and the first to fail is reported.

`string()`: `trim`, `lowercase`, `uppercase`, `non_blank`, `min_length`, `max_length`, `length`,
`starts_with`, `ends_with`, `contains`, `one_of`, `email`, `ip`, `ipv4`, `ipv6`, `ulid`, `cuid`,
`pattern` (feature `regex`), and the conversions `parse::<T: FromStr>()`, `uuid()` (feature
`uuid`) and `url()` (feature `url`). Lengths count characters, not bytes.

`i32()`, `i64()`, `u32()`, `u64()`: `min`, `max`, `range(a..=b)`, `positive`, `multiple_of`,
`one_of`, and for the signed ones `negative`, `non_negative` and `non_positive`. A number with a
fraction, or one the type cannot hold, is `type_mismatch`.

`f64()`: `min`, `max`, `range`, `positive`, `negative`, `non_negative`, `non_positive`, `one_of`.

`decimal()` (feature `decimal`): the numeric constraints plus `multiple_of` and `scale`.
`serde_json` keeps a number as its nearest `f64` unless its `arbitrary_precision` feature is on,
so enable that feature in the application when decimals must be exact.

`bool()`: `is_true`, `is_false`.

Every one of them takes `.message("...")`, which gives the most recent constraint written before
it a custom message. Transformations such as `trim` cannot fail and are passed over, so
`string().trim().message("...")` gives the message to the type check, and
`string().min_length(3).trim().message("...")` gives it to `min_length`.

Whitespace, character counts, string order and number formatting follow Raoh for Java: `trim`
removes the characters up to U+0020 as `String.trim()` does and keeps U+3000, `non_blank` decides
whitespace as `Character.isWhitespace` does, lengths count code points, `one_of` and
`discriminate` sort by code point as Raoh for Java's `CodePointOrder` does, and a fractional bound
appears in a message as `Double.toString` writes it, such as `1.0E7`.

## Objects, lists and maps

- `field(name, d)`: a member that must be there
- `optional_field(name, d)`: `Option<T>`, `None` when the member is missing
- `presence_field(name, d)`: `Presence<T>`, one of `Absent`, `Null` or `Present(T)`
- `d.nullable()`: `Option<T>`, `None` when the value is `null`
- `d.list()`: `Vec<T>`, with `non_empty`, `min_size`, `max_size`, `size` and `unique`
- `dict(d)`: `HashMap<String, T>` from an object used as a map

`object` requires its input to be an object. Anything else is one issue at the object's own path:
`required` for missing or `null`, `type_mismatch` otherwise. A field is not a decoder on its own,
so `optional_field` never reads a scalar as an object without that member.

A missing member and a `null` one are different inputs. `field("note", string().nullable())`
accepts `null` but reports a missing member as `required`, while `optional_field` accepts a
missing member but not `null`. `presence_field` tells all three apart, which is what a PATCH
request needs:

```rust
use raoh::json::prelude::*;

let nickname = object((presence_field("nickname", string()),));

assert_eq!(nickname.decode(&json!({})).unwrap(), (Presence::Absent,));
assert_eq!(nickname.decode(&json!({"nickname": null})).unwrap(), (Presence::Null,));
assert_eq!(
    nickname.decode(&json!({"nickname": "Ken"})).unwrap(),
    (Presence::Present("Ken".to_string()),),
);
```

## Choices

- `enum_of([("red", Color::Red), ...])`: a string naming one of the values, case-insensitively
- `literal("v1")`: exactly that string
- `one_of((a, b, ...))`: the first alternative that decodes, or `one_of_failed` with each
  alternative's issues in `meta.candidates`
- `discriminate("type", (variant("a", da), variant("b", db), ...))`: the variant the member
  `type` names

```rust
use raoh::json::prelude::*;

#[derive(Debug, PartialEq)]
pub enum Contact {
    Email(String),
    Phone(String),
}

fn contact() -> impl Decoder<Value, Output = Contact> {
    discriminate(
        "type",
        (
            variant("email", object((field("address", string().email()),)).map(|(a,)| Contact::Email(a))),
            variant("phone", object((field("number", string()),)).map(|(n,)| Contact::Phone(n))),
        ),
    )
}

let issues = contact().decode(&json!({"type": "fax"})).unwrap_err();
let issue = issues.iter().next().unwrap();
assert_eq!(issue.path().to_string(), "/type");
assert_eq!(issue.meta()["allowed"], json!(["email", "phone"]));
```

## Defaults and recovery

`with_default(v)` gives `v` when the input is missing or `null`, and still reports any other
problem. `recover(v)` gives `v` whatever the problem was.

## Recursive structures

A decoder that refers to itself names its own type, so it returns a `BoxDecoder` and refers to
itself through `lazy`:

```rust
use raoh::json::prelude::*;

#[derive(Debug)]
pub struct Category {
    name: String,
    children: Vec<Category>,
}

fn category() -> BoxDecoder<Value, Category> {
    object((
        field("name", string().non_blank()),
        field("children", lazy(category).list()),
    ))
    .map(|(name, children)| Category { name, children })
    .boxed()
}

let tree = category().decode(&json!({"name": "a", "children": [{"name": "b", "children": []}]}));
assert!(tree.is_ok());
```

## Messages in other languages

`Messages::english()` and `Messages::japanese()` hold the catalogues Raoh for Java ships, word for
word, plus a template for `invalid_format.json`. A catalogue is a stack of layers, as a locale's
`.properties` file sits over its parent's: `japanese()` is a layer over `english()`,
`with_overrides` puts a layer of your own on top, and `falling_back_to` puts another catalogue
beneath. An issue is looked up one layer at a time, by message key and then by code, so a layer
that translates only `invalid_format` wins over the refined `invalid_format.email` beneath it, as
in Raoh for Java. `Messages::from_properties` reads a `.properties` file as Java's
`Properties.load` does, `\uXXXX` escapes included, so an existing Raoh for Java catalogue can be
used as it is. A template's `{name}` placeholders are filled from `meta`.

```rust
use raoh::json::prelude::*;
use raoh::Messages;

let issues = string().min_length(3).decode(&json!("ab")).unwrap_err();
assert_eq!(issues.flatten_with(Messages::japanese())[""], ["3文字以上で入力してください"]);

let ours = Messages::english().with_overrides([("too_short", "{min} characters or more")]);
assert_eq!(issues.flatten_with(&ours)[""], ["3 characters or more"]);
```

Any `Fn(&Issue) -> String` is a resolver too.

## Differences from Raoh for Java

In what it reports:

- `object` checks once that its input is an object and reports one issue at its own path when it
  is not. Raoh for Java checks in each field, reporting `type_mismatch` at every field's path and
  reading a non-object as an object without any `optional_field`.
- JSON Pointers are escaped as RFC 6901 says, as the Souther runtime does: a key `a/b` is written
  `/a~1b`. Raoh for Java 0.7.2 writes `/a/b`.
- `uuid()` parses with the `uuid` crate, which accepts 32 digits without hyphens and the form in
  braces, and refuses Java's short groups such as `1-1-1-1-1`.
- `url()` parses with the `url` crate, which follows the WHATWG URL Standard: it accepts `_` and
  non-ASCII characters in a host, refuses a port above 65535, and normalises the URL, so
  `https://example.com` becomes `https://example.com/`.
- `pattern()` takes the syntax of the `regex` crate, where `\d`, `\w` and `\s` match Unicode
  characters and Java's match ASCII only.
- `serde_json` reads `-0.0` as the same float as `-0`, so the integer decoders read both as 0.
  Raoh for Java refuses `-0.0`.
- A fractional decimal bound outside 0.001 to 10⁷ appears in a message in exponent form, such as
  `5.0E-4` where Raoh for Java writes `0.0005`, because `meta` holds it as a JSON number.
- `strict()` reports unknown members in the order the `Value` keeps its keys. That is the input
  order when `serde_json`'s `preserve_order` feature is enabled, as Raoh for Java reports them,
  and sorted order otherwise.

In the API:

- Combining is done with tuples and `object`, not `combine`. There is no `nested`, because a
  member is handed to its decoder as a `Value` already.
- `flatMap` is `and_then`, and there are no `Result`, `Ok` or `Err` types of its own: decoding
  gives `std::result::Result<T, Issues>`.
- There is no encoder. Serde's `Serialize` covers that direction.
- There is no domain construction guard (`raoh-gsh`). Private fields and module privacy stop a
  domain value from being built anywhere but its own module.
- There are no date and time decoders yet. `string().parse::<T>()` reads any type that implements
  `FromStr`, which includes the date types of `jiff` and `chrono`.

## Development

```sh
cargo test --all-features
```

`scripts/compat/generate.sh` regenerates `tests/compat/expected.json` and copies the message
catalogues from the Raoh for Java version `scripts/compat/pom.xml` names. It needs Java 25 and
Maven.

## License

Apache License 2.0
