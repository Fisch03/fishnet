use litrs::StringLit;
use proc_macro2::{
    token_stream::IntoIter, Delimiter, Ident, Literal, Span, TokenStream, TokenTree,
};
use proc_macro_error::{abort, abort_call_site, emit_error};
use quote::{quote, ToTokens, TokenStreamExt};

#[derive(Debug)]
pub struct ParsedComponent {
    name: String,
    args: TokenStream,
    is_dyn: bool,
    state: Option<ComponentState>,
    style: Option<ComponentStyle>,
    script: ComponentScript,
    render: ComponentRender,
    routes: Vec<ComponentRoute>,
}

impl ParsedComponent {
    fn new(name: &str, args: TokenStream, is_dyn: bool) -> Self {
        Self {
            name: name.to_string(),
            args,

            is_dyn,
            state: None,
            style: None,
            script: ComponentScript {
                script: String::new(),
            },
            render: ComponentRender {
                code: TokenStream::new(),
                markup: None,
            },
            routes: Vec::new(),
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
                let ident = Ident::new(&state.ident, Span::call_site());
                let initializer = match &state.initializer {
                    ComponentStateType::DefaultState(type_name) => {
                        quote! {
                            let #ident = <#type_name  as Default>::default();
                        }
                    }
                    ComponentStateType::CustomState(initializer) => {
                        quote! {
                            let #ident = { #initializer };
                        }
                    }
                };
                (
                    initializer,
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

        let routes = self.routes.iter().map(|route| {
            let path = &route.path;
            let handler_name = &route.handler_name;
            let method = &route.axum_method;

            quote! {
                .route(#path, routing::#method(#handler_name))
            }
        });
        let routes = quote! {
            #(#routes)*
        };

        let route_handlers = self.routes.iter().map(|route| {
            let handler = &route.handler;

            quote! {
                #handler
            }
        });
        let route_handlers = quote! {
            #(#route_handlers)*
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
        let render = match self.is_dyn {
            false => quote! {
                .render(|#state_ident| async move {
                    #code

                    html! {
                        #markup
                    }
                }.boxed())
            },
            true => quote! {
                .render_dynamic(|#state_ident| async move {
                    #code

                    html! {
                        #markup
                    }
                }.boxed())
            },
        };

        tokens.extend(quote! {
            fn #name(#fn_args) -> impl BuildableComponent {
                #init_state

                #route_handlers

                fishnet::component::Component::new(#name_pascal, fishnet::const_nanoid!())
                    #state
                    #routes
                    #style
                    #script
                    #render
            }
        })
    }
}

#[derive(Debug)]
struct ComponentState {
    ident: String,
    initializer: ComponentStateType,
}
#[derive(Debug)]
enum ComponentStateType {
    DefaultState(TokenStream),
    CustomState(TokenStream),
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
struct ComponentRoute {
    path: String,
    handler_name: Ident,
    handler: TokenStream,
    axum_method: Ident,
}

#[derive(Debug)]
enum MacroTypes {
    Style,
    Script,
    Render,
}

pub(crate) fn parse(input: TokenStream) -> ParsedComponent {
    Parser::new(input, false).parse()
}

pub(crate) fn parse_dyn(input: TokenStream) -> ParsedComponent {
    Parser::new(input, true).parse()
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
    fn new(input: TokenStream, is_dyn: bool) -> Self {
        let mut input = input.into_iter();

        let next = input.next();
        match next {
            Some(TokenTree::Ident(ref ident)) if ident.to_string() == "fn" => {}
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
            parsed: ParsedComponent::new(&name, fn_args, is_dyn),
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

                        match self.peek() {
                            Some(TokenTree::Ident(ref ident)) if ident.to_string() == "mut" => {
                                collected.append(self.next().unwrap())
                            }
                            _ => {}
                        };

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
                                if ident.to_string().as_str() == "state"
                                    || ident.to_string() == "state_init" =>
                            {
                                collected.append(next_inner.clone().unwrap());
                                self.advance();

                                let next = self.peek();
                                match next {
                                    Some(TokenTree::Punct(ref punct)) if punct.as_char() == '!' => {
                                        self.advance();
                                        if ident.to_string() == "state" {
                                            self.parse_state();
                                        } else {
                                            self.parse_custom_state();
                                        }
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
                            Some(next_token) => {
                                for token in collected {
                                    self.add_to_code(token);
                                }
                                self.add_to_code(next_token);
                            }
                            None => break,
                        }
                    }
                    _ => self.add_to_code(next.unwrap()),
                },
                Some(TokenTree::Punct(ref punct)) if punct.as_char() == '#' => {
                    let mut collected = TokenStream::new();
                    collected.append(next.unwrap());
                    self.advance();

                    let next = self.next();
                    match next {
                        Some(TokenTree::Group(ref group))
                            if group.delimiter() == Delimiter::Bracket =>
                        {
                            collected.append(next.clone().unwrap());

                            let mut inner = group.stream().into_iter();
                            let next = inner.next();
                            match next {
                                Some(TokenTree::Ident(ref ident))
                                    if ident.to_string() == "route" =>
                                {
                                    self.parse_route(inner);
                                }
                                _ => {
                                    for token in collected {
                                        self.add_to_code(token);
                                    }
                                }
                            }
                        }
                        _ => {
                            for token in collected {
                                self.add_to_code(token);
                            }
                            break;
                        }
                    }
                }
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
            emit_error!(state, "state!/state_init! macro already used!");
            return;
        }

        let ident = self
            .last_ident
            .clone()
            .unwrap_or_else(|| "state".to_string());
        self.parsed.state = Some(ComponentState {
            ident,
            initializer: ComponentStateType::DefaultState(state),
        });
    }

    fn parse_custom_state(&mut self) {
        let state;

        match self.peek() {
            Some(TokenTree::Group(ref group)) => {
                self.advance();
                state = group.stream();
            }
            _ => {
                emit_error!(self.peek(), "expected state_init! macro to have a block");
                return;
            }
        }

        if self.parsed.state.is_some() {
            emit_error!(state, "state!/state_init! macro already used!");
            return;
        }

        let ident = self
            .last_ident
            .clone()
            .unwrap_or_else(|| "state".to_string());
        self.parsed.state = Some(ComponentState {
            ident,
            initializer: ComponentStateType::CustomState(state),
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

    fn parse_route(&mut self, mut inner: IntoIter) {
        let args = match inner.next() {
            Some(TokenTree::Group(ref group)) => group.stream().into_iter().collect::<Vec<_>>(),
            _ => {
                emit_error!(self.peek(), "missing route path");
                return;
            }
        };

        let path = match args.get(0) {
            Some(TokenTree::Literal(ref lit)) => match StringLit::try_from(lit) {
                Ok(lit) => lit.value().to_string(),
                Err(_) => lit.to_string(),
            },
            _ => {
                emit_error!(args.get(0), "expected string literal for route path");
                return;
            }
        };

        let method = match args.get(2) {
            Some(TokenTree::Ident(ref ident)) => {
                Ident::new(&ident.to_string().to_lowercase(), Span::call_site())
            }
            None => Ident::new("get", Span::call_site()),
            _ => {
                emit_error!(args.get(2), "expected method identifier");
                return;
            }
        };

        let handler = self.parse_async_fn();

        self.parsed.routes.push(ComponentRoute {
            path,
            handler_name: handler.0,
            handler: handler.1,
            axum_method: method,
        });
    }

    fn parse_async_fn(&mut self) -> (Ident, TokenStream) {
        let mut body = TokenStream::new();
        body.append(self.expect_ident("async"));
        body.append(self.expect_ident("fn"));
        let name = self.expect_get_ident();
        body.append(name.clone());
        loop {
            let next = self.peek();
            match next {
                Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
                    body.append(group);
                    break;
                }
                Some(ref token) => {
                    body.append(token.clone());
                    self.advance();
                }
                None => abort!(next, "unexpected end of input"),
            }
        }

        (name, body)
    }

    fn expect_ident(&mut self, name: &str) -> Ident {
        let ident = self.expect_get_ident();
        if ident.to_string() != name {
            abort!(ident, format!("expected '{}'", name));
        }
        ident
    }

    fn expect_get_ident(&mut self) -> Ident {
        let next = self
            .next()
            .unwrap_or_else(|| abort_call_site!("unexpected end of input"));
        match next {
            TokenTree::Ident(ident) => ident,
            _ => abort!(next, "expected identifier"),
        }
    }
}
