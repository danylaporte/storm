use crate::StringExt;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::LitStr;

#[derive(Clone, Default)]
pub(super) struct InsertBuilder {
    fields: String,
    values: String,
}

impl InsertBuilder {
    pub fn add(&mut self, column: &str, param_index: &str) {
        self.fields.add_sep(',').add('[').add_str(column).add(']');
        self.values.add_sep(',').add_str("@p").add_str(param_index);
    }

    pub fn to_sql(&self, table: &str) -> String {
        format!(
            "INSERT {} ({}) VALUES ({})",
            table, &self.fields, &self.values
        )
    }
}

#[derive(Clone, Default)]
pub(super) struct ParamsBuilder(Vec<TokenStream>);

impl ParamsBuilder {
    pub fn add_ts(&mut self, ts: TokenStream) -> usize {
        self.0.push(ts);
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl ToTokens for ParamsBuilder {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let p = &self.0;
        tokens.append_all(quote!(&[#(#p,)*][..]));
    }
}

#[derive(Clone, Default)]
pub(super) struct UpdateBuilder {
    fields: String,
    wheres: String,
}

impl UpdateBuilder {
    pub fn add_field(&mut self, column: &str, param_index: &str) {
        self.fields
            .add_sep(',')
            .add('[')
            .add_str(column)
            .add_str("]=@p")
            .add_str(param_index);
    }

    pub fn add_key(&mut self, column: &str, param_index: &str) {
        self.wheres
            .add_sep_str("AND")
            .add_str("([")
            .add_str(column)
            .add_str("]=@p")
            .add_str(param_index)
            .add(')');
    }

    pub fn to_sql(&self, table: &str) -> String {
        format!(
            "UPDATE {} SET {} WHERE {}",
            table, &self.fields, &self.wheres
        )
    }
}

#[derive(Clone, Default)]
pub struct UpsertBuilder {
    insert: InsertBuilder,
    update: UpdateBuilder,
}

impl UpsertBuilder {
    pub fn add_field(&mut self, column: &str, param_index: &str) {
        self.insert.add(column, param_index);
        self.update.add_field(column, param_index);
    }

    pub fn add_key(&mut self, column: &str, param_index: &str) {
        self.insert.add(column, param_index);
        self.update.add_key(column, param_index);
    }

    pub fn to_sql(&self, table: &str) -> String {
        format!(
            "BEGIN TRY
                {insert}
            END TRY
            BEGIN CATCH
                IF ERROR_NUMBER() IN (2601, 2627)
                {update}
            END CATCH",
            insert = self.insert.to_sql(table),
            update = self.update.to_sql(table),
        )
    }

    pub fn to_sql_lit(&self, table: &str) -> LitStr {
        LitStr::new(&self.to_sql(table), Span::call_site())
    }
}
