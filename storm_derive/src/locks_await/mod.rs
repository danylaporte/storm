mod attrs;

use crate::DeriveInputExt;
use attrs::TypeAttrs;
use darling::FromDeriveInput;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Type};

pub(crate) fn locks_await(input: &DeriveInput) -> TokenStream {
    let type_ident = &input.ident;
    let attrs = try_ts!(TypeAttrs::from_derive_input(input).map_err(|e| e.write_errors()));
    let mut init_fields = Vec::new();
    let mut as_refs = Vec::new();

    for field in try_ts!(input.fields()) {
        let f_ident = &field.ident;
        let ty = unref(&field.ty);

        init_fields.push(quote!(#f_ident: ctx.#f_ident().await?,));

        as_refs.push(quote! {
            impl<'a> AsRef<#ty> for #type_ident<'a> {
                fn as_ref(&self) -> &#ty {
                    &self.#f_ident
                }
            }
        });
    }

    let ctx = &attrs.ctx;
    let as_refs = quote!(#(#as_refs)*);
    let init_fields = quote!(#(#init_fields)*);

    quote! {
        impl<'a> #type_ident<'a> {
            pub async fn from_ctx(ctx: &'a dyn #ctx) -> storm::Result<#type_ident<'a>> {
                Ok(Self {
                    #init_fields
                })
            }
        }

        #as_refs
    }
}

fn unref(t: &Type) -> &Type {
    match t {
        Type::Reference(r) => unref(&r.elem),
        _ => t,
    }
}
