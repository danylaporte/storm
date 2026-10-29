use super::{TypeAttrs, builders::DeleteBuilder};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{LitInt, LitStr};

pub(super) struct DeleteSql {
    /// Delete statements, the translated table first.
    pub statements: Vec<LitStr>,
    /// Key parameters, shared by all the statements.
    pub params: Vec<TokenStream>,
}

pub(super) fn delete_sql(attrs: &TypeAttrs, errors: &mut Vec<TokenStream>) -> DeleteSql {
    let mut statements = Vec::with_capacity(2);
    let mut key_count = 0;

    if !attrs.translate_table.is_empty() {
        let keys = attrs.translate_keys(errors);
        key_count = keys.len();
        statements.push(sql(&attrs.translate_table, &keys));
    }

    if !attrs.table.is_empty() {
        let keys = attrs.keys(errors);
        key_count = keys.len();
        statements.push(sql(&attrs.table, &keys));
    }

    let params = if key_count == 1 {
        vec![quote!(k as _)]
    } else {
        (0..key_count)
            .map(|i| {
                let i = LitInt::new(&i.to_string(), Span::call_site());
                quote!(&k.#i as _)
            })
            .collect()
    };

    DeleteSql { statements, params }
}

fn sql(table: &str, keys: &[&str]) -> LitStr {
    let mut builder = DeleteBuilder::default();

    for (index, column) in keys.iter().enumerate() {
        builder.add_key(column, &(index + 1).to_string());
    }

    builder.to_sql_lit(table)
}
