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
use syn::{DeriveInput, Error, Ident, LitInt, LitStr, Type, Visibility};

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
                    let provider: &storm_mssql::MssqlProvider = storm::tri!(self.container().provide(#provider).await);

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

    let load_fn = Ident::new(
        &format!("__load_{}", ident.to_string().to_snake_case()),
        Span::call_site(),
    );

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
    let translated_table_name = LitStr::new(&attrs.translate_table, attrs.translate_table.span());
    let enum_fields_ident = Ident::new(&format!("{ident}Fields"), ident.span());

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
                load_diff_field(
                    &mut diff,
                    field_ident,
                    &enum_fields_ident,
                    &table_name,
                    &column,
                );
            }
        } else {
            let column = continue_ts!(RenameAll::column(rename_all, &attrs.column, field), errors);
            load.add_field(field, &attrs, &column);

            if !attrs.skip_save() && !attrs.skip_diff() {
                load_diff_field(
                    &mut diff,
                    field_ident,
                    &enum_fields_ident,
                    &table_name,
                    &column,
                );
            }
        }
    }

    try_ts!(errors.result());

    let translated_where = translated.to_where_clause();
    let provider = attrs.provider();
    let (metrics_start, metrics_end) = metrics(ident, "load");
    let diff = apply_entity_diff(diff, ident);
    let instrument_ident = Ident::new(
        &format!("{}_load_all_inst", ident.to_string().to_snake_case()),
        ident.span(),
    );

    let test = if attrs.no_test {
        quote!()
    } else {
        quote! {
            #[cfg(test)]
            #[tokio::test]
            async fn #load_test() -> storm::Result<()> {
                let provider = storm::tri!(storm_mssql::create_provider_container_from_env("DB", #provider));
                storm::tri!(storm::provider::LoadAll::<#ident, _, storm::provider::LoadNothing>::load_all(&provider, &()).await);
                Ok(())
            }
        }
    };

    quote! {
        fn #instrument_ident(args: &storm::provider::LoadArgs) -> tracing::Span {
            tracing::debug_span!("load_all", args = ?args, table = #table_name, err = tracing::field::Empty)
        }

        fn #load_fn<'a, C>(provider: &'a storm::provider::ProviderContainer, sql: std::borrow::Cow<'a, str>, params: std::borrow::Cow<'a, [&'a dyn storm_mssql::ToSql]>, args: storm::provider::LoadArgs) -> storm::BoxFuture<'a, storm::Result<C>>
        where
            C: Default + Extend<(<#ident as storm::Entity>::Key, #ident)> #translated_where + Send + 'static,
        {
            let span = #instrument_ident(&args);

            Box::pin(storm::InstrumentErr::instrument_err(async move {
                let provider: &storm_mssql::MssqlProvider = storm::tri!(provider.provide(#provider).await);
                #metrics_start
                #load
                #translated
                #metrics_end
                Ok(map)
            }, span))
        }

        impl<C, FILTER> storm::provider::LoadAll<#ident, FILTER, C> for storm::provider::ProviderContainer
        where
            C: Default + Extend<(<#ident as storm::Entity>::Key, #ident)> #translated_where + Send + 'static,
            FILTER: storm_mssql::FilterSql,
        {
            fn load_all_with_args<'a>(&'a self, filter: &'a FILTER, args: storm::provider::LoadArgs) -> storm::BoxFuture<'a, storm::Result<C>> {
                let (sql, params) = storm_mssql::FilterSql::filter_sql(filter, 0);
                #load_fn(self, sql, params, args)
            }
        }

        impl storm::provider::LoadOne<#ident> for storm::provider::ProviderContainer {
            #[tracing::instrument(name = "load_one", level = "debug", skip(self, k), fields(table = #table_name))]
            fn load_one_with_args<'a>(&'a self, k: &'a <#ident as Entity>::Key, args: storm::provider::LoadArgs) -> storm::BoxFuture<'a, storm::Result<Option<#ident>>> {
                Box::pin(async move {
                    let filter = #filter_sql;
                    let v: storm::provider::LoadOneInternal<#ident> = storm::tri!(storm::provider::LoadAll::load_all_with_args(self, &filter, args).await);
                    Ok(v.into_inner())
                })
            }
        }

        impl storm_mssql::MssqlMeta for #ident {
            const TABLE: &'static str = #table_name;
            const TRANSLATED_TABLE: &'static str = #translated_table_name;
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
    let mut enum_fields = Vec::new();
    let enum_fields_ident = Ident::new(&format!("{ident}Fields"), ident.span());
    let vis = &input.vis;

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
        let field_pascal_ident = Ident::new(&ident.to_string().to_pascal_case(), ident.span());

        if is_translated(&field.ty) {
            translated.add_field(field, column);
            let name_bk = Ident::new(&format!("{ident}_bk"), Span::call_site());
            translated_backup.push(
                quote! { let #name_bk = std::mem::replace(&mut v.#ident, Default::default()); },
            );
            translated_restore.push(quote! { v.#ident = #name_bk; });

            if let Some(diff) = diff.as_mut().filter(|_| !attrs.skip_diff()) {
                diff.push(quote! {
                    if let Some(diff) = storm_mssql::FieldDiff::field_diff(&self.#ident, &old.#ident) {
                        map.insert(#enum_fields_ident::#field_pascal_ident, diff);
                    }
                });

                enum_fields.push(quote!(#field_pascal_ident));
            }

            continue;
        }

        let name = LitStr::new(&format!("[{column}]"), ident.span());

        if attrs.part {
            save_part.push(
                quote!(storm_mssql::SaveEntityPart::save_entity_part(&self.#ident, k, builder);),
            );

            if let Some(diff) = diff.as_mut().filter(|_| !attrs.skip_diff()) {
                diff.push(
                    quote! { storm_mssql::EntityDiff::entity_diff(&self.#ident, &old.#ident, map);},
                );

                // Ignored for now.
                // enum_fields.push(quote!(#ident()));
            }
        } else {
            match attrs.save_with.as_ref() {
                Some(f) => {
                    save_part.push(quote!(builder.add_field_owned(#name, #f(k, self));));

                    if let Some(diff) = diff.as_mut().filter(|_| !attrs.skip_diff()) {
                        diff.push(quote! {
                            if let Some(diff) = storm_mssql::FieldDiff::field_diff(&#f(k, self), &#f(k, old)) {
                                map.insert(#enum_fields_ident::#field_pascal_ident, diff);
                            }
                        });

                        enum_fields.push(quote!(#field_pascal_ident));
                    }
                }
                None => {
                    save_part.push(quote!(builder.add_field_ref(#name, &self.#ident);));

                    if let Some(diff) = diff.as_mut().filter(|_| !attrs.skip_diff()) {
                        diff.push(quote! {
                            if let Some(diff) = storm_mssql::FieldDiff::field_diff(&self.#ident, &old.#ident) {
                                map.insert(#enum_fields_ident::#field_pascal_ident, diff);
                            }
                        });

                        enum_fields.push(quote!(#field_pascal_ident));
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
        let name = LitStr::new(&format!("[{key}]"), ident.span());

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
    let entity_part_key;

    if attrs.reload_on_upsert_or_identity() {
        upsert_trait = quote!(storm::provider::UpsertMut<#ident>);
        upsert_sig = quote!(fn upsert_mut<'a>(&'a self, k: &'a mut <#ident as storm::Entity>::Key, v: &'a mut #ident) -> storm::BoxFuture<'a, storm::Result<()>>);
        entity_part_key = quote!(&k.clone());
    } else {
        upsert_trait = quote!(storm::provider::Upsert<#ident>);
        upsert_sig = quote!(fn upsert<'a>(&'a self, k: &'a <#ident as storm::Entity>::Key, v: &'a #ident) -> storm::BoxFuture<'a, storm::Result<()>>);
        entity_part_key = quote!(k);
    }

    let reload_entity = if attrs.reload_on_upsert() {
        let backup = quote!(#(#translated_backup)*);
        let restore = quote!(#(#translated_restore)*);

        quote! {
            #backup
            *v = storm::tri!(storm::provider::LoadOne::<#ident>::load_one_with_args(
                self.container(),
                &k,
                storm::provider::LoadArgs { use_transaction: true })
                    .await
                    .and_then(|v| v.ok_or_else(|| storm::Error::from(storm_mssql::Error::EntityNotFound { table: #table_name })))
            );
            #restore
        }
    } else {
        quote!()
    };

    let builder_invoke = if is_identity_key {
        quote!(storm::tri!(builder.execute_identity(provider, k).await);)
    } else {
        quote!(storm::tri!(builder.execute(provider).await);)
    };

    let save_part = save_part.ts();
    let wheres = wheres.ts();
    let table = LitStr::new(&attrs.table, ident.span());
    let provider = attrs.provider();
    let (metrics_start, metrics_end) = metrics(ident, "upsert");
    let diff = entity_diff(ident, diff);
    let enum_fields = enum_fields_impl(vis, ident, enum_fields, &enum_fields_ident, attrs.diff);

    quote! {
        impl #upsert_trait for storm::provider::TransactionProvider<'_> {
            #[tracing::instrument(name = "upsert", level = "debug", skip(self, k, v), fields(table = #table_name))]
            #upsert_sig {
                Box::pin(async move {
                    let provider: &storm_mssql::MssqlProvider = storm::tri!(self.container().provide(#provider).await);
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
        #enum_fields

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

#[cfg(not(feature = "telemetry"))]
fn metrics(_ident: &Ident, _ops: &str) -> (TokenStream, TokenStream) {
    (quote!(), quote!())
}

#[cfg(feature = "telemetry")]
fn metrics(ident: &Ident, op: &str) -> (TokenStream, TokenStream) {
    let start = quote! {
        let instant = std::time::Instant::now();
    };

    let op = LitStr::new(op, Span::call_site());
    let ty = LitStr::new(&ident.to_string(), ident.span());

    let end = quote! {
        storm::telemetry::inc_storm_execute_time(instant, #op, #ty);
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
                fn apply_entity_diff<S: std::hash::BuildHasher>(&mut self, diff: &std::collections::HashMap<storm::FieldsOrStr<<Self as storm::EntityFields>::Fields>, storm_mssql::serde_json::Value, S>) -> storm::Result<()> {
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
                fn entity_diff<S: std::hash::BuildHasher>(&self, old: &Self, map: &mut std::collections::HashMap<<Self as storm::EntityFields>::Fields, storm_mssql::serde_json::Value, S>) {
                    #(#diff)*
                }
            }
        }
    } else {
        quote!()
    }
}

fn load_diff_field(
    diff: &mut Option<Vec<TokenStream>>,
    field: &Ident,
    enum_fields_ident: &Ident,
    table: &LitStr,
    column: &str,
) {
    if let Some(diff) = diff.as_mut() {
        let name = Ident::new(&field.to_string().to_pascal_case(), field.span());
        let column = LitStr::new(column, Span::call_site());

        diff.push(quote! {
            storm::tri!(
                storm_mssql::_replace_field_diff(
                    &mut self.#field,
                    #enum_fields_ident::#name,
                    diff,
                    #table,
                    #column
                )
            );
        });
    }
}

fn read_row(column_index: usize, table: &str, column: &str) -> TokenStream {
    let l = LitInt::new(&column_index.to_string(), Span::call_site());
    let table = LitStr::new(table, Span::call_site());
    let column = LitStr::new(column, Span::call_site());
    quote!(storm::tri!(storm_mssql::_macro_load_field(&row, #l, #table, #column)))
}

fn enum_fields_impl(
    vis: &Visibility,
    ident: &Ident,
    items: Vec<TokenStream>,
    enum_ident: &Ident,
    is_diff: bool,
) -> TokenStream {
    if is_diff {
        quote! {
            #[derive(Clone, Copy, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            #vis enum #enum_ident {
                #(#items,)*
            }

            impl storm::EntityFields for #ident {
                type Fields = #enum_ident;
            }
        }
    } else {
        quote!()
    }
}
