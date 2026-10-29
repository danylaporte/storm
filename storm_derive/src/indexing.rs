use crate::{
    TypeExt,
    register::{ctx_var, registration},
};
use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Error, FnArg, Ident, Item, ItemFn, LitStr, ReturnType, Type, spanned::Spanned};

pub(crate) fn indexing(item: Item) -> TokenStream {
    match &item {
        Item::Fn(f) => indexing_fn(f),
        _ => Error::new(item.span(), "Only function is supported.").to_compile_error(),
    }
}

/// The lazy initialization, tracing and telemetry are generic functions of the `storm` crate, so
/// only the glue specific to the index is generated.
fn indexing_fn(f: &ItemFn) -> TokenStream {
    let vis = &f.vis;
    let name = &f.sig.ident;
    let name_str = &name.to_string().to_pascal_case();
    let index_name = Ident::new(name_str, name.span());
    let index_name_lit = LitStr::new(name_str, name.span());
    let index_init_ident = Ident::new(
        &format!("__idx_init_{}", name_str.to_snake_case()),
        name.span(),
    );

    let ty = match &f.sig.output {
        ReturnType::Type(_, t) => t,
        ReturnType::Default => {
            return Error::new(f.sig.output.span(), "Index must have a return value.")
                .to_compile_error();
        }
    };

    let mut call_args = Vec::new();
    let mut dep_idents = Vec::new();
    let mut dep_tys = Vec::new();

    for (index, arg) in f.sig.inputs.iter().enumerate() {
        let arg = match arg {
            FnArg::Typed(t) => t,
            FnArg::Receiver(r) => {
                return Error::new(r.span(), "Self is not expected here.").to_compile_error();
            }
        };

        let ty = unref(&arg.ty);

        if ty.is_storm_ctx() {
            call_args.push(quote!(ctx));
        } else {
            let ident = Ident::new(&format!("var{index}"), arg.span());
            call_args.push(quote!(#ident));
            dep_idents.push(ident);
            dep_tys.push(ty);
        }
    }

    let (deps_pat, sync_deps, async_deps) = match (&dep_idents[..], &dep_tys[..]) {
        ([], _) => (quote!(()), quote!(()), quote!(std::future::ready(Ok(())))),
        ([ident], [ty]) => (
            quote!(#ident),
            quote!(<Self as AsRef<#ty>>::as_ref(self)),
            quote!(storm::AsRefAsync::<#ty>::as_ref_async(self)),
        ),
        _ => (
            quote!((#(#dep_idents,)*)),
            quote!((#(<Self as AsRef<#dep_tys>>::as_ref(self),)*)),
            quote!(async move {
                Ok((#(storm::tri!(storm::AsRefAsync::<#dep_tys>::as_ref_async(self).await),)*))
            }),
        ),
    };

    let (as_ref_wheres, as_ref_async_wheres) = if dep_tys.is_empty() {
        (quote!(), quote!())
    } else {
        (
            quote!(where Self: #(AsRef<#dep_tys>)+*),
            quote!(where Self: #(storm::AsRefAsync<#dep_tys>)+*),
        )
    };

    let build = quote! {
        move |#deps_pat| #index_name(
            #name(#(#call_args),*),
            storm::version_tag::combine(&[#(storm::Tag::tag(#dep_idents)),*]),
        )
    };

    let var = ctx_var(quote!(#index_name));

    let register = registration(
        &index_init_ident,
        quote! {
            <#index_name as storm::indexing::RebuildIndex>::register();
            #(<#index_name as storm::indexing::RebuildIndex>::register_clear_or_touchable::<#dep_tys>();)*
        },
    );

    quote! {
        #vis struct #index_name(#ty, storm::VersionTag);

        impl storm::indexing::RebuildIndex for #index_name {
            #[inline]
            fn var() -> storm::CtxVar<Self> {
                #var
            }
        }

        #register

        impl storm::Clearable for #index_name {
            #[inline]
            fn cleared() -> &'static storm::ClearEvent {
                static E: storm::ClearEvent = storm::ClearEvent::new();
                &E
            }
        }

        impl std::ops::Deref for #index_name {
            type Target = #ty;

            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<L> AsRef<#index_name> for storm::CtxLocks<'_, L> #as_ref_wheres {
            #[inline]
            fn as_ref(&self) -> &#index_name {
                let ctx = self.ctx;
                storm::indexing::__get_or_init_sync(ctx, || #sync_deps, #build)
            }
        }

        impl storm::AsRefAsync<#index_name> for storm::Ctx #as_ref_async_wheres {
            #[inline]
            fn as_ref_async(&self) -> storm::BoxFuture<'_, storm::Result<&'_ #index_name>> {
                let ctx = self;
                storm::indexing::__get_or_init_async(ctx, #async_deps, #build)
            }
        }

        impl storm::Tag for #index_name {
            #[inline]
            fn tag(&self) -> storm::VersionTag {
                self.1
            }
        }

        impl storm::Touchable for #index_name {
            #[inline]
            fn touched() -> &'static storm::TouchedEvent {
                static E: storm::TouchedEvent = storm::TouchedEvent::new();
                &E
            }
        }

        impl storm::CtxTypeInfo for #index_name {
            const NAME: &'static str = #index_name_lit;
        }

        impl storm::Gc for #index_name {
            const SUPPORT_GC: bool = <#ty as storm::Gc>::SUPPORT_GC;

            #[inline]
            fn gc(&mut self) {
                storm::Gc::gc(&mut self.0);
            }
        }

        #f
    }
}

fn unref(t: &Type) -> &Type {
    match t {
        Type::Reference(r) => unref(&r.elem),
        _ => t,
    }
}
