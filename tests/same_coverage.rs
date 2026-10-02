//! Every value a decoder of this crate gives can be an element of a set.
//!
//! The specification's `toSet` takes a `list<E>` for every `E`, and `unique`, `contains` and
//! `contains_all` compare elements of any type with a message form. In Rust they need `Same` of
//! the element, so a decoder whose output has no `Same` narrows the specification's types. Each
//! line below compiles only while the output of that decoder has one; a new decoder, or a new
//! shape of output, belongs here.

use raoh::json::prelude::*;
use raoh::{Same, Set};
use serde_json::json;

fn set_of<D>(element: D) -> impl Decoder<Value, Output = Set<D::Output>>
where
    D: Decoder<Value>,
    D::Output: Same + 'static,
{
    element.list().to_set()
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Color {
    Red,
    Green,
}

raoh::same_by_eq!(Color);

#[test]
fn every_output_can_be_an_element_of_a_set() {
    // Scalars and the conversions of a string.
    let _ = set_of(string());
    let _ = set_of(i32());
    let _ = set_of(i64());
    let _ = set_of(u32());
    let _ = set_of(u64());
    let _ = set_of(f32());
    let _ = set_of(f64());
    let _ = set_of(decimal());
    let _ = set_of(bool());
    let _ = set_of(string().to_int());
    let _ = set_of(string().to_decimal());
    let _ = set_of(string().uuid());
    let _ = set_of(string().uri());
    let _ = set_of(string().instant());
    let _ = set_of(string().date());
    let _ = set_of(string().time());
    let _ = set_of(string().date_time());
    let _ = set_of(string().offset_date_time());
    let _ = set_of(literal("v1"));
    let _ = set_of(enum_of([("red", Color::Red), ("green", Color::Green)]));

    // Structures.
    let _ = set_of(i64().list());
    let _ = set_of(i64().list().to_set());
    let _ = set_of(dict(i64()));
    let _ = set_of(i64().nullable());
    let _ = set_of(object((
        field("a", i64()),
        optional_field("b", i64()),
        presence_field("c", i64()),
        flat(object((field("d", i64()),))),
    )));

    // A product of as many fields as a tuple of fields takes.
    let _ = set_of(object((
        field("f1", i64()),
        field("f2", i64()),
        field("f3", i64()),
        field("f4", i64()),
        field("f5", i64()),
        field("f6", i64()),
        field("f7", i64()),
        field("f8", i64()),
        field("f9", i64()),
        field("f10", i64()),
        field("f11", i64()),
        field("f12", i64()),
        field("f13", i64()),
        field("f14", i64()),
        field("f15", i64()),
        field("f16", i64()),
    )));

    // Combinators that keep their decoder's output.
    let _ = set_of(one_of((i64(), i64())));
    let _ = set_of(discriminate("k", (variant("a", i64()),)));
    let _ = set_of(i64().with_default(0));
    let _ = set_of(i64().recover(0));
}

#[test]
fn maps_and_sets_are_the_same_in_any_order() {
    let maps = set_of(dict(i64()));
    let input: Value = serde_json::from_str(r#"[{"a":1,"b":2},{"b":2,"a":1},{"a":1}]"#).unwrap();
    assert_eq!(maps.decode(&input).unwrap().len(), 2);

    let sets = set_of(i64().list().to_set());
    assert_eq!(sets.decode(&json!([[1, 2], [2, 1], [1]])).unwrap().len(), 2);
}

#[test]
fn a_float_product_keeps_its_zeros_apart() {
    let points = set_of(object((field("x", f64()), field("y", f64()))));
    let input: Value =
        serde_json::from_str(r#"[{"x":0.0,"y":1},{"x":-0.0,"y":1},{"x":0.0,"y":1.0}]"#).unwrap();
    assert_eq!(points.decode(&input).unwrap().len(), 2);
}
