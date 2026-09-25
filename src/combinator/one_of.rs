use crate::codes;
use crate::decoder::Decoder;
use crate::issue::{Issue, Issues};
use crate::path::Path;
use serde_json::{Map, Value};

/// A decoder that tries each decoder of `alternatives` in order and gives the first success.
///
/// When none succeeds it reports one `one_of_failed` issue, whose `candidates` metadata holds
/// each alternative's index and issues.
///
/// ```
/// use raoh::json::prelude::*;
/// use raoh::one_of;
///
/// #[derive(Debug, PartialEq)]
/// enum Id { Number(i64), Text(String) }
///
/// let id = one_of((i64().map(Id::Number), string().map(Id::Text)));
/// assert_eq!(id.decode(&json!("a1")).unwrap(), Id::Text("a1".into()));
/// assert_eq!(id.decode(&json!(true)).unwrap_err().iter().next().unwrap().code(), "one_of_failed");
/// ```
pub fn one_of<A>(alternatives: A) -> OneOf<A> {
    OneOf(alternatives)
}

/// The decoder [`one_of`] returns.
#[derive(Clone, Copy, Debug)]
pub struct OneOf<A>(A);

mod sealed {
    pub trait Sealed<I: ?Sized> {}
}

/// A tuple of decoders over the same input with the same output, tried in order. It cannot be
/// implemented outside this crate.
pub trait Alternatives<I: ?Sized>: sealed::Sealed<I> {
    /// What each alternative gives.
    type Output;

    /// The first success, or every alternative's issues in order.
    #[doc(hidden)]
    fn first_success(&self, input: &I, path: &Path<'_>) -> Result<Self::Output, Vec<Issues>>;
}

impl<I: ?Sized, A: Alternatives<I>> Decoder<I> for OneOf<A> {
    type Output = A::Output;

    fn decode_at(&self, input: &I, path: &Path<'_>) -> Result<A::Output, Issues> {
        self.0.first_success(input, path).map_err(|failures| {
            let candidates: Vec<Value> = failures
                .iter()
                .enumerate()
                .map(|(i, issues)| {
                    let mut candidate = Map::new();
                    candidate.insert("candidate".into(), i.into());
                    candidate.insert("issues".into(), issues.to_json());
                    Value::Object(candidate)
                })
                .collect();
            Issue::at_path(path, codes::ONE_OF_FAILED)
                .with_meta("candidates", candidates)
                .into()
        })
    }
}

macro_rules! alternatives {
    ($First:ident $first:tt $(, $T:ident $idx:tt)*) => {
        impl<I: ?Sized, $First: Decoder<I>, $($T: Decoder<I, Output = $First::Output>),*>
            sealed::Sealed<I> for ($First, $($T,)*)
        {
        }

        impl<I: ?Sized, $First: Decoder<I>, $($T: Decoder<I, Output = $First::Output>),*>
            Alternatives<I> for ($First, $($T,)*)
        {
            type Output = $First::Output;

            fn first_success(
                &self,
                input: &I,
                path: &Path<'_>,
            ) -> Result<Self::Output, Vec<Issues>> {
                let mut failures = Vec::new();
                match self.$first.decode_at(input, path) {
                    Ok(value) => return Ok(value),
                    Err(issues) => failures.push(issues),
                }
                $(
                    match self.$idx.decode_at(input, path) {
                        Ok(value) => return Ok(value),
                        Err(issues) => failures.push(issues),
                    }
                )*
                Err(failures)
            }
        }
    };
}

alternatives!(A 0);
alternatives!(A 0, B 1);
alternatives!(A 0, B 1, C 2);
alternatives!(A 0, B 1, C 2, D 3);
alternatives!(A 0, B 1, C 2, D 3, E 4);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12, O 13);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12, O 13, P 14);
alternatives!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11, N 12, O 13, P 14, Q 15);
