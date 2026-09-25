/// Whether a member was absent, present as null, or present with a value.
///
/// A PATCH request tells these three apart: an absent member leaves a value as it is, a null one
/// clears it, and a present one sets it. `Option<Option<T>>` says the same with less to read by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Presence<T> {
    /// The member is not there.
    #[default]
    Absent,
    /// The member is there and null.
    Null,
    /// The member is there with a value.
    Present(T),
}

impl<T> Presence<T> {
    /// The value, if there is one.
    pub fn present(self) -> Option<T> {
        match self {
            Presence::Present(value) => Some(value),
            _ => None,
        }
    }

    /// Whether the member is there, null or not.
    pub fn is_given(&self) -> bool {
        !matches!(self, Presence::Absent)
    }

    /// The value transformed by `f`, if there is one.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Presence<U> {
        match self {
            Presence::Absent => Presence::Absent,
            Presence::Null => Presence::Null,
            Presence::Present(value) => Presence::Present(f(value)),
        }
    }
}
