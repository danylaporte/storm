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

    let init_tbl_fn = Ident::new(
        &format!("__ctx_init_{}", entity_name.to_snake_case()),
        entity.span(),
    );

    let coll_ty = args.collection.ty(entity);
    let (gc, gc_collect) = gc(input)?;

    Ok(quote! {
        #vis type #table_alias = #coll_ty;

        impl storm::EntityAccessor for #entity {
            type Tbl = #table_alias;

            #[inline]
            fn applied() -> &'static storm::AppliedEvent<Self> {
                static E: storm::AppliedEvent<#entity> = storm::AppliedEvent::new();
                &E
            }

            #[inline]
            fn cleared() -> &'static storm::ClearEvent {
                static E: storm::ClearEvent = storm::ClearEvent::new();
                &E
            }

            #[inline]
            fn removed() -> &'static storm::RemovedEvent<Self> {
                static E: storm::RemovedEvent<#entity> = storm::RemovedEvent::new();
                &E
            }

            #[inline]
            fn removing() -> &'static storm::RemovingEvent<Self> {
                static E: storm::RemovingEvent<#entity> = storm::RemovingEvent::new();
                &E
            }

            #[inline]
            fn tbl_var() -> storm::CtxVar<Self::Tbl> {
                storm::extobj::extobj!(
                    impl storm::CtxExt {
                        V: std::sync::OnceLock<#table_alias>,
                    },
                    crate_path = storm::extobj
                );

                *V
            }

            #[inline]
            fn touched() -> &'static storm::TouchedEvent {
                static E: storm::TouchedEvent = storm::TouchedEvent::new();
                &E
            }

            #[inline]
            fn upserted() -> &'static storm::UpsertedEvent<Self> {
                static E: storm::UpsertedEvent<#entity> = storm::UpsertedEvent::new();
                &E
            }

            #[inline]
            fn upserting() -> &'static storm::UpsertingEvent<Self> {
                static E: storm::UpsertingEvent<#entity> = storm::UpsertingEvent::new();
                &E
            }
        }

        impl storm::CtxTypeInfo for #entity {
            const NAME: &'static str = #table_name_lit;
        }

        #[storm::register]
        fn #init_tbl_fn() {
            storm::__register_apply(#table_alias::__apply_log, storm::ApplyOrder::Table);
            #gc_collect
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

                fn gc(&mut self) {
                    #(storm::Gc::gc(&mut self.#fields);)*
                }
            }
        },
        quote! {
            if <#ident as storm::Gc>::SUPPORT_GC {
                storm::Ctx::on_gc_collect(<#ident as storm::EntityAccessor>::tbl_gc);
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
