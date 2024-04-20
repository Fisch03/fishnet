//! data structures and functions for dealing with js

use std::path::Path;

#[allow(unused_imports)]
use tracing::{debug, instrument};

/// source of a javascript script
#[derive(Debug, Hash, PartialEq, Eq)]
pub enum ScriptType {
    /// an inline script
    Inline(&'static str),
    /// an external script file.
    External(String),
}

/// string representation of a script
#[derive(Debug, Clone)]
pub struct ScriptString(String);

impl ScriptString {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// consume the [`ScriptString`], returning the inner string.
    pub fn consume(self) -> String {
        self.0
    }
}

/// load the [`ScriptType`] into a string, according to its type
impl From<&ScriptType> for ScriptString {
    fn from(script: &ScriptType) -> Self {
        Self(match script {
            ScriptType::Inline(script) => script.to_string(),
            ScriptType::External(path) => {
                let path = Path::new("static/").join(path);
                std::fs::read_to_string(&path).unwrap()
            }
        })
    }
}

///minify the given [`ScriptString`]. this will also wrap it in an IIFE.
#[cfg(feature = "minify-js")]
#[cfg_attr(docsrs, doc(cfg(feature = "minify-js")))]
#[instrument(skip_all, level = "debug")]
pub async fn minify_script(script: ScriptString) -> ScriptString {
    use esbuild_rs::{transform, Format, TransformOptionsBuilder};
    use std::sync::Arc;

    let start = std::time::Instant::now();

    let mut options = TransformOptionsBuilder::new();
    options.format = Format::IIFE;
    options.minify_syntax = true;
    options.minify_whitespace = true;
    options.minify_identifiers = true;
    let options = options.build();

    let in_size = script.0.len();
    let script = transform(Arc::new(script.0.into()), options.clone()).await;

    let script_out = script.code.to_string();
    debug!(
        "minfied script, {:?} bytes -> {:?} bytes. took {:?}",
        in_size,
        script_out.len(),
        start.elapsed()
    );

    ScriptString(script_out)
}
