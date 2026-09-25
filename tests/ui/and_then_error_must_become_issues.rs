use raoh::json::prelude::*;

fn main() {
    let _ = i64().and_then(|n| if n > 0 { Ok(n) } else { Err("not positive") });
}
