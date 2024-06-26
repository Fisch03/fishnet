use crate::css::ast;
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use proc_macro_error::{abort, abort_call_site, emit_error, SpanRange};

pub(crate) fn parse(input: TokenStream) -> ast::Ruleset {
    Parser::new(input).parse()
}

struct Parser {
    input: <TokenStream as IntoIterator>::IntoIter,
}

impl Iterator for Parser {
    type Item = TokenTree;
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.input.next();
        match next {
            Some(TokenTree::Punct(ref punct)) if punct.as_char() == '&' => {
                abort!(
                    punct.span(),
                    "using the nesting selector '&' is currently not supported."
                );
            }
            _ => next,
        }
    }
}

impl Parser {
    fn new(input: TokenStream) -> Self {
        Self {
            input: input.into_iter(),
        }
    }

    fn peek(&mut self) -> Option<TokenTree> {
        self.input.clone().next()
    }

    fn next_allow_nesting(&mut self) -> Option<TokenTree> {
        self.input.next()
    }

    fn advance(&mut self) {
        self.input.next();
    }

    fn parse(&mut self) -> ast::Ruleset {
        let mut output = Vec::new();

        loop {
            match self.peek() {
                None => break,
                Some(TokenTree::Punct(ref punct)) if punct.as_char() == ';' => self.advance(),
                _ => output.push(self.parse_fragment()),
            }
        }

        ast::Ruleset(output)
    }

    fn parse_fragment(&mut self) -> ast::StyleFragment {
        let token = match self.next_allow_nesting() {
            Some(token) => token,
            None => abort_call_site!("unexpected end of input"),
        };

        let result: Option<ast::StyleFragment> = match token {
            TokenTree::Ident(ref ident) => self.parse_declaration_or_qualified(ident.to_string()),
            TokenTree::Literal(ref literal) => self
                .parse_declaration(literal.to_string())
                .map(ast::StyleFragment::TopLevelDeclaration),
            TokenTree::Punct(ref punct) => match punct.as_char() {
                // at-rule
                '@' => self.parse_at_rule().map(ast::StyleFragment::AtRule),
                // combined selectors (with spacing)
                '*' | '>' => self
                    .parse_qualified_rule(format!("{} ", punct))
                    .map(ast::StyleFragment::QualifiedRule),
                // part of a selector (no spacing)
                '.' | '#' | ':' | '&' | '+' => self
                    .parse_qualified_rule(punct.to_string())
                    .map(ast::StyleFragment::QualifiedRule),
                _ => None,
            },
            // TODO: attribute selectors [attr="value"]
            _ => None,
        };

        result.unwrap_or_else(|| {
            emit_error!(token.span(), "parse error");
            ast::StyleFragment::ParseError(SpanRange::single_span(token.span()))
        })
    }

    fn parse_declaration_or_qualified(&mut self, mut ident: String) -> Option<ast::StyleFragment> {
        loop {
            match self.peek() {
                Some(TokenTree::Punct(ref punct)) if punct.as_char() == ':' => {
                    return self
                        .parse_declaration(ident)
                        .map(ast::StyleFragment::TopLevelDeclaration)
                }
                Some(TokenTree::Punct(ref punct)) if punct.as_char() == '-' => {
                    ident.push('-');
                }
                Some(TokenTree::Ident(ref i)) => ident.push_str(&i.to_string()),
                Some(TokenTree::Literal(ref lit)) => ident.push_str(&lit.to_string()),
                _ => {
                    return self
                        .parse_qualified_rule(ident)
                        .map(ast::StyleFragment::QualifiedRule)
                }
            }
            self.advance();
        }
    }

    fn parse_declaration(&mut self, mut ident: String) -> Option<ast::Declaration> {
        loop {
            match self.peek() {
                Some(TokenTree::Punct(ref punct)) if punct.as_char() == ':' => {
                    self.advance();
                    break;
                }
                Some(TokenTree::Punct(ref punct)) if punct.as_char() == '-' => ident.push('-'),
                Some(TokenTree::Ident(ref i)) => ident.push_str(&i.to_string()),
                Some(TokenTree::Literal(ref lit)) => ident.push_str(&lit.to_string()),
                _ => return None,
            }
            self.advance();
        }

        let mut value = String::new();
        let mut maybe_triple_quote = false;
        let mut add_space = true;

        loop {
            match self.peek() {
                Some(TokenTree::Punct(ref punct)) => match punct.as_char() {
                    ';' => {
                        if value.ends_with(' ') {
                            value.pop();
                        }
                        break;
                    }
                    '{' | '}' => {
                        return None;
                    }
                    ':' | '&' => {
                        abort!(punct.span(), "unexpected token");
                    }
                    _ => {
                        if value.ends_with(' ') {
                            value.pop();
                        }
                        add_space = false;
                        value.push(punct.as_char())
                    }
                },
                Some(TokenTree::Literal(ref lit)) => {
                    let lit = lit.to_string();
                    let mut lit = lit.as_str();
                    if maybe_triple_quote {
                        maybe_triple_quote = false;
                    } else if lit == "\"\"" {
                        // triple quoted string incoming? if yes fully escape it
                        add_space = false;
                        maybe_triple_quote = true;
                        lit = "";
                    } else if lit.starts_with("\"#")
                        || lit.ends_with("em\"")
                        || lit.ends_with("ex\"")
                    {
                        // allow escaping values that might have a "..2e.." form since rust treats them
                        // as exponential numbers otherwise
                        lit = &lit[1..lit.len() - 1];
                    }
                    value.push_str(lit);
                }
                Some(TokenTree::Ident(ref ident)) => value.push_str(&ident.to_string()),
                Some(TokenTree::Group(ref group))
                    if group.delimiter() == Delimiter::Parenthesis =>
                {
                    if value.ends_with(' ') {
                        value.pop();
                    }
                    value.push('(');
                    value.push_str(&group.stream().to_string());
                    value.push(')');
                }
                Some(token) => abort!(token.span(), "unexpected token"),
                None => abort_call_site!("unexpected end of input"),
            }
            if add_space {
                value.push(' ');
            }
            add_space = true;

            self.advance();
        }

        Some(ast::Declaration {
            property: ident,
            value,
        })
    }

    fn parse_qualified_rule(&mut self, ident: String) -> Option<ast::QualifiedRule> {
        let selector = self.parse_selector(ident)?;

        let mut declarations = Vec::new();

        match self.peek() {
            Some(TokenTree::Group(ref group)) => {
                self.advance();
                let main_stream = self.input.clone();
                self.input = group.stream().into_iter();
                loop {
                    match self.next() {
                        Some(TokenTree::Ident(ref ident)) => {
                            declarations.push(self.parse_declaration(ident.to_string())?);
                        }
                        Some(TokenTree::Literal(ref lit)) => {
                            declarations.push(self.parse_declaration(lit.to_string())?);
                        }
                        Some(TokenTree::Punct(ref punct)) if punct.as_char() == ';' => {}
                        Some(TokenTree::Punct(ref punct)) => {
                            declarations.push(self.parse_declaration(punct.to_string())?);
                        }
                        Some(_) => break,
                        None => break,
                    }
                }
                self.input = main_stream;
            }
            _ => return None,
        }

        Some(ast::QualifiedRule {
            selector,
            declarations,
        })
    }

    fn parse_selector(&mut self, ident: String) -> Option<ast::Selector> {
        let mut selector = ident;

        loop {
            match self.peek() {
                Some(TokenTree::Punct(ref punct)) => selector.push(punct.as_char()),
                Some(TokenTree::Ident(ref ident)) => selector.push_str(&ident.to_string()),
                Some(TokenTree::Group(ref group))
                    if group.delimiter() == Delimiter::Parenthesis =>
                {
                    selector.push('(');
                    selector.push_str(&group.stream().to_string());
                    selector.push(')');
                }
                Some(TokenTree::Group(_)) => break,
                Some(token) => abort!(token.span(), "unexpected token"),
                None => abort_call_site!("unexpected end of input"),
            }
            self.advance();
        }

        let mut selector = selector.trim().to_string();

        if !selector.starts_with(":") && !selector.starts_with("&") {
            selector.insert_str(0, " ");
        }

        Some(ast::Selector(selector))
    }

    fn parse_at_rule(&mut self) -> Option<ast::AtRule> {
        match self.next() {
            Some(TokenTree::Ident(ref ident)) => match ident.to_string().as_str() {
                "media" => self.parse_media_rule().map(ast::AtRule::Media),
                _ => {
                    let mut rule = format!("@{} ", ident.to_string());
                    loop {
                        match self.next() {
                            Some(TokenTree::Punct(ref punct)) if punct.as_char() == ';' => {
                                rule.push(';');
                                break;
                            }
                            Some(TokenTree::Punct(ref punct)) => rule.push(punct.as_char()),
                            Some(TokenTree::Ident(ref ident)) => rule.push_str(&ident.to_string()),
                            Some(TokenTree::Group(ref group))
                                if group.delimiter() == Delimiter::Parenthesis =>
                            {
                                rule.push('(');
                                rule.push_str(&group.stream().to_string());
                                rule.push(')');
                            }

                            Some(token) => abort!(token.span(), "unexpected token"),
                            None => abort_call_site!("unexpected end of input"),
                        }
                    }
                    Some(ast::AtRule::Other(rule))
                }
            },
            Some(token) => abort!(token.span(), "unexpected token"),
            None => abort_call_site!("unexpected end of input"),
        }
    }

    fn parse_media_rule(&mut self) -> Option<ast::MediaRule> {
        let mut condition = String::new();
        let inner = loop {
            match self.peek() {
                Some(TokenTree::Punct(ref punct)) => condition.push(punct.as_char()),
                Some(TokenTree::Ident(ref ident)) => condition.push_str(&ident.to_string()),
                Some(TokenTree::Group(ref group)) => {
                    if group.delimiter() == Delimiter::Parenthesis {
                        condition.push_str("(");
                        condition.push_str(&group.stream().to_string());
                        condition.push_str(")");
                    } else {
                        self.advance();
                        break group.stream();
                    }
                }
                Some(token) => abort!(token.span(), "unexpected token"),
                None => abort_call_site!("unexpected end of input"),
            }
            condition.push(' ');
            self.advance();
        };

        let mut parser = Parser::new(inner);
        let rules = parser.parse();

        Some(ast::MediaRule { condition, rules })
    }
}
