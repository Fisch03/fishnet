use litrs::StringLit;
use proc_macro2::{Delimiter, Ident, Literal, Span, TokenStream, TokenTree};
use proc_macro_error::{abort, abort_call_site, emit_error};
use quote::{quote, ToTokens, TokenStreamExt};

#[derive(Debug)]
pub struct ParsedComponent {
    name: String,
    args: TokenStream,
    state: Option<ComponentState>,
    style: Option<ComponentStyle>,
    script: ComponentScript,
    render: ComponentRender,
}

impl ParsedComponent {
    fn new(name: &str, args: TokenStream) -> Self {
        Self {
            name: name.to_string(),
            args,

            state: None,
            style: None,
            script: ComponentScript {
                script: String::new(),
            },
            render: ComponentRender {
                code: TokenStream::new(),
                markup: None,
            },
        }
    }
}

impl ToTokens for ParsedComponent {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name_pascal = to_pascal(&self.name);
        let name = Ident::new(&self.name, Span::call_site());

        let fn_args = &self.args;

        let (init_state, state, state_ident) = match &self.state {
            Some(state) => {
                let type_name = &state.type_name;
                let ident = Ident::new(&state.ident, Span::call_site());
                (
                    quote! {
                        let #ident: #type_name = std::default::Default::default();
                    },
                    quote! {
                        .with_state(#ident)
                    },
                    ident,
                )
            }
            None => (
                TokenStream::new(),
                TokenStream::new(),
                Ident::new("_", Span::call_site()),
            ),
        };

        let style = match &self.style {
            Some(style) => {
                let style = &style.style;
                quote! {
                    .style(#style)
                }
            }
            None => TokenStream::new(),
        };

        let script = if self.script.script.is_empty() {
            TokenStream::new()
        } else {
            let script = Literal::string(&self.script.script);
            quote! {
                .add_script(fishnet::js::ScriptType::Inline(#script))
            }
        };

        let markup = match &self.render.markup {
            Some(markup) => markup.clone(),
            None => TokenStream::new(),
        };
        let code = &self.render.code;
        let render = quote! {
            .render(|#state_ident| async move {
                #code

                html! {
                    #markup
                }
            }.boxed())
        };

        tokens.extend(quote! {
            fn #name(#fn_args) -> impl BuildableComponent {
                #init_state

                fishnet::component::Component::new(#name_pascal, fishnet::const_nanoid!())
                    #state
                    #style
                    #script
                    #render
            }
        })
    }
}

#[derive(Debug)]
struct ComponentState {
    type_name: TokenStream,
    ident: String,
}

#[derive(Debug)]
struct ComponentStyle {
    style: TokenStream,
}

#[derive(Debug)]
struct ComponentScript {
    script: String,
}

#[derive(Debug)]
struct ComponentRender {
    code: TokenStream,
    markup: Option<TokenStream>,
}

#[derive(Debug)]
enum MacroTypes {
    Style,
    Script,
    Render,
}

pub(crate) fn parse(input: TokenStream) -> ParsedComponent {
    Parser::new(input).parse()
}

struct Parser {
    input: <TokenStream as IntoIterator>::IntoIter,
    parsed: ParsedComponent,
    last_ident: Option<String>,
}

impl Iterator for Parser {
    type Item = TokenTree;
    fn next(&mut self) -> Option<Self::Item> {
        self.input.next()
    }
}

fn to_pascal(name: &str) -> String {
    let mut name = name.chars();
    let mut next_upper = true;
    let mut out = String::new();

    while let Some(c) = name.next() {
        if c == '_' {
            next_upper = true;
            continue;
        }

        if next_upper {
            out.push(c.to_ascii_uppercase());
            next_upper = false;
        } else {
            out.push(c);
        }
    }

    out
}

impl Parser {
    fn new(input: TokenStream) -> Self {
        let mut input = input.into_iter();

        let next = input.next();
        match next {
            Some(TokenTree::Ident(ref ident)) if ident.to_string() == "fn" => {
                abort!(next.unwrap(), "function has to be async")
            }
            Some(TokenTree::Ident(ref ident)) if ident.to_string() == "async" => {
                input.next();
            }
            Some(token) => abort!(token, "expected function definition"),
            None => abort_call_site!("expected function definition"),
        }

        let name = match input.next() {
            Some(TokenTree::Ident(ident)) => ident.to_string(),
            _ => abort_call_site!("expected function name"),
        };

        let fn_args = match input.next() {
            Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis => {
                group.stream()
            }
            _ => abort_call_site!("expected function arguments"),
        };

        let fn_inner = match input.next() {
            Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
                group.stream()
            }
            _ => abort_call_site!("expected function body"),
        };

        Self {
            input: fn_inner.into_iter(),
            parsed: ParsedComponent::new(&name, fn_args),
            last_ident: None,
        }
    }

    fn peek(&mut self) -> Option<TokenTree> {
        self.input.clone().next()
    }

    fn advance(&mut self) {
        self.input.next();
    }

    fn add_to_code(&mut self, code: TokenTree) {
        self.parsed.render.code.extend(quote!(#code));
    }

    fn parse(mut self) -> ParsedComponent {
        loop {
            let next = self.peek();
            match next.as_ref() {
                Some(TokenTree::Ident(ref ident)) => match ident.to_string().as_str() {
                    "style" => self.parse_macro(MacroTypes::Style),
                    "script" => self.parse_macro(MacroTypes::Script),
                    "html" => self.parse_macro(MacroTypes::Render),
                    "let" => {
                        let mut collected = TokenStream::new();
                        collected.append(next.unwrap());
                        self.advance();

                        let ident = self.next().unwrap_or_else(|| {
                            abort_call_site!("unexpected end of input after 'let'")
                        });
                        self.last_ident = Some(ident.to_string());
                        collected.append(ident);

                        let eq = self.next().unwrap_or_else(|| {
                            abort_call_site!("unexpected end of input");
                        });
                        collected.append(eq);

                        let next_inner = self.peek();
                        match next_inner {
                            Some(TokenTree::Ident(ref ident))
                                if ident.to_string().as_str() == "state" =>
                            {
                                collected.append(next_inner.unwrap());
                                self.advance();

                                let next = self.peek();
                                match next {
                                    Some(TokenTree::Punct(ref punct)) if punct.as_char() == '!' => {
                                        self.advance();
                                        self.parse_state();
                                    }
                                    _ => {
                                        for token in collected {
                                            self.add_to_code(token);
                                        }
                                        self.add_to_code(next.unwrap_or_else(|| {
                                            abort_call_site!(
                                                "unexpected end of input after 'state'"
                                            )
                                        }));
                                    }
                                }
                            }
                            Some(_) => {
                                for token in collected {
                                    self.add_to_code(token);
                                }
                            }
                            None => break,
                        }
                    }
                    _ => self.add_to_code(next.unwrap()),
                },
                Some(_) => self.add_to_code(next.unwrap()),
                None => break,
            }
            self.advance();
        }

        self.parsed
    }

    fn parse_macro(&mut self, macro_type: MacroTypes) {
        let name_token = self
            .next()
            .unwrap_or_else(|| abort_call_site!("expected macro name"));

        match self.peek() {
            Some(TokenTree::Punct(ref punct)) if punct.as_char() == '!' => {
                self.advance();
            }
            Some(token) => {
                self.add_to_code(name_token);
                self.add_to_code(token);
                return;
            }
            _ => {}
        }

        match macro_type {
            MacroTypes::Style => self.parse_style(),
            MacroTypes::Script => self.parse_script(),
            MacroTypes::Render => self.parse_render(),
        }
    }

    fn parse_state(&mut self) {
        let state;

        match self.peek() {
            Some(TokenTree::Group(ref group)) => {
                self.advance();
                state = group.stream();
            }
            _ => {
                emit_error!(self.peek(), "expected state! macro to have a block");
                return;
            }
        }

        if self.parsed.state.is_some() {
            emit_error!(state, "state! macro already used!");
            return;
        }

        let ident = self
            .last_ident
            .clone()
            .unwrap_or_else(|| "state".to_string());
        self.parsed.state = Some(ComponentState {
            type_name: state,
            ident,
        });
    }

    fn parse_style(&mut self) {
        let style;

        match self.peek() {
            Some(TokenTree::Group(ref group)) => {
                self.advance();
                style = group.stream();
            }
            _ => {
                emit_error!(self.peek(), "expected style! macro to have a block");
                return;
            }
        }

        if self.parsed.style.is_some() {
            emit_error!(style, "style! macro already used!");
            return;
        }

        self.parsed.style = Some(ComponentStyle { style });
    }

    fn parse_script(&mut self) {
        let script;

        match self.peek() {
            Some(TokenTree::Group(ref group)) => {
                self.advance();
                match group.stream().into_iter().next() {
                    Some(TokenTree::Literal(ref lit)) => match StringLit::try_from(lit) {
                        Ok(lit) => script = lit.value().to_string(),
                        Err(_) => script = lit.to_string(),
                    },
                    _ => {
                        emit_error!(
                            group.span(),
                            "expected script! macro to contain a string literal"
                        );
                        return;
                    }
                }
            }
            _ => {
                emit_error!(self.peek(), "expected script! macro to have a block");
                return;
            }
        }

        self.parsed.script.script.push_str(&script);
    }

    fn parse_render(&mut self) {
        let render;

        match self.peek() {
            Some(TokenTree::Group(ref group)) => {
                self.advance();
                render = group.stream();
            }
            _ => {
                emit_error!(self.peek(), "expected html! macro to have a block");
                return;
            }
        }

        if self.parsed.render.markup.is_some() {
            emit_error!(render, "html! macro already used!");
            return;
        }

        self.parsed.render.markup = Some(render);
    }
}
