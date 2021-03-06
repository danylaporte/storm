use crate::TokenStreamExt;
use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Error, FnArg, Ident, Item, ItemFn, ReturnType, Type};

pub(crate) fn indexing(item: Item) -> TokenStream {
    match &item {
        Item::Fn(f) => indexing_fn(f),
        _ => return Error::new(item.span(), "Only function is supported.").to_compile_error(),
    }
}

fn indexing_fn(f: &ItemFn) -> TokenStream {
    let vis = &f.vis;
    let name = &f.sig.ident;
    let index_name = Ident::new(&name.to_string().to_pascal_case(), name.span());

    let ty = match &f.sig.output {
        ReturnType::Type(_, t) => t,
        ReturnType::Default => {
            return Error::new(f.sig.output.span(), "Index must have a return value.")
                .to_compile_error()
        }
    };

    let mut args = Vec::new();

    for arg in &f.sig.inputs {
        match arg {
            FnArg::Typed(t) => args.push(t),
            FnArg::Receiver(r) => {
                return Error::new(r.span(), "Self is not expected here.").to_compile_error()
            }
        };
    }

    let as_ref_wheres = args
        .iter()
        .map(|a| unref(&a.ty))
        .map(|t| quote!(AsRef<#t>))
        .fold(None, |acc, v| {
            Some(match acc {
                Some(acc) => quote!(#acc + #v),
                None => quote!(where C: #v),
            })
        });

    let as_ref_opt_wheres_for_ctx = args
        .iter()
        .map(|a| unref(&a.ty))
        .map(|t| quote!(storm::AsRefOpt<#t>))
        .fold(None, |acc, v| {
            Some(match acc {
                Some(acc) => quote!(#acc + #v),
                None => quote!(where C: #v),
            })
        });

    let as_ref_async_wheres = args
        .iter()
        .map(|a| unref(&a.ty))
        .map(|t| quote!(+ for<'v> storm::AsRefAsync<'v, #t>))
        .ts();

    let as_refs = args
        .iter()
        .map(|_| quote!(storm::GetVersion::max(ctx.as_ref(), &mut version),))
        .ts();

    let as_ref_asyncs = args
        .iter()
        .map(|_| {
            quote!(storm::GetVersion::max(
                storm::AsRefAsync::as_ref_async(ctx).await?,
                &mut version
            ),)
        })
        .ts();

    let is_version_obsolete = args
        .iter()
        .map(|t| unref(&t.ty))
        .map(|t| quote!(storm::GetVersionOpt::get_version_opt(&storm::AsRefOpt::<#t>::as_ref_opt(ctx)).unwrap_or(0)))
        .fold(None, |acc, v| {
            Some(match acc {
                Some(acc) => quote!(std::cmp::max(#acc, #v)),
                None => v,
            })
        });

    quote! {
        #vis struct #index_name(#ty, u64);

        impl #index_name {
            pub fn is_version_obsolete<C>(&self, ctx: &C) -> bool
            #as_ref_opt_wheres_for_ctx
            {
                self.1 != #is_version_obsolete
            }
        }

        impl std::ops::Deref for #index_name {
            type Target = #ty;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<C> storm::GetOrLoad<#index_name, C> for storm::AsyncOnceCell<#index_name> #as_ref_wheres
        {
            fn get_or_load(&self, ctx: &C) -> &#index_name {
                let mut version = 0;
                self.get_or_init_sync(|| #index_name(#name(#as_refs), version))
            }
        }

        #[storm::async_trait::async_trait]
        impl<C> storm::Init<C> for #index_name
        where
            C: Send + Sync #as_ref_async_wheres
        {
            async fn init(ctx: &C) -> storm::Result<#index_name> {
                let mut version = 0;
                Ok(#index_name(#name(#as_ref_asyncs), version))
            }
        }

        impl storm::GetVersion for #index_name {
            fn get_version(&self) -> u64 {
                self.1
            }
        }

        impl storm::GetVersionOpt for #index_name {
            fn get_version_opt(&self) -> Option<u64> {
                Some(self.1)
            }
        }

        #f
    }
}

fn unref(t: &Type) -> &Type {
    match t {
        Type::Reference(r) => unref(&*r.elem),
        _ => t,
    }
}
