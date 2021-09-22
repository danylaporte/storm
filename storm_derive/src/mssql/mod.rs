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
    let table_name = LitStr::new(&attrs.table, attrs.table.span());

    let provider = attrs.provider();
    let (metrics_start, metrics_end) = metrics(ident, "delete");

    quote! {
        impl storm::provider::Delete<#ident> for storm::provider::TransactionProvider<'_> {
            #[tracing::instrument(name = "delete", level = "debug", skip(self, k), fields(table = #table_name))]
            fn delete<'a>(&'a self, k: &'a <#ident as storm::Entity>::Key) -> storm::BoxFuture<'a, storm::Result<()>> {
                Box::pin(async move {
                    let provider: &storm_mssql::MssqlProvider = self.container().provide(#provider).await?;

                    #metrics_start

                    #translate
                    #normal

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

    let mut diff = attrs.diff.then(Vec::new);
    let mut errors = Vec::new();
    let mut filter_sql = FilterSqlImpl::default();
    let mut load = LoadFields::new(ident, &attrs);
    let mut translated = LoadTranslated::new(ident, &attrs);
    let table_name = LitStr::new(&attrs.table, attrs.table.span());

    for key in attrs.keys(&mut errors) {
        filter_sql.add_filter(key);
    }

    for field in try_ts!(input.fields()) {
        let field_ident = continue_ts!(field.ident(), errors);

        let attrs = continue_ts!(
            FieldAttrs::from_field(field).map_err(|e| e.write_errors()),
            errors
        );

        attrs.validate_load(&mut errors);

        if is_translated(&field.ty) {
            let column = continue_ts!(RenameAll::column(rename_all, &attrs.column, field), errors);

            load.skip_field(field, &attrs, &mut errors);
            translated.add_field(field, &column);

            if !attrs.skip_diff() {
                load_diff_field(&mut diff, field_ident, &column);
            }
        } else if attrs.skip_load() {
            load.skip_field(field, &attrs, &mut errors);
        } else {
            let column = continue_ts!(RenameAll::column(rename_all, &attrs.column, field), errors);
            load.add_field(field, &attrs, &column);

            if !attrs.skip_diff() {
                load_diff_field(&mut diff, field_ident, &column);
            }
        }
    }

    try_ts!(errors.result());

    let translated_where = translated.to_where_clause();
    let provider = attrs.provider();
    let (metrics_start, metrics_end) = metrics(ident, "load");
    let diff = apply_entity_diff(diff, ident);
    let test;

    if attrs.no_test {
        test = quote!();
    } else {
        test = quote! {
            #[cfg(test)]
            #[tokio::test]
            async fn #load_test() -> storm::Result<()> {
                let provider = storm_mssql::create_provider_container_from_env("DB", #provider)?;
                storm::provider::LoadAll::<#ident, _, storm::provider::LoadNothing>::load_all(&provider, &()).await?;
                Ok(())
            }
        };
    }

    quote! {
        impl<C, FILTER> storm::provider::LoadAll<#ident, FILTER, C> for storm::provider::ProviderContainer
        where
            C: Default + Extend<(<#ident as storm::Entity>::Key, #ident)> #translated_where + Send + 'static,
            FILTER: storm_mssql::FilterSql,
        {
            #[tracing::instrument(name = "load_all", level = "debug", skip(self, filter), fields(table = #table_name))]
            fn load_all_with_args<'a>(&'a self, filter: &'a FILTER, args: storm::provider::LoadArgs) -> storm::BoxFuture<'a, storm::Result<C>> {
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
            #[tracing::instrument(name = "load_one", level = "debug", skip(self, k), fields(table = #table_name))]
            fn load_one_with_args<'a>(&'a self, k: &'a <#ident as Entity>::Key, args: storm::provider::LoadArgs) -> storm::BoxFuture<'a, storm::Result<Option<#ident>>> {
                Box::pin(async move {
                    let filter = #filter_sql;
                    let v: storm::provider::LoadOneInternal<#ident> = storm::provider::LoadAll::load_all_with_args(self, &filter, args).await?;
                    Ok(v.into_inner())
                })
            }
        }

        #diff
        #test
    }
}

pub(crate) fn save(input: &DeriveInput) -> TokenStream {
    let ident = &input.ident;
    let attrs = try_ts!(TypeAttrs::from_derive_input(input).map_err(|e| e.write_errors()));
    let rename_all = attrs.rename_all;

    let mut diff = attrs.diff.then(Vec::new);
    let mut errors = Vec::new();
    let mut save_part = Vec::new();
    let mut wheres = Vec::new();
    let mut translated = SaveTranslated::new(&attrs);
    let mut translated_backup = Vec::new();
    let mut translated_restore = Vec::new();
    let is_identity_key = attrs.is_identity_key();
    let identity_col = attrs.identity.to_lowercase();
    let table_name = LitStr::new(&attrs.table, attrs.table.span());

    let keys = attrs.keys(&mut errors);
    let mut identity_found = is_identity_key;

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
        let col_lc = column.to_lowercase();
        let col_lit = LitStr::new(&col_lc, Span::call_site());
        let is_identity = !identity_col.is_empty() && identity_col == col_lc;

        if is_identity {
            identity_found = true;
            continue;
        }

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

        if is_translated(&field.ty) {
            translated.add_field(field, column);
            let name_bk = Ident::new(&format!("{}_bk", ident), Span::call_site());
            translated_backup.push(
                quote! { let #name_bk = std::mem::replace(&mut v.#ident, Default::default()); },
            );
            translated_restore.push(quote! { v.#ident = #name_bk; });

            if let Some(diff) = diff.as_mut().filter(|_| !attrs.skip_diff()) {
                diff.push(quote! {
                    if let Some(diff) = storm_mssql::FieldDiff::field_diff(&self.#ident, &old.#ident) {
                        map.insert(#col_lit, diff);
                    }
                });
            }

            continue;
        }

        let name = LitStr::new(&format!("[{}]", column), ident.span());

        if attrs.part {
            save_part.push(
                quote!(storm_mssql::SaveEntityPart::save_entity_part(&self.#ident, k, builder);),
            );

            if let Some(diff) = diff.as_mut().filter(|_| !attrs.skip_diff()) {
                diff.push(
                    quote! { storm_mssql::EntityDiff::entity_diff(&self.#ident, &old.#ident, map);},
                );
            }
        } else {
            match attrs.save_with.as_ref() {
                Some(f) => {
                    save_part.push(quote!(builder.add_field_owned(#name, #f(k, self));));

                    if let Some(diff) = diff.as_mut().filter(|_| !attrs.skip_diff()) {
                        diff.push(quote! {
                            if let Some(diff) = storm_mssql::FieldDiff::field_diff(&#f(k, self), &#f(k, old)) {
                                map.insert(#col_lit, diff);
                            }
                        });
                    }
                }
                None => {
                    save_part.push(quote!(builder.add_field_ref(#name, &self.#ident);));

                    if let Some(diff) = diff.as_mut().filter(|_| !attrs.skip_diff()) {
                        diff.push(quote! {
                            if let Some(diff) = storm_mssql::FieldDiff::field_diff(&self.#ident, &old.#ident) {
                                map.insert(#col_lit, diff);
                            }
                        });
                    }
                }
            }
        };
    }

    if !attrs.identity.is_empty() && !identity_found {
        errors.push(
            Error::new(attrs.identity.span(), "Identity field not found.").to_compile_error(),
        );
    }

    let add_key_or_identity = if is_identity_key {
        quote!(add_key_identity)
    } else {
        quote!(add_key_ref)
    };

    for (index, key) in keys.iter().enumerate() {
        let name = LitStr::new(&format!("[{}]", key), ident.span());

        let k = match keys.len() > 1 {
            true => {
                let n = LitInt::new(&index.to_string(), ident.span());
                quote! { &k.#n }
            }
            false if is_identity_key => quote! { *k },
            false => quote! { k },
        };

        wheres.push(quote!(builder.#add_key_or_identity(#name, #k);));
    }

    try_ts!(errors.result());

    let upsert_trait;
    let upsert_sig;
    let builder_invoke;
    let entity_part_key;
    let reload_entity;

    if attrs.reload_on_upsert_or_identity() {
        upsert_trait = quote!(storm::provider::UpsertMut<#ident>);
        upsert_sig = quote!(fn upsert_mut<'a>(&'a self, k: &'a mut <#ident as storm::Entity>::Key, v: &'a mut #ident) -> storm::BoxFuture<'a, storm::Result<()>>);
        entity_part_key = quote!(&k.clone());
    } else {
        upsert_trait = quote!(storm::provider::Upsert<#ident>);
        upsert_sig = quote!(fn upsert<'a>(&'a self, k: &'a <#ident as storm::Entity>::Key, v: &'a #ident) -> storm::BoxFuture<'a, storm::Result<()>>);
        entity_part_key = quote!(k);
    }

    if attrs.reload_on_upsert() {
        let backup = quote!(#(#translated_backup)*);
        let restore = quote!(#(#translated_restore)*);

        reload_entity = quote! {
            #backup
            *v = storm::provider::LoadOne::<#ident>::load_one_with_args(self.container(), &k, storm::provider::LoadArgs { use_transaction: true }).await?.ok_or(storm::Error::EntityNotFound)?;
            #restore
        };
    } else {
        reload_entity = quote!();
    }

    if is_identity_key {
        builder_invoke = quote!(builder.execute_identity(provider, k).await?;);
    } else {
        builder_invoke = quote!(builder.execute(provider).await?;);
    }

    let save_part = save_part.ts();
    let wheres = wheres.ts();
    let table = LitStr::new(&attrs.table, ident.span());
    let provider = attrs.provider();
    let (metrics_start, metrics_end) = metrics(ident, "upsert");
    let diff = entity_diff(ident, diff);

    quote! {
        impl #upsert_trait for storm::provider::TransactionProvider<'_> {
            #[tracing::instrument(name = "upsert", level = "debug", skip(self, k, v), fields(table = #table_name))]
            #upsert_sig {
                Box::pin(async move {
                    let provider: &storm_mssql::MssqlProvider = self.container().provide(#provider).await?;
                    let mut builder = storm_mssql::UpsertBuilder::new(#table);

                    #metrics_start

                    let entity_part_key = #entity_part_key;
                    storm_mssql::SaveEntityPart::save_entity_part(v, entity_part_key, &mut builder);

                    #wheres
                    #builder_invoke
                    #reload_entity
                    #translated

                    #metrics_end
                    Ok(())
                })
            }
        }

        #diff

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

fn apply_entity_diff(diff: Option<Vec<TokenStream>>, ident: &Ident) -> TokenStream {
    if let Some(diff) = diff {
        quote! {
            impl storm_mssql::ApplyEntityDiff for #ident {
                fn apply_entity_diff<S: std::hash::BuildHasher>(&mut self, diff: &mut std::collections::HashMap<String, storm_mssql::serde_json::Value, S>) -> storm_mssql::Result<()> {
                    #(#diff)*
                    Ok(())
                }
            }
        }
    } else {
        quote! {}
    }
}

fn entity_diff(ident: &Ident, diff: Option<Vec<TokenStream>>) -> TokenStream {
    if let Some(diff) = diff {
        quote! {
            impl storm_mssql::EntityDiff for #ident {
                fn entity_diff<S: std::hash::BuildHasher>(&self, old: &Self, map: &mut std::collections::HashMap<&'static str, storm_mssql::serde_json::Value, S>) {
                    #(#diff)*
                }
            }
        }
    } else {
        quote!()
    }
}

fn load_diff_field(diff: &mut Option<Vec<TokenStream>>, field: &Ident, column: &str) {
    if let Some(diff) = diff.as_mut() {
        let lit = LitStr::new(&column.to_lowercase(), Span::call_site());
        diff.push(quote! {storm_mssql::_replace_field_diff(&mut self.#field, #lit, diff)?;});
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
