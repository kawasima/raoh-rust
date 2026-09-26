use raoh::json::prelude::*;
use std::rc::Rc;

fn main() {
    let prefix = Rc::new(String::from("id-"));
    let _ = string().map(move |s| format!("{prefix}{s}")).boxed();
}
