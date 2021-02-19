use crate::{
    token_stream_ext::TokenStreamExt, DeriveInputExt, Errors, FieldExt, RenameAll, StringExt,
};
use darling::{util::SpannedValue, FromDeriveInput, FromField, FromMeta};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, DeriveInput, Error, Ident, LitInt, LitStr};

const SKIP_IS_INCOMPATIBLE: &str = "`skip` is incompatible.";

#[derive(Debug, FromField)]
#[darling(attributes(storm))]
struct FieldAttrs {
    #[darling(default)]
    column: Option<String>,

    #[darling(default)]
    load_with: Option<Ident>,

    #[darling(default)]
    part: bool,

    #[darling(default)]
    save_with: SpannedValue<Option<Ident>>,

    #[darling(default)]
    skip: SpannedValue<Option<bool>>,

    #[darling(default)]
    skip_load: SpannedValue<Option<bool>>,

    #[darling(default)]
    skip_save: SpannedValue<Option<bool>>,
}

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

        if self.part && self.save_with.is_some() {
            errors.push(
                Error::new(self.save_with.span(), "Ignored on part field.").to_compile_error(),
            );
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
    let mut save_part = Vec::new();
    let mut wheres = Vec::new();

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
        let name = LitStr::new(&column, ident.span());

        save_part.push(if attrs.part {
            quote!(storm_mssql::SaveEntityPart::save_entity_part(&self.#ident, k, builder);)
        } else {
            match attrs.save_with.as_ref() {
                Some(f) => quote!(builder.add_field_owned(#name, #f(k, self));),
                None => quote!(builder.add_field_ref(#name, &self.#ident);),
            }
        });
    }

    for (index, key) in keys.iter().enumerate() {
        let name = LitStr::new(key, ident.span());

        let k = match keys.len() > 1 {
            true => {
                let n = LitInt::new(&index.to_string(), ident.span());
                quote! { &k.#n }
            }
            false => quote! { k },
        };

        wheres.push(quote!(builder.add_key_ref(#name, #k);));
    }

    try_ts!(errors.result());

    let save_part = save_part.ts();
    let wheres = wheres.ts();

    //let sql = LitStr::new(&sql, input.span());
    let table = LitStr::new(&attrs.table, ident.span());

    quote! {
        #[async_trait::async_trait]
        impl<'a, F> storm::provider::Upsert<#ident> for storm_mssql::MssqlTransaction<'a, F>
        where
            F: Send + Sync,
        {
            async fn upsert(&self, k: &<#ident as storm::Entity>::Key, v: &#ident) -> storm::Result<()> {
                //storm_mssql::Execute::execute(self, #sql.to_string(), &[#params]).await.map(|_| ())

                let mut builder = storm_mssql::UpsertBuilder::new(#table);

                storm_mssql::SaveEntityPart::save_entity_part(v, k, &mut builder);

                #wheres

                builder.execute(self).await
            }
        }

        impl storm_mssql::SaveEntityPart for #ident {
            fn save_entity_part<'a>(&'a self, k: &'a Self::Key, builder: &mut storm_mssql::UpsertBuilder<'a>) {
                #save_part
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
