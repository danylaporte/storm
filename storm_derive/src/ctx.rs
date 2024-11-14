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

    let coll_ty = args.collection.ty(entity);
    let gc = gc(input)?;

    Ok(quote! {
        #vis type #table_alias = #coll_ty;

        impl storm::EntityAsset for #entity {
            type Tbl = #table_alias;

            #[allow(non_camel_case_types)]
            #[inline]
            fn ctx_var() -> storm::attached::Var<Self::Tbl, storm::CtxVars> {
                storm::attached::var!(VAR: #table_alias, storm::CtxVars);
                *VAR
            }

            #[allow(non_camel_case_types)]
            #[inline]
            fn log_var() -> storm::attached::Var<<Self::Tbl as storm::Asset>::Log, storm::LogVars> {
                storm::attached::var!(VAR: <#table_alias as storm::Asset>::Log, storm::LogVars);
                *VAR
            }

            #[inline]
            fn change() -> &'static storm::ChangeEvent<Self> {
                #[static_init::dynamic]
                static EVENT: storm::ChangeEvent<#entity> = Default::default();
                &EVENT
            }

            #[inline]
            fn changed() -> &'static storm::ChangedEvent<Self> {
                #[static_init::dynamic]
                static EVENT: storm::ChangedEvent<#entity> = Default::default();
                &EVENT
            }

            #[inline]
            fn remove() -> &'static storm::RemoveEvent<<Self as storm::Entity>::Key, <Self as storm::Entity>::TrackCtx> {
                #[static_init::dynamic]
                static EVENT: storm::RemoveEvent<<#entity as storm::Entity>::Key, <#entity as storm::Entity>::TrackCtx> = Default::default();
                &EVENT
            }

            #[inline]
            fn removed() -> &'static storm::RemoveEvent<<Self as storm::Entity>::Key, <Self as storm::Entity>::TrackCtx> {
                #[static_init::dynamic]
                static EVENT: storm::RemoveEvent<<#entity as storm::Entity>::Key, <#entity as storm::Entity>::TrackCtx> = Default::default();
                &EVENT
            }
        }

        impl storm::CtxTypeInfo for #entity {
            const NAME: &'static str = #table_name_lit;
        }

        #gc
    })
}

fn gc(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    let fields = input.fields()?;
    let ident = &input.ident;
    let types = fields.iter().map(|f| &f.ty);
    let fields = fields.iter().map(|f| &f.ident);

    Ok(quote! {
        impl storm::Gc for #ident {
            const SUPPORT_GC: bool = #(<#types as storm::Gc>::SUPPORT_GC ||)* false;

            #[allow(unused_variables)]
            fn gc(&mut self) {
                #(storm::Gc::gc(&mut self.#fields);)*
            }
        }
    })
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
