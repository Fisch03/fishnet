//! data structures and functions for dealing with css

///  function for turning a pascal case string into a kebab case string.
pub(crate) fn pascal_to_kebab(input: &str) -> String {
    let mut out = String::new();

    let mut iter = input.chars();
    out.push(iter.next().unwrap().to_ascii_lowercase());

    for c in iter {
        if c.is_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

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
    pub fn render(&self, toplevel_class: &str) -> StyleString {
        StyleString(self.0.to_string().replace("&", toplevel_class))
    }
}

/// string representation of a rendered [`StyleFragment`].
#[derive(Debug, Clone)]
pub struct StyleString(String);

impl StyleString {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn consume(self) -> String {
        self.0
    }
}
