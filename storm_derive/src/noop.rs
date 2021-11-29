use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub(crate) fn delete(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;

    quote! {
        impl storm::provider::Delete<#ident> for storm::provider::TransactionProvider<'_> {
            fn delete<'a>(&'a self, k: &'a <#ident as storm::Entity>::Key) -> storm::BoxFuture<'a, storm::Result<()>> {
                Box::pin(async { Ok(()) })
            }
        }
    }
}

pub(crate) fn load(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;

    quote! {
        impl<_C, FILTER> storm::provider::LoadAll<#ident, FILTER, _C> for storm::provider::ProviderContainer
        where
            _C: Default + Extend<(<#ident as storm::Entity>::Key, #ident)> + Send + 'static,
            FILTER: Send + Sync,
        {
            fn load_all_with_args<'a>(&'a self, filter: &'a FILTER, args: storm::provider::LoadArgs) -> storm::BoxFuture<'a, storm::Result<_C>> {
                Box::pin(async { Ok(_C::default()) })
            }
        }

        impl storm::provider::LoadOne<#ident> for storm::provider::ProviderContainer {
            fn load_one_with_args<'a>(&'a self, k: &'a <#ident as storm::Entity>::Key, args: storm::provider::LoadArgs) -> storm::BoxFuture<'a, storm::Result<Option<#ident>>> {
                Box::pin(async { Ok(None) })
            }
        }
    }
}

pub(crate) fn save(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;

    quote! {
        impl storm::Insertable for #ident {
            fn insertable<'a, 'b>(
                self,
                _trx: &'b mut storm::CtxTransaction<'a>,
                k: <Self as storm::Entity>::Key,
                _track: &'b <Self as storm::Entity>::TrackCtx,
            ) -> storm::BoxFuture<'b, storm::Result<<Self as storm::Entity>::Key>>
        where
            'a: 'b,
        {
                Box::pin(async move { Ok(k)})
            }
        }

        impl storm::provider::Upsert<#ident> for storm::provider::TransactionProvider<'_> {
            fn upsert<'a>(&'a self, k: &'a <#ident as storm::Entity>::Key, v: &'a #ident) -> storm::BoxFuture<'a, storm::Result<()>> {
                Box::pin(async { Ok(()) })
            }
        }
    }
}
