//! data structures and functions for dealing with css
use std::collections::{hash_map::Entry, HashMap};
use tracing::debug;

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

/// css generated using the [`css!`](crate::css!) macro.
///
/// currently this internally is css with the character `&` being substituted with the top level class used in
/// the render function. this may (and probably will) change at any time, so directly constructing a [`StyleFragment`] without
/// the [`css!`](crate::css!) macro is strongly discouraged.
pub struct StyleFragment<'a> {
    style: &'a str,
    media_queries: &'a [(&'a str, &'a str)],
}

impl<'a> StyleFragment<'_> {
    /// construct a new [`StyleFragment`] using the given string.
    ///
    /// since this is normally only used
    /// via the [`css!`](crate::css!) macro, there is zero validation of the passed in string
    /// slice!
    pub fn new(style: &'a str, media_queries: &'a [(&'a str, &'a str)]) -> StyleFragment<'a> {
        StyleFragment {
            style,
            media_queries,
        }
    }

    /// render the [`StyleFragment`] relative to the passed in `toplevel_class`.
    pub fn render(&self, toplevel_class: &str) -> RenderedStyle {
        RenderedStyle {
            style: self.style.replace("&", toplevel_class),
            media_queries: self
                .media_queries
                .iter()
                .map(|(query, style)| (query.to_string(), style.replace("&", toplevel_class)))
                .collect(),
        }
    }
}

/// string representation of a rendered [`StyleFragment`].
#[derive(Debug, Clone)]
pub struct RenderedStyle {
    style: String,
    media_queries: Vec<(String, String)>,
}

/// a full css stylesheet
///
/// a stylesheet is basically just a collection of [`RenderedStyle`]s. however it caches all its
/// renders, so you can only add to it and never remove things.
pub struct Stylesheet {
    style: String,
    rendered_media_queries: String,

    media_queries: HashMap<String, String>,
    media_queries_size_hint: usize,
    media_queries_changed: bool,
}

impl Stylesheet {
    pub fn new() -> Self {
        Self {
            style: String::new(),
            rendered_media_queries: String::new(),

            media_queries: HashMap::new(),
            media_queries_size_hint: 0,
            media_queries_changed: false,
        }
    }

    pub fn add(&mut self, rendered: &RenderedStyle) {
        self.style.push_str(&rendered.style);

        self.media_queries_changed |= !rendered.media_queries.is_empty();

        for (query, style) in &rendered.media_queries {
            self.media_queries_size_hint += style.len() + query.len();
            match self.media_queries.entry(query.to_string()) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().push('\n');
                    entry.get_mut().push_str(&style);
                }
                Entry::Vacant(entry) => {
                    entry.insert(style.to_string());
                }
            }
        }
    }

    pub fn render(&mut self) -> String {
        if self.media_queries_changed {
            debug!("re-rendering media queries");

            self.rendered_media_queries = self.media_queries.iter().fold(
                String::with_capacity(self.media_queries_size_hint),
                |mut acc, (query, style)| {
                    acc.push_str(&format!("\n@media {}{{\n{}}}\n", query, style));
                    acc
                },
            );

            self.media_queries_changed = false;
        }
        format!("{}{}", self.style, self.rendered_media_queries)
    }
}
