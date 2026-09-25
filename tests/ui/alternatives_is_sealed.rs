use raoh::combinator::Alternatives;
use raoh::json::prelude::*;
use raoh::{Issues, Path};

struct Anything;

impl Alternatives<Value> for Anything {
    type Output = ();

    fn first_success(&self, _: &Value, _: &Path<'_>) -> Result<(), Vec<Issues>> {
        Ok(())
    }
}

fn main() {
    let _ = one_of(Anything);
}
