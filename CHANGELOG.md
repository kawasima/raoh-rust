# Changelog

## 0.9.0-dev - Unreleased

The version is now the version of the Raoh Specification the crate follows. The decoders follow
the Raoh Specification 0.9.0-dev, and `scripts/conformance.sh` checks them
against every case of its suite: core, encode, messages-en and messages-ja are conformant.

What each decoder accepts and reports:

- A number is read from the text it was written with, which `serde_json` keeps with the new
  `arbitrary_precision` feature: `int` refuses `1.0`, `decimal` keeps the scale of `1.50`, and an
  integer of any length is told from a float.
- Each field of an `object` checks for itself that the input is an object: a required field of
  anything else, `null` and a missing member included, is `type_mismatch` with `expected` `object`
  at its own path, and an optional field reads it as not having the member. `object` reported one
  issue at its own path. `discriminate` reads its tag the same way.
- `with_default` gives the default for a null or missing input, looked at before the decoder runs.
  It gave the default whenever every issue was `required`, so an object missing a member was
  defaulted as a whole.
- `trim`, `non_blank`, `lowercase` and `uppercase` follow Unicode 18.0.0 through 199x-notation,
  whatever Rust release the crate is built with, and lengths count Unicode scalar values.
- `email` accepts the specification's ASCII profile of RFC 5321's `Mailbox`; `ulid` takes either
  case and refuses a value past 128 bits; `uuid` reads only the hyphenated 8-4-4-4-12 form.
- `pattern` takes the specification's pattern language instead of the `regex` crate's syntax, and
  matches in one pass over the value.
- `url` reads an RFC 3986 URI with an `http` or `https` scheme and a host, and gives the text as
  written instead of the `url` crate's normalised URL.
- A float in a message is its canonical decimal at its own width: an `f32` bound of 0.1 is `0.1`.
- A template placeholder with no metadata entry stays as written, instead of passing the template
  over.
- `one_of_failed` keeps each candidate's issues as issues, so they are written in the language of
  the whole.

New:

- `f32()`, `uri()`, `normalize`, `to_int`, `to_long`, `to_decimal`, `to_bool`, and the temporal
  conversions `instant`, `date`, `time`, `date_time` and `offset_date_time` with `before`, `after`
  and `between`.
- `Decimal`, of any precision with a 32-bit scale; `Date`, `Time`, `DateTime`, `OffsetDateTime`
  and `Instant`; `Uuid` and `Uri`. They replace `rust_decimal`, `uuid` and `url`, whose features
  are gone.
- `Same`, the value model's sameness, and `Set`, a set by it. `unique`, `contains`,
  `contains_all` and `to_set` compare by it, so they work on lists of floats and decimals as the
  value model compares them, and `to_set` gives a `Set`. Every value a decoder gives has it, maps
  and products of up to 16 values included; `same_by_eq!` gives it to a type of your own, such as
  the enum `enum_of` decodes into, and `meta_by_display!` gives such a type the message form
  `unique` and `contains` write it in.
- `contains`, `contains_all` and `to_set` on lists; `non_empty`, `min_size`, `max_size` and `size`
  on `dict`.
- `flat` fields, `strict(decoder, names)` around any decoder, `discriminate_by`,
  `recover_with`, and `enum_of(...).using(...)` and `literal(...).using(...)` with a message of
  their own.
- `Vec` alternatives for `one_of`, `Vec` variants for `discriminate`, and `Vec`s of boxed fields
  for `object`, for decoders whose parts are decided at run time.
- `raoh::encode`, with a string encoder and an object encoder of properties with defaults.

Cost:

- Reading a number allocates nothing, and lower-casing, upper-casing and normalizing ASCII text
  do not look its characters up in Unicode's tables.
- `pattern` keeps a matcher for each thread that decodes with it at once, with what its matches
  have worked out, so a value is matched in lookups.
- A strict decoder takes time in the members of its object, and `multiple_of` in the digits of the
  decimal, where both took time in their square. `tests/cost.rs` holds every decoder that goes
  over an input to time that grows with it once, and counts the allocations of reading numbers.

Breaking:

- An issue's `meta` holds `MetaValue`s instead of `serde_json::Value`s.
- The features `regex`, `decimal`, `uuid` and `url` are removed; what they gave is always there.
- The MSRV is 1.88.
- `tests/compat`, which compared the decoders with Raoh for Java 0.8, is replaced by the
  specification's suite.

## 0.1.0 - 2026-09-27

First release: a Rust port of Raoh for decoding `serde_json::Value`.

- `Decoder` trait with `map`, `and_then`, `pipe`, `refine`, `with_default`, `recover` and `boxed`.
- Tuples of decoders combine independent parts and report the issues of all of them.
- `raoh::json`: `string`, `i32`, `i64`, `u32`, `u64`, `f64`, `bool` and, with the `decimal`
  feature, `decimal`, each with the constraints of Raoh for Java.
- `object`, `field`, `optional_field`, `presence_field`, `strict`, `list`, `nullable`, `dict`,
  `enum_of`, `literal`, `one_of`, `discriminate` and `lazy`.
- `Issue` and `Issues` with the codes and meta of Raoh for Java. An issue carries no sentence
  unless its creator gives one; the English and Japanese catalogues write it from the message key
  or code, and `Messages::from_properties` reads a Raoh for Java catalogue as it is.
