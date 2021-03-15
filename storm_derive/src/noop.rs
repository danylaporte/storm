use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub(crate) fn delete(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;

    quote! {
        #[storm::async_trait::async_trait]
        impl<'a> storm::provider::Delete<#ident> for storm::provider::TransactionProvider<'a> {
            async fn delete(&self, k: &<#ident as storm::Entity>::Key) -> storm::Result<()> {
                Ok(())
            }
        }
    }
}

pub(crate) fn load(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;

    quote! {
        #[storm::async_trait::async_trait]
        impl<_C, FILTER> storm::provider::LoadAll<#ident, FILTER, _C> for storm::provider::ProviderContainer
        where
            _C: Default + Extend<(<#ident as storm::Entity>::Key, #ident)> + Send + 'static,
            FILTER: Send + Sync,
        {
            async fn load_all(&self, filter: &FILTER) -> storm::Result<_C> {
                Ok(_C::default())
            }
        }

        #[storm::async_trait::async_trait]
        impl storm::provider::LoadOne<#ident> for storm::provider::ProviderContainer {
            async fn load_one(&self, k: &<#ident as Entity>::Key) -> storm::Result<Option<#ident>> {
                Ok(None)
            }
        }
    }
}

pub(crate) fn save(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;

    quote! {
        #[storm::async_trait::async_trait]
        impl<'a> storm::provider::Upsert<#ident> for storm::provider::TransactionProvider<'a> {
            async fn upsert(&self, k: &<#ident as storm::Entity>::Key, v: &#ident) -> storm::Result<()> {
                Ok(())
            }
        }
    }
}
