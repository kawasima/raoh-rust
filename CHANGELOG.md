# Changelog

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
