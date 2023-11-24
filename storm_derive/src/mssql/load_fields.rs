use super::{
    attrs::{check_empty, check_required, FieldAttrs, TypeAttrs},
    builders::SelectBuilder,
    read_row,
};
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt as _};
use syn::{Field, Ident, LitStr};

pub(super) struct LoadFields<'a> {
    attrs: &'a TypeAttrs,
    entity: &'a Ident,
    select: SelectBuilder,
    fields: Vec<TokenStream>,
}

impl<'a> LoadFields<'a> {
    pub fn new(entity: &'a Ident, attrs: &'a TypeAttrs) -> Self {
        Self {
            attrs,
            entity,
            fields: Default::default(),
            select: SelectBuilder::with_alias("t"),
        }
    }

    pub fn add_field(&mut self, field: &Field, attrs: &FieldAttrs, column: &str) {
        let ident = &field.ident;
        let skip_load = attrs.skip_load();

        let column_index = match skip_load {
            true => 0,
            false => self.select.add_field(column),
        };

        let read = match attrs.load_with.as_ref() {
            Some(f) => quote!(storm::tri!(#f(&row))),
            None if skip_load => quote!(Default::default()),
            None => read_row(column_index, &self.attrs.table, column),
        };

        self.fields.push(quote!(#ident: #read,));
    }

    pub fn skip_field(&mut self, field: &Field, attrs: &FieldAttrs, errors: &mut Vec<TokenStream>) {
        check_empty(&attrs.load_with, errors);

        let ident = &field.ident;
        self.fields.push(quote!(#ident: Default::default(),));
    }
}

impl<'a> ToTokens for LoadFields<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut errors = Vec::new();
        let mut select = self.select.clone();

        check_required(&self.attrs.table, &mut errors);

        let keys = add_keys(self.attrs, &mut select, &mut errors);
        let sql = select.to_sql_lit(&self.attrs.table, &self.attrs.where_clause);

        let entity = self.entity;
        let fields = &self.fields;
        let fields = quote!(#(#fields)*);

        let filter_lit = LitStr::new(
            match self.attrs.where_clause.is_empty() {
                true => "{} WHERE {}",
                false => "{} AND {}",
            },
            Span::call_site(),
        );

        tokens.append_all(quote! {
            const SQL: &str = #sql;

            let load_sql = match sql.is_empty() {
                false => format!(#filter_lit, SQL, sql),
                true => SQL.to_string(),
            };

            fn load_row(row: storm_mssql::tiberius::Row) -> storm::Result<(<#entity as storm::Entity>::Key, #entity)> {
                Ok((
                    #keys,
                    #entity { #fields }
                ))
            }

            let mut map: C = storm::tri!(storm_mssql::QueryRows::query_rows(provider, load_sql, &*params, load_row, args.use_transaction).await);
        });

        tokens.append_all(quote!(#(#errors)*));
    }
}

fn add_key(key: &str, select: &mut SelectBuilder, key_ts: &mut Vec<TokenStream>, table: &str) {
    let column_index = select.add_field(key);
    key_ts.push(read_row(column_index, table, key));
}

fn add_keys(
    type_attrs: &TypeAttrs,
    select: &mut SelectBuilder,
    errors: &mut Vec<TokenStream>,
) -> TokenStream {
    let keys = type_attrs.keys(errors);
    let mut ts = Vec::new();

    for key in &keys {
        add_key(key, select, &mut ts, &type_attrs.table);
    }

    if keys.len() == 1 {
        quote!(#(#ts)*)
    } else {
        quote!((#(#ts,)*))
    }
}
