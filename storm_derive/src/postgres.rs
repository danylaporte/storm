use crate::{DeriveInputExt, Errors, FieldExt, StringExt};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, DeriveInput, Error, LitInt, LitStr};

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

pub fn load(input: &DeriveInput) -> TokenStream {
    let ts = try_ts!(load_internal(input));
    quote! { #ts }
}

fn load_internal(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    let ident = &input.ident;

    let mut clauses = String::new();
    let mut errors = Vec::new();
    let mut fields = Vec::new();
    let mut params = Vec::new();
    let mut sql = String::new();
    let mut types = Vec::new();
    let mut param_index = 0;

    for (index, field) in input.fields()?.iter().enumerate() {
        let column = continue_ts!(field.column(), errors);
        sql.add_sep(',').add_field(&column);

        clauses.add_sep_str(" AND ").add_str(&format!(
            r#"COALESCE(${}, "{}")="{}""#,
            index + 1,
            column,
            column,
        ));

        let index = LitInt::new(&index.to_string(), field.span());

        if field.is_key() {
            let param = LitInt::new(&param_index.to_string(), field.span());
            params.push(quote! { &p.#param as _ });

            param_index += 1;
        }

        let m = field.ident()?;

        fields.push(quote! { #m: row.get(#index) });
        types.push(&field.ty);
    }

    errors.result()?;

    if !clauses.is_empty() {
        clauses.insert_str(0, " WHERE ");
    }

    sql.insert_str(0, "SELECT ");
    sql.push_str(" FROM ");
    sql.push_str(&input.table()?);
    sql.push_str(&clauses);

    let sql = LitStr::new(&sql, ident.span());
    let fields = quote! { #(#fields,)* };
    let params = quote! { [#(#params,)*] };
    let types = quote! { (#(Option<#types>,)*) };
    let (impl_generics, ty_generics, where_clause) = &input.generics.split_for_impl();

    Ok(quote! {
        #[storm::async_trait::async_trait]
        impl #impl_generics storm_postgres::Load<#types> for #ident #ty_generics #where_clause {
            async fn load<C>(client: &C, p: &#types) -> storm_postgres::Result<Vec<Self>>
            where
                C: storm_postgres::Query + Send + Sync,
            {
                let p = #params;

                Ok(client
                    .query_rows(#sql, &p)
                    .await?
                    .into_iter()
                    .map(|row| Self { #fields })
                    .collect())
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

        insert_names.add_sep(',').add_field(&column);
        insert_values.add_sep(',').add_param(index + 1);

        if field.is_key() {
            keys.add_sep(',').add_field(&column);
        }

        updates
            .add_sep(',')
            .add_field(&column)
            .add('=')
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
        #[storm::async_trait::async_trait]
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

trait SqlStringExt: StringExt {
    fn add_field(&mut self, field: &str) -> &mut Self {
        self.add_str(field).add('\"').add_str(field).add('\"')
    }

    fn add_param(&mut self, index: usize) -> &mut Self {
        self.add('$').add_str(&index.to_string())
    }
}

impl<T> SqlStringExt for T where T: StringExt {}
