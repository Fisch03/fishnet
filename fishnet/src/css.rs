//! data structures for dealing with css

/// a special type of string generated using the [`css!`](crate::css!) macro.
///
/// currently this internally is a css string with the character `&` being substituted with the top level class used in
/// the render function. this may change at any time, so directly constructing a [`StyleFragment`] without
/// the [`css!`](crate::css!) macro is strongly discouraged.
pub struct StyleFragment(&'static str);

impl StyleFragment {
    /// construct a new [`StyleFragment`] using the given string.
    ///
    /// since this is normally only used
    /// via the [`css!`](crate::css!) macro, there is zero validation of the passed in string
    /// slice!
    pub fn new(input: &'static str) -> Self {
        Self(input)
    }

    /// render the [`StyleFragment`] relative to the passed in `toplevel_class`.
    pub fn render(&self, toplevel_class: &str) -> String {
        self.0.to_string().replace("&", toplevel_class)
    }
}
