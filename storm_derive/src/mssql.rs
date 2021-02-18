use crate::{
    token_stream_ext::TokenStreamExt, DeriveInputExt, Errors, FieldExt, RenameAll, StringExt,
};
use darling::{util::SpannedValue, FromDeriveInput, FromField, FromMeta};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, DeriveInput, Error, Ident, LitInt, LitStr};

#[derive(Debug, FromField)]
#[darling(attributes(storm))]
struct FieldAttrs {
    #[darling(default)]
    column: Option<String>,

    #[darling(default)]
    load_with: Option<Ident>,

    #[darling(default)]
    save_with: SpannedValue<Option<Ident>>,

    #[darling(default)]
    skip: SpannedValue<Option<bool>>,

    #[darling(default)]
    skip_load: SpannedValue<Option<bool>>,

    #[darling(default)]
    skip_save: SpannedValue<Option<bool>>,
}

const SKIP_IS_INCOMPATIBLE: &str = "`skip` is incompatible.";

impl FieldAttrs {
    fn skip_load(&self) -> bool {
        self.skip_load.unwrap_or_default() || self.skip.unwrap_or_default()
    }

    fn skip_save(&self) -> bool {
        self.skip_save.unwrap_or_default() || self.skip.unwrap_or_default()
    }

    fn validate_load(&self, errors: &mut Vec<TokenStream>) {
        if let (Some(true), Some(false)) = (*self.skip, *self.skip_load) {
            errors.push(Error::new(self.skip_load.span(), SKIP_IS_INCOMPATIBLE).to_compile_error());
        }
    }

    fn validate_save(&self, errors: &mut Vec<TokenStream>) {
        if let (Some(true), Some(false)) = (*self.skip, *self.skip_save) {
            errors.push(Error::new(self.skip_save.span(), SKIP_IS_INCOMPATIBLE).to_compile_error());
        }

        if self.skip_save() && self.save_with.is_some() {
            errors.push(Error::new(self.save_with.span(), "Save is skipped.").to_compile_error());
        }
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(storm))]
struct TypeAttrs {
    ident: Ident,
    attrs: Vec<syn::Attribute>,
    table: String,
    keys: String,

    #[darling(default)]
    rename_all: Option<RenameAll>,
}

impl TypeAttrs {
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

#[derive(Debug, FromMeta)]
pub struct Key(String);

pub(crate) fn load(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    let attrs = try_ts!(TypeAttrs::from_derive_input(input).map_err(|e| e.write_errors()));
    let rename_all = attrs.rename_all;

    let mut col_index = 0;
    let mut errors = Vec::new();
    let mut filter_params = Vec::new();
    let mut filter_sql = String::new();
    let mut load_field = Vec::new();
    let mut load_key = Vec::new();
    let mut load_sql = String::new();

    let keys = attrs.keys(&mut errors);

    for key in &keys {
        let col_index_str = &col_index.to_string();

        load_sql.add_sep(',').add_field(key);

        load_key.push({
            let lit = LitInt::new(col_index_str, ident.span());
            quote!(storm_mssql::FromSql::from_sql(row.try_get(#lit).map_err(storm::Error::Mssql)?)?)
        });

        filter_sql
            .add_sep_str("AND")
            .add('(')
            .add_field(key)
            .add_str("=@p")
            .add_str(&(filter_params.len() + 1).to_string())
            .add(')');

        if keys.len() == 1 {
            filter_params.push(quote!(k as _));
        } else {
            let key = Ident::new(col_index_str, ident.span());
            filter_params.push(quote!(&k.#key as _,));
        }

        col_index += 1;
    }

    for field in try_ts!(input.fields()) {
        let attrs: FieldAttrs = continue_ts!(
            FieldAttrs::from_field(field).map_err(|e| e.write_errors()),
            errors
        );

        attrs.validate_load(&mut errors);

        if attrs.skip_load() {
            continue;
        }

        let column = continue_ts!(RenameAll::column(rename_all, &attrs.column, field), errors);

        load_sql.add_sep(',').add_field(&column);

        let ident = continue_ts!(field.ident(), errors);
        let lit = LitInt::new(&col_index.to_string(), field.span());

        load_field.push(match attrs.load_with.as_ref() {
            Some(f) => quote!(#ident: #f(&row)?,),
            None => quote!(#ident: storm_mssql::FromSql::from_sql(row.try_get(#lit).map_err(storm::Error::Mssql)?)?,)
        });

        col_index += 1;
    }

    try_ts!(errors.result());

    load_sql.insert_str(0, "SELECT ");
    load_sql.push_str(" FROM ");
    load_sql.push_str(&attrs.table);

    let load_key = match keys.len() {
        1 => quote!(#(#load_key)*),
        _ => quote!(#(#load_key,)*),
    };

    let filter_params = filter_params.ts();
    let filter_sql = LitStr::new(&filter_sql, ident.span());
    let load_field = load_field.ts();
    let load_sql = LitStr::new(&load_sql, ident.span());

    quote! {
        #[async_trait::async_trait]
        impl<F, FILTER> storm::provider::LoadAll<#ident, FILTER> for storm_mssql::MssqlProvider<F>
        where
            F: storm_mssql::ClientFactory,
            FILTER: storm_mssql::FilterSql,
        {
            async fn load_all<C: Default + Extend<(<#ident as storm::Entity>::Key, #ident)> + Send>(&self, filter: &FILTER) -> storm::Result<C> {
                const SQL: &str = #load_sql;

                let (sql, params) = storm_mssql::FilterSql::filter_sql(filter, 0);

                let sql = match sql.is_empty() {
                    false => format!("{} WHERE {}", SQL, sql),
                    true => SQL.to_string(),
                };

                storm_mssql::QueryRows::query_rows(self, sql, &*params, |row| {
                    Ok((
                        #load_key,
                        #ident { #load_field }
                    ))
                }).await
            }
        }

        #[async_trait::async_trait]
        impl<F> storm::provider::LoadOne<#ident> for storm_mssql::MssqlProvider<F>
        where
            F: storm_mssql::ClientFactory,
        {
            async fn load_one(&self, k: &<#ident as Entity>::Key) -> storm::Result<Option<#ident>> {
                let filter = (#filter_sql, &[#filter_params][..]);
                let v: storm::provider::LoadOneInternal<#ident> = storm::provider::LoadAll::load_all(self, &filter).await?;
                Ok(v.into_inner())
            }
        }
    }
}

pub(crate) fn save(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    let attrs = try_ts!(TypeAttrs::from_derive_input(input).map_err(|e| e.write_errors()));
    let rename_all = attrs.rename_all;

    let mut errors = Vec::new();
    let mut insert_field = String::new();
    let mut insert_value = String::new();
    let mut params = Vec::new();
    let mut update_set = String::new();
    let mut update_where = String::new();

    let keys = attrs.keys(&mut errors);

    for field in try_ts!(input.fields()) {
        let attrs: FieldAttrs = continue_ts!(
            FieldAttrs::from_field(field).map_err(|e| e.write_errors()),
            errors
        );

        attrs.validate_save(&mut errors);

        if attrs.skip_save() {
            continue;
        }

        let column = &continue_ts!(RenameAll::column(rename_all, &attrs.column, field), errors);

        // keys are processed at the end.
        if keys.contains(&column.as_str()) {
            if attrs.save_with.is_some() {
                errors.push(
                    Error::new(attrs.save_with.span(), "Invalid since this field is a key.")
                        .to_compile_error(),
                );
            }
            continue;
        }

        let ident = continue_ts!(field.ident(), errors);
        let param = &(params.len() + 1).to_string();

        insert_field.add_sep(',').add_field(column);
        insert_value.add_sep(',').add_str("@p").add_str(param);

        update_set
            .add_sep(',')
            .add_field(column)
            .add_str("=@p")
            .add_str(param);

        params.push(match attrs.save_with.as_ref() {
            Some(f) => quote!(#f(k, v),),
            None => quote!(&v.#ident,),
        });
    }

    for key in &keys {
        let param = &(params.len() + 1).to_string();

        insert_field.add_sep(',').add_field(key);
        insert_value.add_sep(',').add_str("@p").add_str(param);

        update_where
            .add_sep_str(" AND ")
            .add_field(key)
            .add_str("=@p")
            .add_str(param);

        if keys.len() == 1 {
            params.push(quote!(k,));
        } else {
            let ident = Ident::new(param, input.span());
            params.push(quote!(&k.#ident,));
        }
    }

    try_ts!(errors.result());

    let params = params.ts();

    let sql = format!(
        "BEGIN TRY
            INSERT INTO {table} ({insert_field}) VALUES ({insert_value});
        END TRY
        BEGIN CATCH
            IF ERROR_NUMBER() IN (2601, 2627)
                UPDATE {table} SET {update_set} WHERE {update_where};
        END CATCH
        ",
        table = attrs.table,
        insert_field = insert_field,
        insert_value = insert_value,
        update_set = update_set,
        update_where = update_where,
    );

    let sql = LitStr::new(&sql, input.span());

    quote! {
        #[async_trait::async_trait]
        impl<'a, F> storm::provider::Upsert<#ident> for storm_mssql::MssqlTransaction<'a, F>
        where
            F: Send + Sync,
        {
            async fn upsert(&self, k: &<#ident as storm::Entity>::Key, v: &#ident) -> storm::Result<()> {
                storm_mssql::Execute::execute(self, #sql.to_string(), &[#params]).await.map(|_| ())
            }
        }
    }
}

trait SqlStringExt: StringExt {
    fn add_field(&mut self, field: &str) -> &mut Self {
        self.add('[').add_str(field).add(']')
    }
}

impl<T> SqlStringExt for T where T: StringExt {}
