use crate::{
    token_stream_ext::TokenStreamExt, DeriveInputExt, Errors, FieldExt, RenameAll, StringExt,
};
use darling::{util::SpannedValue, FromDeriveInput, FromField};
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt as _};
use syn::{spanned::Spanned, DeriveInput, Error, Field, Ident, LitInt, LitStr, Type};

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
    fn load_row(&self, row_index: usize) -> TokenStream {
        match &self.load_with {
            Some(f) => quote!(#f(&row)?),
            None => {
                let l = LitInt::new(&row_index.to_string(), Span::call_site());
                quote!(storm_mssql::FromSql::from_sql(row.try_get(#l).map_err(storm::Error::Mssql)?)?)
            }
        }
    }

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

    #[darling(default)]
    translate_table: String,

    #[darling(default)]
    translate_keys: SpannedValue<String>,
}

impl TypeAttrs {
    fn check_translated(&self, has_translated_field: bool, errors: &mut Vec<TokenStream>) {
        if has_translated_field {
            if self.translate_table.is_empty() {
                errors.push(
                    Error::new(
                        self.translate_table.span(),
                        "Translated table must have a translated table name.",
                    )
                    .to_compile_error(),
                );
            }
        } else if !self.translate_table.is_empty() {
            errors.push(
                Error::new(self.translate_table.span(), "No translated field found.")
                    .to_compile_error(),
            );
        }
    }

    fn keys(&self, errors: &mut Vec<TokenStream>) -> Vec<&str> {
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

    fn translate_keys(&self, errors: &mut Vec<TokenStream>) -> Vec<&str> {
        if self.translate_table.is_empty() {
            return Vec::new();
        }

        let keys = self.keys_internal();

        if self.translate_keys.is_empty() {
            return keys;
        }

        let translate_keys = self.translate_keys_internal();

        if translate_keys.len() != keys.len() {
            errors.push(
                Error::new(
                    self.translate_keys.span(),
                    "translate_keys must have the same keys count.",
                )
                .to_compile_error(),
            );
        }

        translate_keys
    }

    fn translate_keys_internal(&self) -> Vec<&str> {
        self.translate_keys
            .split(',')
            .filter(|s| !s.is_empty())
            .collect()
    }
}

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
    let translated = translate_keys_fields.to_tokens(&attrs.translate_table, &attrs.table);

    quote! {
        #[async_trait::async_trait]
        impl<F, FILTER> storm::provider::LoadAll<#ident, FILTER> for storm_mssql::MssqlProvider<F>
        where
            F: storm_mssql::ClientFactory,
            FILTER: storm_mssql::FilterSql,
        {
            async fn load_all<C: Default + Extend<(<#ident as storm::Entity>::Key, #ident)> + Send>(&self, filter: &FILTER) -> storm::Result<C> {
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

trait SqlStringExt: StringExt {
    fn add_field(&mut self, field: &str) -> &mut Self {
        self.add('[').add_str(field).add(']')
    }
}

impl<T> SqlStringExt for T where T: StringExt {}

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
            .add('(')
            .add_field(key)
            .add_str("=@p")
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
        self.select.add_sep(',').add_field(&column);
        self.count += 1;
    }

    fn add_key(&mut self, key: &str) {
        let l = LitInt::new(&self.count.to_string(), Span::call_site());

        self.select.add_sep(',').add_field(key);

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
            &format!("SELECT {} FROM {}", &self.select, table),
            Span::call_site(),
        );

        quote! {
            const SQL: &str = #sql;

            let load_sql = match sql.is_empty() {
                false => format!("{} WHERE {}", SQL, sql),
                true => SQL.to_string(),
            };

            let mut map = storm_mssql::QueryRows::query_rows(self, load_sql, &*params, |row| {
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
    keys: Vec<TokenStream>,
    select: String,
    wheres: String,
}

impl TranslatedKeysFields {
    fn add_field(&mut self, field: &Field, attrs: &FieldAttrs, column: &str) {
        let ident = &field.ident;
        let load = attrs.load_row(self.count);

        self.fields.push(quote!(#ident = #load;));

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

        self.wheres
            .add_sep_str("AND")
            .add_str("(a.[")
            .add_str(translate_key)
            .add_str("]=b.[")
            .add_str(key)
            .add_str("])");

        self.count += 1;
    }

    fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    fn to_tokens(&self, translated_table: &str, main_table: &str) -> TokenStream {
        if self.fields.is_empty() {
            return quote!();
        }

        let keys = &self.keys;

        let keys = match keys.len() {
            1 => quote!(#(#keys)*),
            _ => quote!(#(#keys,)*),
        };

        let sql = LitStr::new(
            &format!("SELECT {} FROM {} a", &self.select, translated_table),
            Span::call_site(),
        );

        let wheres = LitStr::new(
            &format!("{{}} INNER JOIN {} b WHERE {}", main_table, &self.wheres),
            Span::call_site(),
        );

        let fields = &self.fields;
        let fields = quote!(#(#fields)*);

        quote! {
            const TRANSLATED_SQL: &str = #sql;

            let translated_sql = match sql.is_empty() {
                false => format!(#wheres, TRANSLATED_SQL, sql),
                true => TRANSLATED_SQL.to_string(),
            };

            storm_mssql::QueryRows::query_rows(self, translated_sql, &*params, |row| {
                let key = #keys;

                if let Some(val) = map.get_mut(&key) {
                    #fields
                }
            }).await?;
        }
    }
}
