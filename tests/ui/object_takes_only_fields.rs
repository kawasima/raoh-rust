use raoh::json::prelude::*;

fn main() {
    let _ = object((string(), field("a", i64())));
}
