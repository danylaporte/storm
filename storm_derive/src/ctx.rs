use crate::{DeriveInputExt, FieldExt};
use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, DeriveInput, Error, Ident};

pub fn generate(input: &DeriveInput) -> TokenStream {
    let implement = try_ts!(implement(input));

    quote! {
        #implement
    }
}

fn implement(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    const OPTS: &str = "opts";

    let vis = &input.vis;
    let name = &input.ident;
    let name_log = Ident::new(&format!("{}Log", &name), name.span());
    let name_transaction = Ident::new(&format!("{}Transaction", &name), name.span());
    let fields = input.fields()?;

    let opts_ty = fields
        .iter()
        .filter_map(|f| Some((f.ident.as_ref()?, &f.ty)))
        .find(|t| t.0 == OPTS)
        .map(|t| t.1)
        .ok_or_else(|| {
            Error::new(input.span(), format!("Missing a `{}` field.", OPTS)).to_compile_error()
        })?;

    let mut ctx_members = Vec::new();
    let mut log_members = Vec::new();
    let mut trx_members = Vec::new();
    let mut trait_members = Vec::new();
    let mut apply_members = Vec::new();

    for field in fields {
        let name = field.ident()?;

        if name == "opts" {
            continue;
        }

        let pascal_name = Ident::new(&name.to_string().to_pascal_case(), name.span());
        let ty = field.ty();

        log_members.push(quote! {
            #name: storm::TableLog<<<<#ty as storm::TableContainer<#opts_ty>>::Table as storm::Table>::Entity as storm::Entity>::Row>,
        });

        trait_members.push(quote! {
            type #pascal_name;
            async fn #name(&'a self) -> storm::Result<Self::#pascal_name>;
        });

        ctx_members.push(quote! {
            type #pascal_name = &'a <#ty as storm::TableContainer<#opts_ty>>::Table;

            async fn #name(&'a self) -> storm::Result<Self::#pascal_name> {
                storm::TableContainer::<#opts_ty>::ensure(&self.#name, &self.opts).await
            }
        });

        trx_members.push(quote! {
            type #pascal_name = storm::TableTransaction<'a, <#ty as storm::TableContainer<#opts_ty>>::Table>;

            async fn #name(&'a self) -> storm::Result<Self::#pascal_name> {
                Ok(storm::TableTransaction {
                    table: storm::TableContainer::<#opts_ty>::ensure(&self.ctx.#name, &self.ctx.opts).await?,
                    log: &self.log.#name,
                })
            }
        });

        apply_members.push(quote! {
            storm::TableContainer::<#opts_ty>::apply_log(&mut self.#name, log.#name);
        });
    }

    let ctx_members = quote!(#(#ctx_members)*);
    let log_members = quote!(#(#log_members)*);
    let trx_members = quote!(#(#trx_members)*);
    let apply_members = quote!(#(#apply_members)*);
    let trait_members = quote!(#(#trait_members)*);

    Ok(quote! {
        impl #name {
            pub fn apply_log(&mut self, log: #name_log) {
                #apply_members
            }

            pub fn transaction(&self) -> #name_transaction {
                #name_transaction::new(self)
            }
        }

        #[async_trait::async_trait]
        #vis trait Tables<'a> {
            #trait_members
        }

        #[async_trait::async_trait]
        impl<'a> Tables<'a> for #name {
            #ctx_members
        }

        #[derive(Default)]
        #vis struct #name_log {
            #log_members
        }

        #vis struct #name_transaction<'a> {
            ctx: &'a #name,
            log: #name_log,
        }

        impl<'a> #name_transaction<'a> {
            pub fn new(ctx: &'a #name) -> Self {
                Self {
                    ctx,
                    log: Default::default(),
                }
            }

            #[must_use]
            pub async fn commit(self) -> storm::Result<#name_log> {
                // TODO! Add commit to the transaction.

                Ok(self.log)
            }
        }

        #[async_trait::async_trait]
        impl<'a> Tables<'a> for #name_transaction<'a> {
            #trx_members
        }
    })
}
