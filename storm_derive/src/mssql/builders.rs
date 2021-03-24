use crate::StringExt;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::LitStr;

#[derive(Clone, Default)]
pub(super) struct DeleteBuilder {
    wheres: String,
}

impl DeleteBuilder {
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
        format!("DELETE FROM {} WHERE {}", table, self.wheres)
    }

    pub fn to_sql_lit(&self, table: &str) -> LitStr {
        LitStr::new(&self.to_sql(table), Span::call_site())
    }
}

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

#[must_use]
pub(super) struct JoinConditions<'a>(&'a mut String);

impl<'a> JoinConditions<'a> {
    pub fn add(
        &mut self,
        (alias_left, left): (Option<&str>, &str),
        (alias_right, right): (Option<&str>, &str),
    ) {
        self.0.add_sep_str(" AND ");
        add_alias_field(&mut self.0, (alias_left, left));
        self.0.add('=');
        add_alias_field(&mut self.0, (alias_right, right));
    }
}

#[derive(Default)]
pub(super) struct JoinBuilder(String);

impl JoinBuilder {
    pub fn inner_join<'a>(&'a mut self, table: &str, alias: Option<&str>) -> JoinConditions<'a> {
        self.0.add_str(" INNER JOIN ").add_str(table);

        if let Some(alias) = alias {
            self.0.add(' ').add_str(alias);
        }

        JoinConditions(&mut self.0)
    }

    pub fn to_sql(&self) -> &str {
        &self.0
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
pub(super) struct SelectBuilder {
    alias: Option<&'static str>,
    count: usize,
    select: String,
}

impl SelectBuilder {
    pub fn with_alias(alias: &'static str) -> Self {
        Self {
            alias: Some(alias),
            count: 0,
            select: String::new(),
        }
    }

    pub fn add_field(&mut self, column: &str) -> usize {
        let index = self.count;

        self.select.add_sep(',');
        add_alias_field(&mut self.select, (self.alias, column));
        self.count += 1;

        index
    }

    pub fn is_empty(&self) -> bool {
        self.select.is_empty()
    }

    fn to_sql(&self, table: &str) -> String {
        format!(
            "SELECT {} FROM {} {}",
            self.select,
            table,
            self.alias.unwrap_or("")
        )
    }

    pub fn to_sql_lit(&self, table: &str) -> LitStr {
        LitStr::new(&self.to_sql(table), Span::call_site())
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

fn add_alias_field(sql: &mut String, (alias, field): (Option<&str>, &str)) {
    if let Some(alias) = alias {
        sql.add_str(alias).add('.');
    }

    sql.add('[').add_str(field).add(']');
}
