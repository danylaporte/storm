use darling::{FromDeriveInput, FromMeta};
use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Ident, LitStr};

use crate::{derive_input_ext::DeriveInputExt, type_ext::TypeExt};

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

    let screaming_snake_case = entity_name.to_screaming_snake_case();
    let tbl_var = Ident::new(&format!("{}_TBL", screaming_snake_case), input.ident.span());
    let log_var = Ident::new(&format!("{}_TRX", screaming_snake_case), input.ident.span());
    let coll_ty = args.collection.ty(entity);
    let (gc, gc_collect) = gc(input)?;

    Ok(quote! {
        #vis type #table_alias = #coll_ty;

        #[static_init::dynamic]
        static #tbl_var: (storm::TblVar<#table_alias>, storm::Deps) = {
            #gc_collect
            Default::default()
        };

        #[static_init::dynamic]
        static #log_var: storm::LogVar<storm::Log<#entity>> = {
            storm::register_apply_log::<#entity>();
            Default::default()
        };

        impl storm::EntityAccessor for #entity {
            type Tbl = #table_alias;

            #[inline]
            fn entity_var() -> &'static storm::TblVar<Self::Tbl> {
                &#tbl_var.0
            }

            #[inline]
            fn entity_deps() -> &'static storm::Deps {
                &#tbl_var.1
            }
        }

        impl storm::LogAccessor for #entity {
            #[inline]
            fn log_var() -> &'static storm::LogVar<storm::Log<Self>> {
                &#log_var
            }
        }

        impl storm::CtxTypeInfo for #entity {
            const NAME: &'static str = #table_name_lit;
        }

        #gc
    })
}

fn gc(input: &DeriveInput) -> Result<(TokenStream, TokenStream), TokenStream> {
    let fields = input.fields()?;
    let ident = &input.ident;
    let mut vec = Vec::new();

    for field in fields {
        if field.ty.is_cache_island() {
            let ident = &field.ident;
            vec.push(quote!(storm::Gc::gc(&mut self.#ident, ctx);));
        }
    }

    Ok(if vec.is_empty() {
        (quote!(), quote!())
    } else {
        (
            quote! {
                impl storm::Gc for #ident {
                    const SUPPORT_GC: bool = true;

                    fn gc(&mut self, ctx: &storm::GcCtx) {
                        #(#vec)*
                    }
                }
            },
            quote!(storm::gc::collectables::register(|ctx| ctx.tbl_gc::<#ident>());),
        )
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
