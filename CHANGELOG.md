# Changelog

## 0.1.0 (unreleased)

First release: a Rust port of Raoh for decoding `serde_json::Value`.

- `Decoder` trait with `map`, `and_then`, `pipe`, `refine`, `with_default`, `recover` and `boxed`.
- Tuples of decoders combine independent parts and report the issues of all of them.
- `raoh::json`: `string`, `i32`, `i64`, `u32`, `u64`, `f64`, `bool` and, with the `decimal`
  feature, `decimal`, each with the constraints of Raoh for Java.
- `object`, `field`, `optional_field`, `presence_field`, `strict`, `list`, `nullable`, `dict`,
  `enum_of`, `literal`, `one_of`, `discriminate` and `lazy`.
- `Issue` and `Issues` with the codes, message keys and meta of Raoh for Java, English and Japanese
  message catalogues, and `MessageResolver`.
