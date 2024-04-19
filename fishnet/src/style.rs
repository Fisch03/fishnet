use std::cmp::{Eq, PartialEq};
use std::hash::{Hash, Hasher};

pub struct StyleFragment(&'static str);

impl StyleFragment {
    pub fn into_block(self, selector: &str) -> StyleBlock {
        StyleBlock {
            selector: selector.into(),
            content: self.0,
        }
    }
}

#[derive(Debug)]
pub struct StyleBlock {
    selector: String,
    content: &'static str,
}

impl StyleBlock {
    pub fn to_string(&self) -> String {
        format!("{} {{ {} }}", self.selector, self.content)
    }
}

impl Hash for StyleBlock {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.selector.hash(state);
    }
}

impl PartialEq for StyleBlock {
    fn eq(&self, other: &Self) -> bool {
        self.selector == other.selector
    }
}
impl Eq for StyleBlock {}

#[macro_export]
macro_rules! css {
    ($style:literal) => {
        $crate::style::StyleFragment($style)
    };
}
