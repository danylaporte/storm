use crate::{token_stream_ext::TokenStreamExt, DeriveInputExt, Errors, FieldExt, StringExt};
use darling::{FromDeriveInput, FromField, FromMeta};
use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, DeriveInput, Error, Field, Ident, LitInt, LitStr};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(storm))]
pub struct StormTypeAttrs {
    ident: Ident,
    attrs: Vec<syn::Attribute>,
    table: String,
    keys: String,

    #[darling(default)]
    rename_all: Option<StormFieldRenameAll>,
}

impl StormTypeAttrs {
    pub fn keys(&self, errors: &mut Vec<TokenStream>) -> Vec<&str> {
        let vec = self.keys_internal();

        if vec.is_empty() {
            errors.push(
                Error::new(self.keys.span(), "Must specify at least one key.").to_compile_error(),
            );
        }

        vec
    }

    fn keys_internal(&self) -> Vec<&str> {
        self.keys.split(',').filter(|s| !s.is_empty()).collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, FromMeta, PartialEq)]
pub enum StormFieldRenameAll {
    #[darling(rename = "PascalCase")]
    PascalCase,

    #[darling(rename = "camelCase")]
    CamelCase,

    #[darling(rename = "snake_case")]
    SnakeCase,
}

impl StormFieldRenameAll {
    pub fn rename(&self, s: String) -> String {
        match self {
            Self::CamelCase => s.to_camel_case(),
            Self::PascalCase => s.to_pascal_case(),
            Self::SnakeCase => s.to_snake_case(),
        }
    }
}

#[derive(Debug, FromField)]
#[darling(attributes(storm))]
pub struct StormFieldAttrs {
    ident: Option<Ident>,

    #[darling(default)]
    column: Option<String>,
}

impl StormFieldAttrs {
    pub fn column(
        &self,
        field: &Field,
        rename_all: Option<StormFieldRenameAll>,
    ) -> Result<String, TokenStream> {
        if let Some(c) = self.column.as_ref().filter(|c| !c.is_empty()) {
            return Ok(c.clone());
        }

        let s = self
            .ident
            .as_ref()
            .ok_or_else(|| syn::Error::new(field.span(), "Ident expected.").to_compile_error())?
            .to_string();

        Ok(match rename_all {
            Some(r) => r.rename(s),
            None => s,
        })
    }
}

#[derive(Debug, FromMeta)]
pub struct Key(String);

pub(crate) fn load(input: &DeriveInput) -> TokenStream {
    let ts = try_ts!(load_internal(input));
    quote! { #ts }
}

fn load_internal(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    let ident = &input.ident;

    let attrs: StormTypeAttrs =
        FromDeriveInput::from_derive_input(input).map_err(|e| e.write_errors())?;

    let mut errors = Vec::new();
    let mut params_one = Vec::new();
    let mut select_all_field = Vec::new();
    let mut select_all_index = 0;
    let mut select_all_sql = String::new();
    let mut select_key = Vec::new();
    let mut select_one_field = Vec::new();
    let mut select_one_index = 0;
    let mut select_one_sql = String::new();
    let mut where_one_sql = String::new();

    let keys = attrs.keys(&mut errors);
    let rename_all = attrs.rename_all;

    for key in &keys {
        let select_all_index_str = select_all_index.to_string();
        let lit = LitInt::new(&select_all_index_str, ident.span());

        select_all_sql.add_sep(',').add_field(key);

        where_one_sql
            .add_sep_str(" AND ")
            .add_field(key)
            .add_str("=@p")
            .add_str(&select_all_index_str);

        select_key.push(quote! {
            storm_mssql::FromSql::from_sql(row.try_get(#lit).map_err(storm::Error::Mssql)?)?
        });

        if keys.len() == 1 {
            params_one.push(quote!(k));
        } else {
            let key = Ident::new(&select_all_index_str, ident.span());
            params_one.push(quote!(&k.#key,));
        }

        select_all_index += 1;
    }

    for field in input.fields()? {
        let attrs: StormFieldAttrs = continue_ts!(
            StormFieldAttrs::from_field(field).map_err(|e| e.write_errors()),
            errors
        );

        let column = continue_ts!(attrs.column(field, rename_all), errors);
        let ident = continue_ts!(field.ident(), errors);

        select_all_sql.add_sep(',').add_field(&column);
        select_one_sql.add_sep(',').add_field(&column);

        let lit_all = LitInt::new(&select_all_index.to_string(), field.span());
        let lit_one = LitInt::new(&select_one_index.to_string(), field.span());

        select_all_field.push(quote!(#ident: storm_mssql::FromSql::from_sql(row.try_get(#lit_all).map_err(storm::Error::Mssql)?)?,));
        select_one_field.push(quote!(#ident: storm_mssql::FromSql::from_sql(row.try_get(#lit_one).map_err(storm::Error::Mssql)?)?,));

        select_all_index += 1;
        select_one_index += 1;
    }

    select_all_sql.insert_str(0, "SELECT ");
    select_all_sql.push_str(" FROM ");
    select_all_sql.push_str(&attrs.table);

    select_one_sql.insert_str(0, "SELEC ");
    select_one_sql.push_str(" FROM ");
    select_one_sql.push_str(&attrs.table);
    select_one_sql.push_str(" WHERE ");
    select_one_sql.push_str(&where_one_sql);

    let select_key_ts = match keys.len() {
        1 => quote!(#(#select_key)*),
        _ => quote!(#(#select_key,)*),
    };

    errors.result()?;

    let params_one = params_one.ts();
    let select_all_sql = LitStr::new(&select_all_sql, ident.span());
    let select_all_field = select_all_field.ts();
    let select_one_field = select_one_field.ts();

    Ok(quote! {
        #[async_trait::async_trait]
        impl<F> storm::provider::LoadOne<#ident> for storm_mssql::MssqlProvider<F>
        where
            F: storm_mssql::ClientFactory,
        {
            async fn load_one(&self, k: &<#ident as Entity>::Key) -> storm::Result<Option<#ident>> {
                let mut vec: Vec<_> = storm_mssql::QueryRows::query_rows(self, #select_one_sql.to_string(), &[#params_one], |row| Ok(#ident {
                    #select_one_field
                })).await?;

                Ok(vec.pop())
            }
        }

        #[async_trait::async_trait]
        impl<F> storm::provider::LoadAll<#ident> for storm_mssql::MssqlProvider<F>
        where
            F: storm_mssql::ClientFactory,
        {
            async fn load_all<C: Default + Extend<(<#ident as storm::Entity>::Key, #ident)> + Send>(&self) -> storm::Result<C> {
                storm_mssql::QueryRows::query_rows(self, #select_all_sql.to_string(), &[], |row| {
                    Ok((
                        #select_key_ts,
                        #ident { #select_all_field }
                    ))
                }).await
            }
        }
    })
}

pub(crate) fn save(input: &DeriveInput) -> TokenStream {
    let ts = try_ts!(save_internal(input));
    quote! { #ts }
}

fn save_internal(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    let ident = &input.ident;

    let attrs: StormTypeAttrs =
        FromDeriveInput::from_derive_input(input).map_err(|e| e.write_errors())?;

    let mut insert_field_sql = String::new();
    let mut insert_value_sql = String::new();
    let mut update_set_sql = String::new();
    let mut update_where_sql = String::new();
    let mut params = Vec::new();
    let mut errors = Vec::new();

    let keys = attrs.keys(&mut errors);
    let rename_all = attrs.rename_all;

    for field in input.fields()? {
        let attrs: StormFieldAttrs = continue_ts!(
            StormFieldAttrs::from_field(field).map_err(|e| e.write_errors()),
            errors
        );

        let column = &continue_ts!(attrs.column(field, rename_all), errors);
        let ident = continue_ts!(field.ident(), errors);

        // keys are processed at the end.
        if keys.contains(&column.as_str()) {
            continue;
        }

        let param_index = params.len() + 1;
        let param_index_str = &param_index.to_string();

        insert_field_sql.add_sep(',').add_field(column);

        insert_value_sql
            .add_sep(',')
            .add_str("@p")
            .add_str(param_index_str);

        update_set_sql
            .add_sep(',')
            .add_field(column)
            .add_str("=@p")
            .add_str(param_index_str);

        params.push(quote!(&v.#ident,));
    }

    for (index, key) in keys.iter().enumerate() {
        let param_index = params.len() + 1;
        let param_index_str = param_index.to_string();

        insert_field_sql.add_sep(',').add_field(key);

        insert_value_sql
            .add_sep(',')
            .add_str("@p")
            .add_str(&param_index_str);

        update_where_sql
            .add_sep_str(" AND ")
            .add_field(key)
            .add_str("=@p")
            .add_str(&param_index_str);

        if keys.len() == 1 {
            params.push(quote!(k,));
        } else {
            let ident = Ident::new(&(index + 1).to_string(), input.span());
            params.push(quote!(&k.#ident,));
        }
    }

    let params = params.ts();

    let sql = format!(
        "BEGIN TRY
            INSERT INTO {table} ({insert_field_sql}) VALUES ({insert_value_sql});
        END TRY
        BEGIN CATCH
            IF ERROR_NUMBER() IN (2601, 2627)
                UPDATE {table} SET {update_set_sql} WHERE {update_where_sql};
        END CATCH
        ",
        table = attrs.table,
        insert_field_sql = insert_field_sql,
        insert_value_sql = insert_value_sql,
        update_set_sql = update_set_sql,
        update_where_sql = update_where_sql,
    );

    let sql = LitStr::new(&sql, input.span());

    Ok(quote! {
        #[async_trait::async_trait]
        impl<'a, F> storm::provider::Upsert<#ident> for storm_mssql::MssqlTransaction<'a, F>
        where
            F: Send + Sync,
        {
            async fn upsert(&self, k: &<#ident as storm::Entity>::Key, v: &#ident) -> storm::Result<()> {
                storm_mssql::Execute::execute(self, #sql.to_string(), &[#params]).await.map(|_| ())
            }
        }
    })
}

trait SqlStringExt: StringExt {
    fn add_field(&mut self, field: &str) -> &mut Self {
        self.add('[').add_str(field).add(']')
    }
}

impl<T> SqlStringExt for T where T: StringExt {}
