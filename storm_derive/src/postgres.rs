use crate::DeriveInputExt;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub fn from_sql(input: &DeriveInput) -> TokenStream {
    let ts = try_ts!(from_sql_internal(input));
    quote! { #ts }
}

fn from_sql_internal(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    let name = &input.ident;
    let field = input.tuple_single_field()?;
    let t = &field.ty;

    Ok(quote! {
        impl<'a> postgres_types::FromSql<'a> for #name {
            fn from_sql(
                ty: &postgres_types::Type,
                raw: &'a [u8],
            ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
                Ok(#name(postgres_types::FromSql::from_sql(ty, raw)?))
            }

            fn accepts(ty: &postgres_types::Type) -> bool {
                <#t as postgres_types::FromSql>::accepts(ty)
            }
        }
    })
}

pub fn to_sql(input: &DeriveInput) -> TokenStream {
    let ts = try_ts!(to_sql_internal(input));
    quote! { #ts }
}

fn to_sql_internal(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    let name = &input.ident;
    let field = input.tuple_single_field()?;
    let t = &field.ty;

    Ok(quote! {
        impl postgres_types::ToSql for #name {
            fn to_sql(
                &self,
                ty: &postgres_types::Type,
                out: &mut actix_web::web::BytesMut,
            ) -> Result<postgres_types::IsNull, Box<dyn std::error::Error + Sync + Send>>
            where
                Self: Sized,
            {
                self.0.to_sql(ty, out)
            }

            fn accepts(ty: &postgres_types::Type) -> bool
            where
                Self: Sized,
            {
                <#t as postgres_types::ToSql>::accepts(ty)
            }

            postgres_types::to_sql_checked!();
        }
    })
}
