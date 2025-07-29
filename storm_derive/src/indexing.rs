use crate::TypeExt;
use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Error, FnArg, Ident, Item, ItemFn, LitStr, ReturnType, Type};

pub(crate) fn indexing(item: Item) -> TokenStream {
    match &item {
        Item::Fn(f) => indexing_fn(f),
        _ => Error::new(item.span(), "Only function is supported.").to_compile_error(),
    }
}

fn gc(index_name: &Ident, index_ty: &Type) -> (TokenStream, TokenStream) {
    (
        quote! {
            impl storm::Gc for #index_name {
                const SUPPORT_GC: bool = <#index_ty as storm::Gc>::SUPPORT_GC;

                #[inline]
                fn gc(&mut self, ctx: &storm::GcCtx) {
                    self.0.gc(ctx);
                }
            }
        },
        quote! {
            if <#index_ty as storm::Gc>::SUPPORT_GC {
                storm::gc::collectables::register(|ctx| ctx.index_gc::<#index_name>());
            }
        },
    )
}

fn indexing_fn(f: &ItemFn) -> TokenStream {
    let vis = &f.vis;
    let name = &f.sig.ident;
    let name_str = &name.to_string().to_pascal_case();
    let index_name = Ident::new(name_str, name.span());
    let index_name_lit = LitStr::new(name_str, name.span());
    let const_name = name_str.to_screaming_snake_case();
    let index_init_ident = Ident::new(
        &format!("__idx_init_{}", name_str.to_snake_case()),
        name.span(),
    );

    let ctx_var = Ident::new(&format!("__CTX_VAR{const_name}"), name.span());

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

    let mut as_ref_decl = Vec::new();
    let mut as_ref_decl_async = Vec::new();
    let mut as_ref_args = Vec::new();
    let mut as_ref_tag = Vec::new();
    let mut as_ref_async_wheres = quote!();
    let mut as_ref_wheres = quote!();
    let mut deps = Vec::new();

    for (index, arg) in args.iter().enumerate() {
        let ty = unref(&arg.ty);
        let ident = Ident::new(&format!("var{index}"), arg.span());

        if ty.is_storm_ctx() {
            as_ref_decl.push(quote!(let #ident = self.ctx;));
            as_ref_decl_async.push(quote!(let #ident = self;));
            as_ref_args.push(ident);
        } else {
            as_ref_decl.push(quote!(let #ident = self.as_ref();));
            as_ref_decl_async.push(
                quote!(let #ident = storm::tri!(storm::AsRefAsync::as_ref_async(self).await);),
            );
            as_ref_tag.push(quote!(storm::Tag::tag(#ident)));
            as_ref_args.push(ident);
            deps.push(quote!(<#ty as storm::Accessor>::register_deps(<#index_name as storm::Accessor>::clear);));

            let ref_async = quote!(storm::AsRefAsync<#ty>);
            let ref_sync = quote!(AsRef<#ty>);

            if as_ref_wheres.is_empty() {
                as_ref_async_wheres = quote!(where Self: #ref_async);
                as_ref_wheres = quote!(where Self: #ref_sync);
            } else {
                as_ref_async_wheres = quote!(#as_ref_async_wheres + #ref_async);
                as_ref_wheres = quote!(#as_ref_wheres + #ref_sync);
            };
        }
    }

    let get_or_init = quote!({
        let _ = tracing::span!(tracing::Level::DEBUG, <#index_name as storm::CtxTypeInfo>::NAME, obj = storm::OBJ_INDEX, ev = storm::EV_CREATED).entered();
        #index_name(#name(#(#as_ref_args,)*), storm::version_tag::combine(&[#(#as_ref_tag,)*]))
    });

    let (gc, gc_collect) = gc(&index_name, ty);

    quote! {
        #vis struct #index_name(#ty, storm::VersionTag);

        storm::extobj::extobj!{
            impl storm::CtxExt { #ctx_var: std::sync::OnceLock<#index_name> },
            init = #index_init_ident(),
            crate_path = storm::extobj
        }

        impl storm::Accessor for #index_name {
            #[inline]
            fn var() -> storm::CtxVar<Self> {
                *#ctx_var
            }

            #[inline]
            fn deps() -> &'static storm::Deps {
                static DEPS: storm::Deps = storm::parking_lot::RwLock::new(Vec::new());
                &DEPS
            }
        }

        #[doc(hidden)]
        fn #index_init_ident() {
            #(#deps)*
            #gc_collect
        }

        impl std::ops::Deref for #index_name {
            type Target = #ty;

            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<'a, L> AsRef<#index_name> for storm::CtxLocks<'a, L> #as_ref_wheres {
            fn as_ref(&self) -> &#index_name {
                #(#as_ref_decl)*
                self.ctx.ctx_ext_obj().get(<#index_name as storm::Accessor>::var()).get_or_init(move || #get_or_init)
            }
        }

        impl storm::AsRefAsync<#index_name> for storm::Ctx #as_ref_async_wheres {
            fn as_ref_async(&self) -> storm::BoxFuture<'_, storm::Result<&'_ #index_name>> {
                let var = <#index_name as storm::Accessor>::var();

                Box::pin(async move {
                    let ctx = self.ctx_ext_obj();

                    if let Some(v) = ctx.get(var).get() {
                        return Ok(v);
                    }

                    #(#as_ref_decl_async)*
                    Ok(ctx.get(var).get_or_init(|| #get_or_init))
                })
            }
        }

        impl storm::Tag for #index_name {
            #[inline]
            fn tag(&self) -> storm::VersionTag {
                self.1
            }
        }

        impl storm::CtxTypeInfo for #index_name {
            const NAME: &'static str = #index_name_lit;
        }

        #gc

        #f
    }
}

fn unref(t: &Type) -> &Type {
    match t {
        Type::Reference(r) => unref(&r.elem),
        _ => t,
    }
}
