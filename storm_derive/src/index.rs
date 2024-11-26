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
    let asset = resolve_result(&f.sig.output).clone();
    let args = &f.sig.inputs;
    let expr = &f.block;

    quote! {
        #vis struct #index_name(#asset);

        impl #index_name {
            async fn init_imp(#args) -> storm::Result<#asset> {
                #expr
            }
        }

        impl storm::Asset for #index_name {
            #[inline]
            fn ctx_var() -> storm::attached::Var<Self, storm::CtxVars> {
                storm::attached::var!(V: #index_name, storm::CtxVars);
                *V
            }

            #[inline]
            fn log_var() -> storm::attached::Var<<Self as storm::AssetBase>::Log, storm::LogVars> {
                storm::attached::var!(V: <#index_name as storm::AssetBase>::Log, storm::LogVars);
                *V
            }

            async fn init(ctx: &storm::Ctx) -> storm::Result<Self> {
                Self::init_imp(ctx).await.map(Self)
            }
        }

        impl storm::AssetBase for #index_name {
            const SUPPORT_GC: bool = <#asset as storm::AssetBase>::SUPPORT_GC;

            type Log = <#asset as storm::AssetBase>::Log;
            type Trx<'a> = <#asset as storm::AssetBase>::Trx::<'a>;

            #[inline]
            fn apply_log(&mut self, log: Self::Log) -> bool {
                storm::AssetBase::apply_log(&mut self.0, log)
            }

            #[inline]
            fn gc(&mut self) {
                storm::AssetBase::gc(&mut self.0)
            }

            #[inline]
            fn trx<'a>(
                &'a self,
                trx: &'a mut Trx<'a>,
                log: storm::LogToken<Self::Log>,
            ) -> Self::Trx<'a> {
                storm::AssetBase::trx(&self.0, trx, log)
            }
        }

        impl std::ops::Deref for #index_name {
            type Target = #asset;

            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    }
}

#[allow(clippy::panic)]
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
