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

    let screaming_snake = name.to_string().to_screaming_snake_case();
    let static_var = Ident::new(&format!("{}_VAR", screaming_snake), name.span());

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
                None => quote!(where Self: #v),
            })
        })
        .ts();

    let as_ref_async_wheres = args
        .iter()
        .map(|a| unref(&a.ty))
        .map(|t| quote!(storm::AsRefAsync<#t>))
        .fold(None, |acc, v| {
            Some(match acc {
                Some(acc) => quote!(#acc + #v),
                None => quote!(where Ctx: #v),
            })
        })
        .ts();

    let as_refs = args.iter().map(|_| quote!(self.as_ref(),)).ts();

    let mut as_ref_decl = Vec::new();
    let mut as_ref_args = Vec::new();

    for (index, arg) in args.iter().enumerate() {
        let ident = Ident::new(&format!("var{}", index), arg.span());
        as_ref_decl.push(quote!(let #ident = self.as_ref_async().await?;));
        as_ref_args.push(ident);
    }

    let as_ref_decl = quote!(#(#as_ref_decl)*);
    let as_ref_args = quote!(#(#as_ref_args,)*);

    let deps = args
        .iter()
        .map(|t| unref(&t.ty))
        .map(|t| quote!(<#t as storm::Accessor>::register_deps(<#index_name as storm::Accessor>::clear);));

    let deps = quote!(#(#deps)*);

    quote! {
        #[static_init::dynamic]
        static #static_var: (storm::TblVar<#index_name>, storm::Deps) = {
            #deps
            Default::default()
        };

        #vis struct #index_name(#ty);

        impl storm::Accessor for #index_name {
            #[inline]
            fn var() -> &'static storm::TblVar<Self> {
                &#static_var.0
            }

            #[inline]
            fn deps() -> &'static storm::Deps {
                &#static_var.1
            }
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
                let ctx = &self.ctx.var_ctx();
                <#index_name as storm::Accessor>::var().get_or_init(ctx, move || #index_name(#name(#as_refs)))
            }
        }

        impl storm::AsRefAsync<#index_name> for Ctx #as_ref_async_wheres {
            fn as_ref_async(&self) -> storm::BoxFuture<'_, Result<&'_ #index_name>> {
                let var = <#index_name as storm::Accessor>::var();

                Box::pin(async move {
                    let ctx = self.var_ctx();

                    if let Some(v) = var.get(ctx) {
                        return Ok(v);
                    }

                    #as_ref_decl
                    Ok(var.get_or_init(ctx, || #index_name(#name(#as_ref_args))))
                })
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
