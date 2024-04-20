use proc_macro_error::{emit_error, SpanRange};

pub trait ToFmt {
    fn to_fmt(&self) -> String;
    fn indent(input: String) -> String {
        input
            .lines()
            .map(|line| {
                if !line.is_empty() {
                    format!("    {}", line)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

#[derive(Debug)]
pub(crate) struct Ruleset(pub Vec<StyleFragment>);
impl ToFmt for Ruleset {
    fn to_fmt(&self) -> String {
        let mut out = String::new();

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
            out.push_str(
                &StyleFragment::QualifiedRule(QualifiedRule {
                    selector: Selector("".to_string()),
                    declarations: top_level_declarations
                        .iter()
                        .map(|declaration| Declaration {
                            property: declaration.property.clone(),
                            value: declaration.value.clone(),
                        })
                        .collect(),
                })
                .to_fmt(),
            );
        }

        for fragment in &self.0 {
            match fragment {
                StyleFragment::TopLevelDeclaration(_) => {}
                _ => out.push_str(&fragment.to_fmt()),
            }
        }

        out
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
    fn to_fmt(&self) -> String {
        match self {
            StyleFragment::TopLevelDeclaration(declaration) => declaration.to_fmt(),
            StyleFragment::QualifiedRule(rule) => rule.to_fmt(),
            StyleFragment::AtRule(at_rule) => at_rule.to_fmt(),
            StyleFragment::ParseError(span) => {
                emit_error!(span, "parse error");
                String::new()
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Declaration {
    pub property: String,
    pub value: String,
}
impl ToFmt for Declaration {
    fn to_fmt(&self) -> String {
        format!("{}: {};", self.property, self.value)
    }
}

#[derive(Debug)]
pub(crate) struct Selector(pub String);
impl ToFmt for Selector {
    fn to_fmt(&self) -> String {
        if self.0.is_empty() {
            return ".&".to_string();
        }
        format!(".&{}", self.0)
    }
}

#[derive(Debug)]
pub(crate) struct QualifiedRule {
    pub selector: Selector,
    pub declarations: Vec<Declaration>,
}
impl ToFmt for QualifiedRule {
    fn to_fmt(&self) -> String {
        let mut out = String::new();

        if self.declarations.is_empty() {
            return out;
        }

        out.push_str(&self.selector.to_fmt());
        out.push_str(" {\n");
        for declaration in &self.declarations {
            out.push_str(&Self::indent(declaration.to_fmt()));
            out.push('\n');
        }
        out.push_str("}\n\n");
        out
    }
}

#[derive(Debug)]
pub(crate) enum AtRule {
    Media(MediaRule),
    Other(String),
}
impl ToFmt for AtRule {
    fn to_fmt(&self) -> String {
        match self {
            AtRule::Media(media_rule) => media_rule.to_fmt(),
            AtRule::Other(other) => other.clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct MediaRule {
    pub condition: String,
    pub rules: Ruleset,
}
impl ToFmt for MediaRule {
    fn to_fmt(&self) -> String {
        format!(
            "@media {}{{\n{}}}\n",
            self.condition,
            Self::indent(self.rules.to_fmt())
        )
    }
}
