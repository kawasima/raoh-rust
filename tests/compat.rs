//! Holds the decoders to what Raoh for Java gives for the same input.
//!
//! `tests/compat/cases.json` lists inputs by decoder name; `scripts/compat/generate.sh` runs them
//! through the Java decoders of the same name in `scripts/compat/Generate.java` and writes
//! `tests/compat/expected.json`. Each decoder below is the Rust counterpart of the Java one, and
//! gives its output as the JSON the Java one's output serializes to.

use raoh::json::prelude::*;
use raoh::{BoxDecoder, Issue, Messages};
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
        "string_trim" => out(string().trim()),
        "string_lower" => out(string().lowercase()),
        "string_upper" => out(string().uppercase()),
        "string_email" => out(string().email()),
        "string_one_of_astral" => out(string().one_of(["\u{ff21}", "\u{1f600}"])),
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
        "double_min_1e7" => out(f64().min(1e7)),
        "double_max_small" => out(f64().max(1e-4)),
        "double_one_of_big" => out(f64().one_of([1e7, 0.5])),

        #[cfg(feature = "decimal")]
        "decimal" => out(decimal().map(decimal_json)),
        #[cfg(feature = "decimal")]
        "decimal_scale_2" => out(decimal().scale(2).map(decimal_json)),
        #[cfg(feature = "decimal")]
        "decimal_positive" => out(decimal().positive().map(decimal_json)),
        #[cfg(feature = "decimal")]
        "decimal_min_small" => out(decimal()
            .min(rust_decimal::Decimal::new(5, 4))
            .map(decimal_json)),
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
        "optional_only" => out(object((
            optional_field("a", string()),
            optional_field("b", string()),
        ))),

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
                    Err(Issue::new("invalid_value")
                        .with_message("end is before start")
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

/// What each case gives, as JSON: `{"ok": output}` or `{"issues": [...]}`.
fn outcome(result: Result<Value, raoh::Issues>) -> Value {
    match result {
        Ok(value) => json!({ "ok": value }),
        Err(issues) => json!({ "issues": issues.iter().map(issue_json).collect::<Vec<_>>() }),
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

fn failure(path: &str, code: &str, message_key: &str, message: &str, meta: Value) -> Value {
    json!({ "issues": [
        {"path": path, "code": code, "message_key": message_key, "message": message, "meta": meta}
    ]})
}

/// The cases where this crate gives something else than Raoh for Java on purpose, keyed by
/// decoder and input, with what this crate gives and why.
fn divergences() -> Vec<(&'static str, Value, Value, &'static str)> {
    let not_an_object = |actual: &str| {
        failure(
            "",
            "type_mismatch",
            "type_mismatch",
            "expected object",
            json!({"expected": "object", "actual": actual}),
        )
    };
    let required = failure("", "required", "required", "is required", json!({}));
    let object_scope = "object() checks the input is an object once, at its own path; Java checks \
                        it in each field and reads a non-object as holding no optional field";
    let mut list = vec![
        ("person", json!([1]), not_an_object("array"), object_scope),
        ("person", Value::Null, required.clone(), object_scope),
        (
            "person",
            json!("str"),
            not_an_object("string"),
            object_scope,
        ),
        (
            "person_strict",
            json!([1]),
            not_an_object("array"),
            object_scope,
        ),
        (
            "optional_only",
            json!("x"),
            not_an_object("string"),
            object_scope,
        ),
        ("optional_only", Value::Null, required, object_scope),
        (
            "optional_only",
            json!([1]),
            not_an_object("array"),
            object_scope,
        ),
        (
            "shape",
            json!("rect"),
            not_an_object("string"),
            object_scope,
        ),
        (
            "long",
            json!("123456789012345678901"),
            failure(
                "",
                "type_mismatch",
                "type_mismatch",
                "expected long",
                json!({"expected": "long", "actual": "number"}),
            ),
            "serde_json keeps an integer beyond u64 as a float, like 1e20, so it cannot be told \
             apart from a number with an exponent; Jackson keeps it as a BigInteger",
        ),
        (
            "long",
            json!("-0.0"),
            json!({"ok": 0}),
            "serde_json reads -0 and -0.0 as the same float, so both are read as the integer 0; \
             Jackson reads -0.0 as a double",
        ),
    ];
    if cfg!(feature = "decimal") {
        list.push((
            "decimal_min_small",
            json!(0.0001),
            failure(
                "",
                "out_of_range",
                "out_of_range.minimum",
                "must be at least 5.0E-4",
                json!({"min": 0.0005, "actual": 0.0001}),
            ),
            "meta holds a decimal as a JSON number, which a message writes as a Java double",
        ));
    }
    if cfg!(feature = "uuid") {
        let uuid_crate = "uuid() parses with the uuid crate, which also accepts the forms \
                          without hyphens and in braces";
        let uuid = json!({"ok": "123e4567-e89b-12d3-a456-426614174000"});
        list.push((
            "string_uuid",
            json!("123e4567e89b12d3a456426614174000"),
            uuid.clone(),
            uuid_crate,
        ));
        list.push((
            "string_uuid",
            json!("{123e4567-e89b-12d3-a456-426614174000}"),
            uuid,
            uuid_crate,
        ));
    }
    if cfg!(feature = "url") {
        let url_crate = "url() parses with the url crate, which follows the WHATWG URL Standard \
                         and gives the URL normalised";
        list.push((
            "string_url",
            json!("http://my_host.com"),
            json!({"ok": "http://my_host.com/"}),
            url_crate,
        ));
        list.push((
            "string_url",
            json!("https://example.com"),
            json!({"ok": "https://example.com/"}),
            url_crate,
        ));
        list.push((
            "string_url",
            json!("https://日本.jp/"),
            json!({"ok": "https://xn--wgv71a.jp/"}),
            url_crate,
        ));
        list.push((
            "string_url",
            json!("http://example.com:99999/"),
            failure(
                "",
                "invalid_format",
                "invalid_format.url",
                "not a valid URL",
                json!({}),
            ),
            url_crate,
        ));
    }
    list
}

#[test]
fn every_case_gives_what_raoh_for_java_gives() {
    let expected: Vec<Value> = serde_json::from_str(include_str!("compat/expected.json")).unwrap();
    let divergences = divergences();
    let mut mismatches = Vec::new();
    let mut skipped = Vec::new();
    let mut diverged = 0;
    // A divergence Raoh for Java has caught up with is removed, not kept as a record of a
    // difference that no longer exists.
    let mut stale = Vec::new();
    for case in &expected {
        let name = case["decoder"].as_str().unwrap();
        // A number whose text matters, such as -0, is given as the JSON text to read.
        let (input, key) = match case.get("input_json").and_then(Value::as_str) {
            Some(text) => (serde_json::from_str(text).unwrap(), json!(text)),
            None => (case["input"].clone(), case["input"].clone()),
        };
        let Some(decoder) = decoder(name) else {
            skipped.push(name);
            continue;
        };
        let java = match case.get("ok") {
            Some(value) => json!({ "ok": value }),
            None => json!({ "issues": case["issues"] }),
        };
        let wanted = match divergences
            .iter()
            .find(|(d, i, _, _)| *d == name && *i == key)
        {
            Some((_, _, ours, _)) => {
                diverged += 1;
                if *ours == java {
                    stale.push(format!("{name} {key}"));
                }
                ours.clone()
            }
            None => java,
        };
        let actual = outcome(decoder.decode(&input));
        if actual != wanted {
            mismatches.push(format!("{name} {key}\n  want: {wanted}\n  rust: {actual}"));
        }
    }
    if cfg!(all(
        feature = "regex",
        feature = "uuid",
        feature = "url",
        feature = "decimal"
    )) {
        assert!(skipped.is_empty(), "no Rust decoder for {skipped:?}");
        assert_eq!(diverged, divergences.len(), "a divergence names no case");
    }
    assert!(
        stale.is_empty(),
        "these divergences now give what Raoh for Java gives; remove them: {stale:?}"
    );
    assert!(
        mismatches.is_empty(),
        "{} of {} cases differ:\n{}",
        mismatches.len(),
        expected.len(),
        mismatches.join("\n")
    );
}

/// Every template of Raoh for Java's catalogues is in this crate's, word for word, under the
/// same key; the copies in `tests/compat/java/` come from the jar `generate.sh` ran.
#[test]
fn the_catalogues_hold_raoh_for_javas_templates() {
    let pairs = [
        (
            include_str!("compat/java/messages.properties"),
            Messages::english(),
        ),
        (
            include_str!("compat/java/messages_ja.properties"),
            Messages::japanese(),
        ),
    ];
    for (java, ours) in pairs {
        let java = Messages::from_properties(java).unwrap();
        assert!(java.templates().count() > 40);
        for (key, template) in java.templates() {
            assert_eq!(ours.template(key), Some(template), "{key}");
        }
    }
}
