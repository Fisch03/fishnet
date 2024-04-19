mod css;
use crate::css::ToFmt;

use proc_macro2::TokenStream;
use proc_macro_error::proc_macro_error;
use quote::quote;

#[proc_macro]
#[proc_macro_error]
pub fn css(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: TokenStream = input.into();

    let parsed = css::parse(input);
    let parsed = parsed.to_fmt();

    quote!({
        extern crate fishnet;

        fishnet::css::StyleFragment::new(#parsed)
    })
    .into()
}
