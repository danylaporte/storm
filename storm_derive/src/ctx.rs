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

    let tbl_var = Ident::new(
        &format!("___{}_tbl", entity_name.to_snake_case()),
        input.ident.span(),
    );
    let coll_ty = args.collection.ty(entity);
    let (gc, gc_collect) = gc(input)?;

    Ok(quote! {
        #vis type #table_alias = #coll_ty;

        fn #tbl_var() -> &'static (storm::TblVar<#table_alias>, storm::Deps, storm::OnRemove<#entity>) {
            static CELL: storm::OnceCell<(storm::TblVar<#table_alias>, storm::Deps, storm::OnRemove<#entity>)> = storm::OnceCell::new();

            CELL.get_or_init(|| {
                #gc_collect
                Default::default()
            })
        }

        impl storm::EntityAccessor for #entity {
            type Tbl = #table_alias;

            #[inline]
            fn entity_var() -> &'static storm::TblVar<Self::Tbl> {
                &#tbl_var().0
            }

            #[inline]
            fn entity_deps() -> &'static storm::Deps {
                &#tbl_var().1
            }

            #[inline]
            fn on_remove() -> &'static storm::OnRemove<Self> {
                &#tbl_var().2
            }
        }

        impl storm::LogAccessor for #entity {
            #[inline]
            fn log_var() -> &'static storm::LogVar<storm::Log<Self>> {
                static CELL: storm::OnceCell<storm::LogVar<storm::Log<#entity>>> = storm::OnceCell::new();

                CELL.get_or_init(|| {
                    storm::register_apply_log::<#entity>();
                    storm::LogVar::default()
                })
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
