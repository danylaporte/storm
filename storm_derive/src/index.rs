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
    let obj = resolve_result(&f.sig.output).clone();
    let args = &f.sig.inputs;
    let expr = &f.block;

    quote! {
        #vis struct #index_name(#obj);

        impl #index_name {
            async fn init_imp(#args) -> storm::Result<#obj> {
                #expr
            }
        }

        impl storm::Gc for #index_name {
            const SUPPORT_GC: bool = <#obj as storm::Gc>::SUPPORT_GC;

            #[inline]
            fn gc(&mut self) {
                storm::Gc::gc(&mut self.0)
            }
        }

        impl storm::Obj for #index_name {
            #[inline]
            fn ctx_var() -> storm::attached::Var<Self, storm::CtxVars> {
                storm::attached::var!(V: #index_name, storm::CtxVars);
                *V
            }

            async fn init(ctx: &storm::Ctx) -> storm::Result<Self> {
                Self::init_imp(ctx).await.map(Self)
            }

            #[allow(non_camel_case_types)]
            #[inline]
            fn loaded() -> &'static storm::LoadedEvent {
                #[static_init::dynamic]
                static EVENT: storm::LoadedEvent = Default::default();

                &EVENT
            }

            #[inline]
            fn log_var() -> storm::attached::Var<<Self as storm::ObjBase>::Log, storm::LogVars> {
                storm::attached::var!(V: <#index_name as storm::ObjBase>::Log, storm::LogVars);
                *V
            }
        }

        impl storm::ObjBase for #index_name {
            type Log = <#obj as storm::ObjBase>::Log;
            type Trx<'a> = <#obj as storm::ObjBase>::Trx::<'a>;

            #[inline]
            fn apply_log(&mut self, log: Self::Log) -> bool {
                storm::ObjBase::apply_log(&mut self.0, log)
            }

            #[inline]
            fn trx<'a>(
                &'a self,
                trx: &'a mut storm::Trx<'a>,
                log: storm::LogToken<Self::Log>,
            ) -> Self::Trx<'a> {
                storm::ObjBase::trx(&self.0, trx, log)
            }
        }

        impl std::ops::Deref for #index_name {
            type Target = #obj;

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
