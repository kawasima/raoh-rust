//! Where in the input a value was found.
//!
//! A decoder walks down the input with a [`Path`], a list borrowed from the stack: going one level
//! deeper allocates nothing, so a successful decode builds no paths at all. Only when an [`Issue`]
//! is recorded is the path copied out into an owned [`Pointer`].
//!
//! [`Issue`]: crate::Issue

use std::fmt;

/// One step below a parent: an object key or an array index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment<'a> {
    /// A member of an object.
    Key(&'a str),
    /// An element of an array.
    Index(usize),
}

/// The position a decoder is reading at, as a list borrowed from the caller's stack.
#[derive(Clone, Copy, Debug)]
pub struct Path<'a> {
    parent: Option<&'a Path<'a>>,
    segment: Option<Segment<'a>>,
}

impl Path<'static> {
    /// The root of the input.
    pub const ROOT: Path<'static> = Path {
        parent: None,
        segment: None,
    };
}

impl<'a> Path<'a> {
    /// The path of the member `key` below this one.
    pub fn key<'b>(&'b self, key: &'b str) -> Path<'b> {
        Path {
            parent: Some(self),
            segment: Some(Segment::Key(key)),
        }
    }

    /// The path of the element at `index` below this one.
    pub fn index(&self, index: usize) -> Path<'_> {
        Path {
            parent: Some(self),
            segment: Some(Segment::Index(index)),
        }
    }

    /// Whether this is the root of the input.
    pub fn is_root(&self) -> bool {
        self.segment.is_none()
    }

    /// Copies this path out into an owned [`Pointer`].
    pub fn to_pointer(&self) -> Pointer {
        let mut segments = Vec::new();
        let mut at = Some(self);
        while let Some(path) = at {
            match path.segment {
                Some(Segment::Key(key)) => segments.push(key.to_owned()),
                Some(Segment::Index(index)) => segments.push(index.to_string()),
                None => {}
            }
            at = path.parent;
        }
        segments.reverse();
        Pointer { segments }
    }
}

impl fmt::Display for Path<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_pointer().fmt(f)
    }
}

/// An owned path, written as a JSON Pointer (RFC 6901) when displayed.
///
/// The root is the empty pointer `""`; a member `a` of an element `0` of `items` is `/items/0/a`.
/// A `~` in a key is written `~0` and a `/` is written `~1`, so a key holding either is still one
/// segment.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pointer {
    segments: Vec<String>,
}

impl Pointer {
    /// The root of the input.
    pub fn root() -> Self {
        Self::default()
    }

    /// The segments from the root down, keys and indices alike as text.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Whether this is the root of the input.
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// This pointer with `segment` appended.
    pub fn join(mut self, segment: impl Into<String>) -> Self {
        self.segments.push(segment.into());
        self
    }

    /// This pointer read as relative to `prefix`: `prefix`'s segments followed by these.
    pub fn prefixed(&self, prefix: &Path<'_>) -> Pointer {
        let mut pointer = prefix.to_pointer();
        pointer.segments.extend(self.segments.iter().cloned());
        pointer
    }

    /// Reads a JSON Pointer, undoing the `~0` and `~1` escapes. Returns `None` when `text` is
    /// neither empty nor starts with `/`, or holds a `~` not followed by `0` or `1`.
    pub fn parse(text: &str) -> Option<Self> {
        if text.is_empty() {
            return Some(Self::root());
        }
        let rest = text.strip_prefix('/')?;
        let mut segments = Vec::new();
        for raw in rest.split('/') {
            let mut segment = String::with_capacity(raw.len());
            let mut chars = raw.chars();
            while let Some(c) = chars.next() {
                if c == '~' {
                    match chars.next() {
                        Some('0') => segment.push('~'),
                        Some('1') => segment.push('/'),
                        _ => return None,
                    }
                } else {
                    segment.push(c);
                }
            }
            segments.push(segment);
        }
        Some(Self { segments })
    }
}

impl<S: Into<String>> FromIterator<S> for Pointer {
    fn from_iter<T: IntoIterator<Item = S>>(iter: T) -> Self {
        Self {
            segments: iter.into_iter().map(Into::into).collect(),
        }
    }
}

impl fmt::Display for Pointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for segment in &self.segments {
            f.write_str("/")?;
            for c in segment.chars() {
                match c {
                    '~' => f.write_str("~0")?,
                    '/' => f.write_str("~1")?,
                    c => fmt::Write::write_char(f, c)?,
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn root_is_the_empty_pointer() {
        assert_eq!(Path::ROOT.to_pointer().to_string(), "");
        assert!(Path::ROOT.is_root());
    }

    #[test]
    fn keys_and_indices_are_written_in_order() {
        let items = Path::ROOT.key("items");
        let first = items.index(0);
        let name = first.key("name");
        assert_eq!(name.to_string(), "/items/0/name");
    }

    #[test]
    fn tilde_and_slash_are_escaped() {
        let path = Path::ROOT.key("a/b~c");
        assert_eq!(path.to_string(), "/a~1b~0c");
    }

    #[test]
    fn an_empty_key_is_one_segment() {
        let path = Path::ROOT.key("");
        assert_eq!(path.to_string(), "/");
        assert_eq!(path.to_pointer().segments(), [""]);
    }

    #[test]
    fn prefixed_puts_the_prefix_first() {
        let user = Path::ROOT.key("user");
        let relative: Pointer = ["id"].into_iter().collect();
        assert_eq!(relative.prefixed(&user).to_string(), "/user/id");
    }

    proptest! {
        #[test]
        fn a_written_pointer_reads_back_to_its_segments(segments in prop::collection::vec(".*", 0..5)) {
            let pointer: Pointer = segments.iter().cloned().collect();
            let read = Pointer::parse(&pointer.to_string()).unwrap();
            prop_assert_eq!(read.segments(), &segments[..]);
        }
    }
}
