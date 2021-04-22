use darling::{FromDeriveInput, FromMeta};
use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Ident};

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
    let table_alias = Ident::new(&table_name, entity.span());

    let screaming_snake_case = entity_name.to_screaming_snake_case();
    let tbl_var = Ident::new(&format!("{}_TBL", screaming_snake_case), input.ident.span());
    let log_var = Ident::new(&format!("{}_TRX", screaming_snake_case), input.ident.span());
    let coll_ty = args.collection.ty(entity);

    Ok(quote! {
        #vis type #table_alias = #coll_ty;

        #[static_init::dynamic]
        static #tbl_var: (storm::TblVar<#table_alias>, storm::Deps) = Default::default();

        #[static_init::dynamic]
        static #log_var: storm::LogVar<storm::Log<#entity>> = {
            storm::register_apply_log::<#entity>();
            Default::default()
        };

        impl storm::EntityAccessor for #entity {
            type Coll = #table_alias;

            #[inline]
            fn entity_var() -> &'static storm::TblVar<Self::Coll> {
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
