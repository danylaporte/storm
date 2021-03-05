use crate::{
    token_stream_ext::TokenStreamExt, DeriveInputExt, Errors, FieldExt, RenameAll, StringExt,
};
use darling::{util::SpannedValue, FromDeriveInput, FromField, FromMeta};
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

    for key in attrs.keys(&mut errors) {
        load_keys_fields.add_key(key);
        filter_sql.add_filter(key);
    }

    for field in try_ts!(input.fields()) {
        continue_ts!(field.ident(), errors);

        let attrs = continue_ts!(
            FieldAttrs::from_field(field).map_err(|e| e.write_errors()),
            errors
        );

        attrs.validate_load(&mut errors);

        let column = continue_ts!(RenameAll::column(rename_all, &attrs.column, field), errors);

        load_keys_fields.add_field(field, &attrs, &column);
    }

    //attrs.check_translated(!load_translated_field.is_empty(), &mut errors);

    try_ts!(errors.result());

    load_keys_fields.finish_with_table(&attrs.table);

    let row_fold_ts = load_keys_fields.row_fold_ts(ident);
    let load_sql = load_keys_fields.sql_ts();

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

                storm_mssql::QueryRows::query_rows(self, sql, &*params, #row_fold_ts).await
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

#[derive(Default)]
struct LoadKeysFields {
    count: usize,
    fields: Vec<TokenStream>,
    keys: Vec<TokenStream>,
    sql: String,
}

impl LoadKeysFields {
    fn add_field(&mut self, field: &Field, attrs: &FieldAttrs, column: &str) {
        let ident = &field.ident;

        if attrs.skip_load() {
            self.fields.push(quote!(#ident: Default::default(),));
            return;
        }

        let lit = LitInt::new(&self.count.to_string(), field.span());

        self.fields.push(match attrs.load_with.as_ref() {
                Some(f) => quote!(#ident: #f(&row)?,),
                None => quote!(#ident: storm_mssql::FromSql::from_sql(row.try_get(#lit).map_err(storm::Error::Mssql)?)?,)
            });

        self.sql.add_sep(',').add_field(&column);
        self.count += 1;
    }

    fn add_key(&mut self, key: &str) {
        let l = LitInt::new(&self.count.to_string(), Span::call_site());

        self.sql.add_sep(',').add_field(key);
        self.keys.push(
            quote!(storm_mssql::FromSql::from_sql(row.try_get(#l).map_err(storm::Error::Mssql)?)?),
        );
        self.count += 1;
    }

    fn finish_with_table(&mut self, table: &str) {
        self.sql.insert_str(0, "SELECT ");
        self.sql.push_str(" FROM ");
        self.sql.push_str(table);
    }

    fn row_fold_ts(&self, entity: &Ident) -> TokenStream {
        let keys = &self.keys;

        let keys = match keys.len() {
            1 => quote!(#(#keys)*),
            _ => quote!(#(#keys,)*),
        };

        let fields = &self.fields;
        let fields = quote!(#(#fields)*);

        quote! { |row| {
            Ok((
                #keys,
                #entity { #fields }
            ))
        }}
    }

    fn sql_ts(&self) -> TokenStream {
        let sql = LitStr::new(&self.sql, Span::call_site());
        quote!(#sql)
    }
}
