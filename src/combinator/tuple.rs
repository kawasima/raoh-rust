use crate::decoder::Decoder;
use crate::issue::Issues;
use crate::path::Path;

/// Runs every decoder of the tuple on the same input and keeps every issue, in the order the
/// decoders are written.
macro_rules! tuple_decoder {
    ($($T:ident $v:ident $idx:tt),+) => {
        impl<I: ?Sized, $($T: Decoder<I>),+> Decoder<I> for ($($T,)+) {
            type Output = ($($T::Output,)+);

            fn decode_at(&self, input: &I, path: &Path<'_>) -> Result<Self::Output, Issues> {
                let mut issues = Issues::new();
                $(
                    let $v = match self.$idx.decode_at(input, path) {
                        Ok(value) => Some(value),
                        Err(found) => {
                            issues.merge(found);
                            None
                        }
                    };
                )+
                match ($($v,)+) {
                    ($(Some($v),)+) => Ok(($($v,)+)),
                    _ => Err(issues),
                }
            }
        }
    };
}

tuple_decoder!(A a 0);
tuple_decoder!(A a 0, B b 1);
tuple_decoder!(A a 0, B b 1, C c 2);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10, M m 11);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10, M m 11, N n 12);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10, M m 11, N n 12, O o 13);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10, M m 11, N n 12, O o 13, P p 14);
tuple_decoder!(A a 0, B b 1, C c 2, D d 3, E e 4, F f 5, G g 6, H h 7, J j 8, K k 9, L l 10, M m 11, N n 12, O o 13, P p 14, Q q 15);
