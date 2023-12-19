use crate::DeriveInputExt;
use itertools::Itertools;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Ident, LitInt, Type};

#[allow(clippy::expect_used)]
pub(crate) fn locks_await(input: &DeriveInput) -> TokenStream {
    let type_ident = &input.ident;
    let mut init_fields = Vec::new();
    let mut as_refs = Vec::new();
    let mut tags = Vec::new();
    let mut declare = Vec::new();
    let mut part = Vec::new();

    for (idx, chunks) in input
        .fields()
        .expect("fields")
        .iter()
        .chunks(5)
        .into_iter()
        .enumerate()
    {
        part.clear();
        part.extend(chunks);

        if part.len() == 5 {
            let block = Ident::new(&format!("block{idx}"), Span::call_site());
            declare.push(quote! {let #block = storm::tri!(storm::async_ref_block5(ctx).await); });

            for (idx, field) in part.iter().enumerate() {
                let f_ident = &field.ident;
                let idx = LitInt::new(&idx.to_string(), Span::call_site());
                let ty = unref(&field.ty);

                init_fields.push(quote! { #f_ident: #block.#idx, });

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
        } else {
            for field in &part {
                let f_ident = &field.ident;
                let ty = unref(&field.ty);

                init_fields.push(
                    quote!(#f_ident: storm::tri!(storm::AsRefAsync::as_ref_async(ctx).await),),
                );

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
        }
    }

    let declare = quote!(#(#declare)*);
    let as_refs = quote!(#(#as_refs)*);
    let init_fields = quote!(#(#init_fields)*);
    let tags = quote!(storm::version_tag::combine(&[#(#tags,)*]));

    quote! {
        impl<'a> storm::AsyncTryFrom<'a, &'a storm::Ctx> for #type_ident<'a> {
            #[allow(clippy::eval_order_dependence)]
            fn async_try_from(ctx: &'a storm::Ctx) -> storm::BoxFuture<'a, storm::Result<#type_ident<'a>>> {
                Box::pin(async move {
                    #declare

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
                        locks: storm::tri!(storm::AsyncTryFrom::async_try_from(ctx).await),
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
