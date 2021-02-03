use crate::{DeriveInputExt, Errors, FieldExt};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, DeriveInput, Error, LitStr};

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
                out: &mut storm_postgres::BytesMut,
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

pub fn upsert(input: &DeriveInput) -> TokenStream {
    let ts = try_ts!(upsert_internal(input));
    quote! { #ts }
}

fn upsert_internal(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    let table = input.table()?;
    let fields = input.fields()?;
    let ident = &input.ident;

    let mut errors = Vec::new();
    let mut insert_names = String::new();
    let mut insert_values = String::new();
    let mut keys = String::new();
    let mut params = Vec::new();
    let mut updates = String::new();

    for (index, field) in fields.iter().enumerate() {
        let column = continue_ts!(field.column(), errors);
        let field_ident = continue_ts!(field.ident(), errors);

        insert_names.add_sep(",").add_field(&column);
        insert_values.add_sep(",").add_param(index + 1);

        if field.is_key() {
            keys.add_sep(",").add_field(&column);
        }

        updates
            .add_sep(",")
            .add_field(&column)
            .add("=")
            .add_param(index + 1);

        params.push(quote! { &self.#field_ident as &(dyn tokio_postgres::types::ToSql + Sync) });
    }

    let params = quote! { [#(#params,)*] };

    errors.result()?;

    if keys.is_empty() {
        return Err(Error::new(input.span(), "[key] attribute expected.").to_compile_error());
    }

    if !updates.is_empty() {
        updates.insert_str(0, &format!(" ON CONFLICT ({}) DO UPDATE SET", keys));
    }

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}){}",
        table, insert_names, insert_values, updates,
    );

    let sql = LitStr::new(&sql, input.span());
    let (impl_generics, ty_generics, where_clause) = &input.generics.split_for_impl();

    Ok(quote! {
        #[async_trait::async_trait]
        impl #impl_generics storm_postgres::Upsert for #ident #ty_generics #where_clause {
            async fn upsert<C>(&self, client: &C) -> storm_postgres::Result<u64>
            where
                C: storm_postgres::Execute + Send + Sync,
            {
                let p = #params;
                client.execute(#sql, &p).await
            }
        }
    })
}

trait SqlStringExt {
    fn add(&mut self, s: &str) -> &mut Self;
    fn add_sep(&mut self, sep: &str) -> &mut Self;
    fn add_diff(&mut self, field: &str, index: usize) -> &mut Self;
    fn add_field(&mut self, field: &str) -> &mut Self;
    fn add_field_with_alias(&mut self, field: &str, table_alias: &str) -> &mut Self;
    fn add_param(&mut self, index: usize) -> &mut Self;
}

impl SqlStringExt for String {
    fn add(&mut self, s: &str) -> &mut Self {
        self.push_str(s);
        self
    }

    fn add_sep(&mut self, sep: &str) -> &mut Self {
        if !self.is_empty() {
            self.push_str(sep)
        }
        self
    }

    fn add_diff(&mut self, field: &str, index: usize) -> &mut Self {
        let s = format!(
            "[{field}] != @p{index} OR ([{field}] IS NULL AND @p{index} IS NOT NULL) OR ([{field}] IS NOT NULL AND @p{index} IS NULL)",
            field = field,
            index = index,
        );
        self.push_str(&s);
        self
    }

    fn add_field(&mut self, field: &str) -> &mut Self {
        self.push('\"');
        self.push_str(field);
        self.push('\"');
        self
    }

    fn add_field_with_alias(&mut self, field: &str, table_alias: &str) -> &mut Self {
        self.push('\"');
        self.push_str(table_alias);
        self.push('.');
        self.push_str(field);
        self.push('\"');
        self
    }

    fn add_param(&mut self, index: usize) -> &mut Self {
        self.push('$');
        self.push_str(&index.to_string());
        self
    }
}
