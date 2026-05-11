//! Derive macro for `FromRow` — maps a database row to a Rust struct.
//!
//! # Example
//!
//! ```ignore
//! #[derive(FromRow)]
//! struct User {
//!     id: i32,
//!     name: String,
//! }
//! ```

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn derive_from_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let data = input.data;

    let fields = match data {
        syn::Data::Struct(s) => s.fields,
        _ => panic!("FromRow can only be derived for structs"),
    };

    let mut field_init = vec![];
    let mut col_idx = 0usize;

    for field in fields.iter() {
        let ident = field.ident.as_ref().expect("FromRow requires named fields");
        let ty = &field.ty;
        let ty_str = quote!(#ty).to_string();

        let extract = typed_extract_expr(ty, &ty_str, col_idx);
        field_init.push(quote! {
            #ident: #extract
        });
        col_idx += 1;
    }

    let expanded = quote! {
        impl FromRow for #name {
            fn from_row(row: &dameng_protocol::Row, columns: &[dameng_protocol::Column]) -> std::result::Result<Self, tokio_dameng::error::Error> {
                use tokio_dameng::sqlx::row_ext::RowExt;
                Ok(Self {
                    #(#field_init),*
                })
            }
        }
    };

    expanded.into()
}

fn typed_extract_expr(ty: &syn::Type, ty_str: &str, col_idx: usize) -> proc_macro2::TokenStream {
    let idx = col_idx;
    match ty_str {
        "i32" => {
            quote! { row.get_i32(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "i64" => {
            quote! { row.get_i64(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "i16" => {
            quote! { row.get_i16(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "i8" => {
            quote! { row.get_i8(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "u32" => {
            quote! { row.get_i32(#idx).map(|v| v as u32).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "u64" => {
            quote! { row.get_i64(#idx).map(|v| v as u64).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "u16" => {
            quote! { row.get_i16(#idx).map(|v| v as u16).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "u8" => {
            quote! { row.get_i8(#idx).map(|v| v as u8).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "f32" => {
            quote! { row.get_f32(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "f64" => {
            quote! { row.get_f64(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "String" => {
            quote! { row.get_str(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "bool" => {
            quote! { row.get_i32(#idx).map(|v| v != 0).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "Vec<u8>" => {
            quote! { row.get_bytes(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "Option<i32>" => {
            quote! { row.get_opt_i32(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "Option<i64>" => {
            quote! { row.get_opt_i64(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "Option<String>" => {
            quote! { row.get_opt_str(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "Option<f64>" => {
            quote! { row.get_opt_f64(#idx).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        "Option<bool>" => {
            quote! { row.get_opt_i32(#idx).map(|v| v.map(|x| x != 0)).map_err(tokio_dameng::error::Error::DecodeError)? }
        }
        _ => {
            quote! {
                String::from(row.get_str(#idx).map_err(tokio_dameng::error::Error::DecodeError)?)
                    .parse::<#ty>().map_err(tokio_dameng::error::Error::DecodeError)?
            }
        }
    }
}
