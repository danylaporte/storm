use crate::{field_ext::TypeInfo, DeriveInputExt, FieldExt, TokenStreamExt};
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

    let ctx_name = &input.ident;
    let log_name = Ident::new(&format!("{}Log", &ctx_name), ctx_name.span());
    let tbl_name = Ident::new(&format!("{}Tables", &ctx_name), ctx_name.span());
    let trx_name = Ident::new(&format!("{}Transaction", &ctx_name), ctx_name.span());
    let trx_tbl_name = Ident::new(&format!("{}TransactionTables", &ctx_name), ctx_name.span());
    let fields = input.fields()?;

    let mut apply_log = Vec::new();
    let mut globals = Vec::new();
    let mut log_members = Vec::new();
    let mut log_members_new = Vec::new();
    let mut tbl_members = Vec::new();
    let mut trx_members = Vec::new();
    let mut trx_members_new = Vec::new();
    let mut trx_members_new_log = Vec::new();
    let mut trx_members_new_trx = Vec::new();
    let mut trx_tbl_members = Vec::new();

    for field in fields {
        let vis = &field.vis;
        let name = field.ident()?;
        let name_mut = Ident::new(&format!("{}_mut", &name), name.span());
        let alias = Ident::new(&name.to_string().to_pascal_case(), name.span());

        let ty = &field.ty;
        let type_info = field.type_info();

        globals.push(quote!(#vis type #alias = <#ty as storm::CtxMember>::Member;));
        log_members_new.push(quote!(#name: storm::mem::Commit::commit(self.#name),));
        trx_members_new_log.push(quote!(#name: #name.log,));
        trx_members_new_trx.push(quote!(#name: #name.table,));

        match type_info {
            TypeInfo::OnceCell => {
                tbl_members.push(quote! {
                    async fn #name<'a>(&'a self) -> storm::Result<&'a #alias>
                    where
                        #alias: storm::Init<Self::Provider>,
                        Self::Provider: for<'c> storm::provider::Gate<'c>,
                    {
                        let (ctx, provider) = self.ctx();

                        storm::GetOrLoad::get_or_load(&ctx.#name, provider).await
                    }
                });

                trx_members.push(quote! {
                    #vis #name: storm::TrxCell<'a, #alias>,
                });

                trx_members_new.push(quote!(#name: storm::TrxCell::new(&self.#name),));

                trx_tbl_members.push(quote! {
                    async fn #name<'b>(&'b self) -> storm::Result<storm::Connected<&'b <#alias as storm::mem::Transaction<'a>>::Transaction, &'b <Self as #trx_tbl_name<'a>>::Provider>>
                    where
                        'a: 'b,
                        #alias: storm::Init<Self::Provider>,
                        Self::Provider: for<'c> storm::provider::Gate<'c>,
                    {
                        let (ctx, provider) = self.ctx();

                        Ok(storm::Connected {
                            ctx: ctx.#name.get_or_init(provider).await?,
                            provider,
                        })                        
                    }

                    async fn #name_mut<'b>(&'b mut self) -> storm::Result<storm::Connected<&'b mut <#alias as storm::mem::Transaction<'a>>::Transaction, &'b <Self as #trx_tbl_name<'a>>::Provider>>
                    where
                        'a: 'b,
                        #alias: storm::Init<Self::Provider>,
                        Self::Provider: for<'c> storm::provider::Gate<'c>,
                    {
                        let (ctx, provider) = self.ctx_mut();

                        Ok(storm::Connected {
                            ctx: ctx.#name.get_mut_or_init(provider).await?,
                            provider,
                        })                        
                    }
                });

                apply_log.push(quote!(self.#name.apply_log_opt(log.#name);));
                log_members.push(quote!(#name: Option<<#ty as storm::ApplyLog>::Log>,));
            }
            TypeInfo::Other => {
                tbl_members.push(quote! {
                    #[must_use]
                    fn #name(&self) -> &#ty {
                        &self.ctx().0.#name
                    }
                });

                trx_members.push(quote! {
                    #vis #name: <#ty as storm::mem::Transaction<'a>>::Transaction,
                });

                trx_members_new.push(quote! {
                    #name: storm::mem::Transaction::<'a>::transaction(&self.#name),
                });

                trx_tbl_members.push(quote! {
                    #[must_use]
                    fn #name<'b>(&'b self) -> storm::Connected<&'b<#ty as storm::mem::Transaction<'a>>::Transaction, &'b Self::Provider>
                    where
                        #ty: storm::mem::Transaction<'a>,
                        'a: 'b,
                    {
                        let (ctx, provider) = self.ctx();

                        storm::Connected {
                            ctx: &ctx.#name,
                            provider,
                        }
                    }

                    #[must_use]
                    fn #name_mut<'b>(&'b mut self) -> storm::Connected<&'b mut <#ty as storm::mem::Transaction<'a>>::Transaction, &'b Self::Provider>
                    where
                        'a: 'b,
                        #ty: storm::mem::Transaction<'a>,
                    {
                        let (ctx, provider) = self.ctx_mut();

                        storm::Connected {
                            ctx: &mut ctx.#name,
                            provider,
                        }
                    }
                });

                apply_log.push(quote!(self.#name.apply_log(log.#name);));
                log_members.push(quote!(#name: <#ty as storm::ApplyLog>::Log,));
            }
        }
    }

    let apply_log = apply_log.ts();
    let globals = globals.ts();
    let log_members = log_members.ts();
    let log_members_new = log_members_new.ts();
    let tbl_members = tbl_members.ts();
    let trx_members = trx_members.ts();
    let trx_members_new = trx_members_new.ts();
    let trx_tbl_members = trx_tbl_members.ts();

    Ok(quote! {
        #[must_use]
        #[derive(Default)]
        #vis struct #log_name {
            #log_members
        }

        #[must_use]
        #vis struct #trx_name<'a> {
            #trx_members
        }

        impl storm::ApplyLog for #ctx_name {
            type Log = #log_name;

            fn apply_log(&mut self, log: Self::Log) {
                #apply_log
            }
        }

        impl<'a> AsMut<#trx_name<'a>> for #trx_name<'a> {
            fn as_mut(&mut self) -> &mut #trx_name<'a> {
                self
            }
        }

        impl AsRef<#ctx_name> for #ctx_name {
            fn as_ref(&self) -> &#ctx_name {
                self
            }
        }

        impl<'a> AsRef<#trx_name<'a>> for #trx_name<'a> {
            fn as_ref(&self) -> &#trx_name<'a> {
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

        #[async_trait::async_trait]
        #vis trait #tbl_name {
            type Provider: Sync;

            fn ctx(&self) -> (&#ctx_name, &Self::Provider);

            #tbl_members
        }

        #[async_trait::async_trait]
        impl<'a, C, P> #tbl_name for storm::Connected<C, P>
        where
            C: AsRef<#ctx_name>,
            P: Sync,
        {
            type Provider = P;

            fn ctx(&self) -> (&#ctx_name, &Self::Provider) {
                (self.ctx.as_ref(), &self.provider)
            }
        }

        #[async_trait::async_trait]
        #vis trait #trx_tbl_name<'a> {
            type Provider: Sync;

            #[must_use]
            fn ctx(&self) -> (&#trx_name<'a>, &Self::Provider);

            #[must_use]
            fn ctx_mut(&mut self) -> (&mut #trx_name<'a>, &Self::Provider);

            #trx_tbl_members
        }

        #[async_trait::async_trait]
        impl<'a, T, P> #trx_tbl_name<'a> for storm::Connected<T, P>
        where
            T: AsRef<#trx_name<'a>> + AsMut<#trx_name<'a>>,
            P: Sync,
        {
            type Provider = P;

            fn ctx(&self) -> (&#trx_name<'a>, &P) {
                (self.ctx.as_ref(), &self.provider)
            }

            fn ctx_mut(&mut self) -> (&mut #trx_name<'a>, &P) {
                (self.ctx.as_mut(), &self.provider)
            }
        }

        #globals
    })
}
