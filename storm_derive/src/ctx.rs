use crate::derive_input_ext::DeriveInputExt;
use darling::{FromDeriveInput, FromMeta};
use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Ident, LitStr};

pub fn generate(input: &DeriveInput) -> TokenStream {
    let implement = try_ts!(implement(input));

    quote! {
        #implement
    }
}

fn implement(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    let vis = &input.vis;
    let entity = &input.ident;
    let args = TypeArgs::from_derive_input(input).map_err(|e| e.write_errors())?;

    let entity_name = entity.to_string();
    let table_name = entity_name.to_plural();
    let table_name_lit = LitStr::new(&entity_name, entity.span());
    let table_alias = Ident::new(&table_name, entity.span());
    let const_name = entity_name.to_screaming_snake_case();

    let init_tbl_fn = Ident::new(
        &format!("__ctx_init_{}", entity_name.to_snake_case()),
        entity.span(),
    );

    let ctx_var = Ident::new(&format!("__CTX_VAR{const_name}"), entity.span());
    let log_var = Ident::new(&format!("__LOG_VAR{const_name}"), entity.span());

    let coll_ty = args.collection.ty(entity);
    let (gc, gc_collect) = gc(input)?;

    Ok(quote! {
        #vis type #table_alias = #coll_ty;

        storm::extobj::extobj!(
            impl storm::CtxExt { #ctx_var: std::sync::OnceLock<#table_alias> },
            init = #init_tbl_fn(),
            crate_path = storm::extobj
        );

        storm::extobj::extobj!(
            impl storm::LogExt { #log_var: std::sync::OnceLock<storm::Log<#entity>> },
            crate_path = storm::extobj
        );

        impl storm::EntityAccessor for #entity {
            type Tbl = #table_alias;

            #[inline]
            fn ctx_var() -> storm::CtxVar<Self::Tbl> {
                *#ctx_var
            }

            #[inline]
            fn entity_deps() -> &'static storm::Deps {
                static DEPS: storm::Deps = storm::parking_lot::RwLock::new(Vec::new());
                &DEPS
            }

            #[inline]
            fn entity_inits() -> &'static storm::Inits<#table_alias> {
                static INITS: storm::Inits<#table_alias> = storm::Inits::new();
                &INITS
            }

            fn on_change() -> &'static storm::OnChange<Self> {
                static E: std::sync::OnceLock<storm::OnChange<#entity>> = std::sync::OnceLock::new();
                E.get_or_init(Default::default)
            }

            fn on_changed() -> &'static storm::OnChanged<Self> {
                static E: std::sync::OnceLock<storm::OnChanged<#entity>> = std::sync::OnceLock::new();
                E.get_or_init(Default::default)
            }

            fn on_remove() -> &'static storm::OnRemove<Self> {
                static E: std::sync::OnceLock<storm::OnRemove<#entity>> = std::sync::OnceLock::new();
                E.get_or_init(Default::default)
            }
        }

        impl storm::LogAccessor for #entity {
            #[inline]
            fn log_var() -> storm::LogVar<storm::Log<Self>> {
                *#log_var
            }
        }

        impl storm::CtxTypeInfo for #entity {
            const NAME: &'static str = #table_name_lit;
        }

        #[doc(hidden)]
        fn #init_tbl_fn() {
            #gc_collect
            storm::register_apply_log::<#entity>();
        }

        #gc
    })
}

fn gc(input: &DeriveInput) -> Result<(TokenStream, TokenStream), TokenStream> {
    let fields = input.fields()?;
    let ident = &input.ident;
    let types = fields.iter().map(|f| &f.ty);
    let fields = fields.iter().map(|f| &f.ident);

    Ok((
        quote! {
            impl storm::Gc for #ident {
                const SUPPORT_GC: bool = #(<#types as storm::Gc>::SUPPORT_GC ||)* false;

                #[allow(unused_variables)]
                fn gc(&mut self, ctx: &storm::GcCtx) {
                    #(storm::Gc::gc(&mut self.#fields, ctx);)*
                }
            }
        },
        quote! {
            if <#ident as storm::Gc>::SUPPORT_GC {
                storm::gc::collectables::register(|ctx| ctx.tbl_gc::<#ident>());
            }
        },
    ))
}

#[derive(Clone, Copy, Debug, Eq, FromMeta, PartialEq)]
enum Collection {
    HashTable,
    VecTable,
}

impl Collection {
    fn ty(&self, entity: &Ident) -> TokenStream {
        match self {
            Self::HashTable => quote!(storm::HashTable<#entity>),
            Self::VecTable => quote!(storm::VecTable<#entity>),
        }
    }
}

impl Default for Collection {
    fn default() -> Self {
        Self::VecTable
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(storm), allow_unknown_fields)]
struct TypeArgs {
    #[darling(default)]
    collection: Collection,
}
