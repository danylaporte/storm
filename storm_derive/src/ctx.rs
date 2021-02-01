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
    let name_tables = Ident::new(&format!("{}Tables", &name), name.span());
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

    let mut apply_members = Vec::new();
    let mut ctx_members = Vec::new();
    let mut log_members = Vec::new();
    let mut new_members = Vec::new();
    let mut trait_members = Vec::new();
    let mut trx_members = Vec::new();
    let mut trx_members_mut = Vec::new();

    for field in fields {
        let name = field.ident()?;
        let name_mut = Ident::new(&format!("{}_mut", name), name.span());

        if name == "opts" {
            continue;
        }

        let pascal_name = Ident::new(&name.to_string().to_pascal_case(), name.span());
        let ty = field.ty();

        apply_members.push(quote! {
            storm::TableContainer::<#opts_ty>::apply_log(&mut self.#name, log.#name);
        });

        ctx_members.push(quote! {
            type #pascal_name = &'a <#ty as storm::TableContainer<#opts_ty>>::Table;

            async fn #name(&'a self) -> storm::Result<Self::#pascal_name> {
                storm::TableContainer::<#opts_ty>::ensure(&self.#name, &self.opts).await
            }
        });

        log_members.push(quote! {
            #name: storm::TableLog<<#ty as storm::TableContainer<#opts_ty>>::Table>,
        });

        new_members.push(quote! {
            #name: Default::default(),
        });

        trait_members.push(quote! {
            type #pascal_name;
            async fn #name(&'a self) -> storm::Result<Self::#pascal_name>;
        });

        trx_members.push(quote! {
            type #pascal_name = storm::TableTransaction<'a, &'a storm::TableLog<<#ty as storm::TableContainer<#opts_ty>>::Table>, <#ty as storm::TableContainer<#opts_ty>>::Table>;

            async fn #name(&'a self) -> storm::Result<Self::#pascal_name> {
                Ok(storm::TableTransaction {
                    table: storm::TableContainer::<#opts_ty>::ensure(&self.ctx.#name, &self.ctx.opts).await?,
                    log: &self.log.#name,
                })
            }
        });

        trx_members_mut.push(quote! {
            pub async fn #name_mut(&mut self) -> storm::Result<storm::TableTransaction<'_, &mut storm::TableLog<<#ty as storm::TableContainer<#opts_ty>>::Table>, <#ty as storm::TableContainer<#opts_ty>>::Table>> {
                Ok(storm::TableTransaction {
                    log: &mut self.log.#name,
                    table: storm::TableContainer::<#opts_ty>::ensure(&self.ctx.#name, &self.ctx.opts).await?,
                })
            }
        });
    }

    let apply_members = quote!(#(#apply_members)*);
    let ctx_members = quote!(#(#ctx_members)*);
    let log_members = quote!(#(#log_members)*);
    let new_members = quote!(#(#new_members)*);
    let trait_members = quote!(#(#trait_members)*);
    let trx_members = quote!(#(#trx_members)*);
    let trx_members_mut = quote!(#(#trx_members_mut)*);

    Ok(quote! {
        impl #name {
            pub fn new(opts: #opts_ty) -> Self {
                Self {
                    #new_members
                    opts,
                }
            }

            pub fn apply_log(&mut self, log: #name_log) {
                #apply_members
            }

            pub fn transaction(&self) -> #name_transaction {
                #name_transaction {
                    ctx: self,
                    log: Default::default(),
                }
            }
        }

        #[async_trait::async_trait]
        #vis trait #name_tables<'a> {
            #trait_members
        }

        #[async_trait::async_trait]
        impl<'a> #name_tables<'a> for #name {
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
            #trx_members_mut

            #[must_use]
            pub async fn commit(self) -> storm::Result<#name_log> {
                // TODO! Add commit to the transaction.

                Ok(self.log)
            }
        }

        #[async_trait::async_trait]
        impl<'a> #name_tables<'a> for #name_transaction<'a> {
            #trx_members
        }
    })
}
