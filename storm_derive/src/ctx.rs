use crate::{field_ext::TypeInfo, DeriveInputExt, FieldExt, TokenStreamExt};
use darling::FromField;
use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, GenericArgument, Ident, PathArguments, Type};

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
    let mut apply_log_indexes = Vec::new();
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

        let field_attrs = FieldAttrs::from_field(field).map_err(|e| e.write_errors())?;
        let ty = &field.ty;

        if field_attrs.index {
            let ty = get_async_once_cell_ty(ty);
            apply_log_indexes.push(quote! {
                if self.#name.get().map_or(false, |c| c.is_version_obsolete(self)) {
                    changed = true;
                    self.#name.take();
                }
            });

            globals.push(quote! {
                impl<C> storm::GetOrLoad<#ty, C> for #ctx_name
                where
                    storm::AsyncOnceCell<#ty>: storm::GetOrLoad<#ty, C>
                {
                    fn get_or_load<'a>(&'a self, ctx: &C) -> &'a #ty {
                        storm::GetOrLoad::get_or_load(&self.#name, ctx)
                    }
                }

                impl storm::AsRefOpt<#ty> for #ctx_name {
                    fn as_ref_opt(&self) -> Option<&#ty> {
                        self.#name.get()
                    }
                }
            });

            tbl_members.push(quote! {
                async fn #name<'a>(&'a self) -> storm::Result<&'a #ty>
                where
                    #ty: storm::Init<storm::ConnectedRef<'a, &'a #ctx_name>>,
                {
                    let (ctx, provider) = self.ctx();
                    let connected = storm::ConnectedRef { ctx, provider };

                    storm::GetOrLoadAsync::get_or_load_async(&ctx.#name, &connected).await
                }
            });
        } else {
            log_members_new.push(quote!(#name: storm::mem::Commit::commit(self.#name),));
            trx_members_new_log.push(quote!(#name: #name.log,));
            trx_members_new_trx.push(quote!(#name: #name.table,));

            match field.type_info() {
                TypeInfo::AsyncOnceCell => {
                    let ty = get_async_once_cell_ty(ty);
                    let alias = Ident::new(&name.to_string().to_pascal_case(), name.span());

                    globals.push(quote! {
                        #vis type #alias = #ty;

                        impl storm::AsRefOpt<#ty> for #ctx_name {
                            fn as_ref_opt(&self) -> Option<&#ty> {
                                self.#name.get()
                            }
                        }
                    });

                    tbl_members.push(quote! {
                        async fn #name<'a>(&'a self) -> storm::Result<&'a #alias>
                        where
                            #alias: storm::Init<storm::provider::ProviderContainer>,
                        {
                            let (ctx, provider) = self.ctx();

                            storm::GetOrLoadAsync::get_or_load_async(&ctx.#name, provider).await
                        }
                    });

                    trx_members.push(quote! {
                        #vis #name: storm::TrxCell<'a, #alias>,
                    });

                    trx_members_new.push(quote!(#name: storm::TrxCell::new(&self.#name),));

                    trx_tbl_members.push(quote! {
                        async fn #name<'b>(&'b self) -> storm::Result<storm::ConnectedTrxRef<'b, &'b <#alias as storm::Transaction<'a>>::Transaction>>
                        where
                            'a: 'b,
                            #alias: storm::Init<storm::provider::ProviderContainer>,
                        {
                            let (ctx, provider) = self.ctx();
                            Ok(storm::ConnectedTrxRef::new(ctx.#name.get_or_init(provider).await?, provider))
                        }

                        async fn #name_mut<'b>(&'b mut self) -> storm::Result<storm::ConnectedTrxRef<'b, &'b mut <#alias as storm::Transaction<'a>>::Transaction>>
                        where
                            'a: 'b,
                            #alias: storm::Init<storm::provider::ProviderContainer>,
                        {
                            let (ctx, provider) = self.ctx_mut();
                            Ok(storm::ConnectedTrxRef::new(ctx.#name.get_mut_or_init(provider).await?, provider))
                        }
                    });

                    apply_log.push(quote!(self.#name.apply_log_opt(log.#name);));
                    log_members.push(quote!(#name: Option<<#ty as storm::ApplyLog>::Log>,));

                    globals.push(quote! {
                        #[storm::async_trait::async_trait]
                        impl<P> storm::GetOrLoadAsync<#alias, P> for #ctx_name
                        where
                            P: Sync,
                            #alias: storm::Init<P> + Send + Sync,
                        {
                            async fn get_or_load_async<'a>(&'a self, provider: &P) -> storm::Result<&'a #alias> {
                                storm::GetOrLoadAsync::get_or_load_async(&self.#name, provider).await
                            }
                        }
                    });
                }
                TypeInfo::Other => {
                    let alias = Ident::new(&name.to_string().to_pascal_case(), name.span());
                    globals.push(quote! {
                        #vis type #alias = #ty;

                        impl storm::AsRefOpt<#ty> for #ctx_name {
                            fn as_ref_opt(&self) -> Option<&#ty> {
                                Some(&self.#name)
                            }
                        }
                    });

                    tbl_members.push(quote! {
                        #[must_use]
                        fn #name(&self) -> &#ty {
                            &self.ctx().0.#name
                        }
                    });

                    trx_members.push(quote! {
                        #vis #name: <#ty as storm::Transaction<'a>>::Transaction,
                    });

                    trx_members_new.push(quote! {
                        #name: storm::Transaction::<'a>::transaction(&self.#name),
                    });

                    trx_tbl_members.push(quote! {
                        #[must_use]
                        fn #name<'b>(&'b self) -> storm::ConnectedTrxRef<'b, &'b <#ty as storm::Transaction<'a>>::Transaction>
                        where
                            #ty: storm::Transaction<'a>,
                            'a: 'b,
                        {
                            let (ctx, provider) = self.ctx();
                            storm::ConnectedTrxRef::new(&ctx.#name, provider)
                        }

                        #[must_use]
                        fn #name_mut<'b>(&'b mut self) -> storm::ConnectedTrxRef<'b, &'b mut <#ty as storm::Transaction<'a>>::Transaction>
                        where
                            'a: 'b,
                            #ty: storm::Transaction<'a>,
                        {
                            let (ctx, provider) = self.ctx_mut();
                            storm::ConnectedTrxRef::new(&mut ctx.#name, provider)
                        }
                    });

                    apply_log.push(quote!(self.#name.apply_log(log.#name);));
                    log_members.push(quote!(#name: <#ty as storm::ApplyLog>::Log,));
                }
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

    let apply_log_indexes = match apply_log_indexes.is_empty() {
        true => quote!(),
        false => quote! {
            let mut changed = true;
            while changed {
                changed = false;
                #(#apply_log_indexes)*
            }
        },
    };

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
                #apply_log_indexes
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

        impl<'a> storm::Transaction<'a> for #ctx_name {
            type Transaction = #trx_name<'a>;

            fn transaction(&'a self) -> Self::Transaction {
                #trx_name {
                    #trx_members_new
                }
            }
        }

        #[storm::async_trait::async_trait]
        #vis trait #tbl_name: Send + Sync {
            fn ctx(&self) -> (&#ctx_name, &storm::provider::ProviderContainer);

            #tbl_members
        }

        #[storm::async_trait::async_trait]
        impl<'a, C> #tbl_name for storm::Connected<C>
        where
            C: AsRef<#ctx_name> + Send + Sync,
        {
            fn ctx(&self) -> (&#ctx_name, &storm::provider::ProviderContainer) {
                (self.ctx.as_ref(), &self.provider)
            }
        }

        #[storm::async_trait::async_trait]
        #vis trait #trx_tbl_name<'a> {
            #[must_use]
            fn ctx(&self) -> (&#trx_name<'a>, &storm::provider::TransactionProvider<'a>);

            #[must_use]
            fn ctx_mut(&mut self) -> (&mut #trx_name<'a>, &storm::provider::TransactionProvider<'a>);

            #trx_tbl_members
        }

        #[storm::async_trait::async_trait]
        impl<'a, T> #trx_tbl_name<'a> for storm::ConnectedTrx<'a, T>
        where
            T: AsRef<#trx_name<'a>> + AsMut<#trx_name<'a>>,
        {
            fn ctx(&self) -> (&#trx_name<'a>, &storm::provider::TransactionProvider<'a>) {
                (self.trx.as_ref(), &self.provider)
            }

            fn ctx_mut(&mut self) -> (&mut #trx_name<'a>, &storm::provider::TransactionProvider<'a>) {
                (self.trx.as_mut(), &self.provider)
            }
        }

        #globals
    })
}

#[derive(Debug, FromField)]
#[darling(attributes(storm))]
struct FieldAttrs {
    #[darling(default)]
    index: bool,
}

fn get_async_once_cell_ty(t: &Type) -> &Type {
    match t {
        Type::Reference(r) => get_async_once_cell_ty(&r.elem),
        Type::Path(p) => p
            .path
            .segments
            .last()
            .and_then(|s| match &s.arguments {
                PathArguments::AngleBracketed(a) => a.args.first(),
                _ => None,
            })
            .and_then(|a| match a {
                GenericArgument::Type(t) => Some(t),
                _ => None,
            })
            .unwrap_or(t),
        _ => t,
    }
}
