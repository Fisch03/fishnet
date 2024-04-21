use proc_macro2::TokenStream;
use proc_macro_error::{emit_error, SpanRange};
use quote::{quote, ToTokens};

#[derive(Debug)]
pub struct StyleFmt {
    indent_lvl: usize,
    style: String,
    media_queries: Vec<(String, String)>,
}

impl StyleFmt {
    pub fn new() -> Self {
        Self {
            indent_lvl: 0,
            style: String::new(),
            media_queries: Vec::new(),
        }
    }

    fn finish(&mut self) {
        self.style = self.style.trim_end().to_string();
        self.style.push('\n');
    }
    // i hate this...
    fn push_style(&mut self, style: &str) {
        self.style.push_str(&self.indent(style));
    }
    fn push_style_no_newline(&mut self, style: &str) {
        self.style.push_str(&self.indent(style).trim_end());
    }
    fn push_style_no_indent(&mut self, style: &str) {
        self.style.push_str(style);
    }

    fn push_media_query(&mut self, query: &str, style: &str) {
        self.media_queries
            .push((query.to_string(), style.to_string()));
    }

    fn enter_indent(&mut self) {
        self.indent_lvl += 1;
    }
    fn exit_indent(&mut self) {
        self.indent_lvl -= 1;
    }

    fn indent(&self, input: &str) -> String {
        let indent = "    ".repeat(self.indent_lvl);

        let mut out = String::with_capacity(input.len());

        for line in input.lines() {
            if line.is_empty() {
                out.push('\n');
                continue;
            }

            out.push_str(&indent);
            out.push_str(line);
            out.push('\n');
        }

        out
    }
}

impl ToTokens for StyleFmt {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut style_stream = TokenStream::new();

        let style = &self.style;

        style_stream.extend(quote! {
            #style
        });

        let mut media_queries = TokenStream::new();
        for (query, style) in &self.media_queries {
            media_queries.extend(quote! {
                ( #query, #style ),
            });
        }

        tokens.extend(quote! {
            #style,
            &[#media_queries]
        });
    }
}

pub trait ToFmt {
    fn to_fmt(&self, style: &mut StyleFmt);
}

#[derive(Debug)]
pub(crate) struct Ruleset(pub Vec<StyleFragment>);
impl ToFmt for Ruleset {
    fn to_fmt(&self, style: &mut StyleFmt) {
        // bring top-level declarations to the top and wrap them in a single block
        let mut top_level_declarations = Vec::new();
        for fragment in &self.0 {
            match fragment {
                StyleFragment::TopLevelDeclaration(declaration) => {
                    top_level_declarations.push(declaration)
                }
                _ => {}
            }
        }
        if !top_level_declarations.is_empty() {
            StyleFragment::QualifiedRule(QualifiedRule {
                selector: Selector("".to_string()),
                declarations: top_level_declarations
                    .iter()
                    .map(|declaration| Declaration {
                        property: declaration.property.clone(),
                        value: declaration.value.clone(),
                    })
                    .collect(),
            })
            .to_fmt(style);
        }

        for fragment in &self.0 {
            match fragment {
                StyleFragment::TopLevelDeclaration(_) => {}
                _ => fragment.to_fmt(style),
            }
        }

        style.finish();
    }
}

#[derive(Debug)]
pub(crate) enum StyleFragment {
    TopLevelDeclaration(Declaration),
    QualifiedRule(QualifiedRule),
    AtRule(AtRule),
    ParseError(SpanRange),
}
impl ToFmt for StyleFragment {
    fn to_fmt(&self, style: &mut StyleFmt) {
        match self {
            StyleFragment::TopLevelDeclaration(declaration) => declaration.to_fmt(style),
            StyleFragment::QualifiedRule(rule) => rule.to_fmt(style),
            StyleFragment::AtRule(at_rule) => at_rule.to_fmt(style),
            StyleFragment::ParseError(span) => {
                emit_error!(span, "parse error");
            }
        };
    }
}

#[derive(Debug)]
pub(crate) struct Declaration {
    pub property: String,
    pub value: String,
}
impl ToFmt for Declaration {
    fn to_fmt(&self, style: &mut StyleFmt) {
        style.push_style(&format!("{}: {};\n", self.property, self.value));
    }
}

#[derive(Debug)]
pub(crate) struct Selector(pub String);
impl ToFmt for Selector {
    fn to_fmt(&self, style: &mut StyleFmt) {
        style.push_style_no_newline(".");
        if !self.0.contains("&") {
            style.push_style_no_indent("&");
        }
        style.push_style_no_indent(&self.0);
    }
}

#[derive(Debug)]
pub(crate) struct QualifiedRule {
    pub selector: Selector,
    pub declarations: Vec<Declaration>,
}
impl ToFmt for QualifiedRule {
    fn to_fmt(&self, style: &mut StyleFmt) {
        if self.declarations.is_empty() {
            return;
        }

        self.selector.to_fmt(style);
        style.push_style_no_indent(" {\n");
        style.enter_indent();
        for declaration in &self.declarations {
            declaration.to_fmt(style);
        }
        style.exit_indent();
        style.push_style("}\n\n");
    }
}

#[derive(Debug)]
pub(crate) enum AtRule {
    Media(MediaRule),
    Other(String),
}
impl ToFmt for AtRule {
    fn to_fmt(&self, style: &mut StyleFmt) {
        match self {
            AtRule::Media(media_rule) => media_rule.to_fmt(style),
            AtRule::Other(other) => style.push_style(&other),
        }
    }
}

#[derive(Debug)]
pub(crate) struct MediaRule {
    pub condition: String,
    pub rules: Ruleset,
}
impl ToFmt for MediaRule {
    fn to_fmt(&self, style: &mut StyleFmt) {
        let mut inner_style = StyleFmt::new();
        inner_style.enter_indent();
        self.rules.to_fmt(&mut inner_style);
        inner_style.exit_indent();

        style.push_media_query(&self.condition, &inner_style.style);
    }
}
