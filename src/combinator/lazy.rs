use crate::decoder::Decoder;
use crate::issue::Issues;
use crate::path::Path;

/// A decoder built by `make` each time it decodes, so a decoder can refer to itself.
///
/// A recursive decoder names its own type, so it returns a [`BoxDecoder`](crate::BoxDecoder):
///
/// ```
/// use raoh::json::prelude::*;
/// use raoh::{lazy, BoxDecoder};
///
/// struct Category { name: String, children: Vec<Category> }
///
/// fn category() -> BoxDecoder<Json, Category> {
///     object((
///         field("name", string()),
///         field("children", lazy(category).list()),
///     ))
///     .map(|(name, children)| Category { name, children })
///     .boxed()
/// }
///
/// let tree = category()
///     .decode(&json!({"name": "a", "children": [{"name": "b", "children": []}]}))
///     .unwrap();
/// assert_eq!(tree.children[0].name, "b");
/// ```
pub fn lazy<F>(make: F) -> Lazy<F> {
    Lazy(make)
}

/// The decoder [`lazy`] returns.
#[derive(Clone, Copy, Debug)]
pub struct Lazy<F>(F);

impl<I: ?Sized, F, D> Decoder<I> for Lazy<F>
where
    F: Fn() -> D,
    D: Decoder<I>,
{
    type Output = D::Output;

    fn decode_at(&self, input: &I, path: &Path<'_>) -> Result<D::Output, Issues> {
        (self.0)().decode_at(input, path)
    }
}
