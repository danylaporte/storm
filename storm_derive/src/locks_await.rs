use crate::DeriveInputExt;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Type};

pub(crate) fn locks_await(input: &DeriveInput) -> TokenStream {
    let type_ident = &input.ident;
    let mut init_fields = Vec::new();
    let mut as_refs = Vec::new();
    let mut tags = Vec::new();

    for field in try_ts!(input.fields()) {
        let f_ident = &field.ident;
        let ty = unref(&field.ty);

        init_fields.push(quote!(#f_ident: ctx.ref_as::<#ty>().await?,));

        as_refs.push(quote! {
            impl<'a> AsRef<#ty> for #type_ident<'a> {
                #[inline]
                fn as_ref(&self) -> &#ty {
                    &self.#f_ident
                }
            }
        });

        tags.push(quote!(storm::Tag::tag(&self.#f_ident)));
    }

    let as_refs = quote!(#(#as_refs)*);
    let init_fields = quote!(#(#init_fields)*);
    let tags = quote!(storm::version_tag::combine(&[#(#tags,)*]));

    quote! {
        impl<'a> storm::AsyncTryFrom<'a, &'a storm::Ctx> for #type_ident<'a> {
            fn async_try_from(ctx: &'a storm::Ctx) -> storm::BoxFuture<'a, storm::Result<#type_ident<'a>>> {
                Box::pin(async move {
                    Ok(#type_ident {
                        #init_fields
                    })
                })
            }
        }

        impl<'a> #type_ident<'a> {
            pub fn from_ctx(ctx: &'a storm::Ctx) -> storm::BoxFuture<'a, storm::Result<storm::CtxLocks<'a, #type_ident<'a>>>> {
                Box::pin(async move {
                    Ok(storm::CtxLocks {
                        locks: storm::AsyncTryFrom::async_try_from(ctx).await?,
                        ctx,
                    })
                })
            }
        }

        impl<'a> storm::Tag for #type_ident<'a> {
            fn tag(&self) -> storm::VersionTag {
                #tags
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
