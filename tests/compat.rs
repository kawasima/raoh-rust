//! Holds the decoders to what Raoh for Java gives for the same input.
//!
//! `tests/compat/cases.json` lists inputs by decoder name; `scripts/compat/generate.sh` runs them
//! through the Java decoders of the same name in `scripts/compat/Generate.java` and writes
//! `tests/compat/expected.json`. Each decoder below is the Rust counterpart of the Java one, and
//! gives its output as the JSON the Java one's output serializes to.

use raoh::json::prelude::*;
use raoh::{BoxDecoder, Issue};
use serde::Serialize;

fn out<D>(decoder: D) -> BoxDecoder<Value, Value>
where
    D: Decoder<Value> + Send + Sync + 'static,
    D::Output: Serialize,
{
    decoder.map(|v| serde_json::to_value(v).unwrap()).boxed()
}

fn presence(p: Presence<i32>) -> Value {
    match p {
        Presence::Absent => json!("absent"),
        Presence::Null => json!("null"),
        Presence::Present(v) => json!({ "present": v }),
    }
}

/// The Rust counterpart of a Java decoder, or `None` when this build lacks the feature it needs.
fn decoder(name: &str) -> Option<BoxDecoder<Value, Value>> {
    Some(match name {
        "string" => out(string()),
        "string_normalized_email" => out(string().trim().lowercase().email()),
        "string_non_blank" => out(string().non_blank()),
        "string_min_3" => out(string().min_length(3)),
        "string_max_3" => out(string().max_length(3)),
        "string_length_4" => out(string().length(4)),
        "string_one_of" => out(string().one_of(["b", "a"])),
        "string_starts_with" => out(string().starts_with("ab")),
        "string_ends_with" => out(string().ends_with("ab")),
        "string_contains" => out(string().contains("ab")),
        "string_ipv4" => out(string().ipv4()),
        "string_ipv6" => out(string().ipv6()),
        "string_ip" => out(string().ip()),
        "string_ulid" => out(string().ulid()),
        "string_cuid" => out(string().cuid()),
        #[cfg(feature = "regex")]
        "string_pattern" => out(string().pattern(r"[a-z]+\d")),
        #[cfg(feature = "uuid")]
        "string_uuid" => out(string().uuid().map(|u| u.to_string())),
        #[cfg(feature = "url")]
        "string_url" => out(string().url().map(|u| u.to_string())),

        "int" => out(i32()),
        "int_min_1" => out(i32().min(1)),
        "int_max_10" => out(i32().max(10)),
        "int_range" => out(i32().range(0..=150)),
        "int_positive" => out(i32().positive()),
        "int_negative" => out(i32().negative()),
        "int_non_negative" => out(i32().non_negative()),
        "int_non_positive" => out(i32().non_positive()),
        "int_multiple_of_3" => out(i32().multiple_of(3)),
        "int_one_of" => out(i32().one_of([3, 1])),
        "long" => out(i64()),

        "double" => out(f64()),
        "double_positive" => out(f64().positive()),
        "double_negative" => out(f64().negative()),
        "double_range" => out(f64().range(0.5..=1.5)),
        "double_min" => out(f64().min(0.5)),
        "double_one_of" => out(f64().one_of([2.0, 1.0])),

        #[cfg(feature = "decimal")]
        "decimal" => out(decimal().map(decimal_json)),
        #[cfg(feature = "decimal")]
        "decimal_scale_2" => out(decimal().scale(2).map(decimal_json)),
        #[cfg(feature = "decimal")]
        "decimal_positive" => out(decimal().positive().map(decimal_json)),
        #[cfg(feature = "decimal")]
        "decimal_range" => out(decimal()
            .range(rust_decimal::Decimal::ZERO..=rust_decimal::Decimal::TEN)
            .map(decimal_json)),

        "bool" => out(bool()),
        "bool_is_true" => out(bool().is_true()),

        "list_int" => out(i32().list()),
        "list_non_empty" => out(i32().list().non_empty()),
        "list_min_2" => out(i32().list().min_size(2)),
        "list_max_2" => out(i32().list().max_size(2)),
        "list_size_2" => out(i32().list().size(2)),
        "list_unique" => out(i32().list().unique()),

        "person" => out(object((field("name", string()), field("age", i32())))),
        "person_strict" => out(object((field("name", string()), field("age", i32()))).strict()),
        "escaped_keys" => out(object((field("a/b", i32()), field("~c", i32())))),
        "optional" => out(object((
            field("id", i32()),
            optional_field("nick", string()),
        ))),
        "nullable" => out(object((
            field("id", i32()),
            field("note", string().nullable()),
        ))),
        "presence" => out(object((field("id", i32()), presence_field("n", i32())))
            .map(|(id, n)| json!([id, presence(n)]))),
        "nested" => out(object((
            field(
                "items",
                object((
                    field("name", string().non_blank()),
                    field("qty", i32().positive()),
                ))
                .list(),
            ),
            field("count", i32().with_default(0)),
        ))),
        "dict" => out(dict(i32())),

        "enum" => out(enum_of([("red", "red"), ("green", "green")])),
        "literal" => out(literal("v1")),
        "shape" => out(discriminate(
            "kind",
            (
                variant(
                    "square",
                    object((field("side", i32()), field("kind", string()))).map(|(s, _)| s * s),
                ),
                variant(
                    "rect",
                    object((field("w", i32()), field("h", i32()))).map(|(w, h)| w * h),
                ),
            ),
        )),
        "one_of" => out(one_of((
            i32().map(|n| n.to_string()),
            string().min_length(3),
        ))),
        "with_default" => out(object((
            field("id", i32().with_default(0)),
            field("page", i32().with_default(1)),
        ))),
        "recover" => out(object((
            field("id", i32().with_default(0)),
            field("page", i32().recover(1)),
        ))),
        "period" => out(
            object((field("start", i32()), field("end", i32()))).and_then(|(start, end)| {
                if start <= end {
                    Ok((start, end))
                } else {
                    Err(Issue::new("invalid_value", "end is before start")
                        .at(["end"].into_iter().collect()))
                }
            }),
        ),
        _ => return None,
    })
}

#[cfg(feature = "decimal")]
fn decimal_json(d: rust_decimal::Decimal) -> Value {
    serde_json::from_str(&d.to_string()).unwrap()
}

/// Cases where this crate gives something else than Raoh for Java on purpose, with what it gives.
///
/// Raoh for Java 0.7.2 writes a path without escaping `/` and `~` in a key. This crate writes
/// RFC 6901 pointers, as the Souther runtime does, so a key holding either stays one segment.
fn divergence(name: &str) -> Option<Value> {
    match name {
        "escaped_keys" => Some(json!({ "issues": [
            {"path": "/a~1b", "code": "required", "message_key": "required",
             "message": "is required", "meta": {}},
            {"path": "/~0c", "code": "required", "message_key": "required",
             "message": "is required", "meta": {}},
        ]})),
        _ => None,
    }
}

fn issue_json(issue: &Issue) -> Value {
    json!({
        "path": issue.path().to_string(),
        "code": issue.code(),
        "message_key": issue.message_key(),
        "message": issue.message(),
        "meta": issue.meta(),
    })
}

#[test]
fn every_case_gives_what_raoh_for_java_gives() {
    let expected: Vec<Value> = serde_json::from_str(include_str!("compat/expected.json")).unwrap();
    let mut mismatches = Vec::new();
    let mut skipped = Vec::new();
    for case in &expected {
        let name = case["decoder"].as_str().unwrap();
        let input = &case["input"];
        let Some(decoder) = decoder(name) else {
            skipped.push(name);
            continue;
        };
        let actual = match decoder.decode(input) {
            Ok(value) => json!({ "ok": value }),
            Err(issues) => json!({ "issues": issues.iter().map(issue_json).collect::<Vec<_>>() }),
        };
        let wanted = match (divergence(name), case.get("ok")) {
            (Some(ours), _) => ours,
            (None, Some(value)) => json!({ "ok": value }),
            (None, None) => json!({ "issues": case["issues"] }),
        };
        if actual != wanted {
            mismatches.push(format!(
                "{name} {input}\n  java: {wanted}\n  rust: {actual}"
            ));
        }
    }
    if cfg!(all(
        feature = "regex",
        feature = "uuid",
        feature = "url",
        feature = "decimal"
    )) {
        assert!(skipped.is_empty(), "no Rust decoder for {skipped:?}");
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} cases differ:\n{}",
        mismatches.len(),
        expected.len(),
        mismatches.join("\n")
    );
}
