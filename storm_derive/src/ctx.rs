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

    let ctx_name = &input.ident;
    let log_name = Ident::new(&format!("{}Log", &ctx_name), ctx_name.span());
    let trx_name = Ident::new(&format!("{}Transaction", &ctx_name), ctx_name.span());
    let fields = input.fields()?;

    let mut apply_log = Vec::new();
    let mut as_ref_impl = Vec::new();
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
                as_ref_impl.push(quote! {
                    impl<'a> AsRef<<#ty as storm::mem::Transaction<'a>>::Transaction> for #trx_name<'a> {
                        fn as_ref(&self) -> &<#ty as storm::mem::Transaction<'a>>::Transaction {
                            &self.#name
                        }
                    }

                    impl AsRef<#ty> for #ctx_name {
                        fn as_ref(&self) -> &#ty {
                            &self.#name
                        }
                    }
                });

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
    let as_ref_impl = as_ref_impl.ts();
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

        impl storm::ApplyLog for #ctx_name {
            type Log = #log_name;

            fn apply_log(&mut self, log: Self::Log) {
                #apply_log
            }
        }

        #as_ref_impl

        impl AsRef<#ctx_name> for #ctx_name {
            fn as_ref(&self) -> &#ctx_name {
                self
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

        impl<'a> storm::mem::Transaction<'a> for #ctx_name {
            type Transaction = #trx_name<'a>;

            fn transaction(&'a self) -> Self::Transaction {
                #trx_name {
                    #trx_members_new
                }
            }
        }
    })
}
