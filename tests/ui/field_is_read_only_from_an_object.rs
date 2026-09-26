use raoh::json::prelude::*;

fn main() {
    let _ = field("a", i64()).decode(&json!({"a": 1}));
    let _ = (field("a", i64()), field("b", i64())).decode(&json!({"a": 1, "b": 2}));
}
