use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    spanned::Spanned, Error, GenericArgument, Ident, Item, ItemFn, PathArguments, ReturnType, Type,
};

pub(crate) fn index(item: Item) -> TokenStream {
    match &item {
        Item::Fn(f) => index_fn(f),
        _ => Error::new(item.span(), "Only function is supported.").to_compile_error(),
    }
}

fn index_fn(f: &ItemFn) -> TokenStream {
    let vis = &f.vis;
    let name = &f.sig.ident;
    let name_str = &name.to_string().to_pascal_case();
    let index_name = Ident::new(name_str, name.span());
    let mut asset = resolve_result(&f.sig.output).clone();
    let args = &f.sig.inputs;
    let expr = &f.block;

    replace_self(&mut asset, &index_name);

    quote! {
        #vis struct #index_name;

        impl storm::AssetProxy for #index_name {
            type Asset = #asset;

            #[inline]
            fn ctx_var() -> storm::attached::Var<Self::Asset, storm::CtxVars> {
                storm::attached::var!(V: #asset, storm::CtxVars);
                &V
            }

            #[inline]
            fn log_var() -> storm::attached::Var<<Self::Asset as storm::Asset>::Log, storm::LogVars> {
                storm::attached::var!(V: <#asset as storm::Asset>::Log, storm::LogVars);
                &V
            }

            async fn init(#args) -> storm::Result<#asset> {
                #expr
            }
        }
    }
}

fn resolve_result(t: &ReturnType) -> &Type {
    if let ReturnType::Type(_, t) = t {
        if let Type::Path(p) = &**t {
            if let Some(s) = p.path.segments.last() {
                if let PathArguments::AngleBracketed(b) = &s.arguments {
                    if let Some(GenericArgument::Type(t)) = b.args.first() {
                        return t;
                    }
                }
            }
        }
    }

    panic!("invalid result, expected a Result<..>")
}

fn replace_self(t: &mut Type, new: &Ident) {
    match t {
        Type::Path(p) => {
            if let Some(t) = &mut p.qself {
                replace_self(&mut t.ty, new);
            }

            p.path.segments.iter_mut().for_each(|a| {
                replace_self_ident(&mut a.ident, new);

                match &mut a.arguments {
                    PathArguments::AngleBracketed(a) => a.args.iter_mut().for_each(|a| match a {
                        GenericArgument::AssocConst(c) => {
                            replace_self_ident(&mut c.ident, new);
                        }
                        GenericArgument::AssocType(a) => replace_self_ident(&mut a.ident, new),
                        GenericArgument::Type(t) => replace_self(t, new),
                        _ => {}
                    }),
                    _ => {}
                }
            });
        }
        _ => {}
    }
}

fn replace_self_ident(ident: &mut Ident, new: &Ident) {
    if *ident == "Self" {
        *ident = new.clone();
    }
}
