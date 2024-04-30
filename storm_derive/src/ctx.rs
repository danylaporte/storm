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
    let (gc, gc_collect) = gc(input)?;

    Ok(quote! {
        #vis type #table_alias = #coll_ty;

        impl storm::EntityAccessor for #entity {
            type Tbl = #table_alias;

            #[allow(non_camel_case_types)]
            #[inline]
            fn entity_var() -> storm::TblVar<Self::Tbl> {
                storm::attached::var!(T: #table_alias, storm::vars::Tbl);

                // Garbage collection static registering
                #[static_init::dynamic]
                static G: () = {
                    #gc_collect
                };

                *T
            }

            #[inline]
            fn entity_deps() -> &'static storm::Deps {
                static DEPS: storm::Deps = storm::parking_lot::RwLock::new(Vec::new());
                &DEPS
            }

            #[inline]
            fn on_changed() -> &'static storm::OnChanged<Self> {
                #[static_init::dynamic]
                static E: storm::OnChanged<#entity> = Default::default();
                &E
            }

            #[inline]
            fn on_remove() -> &'static storm::OnRemove<Self> {
                #[static_init::dynamic]
                static E: storm::OnRemove<#entity> = Default::default();
                &E
            }
        }

        impl storm::LogAccessor for #entity {
            #[inline]
            fn log_var() -> storm::LogVar<storm::Log<Self>> {
                storm::attached::var!(L: storm::Log<#entity>, storm::vars::Log);

                #[static_init::dynamic]
                static R: () = storm::register_apply_log::<#entity>();

                *L
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
