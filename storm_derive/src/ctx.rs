use crate::{field_ext::TypeInfo, DeriveInputExt, FieldExt, TokenStreamExt};
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
    let name = &input.ident;

    let log_name = Ident::new(&format!("{}Log", &name), name.span());
    let trx_name = Ident::new(&format!("{}Transaction", &name), name.span());
    let fields = input.fields()?;

    let mut apply_log = Vec::new();
    let mut log_members = Vec::new();
    let mut log_members_new = Vec::new();
    let mut trx_members = Vec::new();
    let mut trx_members_new = Vec::new();
    let mut trx_members_new_log = Vec::new();
    let mut trx_members_new_trx = Vec::new();

    for field in fields {
        let vis = &field.vis;
        let name = field.ident()?;

        let ty = &field.ty;
        let type_info = field.type_info();

        trx_members_new_log.push(quote!(#name: #name.log,));
        trx_members_new_trx.push(quote!(#name: #name.table,));
        log_members_new.push(quote!(#name: storm::mem::Commit::commit(self.#name),));

        match type_info {
            TypeInfo::OnceCell => {
                trx_members.push(quote! {
                    #vis #name: storm::TrxCell<'a, <#ty as storm::CtxTypes<'a>>::Output>,
                });

                trx_members_new.push(quote!(#name: storm::TrxCell::new(&self.#name),));

                apply_log.push(quote!(self.#name.apply_log_opt(log.#name);));
                log_members.push(quote!(#name: Option<<#ty as storm::ApplyLog>::Log>,));
            }
            TypeInfo::Other => {
                trx_members.push(quote! {
                    #vis #name: <#ty as storm::CtxTypes<'a>>::Transaction,
                });

                trx_members_new.push(quote! {
                    #name: storm::mem::Transaction::transaction(&self.#name),
                });

                apply_log.push(quote!(self.#name.apply_log(log.#name);));
                log_members.push(quote!(#name: <#ty as storm::ApplyLog>::Log,));
            }
        }
    }

    let apply_log = apply_log.ts();
    let log_members = log_members.ts();
    let log_members_new = log_members_new.ts();
    let trx_members = trx_members.ts();
    let trx_members_new = trx_members_new.ts();

    Ok(quote! {
        #[derive(Default)]
        #vis struct #log_name {
            #log_members
        }

        #vis struct #trx_name<'a> {
            #trx_members
        }

        impl storm::ApplyLog for #name {
            type Log = #log_name;

            fn apply_log(&mut self, log: Self::Log) {
                #apply_log
            }
        }

        impl<'a> storm::mem::Commit for #trx_name<'a> {
            type Log = #log_name;

            fn commit(self) -> Self::Log {
                #log_name {
                    #log_members_new
                }
            }
        }

        impl<'a> storm::mem::Transaction<'a> for #name {
            type Transaction = #trx_name<'a>;

            fn transaction(&'a self) -> Self::Transaction {
                #trx_name {
                    #trx_members_new
                }
            }
        }
    })
}
