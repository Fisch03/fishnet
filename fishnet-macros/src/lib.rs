mod css;
use crate::css::ToFmt;

use nanoid::nanoid;
use proc_macro2::{TokenStream, TokenTree};
use proc_macro_error::{abort, proc_macro_error};
use quote::quote;

const ID_ALPHABET: [char; 52] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

#[proc_macro]
#[proc_macro_error]
pub fn css(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: TokenStream = input.into();

    let parsed = css::parse(input);
    let mut fmt = css::StyleFmt::new();
    parsed.to_fmt(&mut fmt);

    let out = quote!({
        extern crate fishnet;

        fishnet::css::StyleFragment::new(#fmt)
    });

    out.into()
}

#[proc_macro]
#[proc_macro_error]
pub fn const_nanoid(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: TokenStream = input.into();
    let input = input.into_iter().collect::<Vec<_>>();

    let id = match input.len() {
        0 => nanoid!(5, &ID_ALPHABET),
        1 => {
            let len = parse_token_usize(&input[0]);
            nanoid!(len, &ID_ALPHABET)
        }
        _ => abort!(input[1], "expected at most one argument"),
    };

    quote!(#id).into()
}

fn parse_token_usize(token: &TokenTree) -> usize {
    match token {
        TokenTree::Literal(lit) => lit
            .to_string()
            .parse()
            .unwrap_or_else(|_| abort!(lit, "expected integer literal")),
        _ => abort!(token, "expected literal"),
    }
}
