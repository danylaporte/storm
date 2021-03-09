mod attrs;
mod builders;
mod save_translated;

use crate::{
    token_stream_ext::TokenStreamExt, DeriveInputExt, Errors, FieldExt, RenameAll, StringExt,
};
use attrs::{FieldAttrs, TypeAttrs};
use darling::{FromDeriveInput, FromField};
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt as _};
use save_translated::SaveTranslated;
use syn::{DeriveInput, Error, Field, Ident, LitInt, LitStr, Type};

pub(crate) fn load(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    let attrs = try_ts!(TypeAttrs::from_derive_input(input).map_err(|e| e.write_errors()));
    let rename_all = attrs.rename_all;

    let mut errors = Vec::new();
    let mut filter_sql = FilterSqlImpl::default();
    let mut load_keys_fields = LoadKeysFields::default();
    let mut translate_keys_fields = TranslatedKeysFields::default();

    for key in attrs.keys(&mut errors) {
        load_keys_fields.add_key(key);
        filter_sql.add_filter(key);
    }

    for (translated_key, key) in attrs
        .translate_keys(&mut errors)
        .iter()
        .zip(attrs.keys_internal())
    {
        translate_keys_fields.add_key(translated_key, key);
    }

    for field in try_ts!(input.fields()) {
        continue_ts!(field.ident(), errors);

        let attrs = continue_ts!(
            FieldAttrs::from_field(field).map_err(|e| e.write_errors()),
            errors
        );

        attrs.validate_load(&mut errors);

        if is_translated(&field.ty) {
            let column = continue_ts!(RenameAll::column(rename_all, &attrs.column, field), errors);

            load_keys_fields.skip_field(field);
            translate_keys_fields.add_field(field, &attrs, &column);
        } else if attrs.skip_load() {
            load_keys_fields.skip_field(field);
        } else {
            let column = continue_ts!(RenameAll::column(rename_all, &attrs.column, field), errors);
            load_keys_fields.add_field(field, &attrs, &column);
        }
    }

    attrs.check_translated(!translate_keys_fields.is_empty(), &mut errors);

    try_ts!(errors.result());

    let load = load_keys_fields.to_tokens(ident, &attrs.table);
    let translated_where = translate_keys_fields.to_where_clause(&ident);
    let translated = translate_keys_fields.to_tokens(&ident, &attrs.translate_table, &attrs.table);

    quote! {
        #[async_trait::async_trait]
        impl<C, F, FILTER> storm::provider::LoadAll<#ident, FILTER, C> for storm_mssql::MssqlProvider<F>
        where
            C: Default + Extend<(<#ident as storm::Entity>::Key, #ident)> #translated_where + Send + 'static,
            F: storm_mssql::ClientFactory,
            FILTER: storm_mssql::FilterSql,
        {
            async fn load_all(&self, filter: &FILTER) -> storm::Result<C> {
                let (sql, params) = storm_mssql::FilterSql::filter_sql(filter, 0);
                #load
                #translated
                Ok(map)
            }
        }

        #[async_trait::async_trait]
        impl<F> storm::provider::LoadOne<#ident> for storm_mssql::MssqlProvider<F>
        where
            F: storm_mssql::ClientFactory,
        {
            async fn load_one(&self, k: &<#ident as Entity>::Key) -> storm::Result<Option<#ident>> {
                let filter = #filter_sql;
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
    let mut translated = SaveTranslated::new(&attrs);

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

        if is_translated(&field.ty) {
            translated.add_field(field, column);
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
    let table = LitStr::new(&attrs.table, ident.span());

    quote! {
        #[async_trait::async_trait]
        impl<'a, F> storm::provider::Upsert<#ident> for storm_mssql::MssqlTransaction<'a, F>
        where
            F: Send + Sync,
        {
            async fn upsert(&self, k: &<#ident as storm::Entity>::Key, v: &#ident) -> storm::Result<()> {
                let mut builder = storm_mssql::UpsertBuilder::new(#table);

                storm_mssql::SaveEntityPart::save_entity_part(v, k, &mut builder);

                #wheres

                builder.execute(self).await?;

                #translated

                Ok(())
            }
        }

        impl storm_mssql::SaveEntityPart for #ident {
            fn save_entity_part<'a>(&'a self, k: &'a Self::Key, builder: &mut storm_mssql::UpsertBuilder<'a>) {
                #save_part
            }
        }
    }
}

fn is_translated(t: &Type) -> bool {
    match t {
        Type::Path(p) => p
            .path
            .segments
            .iter()
            .last()
            .map_or(false, |s| &s.ident == "Translated"),
        _ => false,
    }
}

/// Creates a where clauses and parameters for the load sql query.
#[derive(Default)]
struct FilterSqlImpl {
    params: Vec<TokenStream>,
    sql: String,
}

impl FilterSqlImpl {
    fn add_filter(&mut self, key: &str) {
        self.sql
            .add_sep_str("AND")
            .add_str("(t.[")
            .add_str(key)
            .add_str("]=@p")
            .add_str(&(self.params.len() + 1).to_string())
            .add(')');

        let index = LitInt::new(&self.params.len().to_string(), Span::call_site());
        self.params.push(quote!(&k.#index as _,));
    }
}

impl ToTokens for FilterSqlImpl {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let params = &self.params;

        let params = if params.len() == 1 {
            quote!(k as _)
        } else {
            quote!(#(#params)*)
        };

        let sql = LitStr::new(&self.sql, Span::call_site());
        tokens.append_all(quote!((#sql, &[#params][..])));
    }
}

/// creates a select sql query for each field and keys and generate the
/// loading code
#[derive(Default)]
struct LoadKeysFields {
    count: usize,
    fields: Vec<TokenStream>,
    keys: Vec<TokenStream>,
    select: String,
}

impl LoadKeysFields {
    fn add_field(&mut self, field: &Field, attrs: &FieldAttrs, column: &str) {
        let ident = &field.ident;
        let load = attrs.load_row(self.count);

        self.fields.push(quote!(#ident: #load,));

        self.select
            .add_sep(',')
            .add_str("t.[")
            .add_str(column)
            .add(']');

        self.count += 1;
    }

    fn add_key(&mut self, key: &str) {
        let l = LitInt::new(&self.count.to_string(), Span::call_site());

        self.select
            .add_sep(',')
            .add_str("t.[")
            .add_str(key)
            .add(']');

        self.keys.push(
            quote!(storm_mssql::FromSql::from_sql(row.try_get(#l).map_err(storm::Error::Mssql)?)?),
        );

        self.count += 1;
    }

    fn skip_field(&mut self, field: &Field) {
        let ident = &field.ident;
        self.fields.push(quote!(#ident: Default::default(),));
    }

    fn to_tokens(&self, entity: &Ident, table: &str) -> TokenStream {
        let keys = &self.keys;

        let keys = match keys.len() {
            1 => quote!(#(#keys)*),
            _ => quote!(#(#keys,)*),
        };

        let fields = &self.fields;
        let fields = quote!(#(#fields)*);

        let sql = LitStr::new(
            &format!("SELECT {} FROM {} t", &self.select, table),
            Span::call_site(),
        );

        quote! {
            const SQL: &str = #sql;

            let load_sql = match sql.is_empty() {
                false => format!("{} WHERE {}", SQL, sql),
                true => SQL.to_string(),
            };

            let mut map: C = storm_mssql::QueryRows::query_rows(self, load_sql, &*params, |row| {
                Ok((
                    #keys,
                    #entity { #fields }
                ))
            }).await?;
        }
    }
}

/// creates a select sql query for each field and keys and generate the
/// loading code for translated fields.
#[derive(Default)]
struct TranslatedKeysFields {
    count: usize,
    fields: Vec<TokenStream>,
    joins: String,
    keys: Vec<TokenStream>,
    select: String,
}

impl TranslatedKeysFields {
    fn add_field(&mut self, field: &Field, attrs: &FieldAttrs, column: &str) {
        let ident = &field.ident;
        let load = attrs.load_row(self.count);

        self.fields.push(quote! {
            let v: &str = #load;
            val.#ident.set(culture, v);
        });

        self.select
            .add_sep(',')
            .add_str("a.[")
            .add_str(column)
            .add(']');

        self.count += 1;
    }

    fn add_key(&mut self, translate_key: &str, key: &str) {
        let l = LitInt::new(&self.count.to_string(), Span::call_site());

        self.keys.push(
            quote!(storm_mssql::FromSql::from_sql(row.try_get(#l).map_err(storm::Error::Mssql)?)?),
        );

        self.select
            .add_sep(',')
            .add_str("a.[")
            .add_str(translate_key)
            .add(']');

        self.joins
            .add_sep_str("AND")
            .add_str("(a.[")
            .add_str(translate_key)
            .add_str("]=t.[")
            .add_str(key)
            .add_str("])");

        self.count += 1;
    }

    fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    fn to_tokens(&self, entity: &Ident, translated_table: &str, main_table: &str) -> TokenStream {
        if self.fields.is_empty() {
            return quote!();
        }

        let keys = &self.keys;

        let keys = match keys.len() {
            1 => quote!(#(#keys)*),
            _ => quote!(#(#keys,)*),
        };

        let sql = LitStr::new(
            &format!(
                "SELECT {},a.[Culture] FROM {} a",
                &self.select, translated_table
            ),
            Span::call_site(),
        );

        let joins = LitStr::new(
            &format!(
                "{{}} INNER JOIN {} t ON {} WHERE {{}}",
                main_table, &self.joins
            ),
            Span::call_site(),
        );

        let fields = &self.fields;
        let fields = quote!(#(#fields)*);
        let culture = LitInt::new(&self.count.to_string(), Span::call_site());

        quote! {
            const TRANSLATED_SQL: &str = #sql;

            let translated_sql = match sql.is_empty() {
                false => format!(#joins, TRANSLATED_SQL, sql),
                true => TRANSLATED_SQL.to_string(),
            };

            let _: storm::provider::LoadDoNothing = storm_mssql::QueryRows::query_rows(self, translated_sql, &*params, |row| {
                let key: <#entity as storm::Entity>::Key = #keys;
                let culture = storm_mssql::FromSql::from_sql(row.try_get(#culture).map_err(storm::Error::Mssql)?)?;

                if let Some(val) = map.get_mut(&key) {
                    #fields
                }

                Ok(())
            }).await?;
        }
    }

    fn to_where_clause(&self, ident: &Ident) -> TokenStream {
        match self.fields.is_empty() {
            false => quote!(+ storm::GetMut<#ident>),
            true => quote!(),
        }
    }
}
