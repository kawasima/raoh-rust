//! The codes an [`Issue`](crate::Issue) carries and the message keys that refine them.
//!
//! Both are shared with Raoh for Java and PHP, so a message catalogue written for one resolves the
//! issues of the others.

/// What kind of problem an issue reports.
pub mod codes {
    /// The value is missing or null.
    pub const REQUIRED: &str = "required";
    /// The string is empty or whitespace only.
    pub const BLANK: &str = "blank";
    /// The string has fewer characters than allowed.
    pub const TOO_SHORT: &str = "too_short";
    /// The string has more characters than allowed.
    pub const TOO_LONG: &str = "too_long";
    /// The string does not have exactly the required number of characters.
    pub const INVALID_LENGTH: &str = "invalid_length";
    /// The number is outside its bounds.
    pub const OUT_OF_RANGE: &str = "out_of_range";
    /// The number is not a multiple of the divisor.
    pub const NOT_MULTIPLE_OF: &str = "not_multiple_of";
    /// The decimal has more fraction digits than allowed.
    pub const INVALID_SCALE: &str = "invalid_scale";
    /// The list has fewer elements than allowed.
    pub const TOO_SMALL: &str = "too_small";
    /// The list has more elements than allowed.
    pub const TOO_BIG: &str = "too_big";
    /// The list does not have exactly the required number of elements.
    pub const INVALID_SIZE: &str = "invalid_size";
    /// The value is not the one required.
    pub const INVALID_VALUE: &str = "invalid_value";
    /// The string does not have the required form.
    pub const INVALID_FORMAT: &str = "invalid_format";
    /// The value is of another JSON type than the one required.
    pub const TYPE_MISMATCH: &str = "type_mismatch";
    /// The object has a member no decoder declares.
    pub const UNKNOWN_FIELD: &str = "unknown_field";
    /// The list lacks a required element.
    pub const MISSING_ELEMENT: &str = "missing_element";
    /// The list lacks some of the required elements.
    pub const MISSING_ELEMENTS: &str = "missing_elements";
    /// The list holds an element more than once.
    pub const DUPLICATE_ELEMENT: &str = "duplicate_element";
    /// The value is not one of the allowed values.
    pub const NOT_ALLOWED: &str = "not_allowed";
    /// None of the alternatives decoded the value.
    pub const ONE_OF_FAILED: &str = "one_of_failed";
    /// A member is missing.
    pub const MISSING_FIELD: &str = "missing_field";
}

/// Keys that name which constraint produced an issue, where one code covers several.
///
/// An issue whose message key is not one of these uses its code as its key.
pub mod message_keys {
    /// `out_of_range` from a lower bound.
    pub const OUT_OF_RANGE_MINIMUM: &str = "out_of_range.minimum";
    /// `out_of_range` from an upper bound.
    pub const OUT_OF_RANGE_MAXIMUM: &str = "out_of_range.maximum";
    /// `out_of_range` from both bounds.
    pub const OUT_OF_RANGE_RANGE: &str = "out_of_range.range";
    /// `out_of_range` from `positive()`.
    pub const OUT_OF_RANGE_POSITIVE: &str = "out_of_range.positive";
    /// `out_of_range` from `negative()`.
    pub const OUT_OF_RANGE_NEGATIVE: &str = "out_of_range.negative";
    /// `out_of_range` from `non_negative()`.
    pub const OUT_OF_RANGE_NON_NEGATIVE: &str = "out_of_range.non_negative";
    /// `out_of_range` from `non_positive()`.
    pub const OUT_OF_RANGE_NON_POSITIVE: &str = "out_of_range.non_positive";
    /// `too_small` from `non_empty()`.
    pub const TOO_SMALL_NONEMPTY: &str = "too_small.nonempty";
}
