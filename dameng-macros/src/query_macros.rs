//! `query!`, `query_as!`, `query_scalar!` macros for tokio-dameng.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parser, Expr, LitStr};

pub fn dameng_query(input: TokenStream) -> TokenStream {
    let (sql, _params) = parse_query_input(input);
    let sql_lit = LitStr::new(&sql, proc_macro2::Span::call_site());

    let expanded = quote! {
        tokio_dameng::sqlx::Query::new(#sql_lit)
    };

    expanded.into()
}

pub fn dameng_query_as(input: TokenStream) -> TokenStream {
    let (sql, params) = parse_query_input(input);
    let sql_lit = LitStr::new(&sql, proc_macro2::Span::call_site());

    if params.is_empty() {
        let expanded = quote! {
            tokio_dameng::sqlx::QueryAs::new(#sql_lit)
        };
        expanded.into()
    } else {
        let param_tokens: Vec<_> = params.iter().map(|p| quote! { #p }).collect();
        let expanded = quote! {
            {
                let __q = tokio_dameng::sqlx::QueryAs::new(#sql_lit);
                #(__q.bind(#param_tokens)),*;
            }
        };
        expanded.into()
    }
}

pub fn dameng_query_scalar(input: TokenStream) -> TokenStream {
    let (sql, params) = parse_query_input(input);
    let sql_lit = LitStr::new(&sql, proc_macro2::Span::call_site());

    if params.is_empty() {
        let expanded = quote! {
            tokio_dameng::sqlx::QueryScalar::new(#sql_lit)
        };
        expanded.into()
    } else {
        let param_tokens: Vec<_> = params.iter().map(|p| quote! { #p }).collect();
        let expanded = quote! {
            {
                let __q = tokio_dameng::sqlx::QueryScalar::new(#sql_lit);
                #(__q.bind(#param_tokens)),*;
            }
        };
        expanded.into()
    }
}

/// Parse macro input: `query!("SELECT ...", param1, param2, ...)`
fn parse_query_input(input: TokenStream) -> (String, Vec<Expr>) {
    let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
    let expressions = match parser.parse(input) {
        Ok(e) => e,
        Err(_) => return (String::new(), vec![]),
    };

    let mut params = vec![];
    let sql = if let Some(Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(s),
        ..
    })) = expressions.first()
    {
        s.value()
    } else {
        String::new()
    };

    for expr in expressions.iter().skip(1) {
        params.push(expr.clone());
    }

    (sql, params)
}
