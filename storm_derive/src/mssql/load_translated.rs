use super::{
    attrs::{check_empty, check_required, TypeAttrs},
    builders::{JoinBuilder, JoinConditions, SelectBuilder},
    read_row,
};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt as _};
use syn::{Field, LitStr};

/// creates a select sql query for each field and keys and generate the
/// loading code for translated fields.
pub(super) struct LoadTranslated<'a> {
    attrs: &'a TypeAttrs,
    entity: &'a Ident,
    select: SelectBuilder,
    fields: Vec<TokenStream>,
}

impl<'a> LoadTranslated<'a> {
    pub fn new(entity: &'a Ident, attrs: &'a TypeAttrs) -> Self {
        Self {
            attrs,
            entity,
            fields: Default::default(),
            select: SelectBuilder::with_alias("a"),
        }
    }

    pub fn add_field(&mut self, field: &Field, column: &str) {
        let ident = &field.ident;
        let index = self.select.add_field(column);
        let read = read_row(index);

        self.fields.push(quote! {
            let v: Option<&str> = #read;
            if let Some(v) = v {
                val.#ident.set(culture, v);
            }
        });
    }

    pub fn to_where_clause(&self) -> TokenStream {
        let entity = &self.entity;

        match self.fields.is_empty() {
            false => quote!(+ storm::GetMut<#entity>),
            true => quote!(),
        }
    }
}

impl ToTokens for LoadTranslated<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut errors = Vec::new();

        if self.select.is_empty() {
            check_empty(&self.attrs.translate_table, &mut errors);
            check_empty(&self.attrs.translate_keys, &mut errors);
        } else {
            check_required(&self.attrs.translate_table, &mut errors);

            let mut select = self.select.clone();
            let mut joins = JoinBuilder::default();
            let mut conds = joins.inner_join(&self.attrs.table, Some("t"));

            let keys = add_keys(self.attrs, &mut conds, &mut select, &mut errors);
            let culture = read_row(select.add_field("Culture"));
            let sql = select.to_sql_lit(&self.attrs.translate_table, &self.attrs.where_clause);

            let joins = format!("{{}} {} WHERE {{}}", joins.to_sql());
            let joins = LitStr::new(&joins, Span::call_site());
            let entity = self.entity;
            let fields = &self.fields;
            let fields = quote!(#(#fields)*);

            tokens.append_all(quote! {
                const TRANSLATED_SQL: &str = #sql;

                let translated_sql = match sql.is_empty() {
                    false => format!(#joins, TRANSLATED_SQL, sql),
                    true => TRANSLATED_SQL.to_string(),
                };

                let _: storm::provider::LoadDoNothing = storm::tri!(storm_mssql::QueryRows::query_rows(provider, translated_sql, &*params, |row| {
                    let key: <#entity as storm::Entity>::Key = #keys;
                    let culture = #culture;

                    if let Some(val) = map.get_mut(&key) {
                        #fields
                    }

                    Ok(())
                }, /*use_transaction*/ true).await);
            });
        }

        tokens.append_all(quote!(#(#errors)*));
    }
}

fn add_key(
    translate_key: &str,
    key: &str,
    joins: &mut JoinConditions,
    select: &mut SelectBuilder,
    key_ts: &mut Vec<TokenStream>,
) {
    let column_index = select.add_field(translate_key);

    key_ts.push(read_row(column_index));
    joins.add((Some("a"), translate_key), (Some("t"), key));
}

fn add_keys(
    type_attrs: &TypeAttrs,
    joins: &mut JoinConditions,
    select: &mut SelectBuilder,
    errors: &mut Vec<TokenStream>,
) -> TokenStream {
    let translate_keys = type_attrs.translate_keys(errors);
    let keys = type_attrs.keys_internal();
    let mut ts = Vec::new();

    for (translate_key, key) in translate_keys.iter().zip(&keys) {
        add_key(translate_key, key, joins, select, &mut ts);
    }

    if keys.len() == 1 {
        quote!(#(#ts)*)
    } else {
        quote!((#(#ts,)*))
    }
}
