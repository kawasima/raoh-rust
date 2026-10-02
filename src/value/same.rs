//! Sameness as the value model of the Raoh Specification has it, apart from Rust's `Eq`.

use crate::meta::MetaValue;
use crate::presence::Presence;
use crate::value::float::Float;
use crate::{Date, DateTime, Decimal, Instant, OffsetDateTime, Time, Uri, Uuid};
use indexmap::IndexSet;
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};

/// Whether two values are the same value, as the value model of the Raoh Specification says.
///
/// It is not `Eq`. Two floats are the same when they are the same IEEE 754 value, except that +0
/// and -0 differ and every NaN is the same, so `f64` has sameness though it has no `Eq`. Two
/// decimals are the same only with the same scale, `1.5` and `1.50` differing. Two offset
/// date-times are the same only at the same offset.
///
/// `unique`, `contains`, `contains_all` and `to_set` compare elements by it, so a list of any
/// type that has it can use them. `same_hash` must give equal hashes to values that are the same.
pub trait Same {
    /// Whether `self` and `other` are the same value.
    fn same(&self, other: &Self) -> bool;

    /// Feeds `state` with what makes the value the value it is.
    fn same_hash<H: Hasher>(&self, state: &mut H);
}

macro_rules! same_by_eq {
    ($($t:ty),*) => {
        $(impl Same for $t {
            fn same(&self, other: &Self) -> bool {
                self == other
            }

            fn same_hash<H: Hasher>(&self, state: &mut H) {
                self.hash(state);
            }
        })*
    };
}

// For these the value model's sameness is Rust's equality.
same_by_eq!(
    bool,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    char,
    String,
    Decimal,
    Date,
    Time,
    DateTime,
    OffsetDateTime,
    Instant,
    Uuid,
    Uri,
    MetaValue
);

impl Same for str {
    fn same(&self, other: &Self) -> bool {
        self == other
    }

    fn same_hash<H: Hasher>(&self, state: &mut H) {
        self.hash(state);
    }
}

macro_rules! same_float {
    ($($t:ty),*) => {
        $(impl Same for $t {
            fn same(&self, other: &Self) -> bool {
                crate::value::float::float_same(*self, *other)
            }

            fn same_hash<H: Hasher>(&self, state: &mut H) {
                self.canonical_bits().hash(state);
            }
        })*
    };
}

same_float!(f32, f64);

impl<T: Same + ?Sized> Same for &T {
    fn same(&self, other: &Self) -> bool {
        (**self).same(*other)
    }

    fn same_hash<H: Hasher>(&self, state: &mut H) {
        (**self).same_hash(state);
    }
}

impl<T: Same + ?Sized> Same for Box<T> {
    fn same(&self, other: &Self) -> bool {
        (**self).same(&**other)
    }

    fn same_hash<H: Hasher>(&self, state: &mut H) {
        (**self).same_hash(state);
    }
}

/// The same length, and the same element at each position.
impl<T: Same> Same for Vec<T> {
    fn same(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().zip(other).all(|(a, b)| a.same(b))
    }

    fn same_hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for item in self {
            item.same_hash(state);
        }
    }
}

/// Both empty, or the same value.
impl<T: Same> Same for Option<T> {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (None, None) => true,
            (Some(a), Some(b)) => a.same(b),
            _ => false,
        }
    }

    fn same_hash<H: Hasher>(&self, state: &mut H) {
        match self {
            None => 0u8.hash(state),
            Some(v) => {
                1u8.hash(state);
                v.same_hash(state);
            }
        }
    }
}

/// The same case, and the same value when present.
impl<T: Same> Same for Presence<T> {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Presence::Absent, Presence::Absent) | (Presence::Null, Presence::Null) => true,
            (Presence::Present(a), Presence::Present(b)) => a.same(b),
            _ => false,
        }
    }

    fn same_hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Presence::Absent => 0u8.hash(state),
            Presence::Null => 1u8.hash(state),
            Presence::Present(v) => {
                2u8.hash(state);
                v.same_hash(state);
            }
        }
    }
}

/// A product: the same element at each position.
macro_rules! same_tuple {
    ($($T:ident $idx:tt),+) => {
        impl<$($T: Same),+> Same for ($($T,)+) {
            fn same(&self, other: &Self) -> bool {
                $(self.$idx.same(&other.$idx))&&+
            }

            fn same_hash<S: Hasher>(&self, state: &mut S) {
                $(self.$idx.same_hash(state);)+
            }
        }
    };
}

same_tuple!(A 0);
same_tuple!(A 0, B 1);
same_tuple!(A 0, B 1, C 2);
same_tuple!(A 0, B 1, C 2, D 3);
same_tuple!(A 0, B 1, C 2, D 3, E 4);
same_tuple!(A 0, B 1, C 2, D 3, E 4, F 5);
same_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
same_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);

/// A value keyed by its sameness, so that the standard collections compare it as the value model
/// does.
pub(crate) struct ByValue<T>(pub(crate) T);

impl<T: Same> PartialEq for ByValue<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
}

impl<T: Same> Eq for ByValue<T> {}

impl<T: Same> Hash for ByValue<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.same_hash(state);
    }
}

impl<T: Clone> Clone for ByValue<T> {
    fn clone(&self) -> Self {
        ByValue(self.0.clone())
    }
}

/// A finite set as the value model has it: each value once, by [`Same`], whatever Rust's `Eq`
/// says or whether the type has one. It keeps the values in the order each was first added.
///
/// ```
/// use raoh::Set;
///
/// let zeros: Set<f64> = [0.0, -0.0, 0.0, f64::NAN, f64::NAN].into_iter().collect();
/// assert_eq!(zeros.len(), 3);
/// assert!(zeros.contains(&-0.0));
/// ```
pub struct Set<T> {
    items: IndexSet<ByValue<T>>,
}

impl<T: Same> Set<T> {
    /// An empty set.
    pub fn new() -> Self {
        Self {
            items: IndexSet::new(),
        }
    }

    /// Adds `value`, and says whether it was not there yet.
    pub fn insert(&mut self, value: T) -> bool {
        self.items.insert(ByValue(value))
    }

    /// Whether the set holds a value the same as `value`.
    pub fn contains(&self, value: &T) -> bool
    where
        T: Clone,
    {
        self.items.contains(&ByValue(value.clone()))
    }

    /// The number of values.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there is no value.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Each value, in the order it was first added.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|v| &v.0)
    }
}

impl<T: Same> Default for Set<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Clone for Set<T> {
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Set<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set()
            .entries(self.items.iter().map(|v| &v.0))
            .finish()
    }
}

impl<T: Same> FromIterator<T> for Set<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            items: iter.into_iter().map(ByValue).collect(),
        }
    }
}

impl<T> IntoIterator for Set<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    /// The values, in the order each was first added.
    fn into_iter(self) -> Self::IntoIter {
        self.items
            .into_iter()
            .map(|v| v.0)
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// The same values, in any order.
impl<T: Same> PartialEq for Set<T> {
    fn eq(&self, other: &Self) -> bool {
        self.same(other)
    }
}

/// The same values, in any order.
impl<T: Same> Same for Set<T> {
    fn same(&self, other: &Self) -> bool {
        self.len() == other.len() && other.items.iter().all(|v| self.items.contains(v))
    }

    fn same_hash<H: Hasher>(&self, state: &mut H) {
        // Summed, so that the order the values were added in does not change the hash.
        let sum = self.items.iter().fold(0u64, |sum, v| {
            let mut one = DefaultHasher::new();
            v.0.same_hash(&mut one);
            sum.wrapping_add(one.finish())
        });
        self.len().hash(state);
        sum.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_are_the_same_as_the_value_model_says() {
        assert!(!0.0f64.same(&-0.0));
        assert!(f64::NAN.same(&-f64::NAN));
        let set: Set<f32> = [-0.0, 0.0, -0.0, 0.0].into_iter().collect();
        assert_eq!(
            set.into_iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            [(-0.0f32).to_bits(), 0]
        );
    }

    #[test]
    fn decimals_keep_their_scale_and_sets_ignore_order() {
        let a: Decimal = "1.5".parse().unwrap();
        let b: Decimal = "1.50".parse().unwrap();
        assert!(!a.same(&b));
        let x: Set<i32> = [1, 2, 3].into_iter().collect();
        let y: Set<i32> = [3, 1, 2].into_iter().collect();
        assert!(x.same(&y));
        let hash = |s: &Set<i32>| {
            let mut h = DefaultHasher::new();
            s.same_hash(&mut h);
            h.finish()
        };
        assert_eq!(hash(&x), hash(&y));
    }
}
