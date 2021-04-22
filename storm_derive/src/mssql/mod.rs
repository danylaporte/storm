mod attrs;
mod builders;
mod delete;
mod load_fields;
mod load_translated;
mod save_translated;

use crate::{
    token_stream_ext::TokenStreamExt, DeriveInputExt, Errors, FieldExt, RenameAll, StringExt,
};
use attrs::{FieldAttrs, TypeAttrs};
use darling::{FromDeriveInput, FromField};
use delete::Delete;
use inflector::Inflector;
use load_fields::LoadFields;
use load_translated::LoadTranslated;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt as _};
use save_translated::SaveTranslated;
use syn::{DeriveInput, Error, Ident, LitInt, LitStr, Type};

pub(crate) fn delete(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    let attrs = try_ts!(TypeAttrs::from_derive_input(input).map_err(|e| e.write_errors()));
    let normal = Delete::<delete::selectors::Normal>::new(&attrs);
    let translate = Delete::<delete::selectors::Translate>::new(&attrs);

    let provider = attrs.provider();
    let (metrics_start, metrics_end) = metrics(&ident, "delete");

    quote! {
        impl storm::provider::Delete<#ident> for storm::provider::TransactionProvider<'_> {
            fn delete<'a>(&'a self, k: &'a <#ident as storm::Entity>::Key) -> storm::BoxFuture<'a, storm::Result<()>> {
                Box::pin(async move {
                    let provider: &storm_mssql::MssqlProvider = self.container().provide(#provider).await?;

                    #metrics_start

                    #normal
                    #translate

                    #metrics_end
                    Ok(())
                })
            }
        }
    }
}

pub(crate) fn load(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    let attrs = try_ts!(TypeAttrs::from_derive_input(input).map_err(|e| e.write_errors()));
    let rename_all = attrs.rename_all;
    let load_test = Ident::new(
        &format!("test_{}_load", ident.to_string().to_snake_case()),
        ident.span(),
    );

    let mut errors = Vec::new();
    let mut filter_sql = FilterSqlImpl::default();
    let mut load = LoadFields::new(ident, &attrs);
    let mut translated = LoadTranslated::new(ident, &attrs);

    for key in attrs.keys(&mut errors) {
        filter_sql.add_filter(key);
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

            load.skip_field(field, &attrs, &mut errors);
            translated.add_field(field, &column);
        } else if attrs.skip_load() {
            load.skip_field(field, &attrs, &mut errors);
        } else {
            let column = continue_ts!(RenameAll::column(rename_all, &attrs.column, field), errors);
            load.add_field(field, &attrs, &column);
        }
    }

    try_ts!(errors.result());

    let translated_where = translated.to_where_clause();
    let provider = attrs.provider();
    let (metrics_start, metrics_end) = metrics(&ident, "load");

    quote! {
        impl<C, FILTER> storm::provider::LoadAll<#ident, FILTER, C> for storm::provider::ProviderContainer
        where
            C: Default + Extend<(<#ident as storm::Entity>::Key, #ident)> #translated_where + Send + 'static,
            FILTER: storm_mssql::FilterSql,
        {
            fn load_all<'a>(&'a self, filter: &'a FILTER) -> storm::BoxFuture<'a, storm::Result<C>> {
                Box::pin(async move {
                    let provider: &storm_mssql::MssqlProvider = self.provide(#provider).await?;
                    let (sql, params) = storm_mssql::FilterSql::filter_sql(filter, 0);
                    #metrics_start
                    #load
                    #translated
                    #metrics_end
                    Ok(map)
                })
            }
        }

        impl storm::provider::LoadOne<#ident> for storm::provider::ProviderContainer {
            fn load_one<'a>(&'a self, k: &'a <#ident as Entity>::Key) -> storm::BoxFuture<'a, storm::Result<Option<#ident>>> {
                Box::pin(async move {
                    let filter = #filter_sql;
                    let v: storm::provider::LoadOneInternal<#ident> = storm::provider::LoadAll::load_all(self, &filter).await?;
                    Ok(v.into_inner())
                })
            }
        }

        #[cfg(test)]
        #[tokio::test]
        async fn #load_test() -> storm::Result<()> {
            let provider = storm_mssql::create_provider_container_from_env("DB", #provider)?;
            storm::provider::LoadAll::<#ident, _, storm::provider::LoadNothing>::load_all(&provider, &()).await?;
            Ok(())
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
    let provider = attrs.provider();
    let (metrics_start, metrics_end) = metrics(&ident, "upsert");

    quote! {
        impl storm::provider::Upsert<#ident> for storm::provider::TransactionProvider<'_> {
            fn upsert<'a>(&'a self, k: &'a <#ident as storm::Entity>::Key, v: &'a #ident) -> storm::BoxFuture<'a, storm::Result<()>> {
                Box::pin(async move {
                    let provider: &storm_mssql::MssqlProvider = self.container().provide(#provider).await?;
                    let mut builder = storm_mssql::UpsertBuilder::new(#table);

                    #metrics_start

                    storm_mssql::SaveEntityPart::save_entity_part(v, k, &mut builder);

                    #wheres

                    builder.execute(provider).await?;

                    #translated

                    #metrics_end
                    Ok(())
                })
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

#[cfg(not(feature = "metrics"))]
fn metrics(_ident: &Ident, _ops: &str) -> (TokenStream, TokenStream) {
    (quote!(), quote!())
}

#[cfg(feature = "metrics")]
fn metrics(ident: &Ident, ops: &str) -> (TokenStream, TokenStream) {
    let start = quote! {
        let instant = std::time::Instant::now();
    };

    let ops = LitStr::new(ops, Span::call_site());
    let typ = LitStr::new(&ident.to_string(), ident.span());

    let end = quote! {
        use storm::metrics;
        metrics::histogram!("storm.execution_time", instant.elapsed().as_secs_f64(), "operation" => #ops, "type" => #typ);
    };

    (start, end)
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

fn read_row(column_index: usize) -> TokenStream {
    let l = LitInt::new(&column_index.to_string(), Span::call_site());
    quote!(storm_mssql::FromSql::from_sql(row.try_get(#l).map_err(storm::Error::Mssql)?)?)
}

fn read_row_with(column_index: usize, attrs: &FieldAttrs) -> TokenStream {
    match attrs.load_with.as_ref() {
        Some(f) => quote!(#f(&row)?),
        None => read_row(column_index),
    }
}
