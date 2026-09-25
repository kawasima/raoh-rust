use raoh::json::prelude::*;
use raoh::json::FieldSet;
use raoh::{Issues, Path};
use serde_json::Map;

struct Anything;

impl FieldSet for Anything {
    type Output = ();

    fn decode_fields(&self, _: &Map<String, Value>, _: &Path<'_>) -> Result<(), Issues> {
        Ok(())
    }

    fn field_names<'a>(&'a self, _: &mut Vec<&'a str>) {}
}

fn main() {
    let _ = object(Anything).strict();
}
