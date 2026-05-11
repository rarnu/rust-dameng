//! Proc macros for tokio-dameng sqlx-compatible layer.

use proc_macro::TokenStream;

mod from_row;
mod query_macros;

#[proc_macro_derive(FromRow)]
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    from_row::derive_from_row(input)
}

#[proc_macro]
pub fn dameng_query(input: TokenStream) -> TokenStream {
    query_macros::dameng_query(input)
}

#[proc_macro]
pub fn dameng_query_as(input: TokenStream) -> TokenStream {
    query_macros::dameng_query_as(input)
}

#[proc_macro]
pub fn dameng_query_scalar(input: TokenStream) -> TokenStream {
    query_macros::dameng_query_scalar(input)
}
