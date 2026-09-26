use raoh::json::prelude::*;

fn main() {
    let _ = (field("a", i64()), field("b", i64())).strict();
}
